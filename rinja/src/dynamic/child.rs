use std::borrow::Cow;
use std::env::args;
use std::fmt::Write;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::process::exit;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;
use std::vec::Vec;
use std::{eprintln, format};

use linkme::distributed_slice;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::spawn;
use tokio::sync::{Mutex, oneshot};

use super::{DYNAMIC_ENVIRON_KEY, MainRequest, MainResponse, Outcome};

const PROCESSORS: usize = 4;

#[inline(never)]
pub(crate) fn run_dynamic_main() {
    std::env::set_var(DYNAMIC_ENVIRON_KEY, "-");

    let mut entries: Vec<_> = DYNAMIC_TEMPLATES.iter().map(|entry| entry.name()).collect();
    entries.sort_unstable();
    eprintln!("templates implemented by subprocess: {entries:?}");
    for window in entries.windows(2) {
        if let &[a, b] = window {
            if a == b {
                eprintln!("duplicated dynamic template {a:?}");
            }
        }
    }

    let sock_addr: SocketAddr = {
        let mut args = args().fuse();
        let (_, Some("--__rinja_dynamic"), Some(sock_addr), None) = (
            args.next(),
            args.next().as_deref(),
            args.next(),
            args.next(),
        ) else {
            eprintln!("child process got unexpected arguments");
            exit(1);
        };
        match serde_json::from_str(&sock_addr) {
            Ok(sock_addr) => sock_addr,
            Err(err) => {
                eprintln!("subprocess could not interpret socket addr: {err}");
                exit(1);
            }
        }
    };

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("could not start tokio runtime: {err}");
            exit(1);
        }
    };
    let _ = rt.block_on(async move {
        let sock = match TcpStream::connect(sock_addr).await {
            Ok(sock) => sock,
            Err(err) => {
                eprintln!("subprocess could not connect to parent process: {err}");
                exit(1);
            }
        };
        let _: Result<(), std::io::Error> = sock.set_linger(None);
        let _: Result<(), std::io::Error> = sock.set_nodelay(true);
        let (read, write) = sock.into_split();

        let stdout = Arc::new(Mutex::new(write));
        let stdin = Arc::new(Mutex::new(BufReader::new(read)));
        let (done_tx, done_rx) = oneshot::channel();
        let done = Arc::new(Mutex::new(Some(done_tx)));

        let mut threads = Vec::with_capacity(PROCESSORS);
        for _ in 0..PROCESSORS {
            threads.push(spawn(dynamic_processor(
                Arc::clone(&stdout),
                Arc::clone(&stdin),
                Arc::clone(&done),
            )));
        }

        done_rx.await.map_err(|err| {
            std::io::Error::new(ErrorKind::BrokenPipe, format!("lost result channel: {err}"));
        })
    });
    rt.shutdown_timeout(Duration::from_secs(5));
    exit(0)
}

async fn dynamic_processor(
    stdout: Arc<Mutex<OwnedWriteHalf>>,
    stdin: Arc<Mutex<BufReader<OwnedReadHalf>>>,
    done: Arc<Mutex<Option<oneshot::Sender<std::io::Result<()>>>>>,
) {
    let done = move |result: Result<(), std::io::Error>| {
        let done = Arc::clone(&done);
        async move {
            let mut lock = done.lock().await;
            if let Some(done) = lock.take() {
                let _: Result<_, _> = done.send(result);
            }
        }
    };

    let mut line_buf = String::new();
    let mut response_buf = String::new();
    loop {
        line_buf.clear();
        match stdin.lock().await.read_line(&mut line_buf).await {
            Ok(n) if n > 0 => {}
            result => return done(result.map(|_| ())).await,
        }
        let line = line_buf.trim_ascii();
        if line.is_empty() {
            continue;
        }

        let MainRequest { callid, name, data } = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(err) => {
                let err = format!("could not deserialize request: {err}");
                return done(Err(std::io::Error::new(ErrorKind::InvalidData, err))).await;
            }
        };
        response_buf.clear();

        let mut outcome = Outcome::NotFound;
        for entry in DYNAMIC_TEMPLATES {
            if entry.name() == name {
                outcome = entry.dynamic_render(&mut response_buf, &data);
                break;
            }
        }

        // SAFETY: `serde_json` writes valid UTF-8 data
        let mut line = unsafe { line_buf.as_mut_vec() };

        line.clear();
        if let Err(err) = serde_json::to_writer(&mut line, &MainResponse { callid, outcome }) {
            let err = format!("could not serialize response: {err}");
            return done(Err(std::io::Error::new(ErrorKind::InvalidData, err))).await;
        }
        line.push(b'\n');

        let is_done = {
            let mut stdout = stdout.lock().await;
            stdout.write_all(line).await.is_err() || stdout.flush().await.is_err()
        };
        if is_done {
            return done(Ok(())).await;
        }
    }
}

/// Used by [`Template`][rinja_derive::Template] to register a template for dynamic processing.
#[macro_export]
macro_rules! register_dynamic_template {
    (
        name: $Name:ty,
        type: $Type:ty,
    ) => {
        const _: () = {
            #[$crate::helpers::linkme::distributed_slice($crate::helpers::DYNAMIC_TEMPLATES)]
            #[linkme(crate = $crate::helpers::linkme)]
            static DYNAMIC_TEMPLATES: &'static dyn $crate::helpers::DynamicTemplate = &Dynamic;

            struct Dynamic;

            impl $crate::helpers::DynamicTemplate for Dynamic {
                #[inline]
                fn name(&self) -> &$crate::helpers::core::primitive::str {
                    $crate::helpers::core::any::type_name::<$Name>()
                }

                fn dynamic_render<'a>(
                    &self,
                    buf: &'a mut rinja::helpers::alloc::string::String,
                    value: &rinja::helpers::core::primitive::str,
                ) -> rinja::helpers::Outcome<'a> {
                    let result = rinja::helpers::from_json::<$Type>(value).map(|tmpl| {
                        buf.clear();
                        let _ = buf.try_reserve(<Tmpl as $crate::Template>::SIZE_HINT);
                        tmpl.render_into(buf)
                    });
                    $crate::helpers::use_dynamic_render_result(buf, result)
                }
            }
        };
    };
}

/// Convert the result of [`serde::from_json()`] → [`Template::render()`] to an [`Outcome`].
pub fn use_dynamic_render_result(
    buf: &mut String,
    result: Result<Result<(), crate::Error>, serde_json::Error>,
) -> Outcome<'_> {
    let result = match &result {
        Ok(Ok(())) => return Outcome::Success(Cow::Borrowed(buf)),
        Ok(Err(err)) => Ok(err),
        Err(err) => Err(err),
    };

    buf.clear();
    let result = match result {
        Ok(e) => write!(buf, "{e}").map(|_| Outcome::Render(Cow::Borrowed(buf))),
        Err(e) => write!(buf, "{e}").map(|_| Outcome::Deserialize(Cow::Borrowed(buf))),
    };
    result.unwrap_or(Outcome::Fmt)
}

/// List of implemented dynamic templates. Filled through
/// [`register_dynamic_template!`][crate::register_dynamic_template].
#[distributed_slice]
pub static DYNAMIC_TEMPLATES: [&'static dyn DynamicTemplate];

/// A dynamic template implementation
pub trait DynamicTemplate: Send + Sync {
    /// The type name of the template.
    fn name(&self) -> &str;

    /// Take a JSON `value` to to render the template into `buf`.
    fn dynamic_render<'a>(&self, buf: &'a mut String, value: &str) -> Outcome<'a>;
}
