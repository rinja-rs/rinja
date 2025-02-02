use std::boxed::Box;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::env::current_exe;
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::ops::ControlFlow;
use std::process::{Stdio, exit};
use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::{eprintln, format};

use notify::{Watcher, recommended_watcher};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::net::tcp::OwnedReadHalf;
use tokio::process::Command;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::{Instant, sleep, sleep_until, timeout, timeout_at};
use tokio::{select, try_join};

use super::{Outcome, am_dynamic_child};
use crate::dynamic::{DYNAMIC_ENVIRON_KEY, DYNAMIC_ENVIRON_VALUE, MainRequest, MainResponse};
use crate::{Error, Values};

static QUEUE: Queue = Queue::new();
static RUNTIME: std::sync::Mutex<Option<Handle>> = std::sync::Mutex::new(None);

const QUEUE_SIZE: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(10);

/// Renders a template dynamically we are inside the parent process.
#[inline]
pub fn maybe_render_dynamic_into(
    name: &dyn Fn() -> &'static str,
    serialize: &dyn Fn() -> Result<String, serde_json::Error>,
    dest: &mut (impl std::fmt::Write + ?Sized),
    values: &dyn Values,
) -> Result<ControlFlow<()>, Error> {
    if am_dynamic_child() {
        return Ok(ControlFlow::Continue(()));
    }

    let _ = values; // TODO
    let data = request(name, serialize)?;
    dest.write_str(&data)?;
    Ok(ControlFlow::Break(()))
}

#[inline]
fn request(
    name: &dyn Fn() -> &'static str,
    serialize: &dyn Fn() -> Result<String, serde_json::Error>,
) -> Result<String, Error> {
    let callid = QUEUE.callid.fetch_add(1, Ordering::Relaxed);
    let request = MainRequest {
        callid,
        name: std::borrow::Cow::Borrowed(name()),
        data: serialize()
            .map_err(|err| Error::custom(format!("could not serialize template: {err}")))?
            .into(),
    };
    let mut request = serde_json::to_string(&{ request })
        .map_err(|err| Error::custom(format!("could not serialize request: {err}")))?;
    request.push('\n');

    let deadline = Instant::now() + TIMEOUT;
    let (response_tx, response_rx) = oneshot::channel();
    let request = Box::new(Request {
        callid,
        request,
        response: response_tx,
    });

    let result_arc = Arc::new(parking_lot::Mutex::new(None));

    let runtime = RUNTIME.lock().unwrap().clone().unwrap();
    runtime.spawn({
        let mut result_guard = result_arc.try_lock_arc().unwrap();
        async move {
            *result_guard = Some(handle_request(deadline, request, response_rx).await);
        }
    });

    let mut result_guard = result_arc.lock();
    match result_guard.take() {
        Some(result) => result,
        None => Err(Error::custom("lost connection to handler thread (panic?)")),
    }
}

async fn handle_request(
    deadline: Instant,
    request: Box<Request>,
    response_rx: oneshot::Receiver<Result<String, Error>>,
) -> Result<String, Error> {
    let insert = async {
        QUEUE
            .get_channel()
            .await
            .0
            .lock()
            .await
            .send(request)
            .await
            .map_err(|_| Error::custom("channel was closed unexpectedly (impossible)"))
    };
    let response = async {
        match response_rx.await {
            Ok(response) => response,
            Err(_) => Err(Error::custom("request got lost")),
        }
    };
    let write_and_read = async {
        let (_, data) = try_join!(insert, response)?;
        Ok(data)
    };

    if let Ok(result) = timeout_at(deadline, write_and_read).await {
        result
    } else {
        Err(Error::custom("deadline expired"))
    }
}

#[inline(never)]
pub(crate) fn run_dynamic_main() {
    struct Bomb;

    impl Drop for Bomb {
        fn drop(&mut self) {
            exit(1);
        }
    }

    std::thread::spawn(move || {
        let _bomb = Bomb;
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => {
                *RUNTIME.try_lock().unwrap() = Some(rt.handle().clone());
                match rt.block_on(dynamic_dispatcher()) {
                    Err(err) => eprintln!("dynamic dispatcher execution stopped: {err}"),
                }
            }
            Err(err) => {
                eprintln!("could not start tokio runtime: {err}");
            }
        };
    });
}

async fn dynamic_dispatcher() -> std::io::Result<Infallible> {
    let (stdout_tx, stdout_rx) = mpsc::channel(1);
    let pending_responses: Arc<Mutex<BTreeMap<u64, RequestResponse>>> = Arc::default();

    let read_loop = read_loop(stdout_rx, Arc::clone(&pending_responses));
    let respawn_loop = respawn_loop(stdout_tx, pending_responses);

    #[allow(unreachable_code)] // The `Ok(!)` path cannot be entered.
    match try_join!(read_loop, respawn_loop) {
        Err(err) => Err(err),
    }
}

type PendingResponses = Arc<Mutex<BTreeMap<u64, oneshot::Sender<Result<String, Error>>>>>;

async fn read_loop(
    mut stdout_rx: mpsc::Receiver<OwnedReadHalf>,
    pending_responses: PendingResponses,
) -> Result<Infallible, std::io::Error> {
    let mut line_buf = String::new();
    'read_loop: loop {
        let read_sock = stdout_rx.recv().await.unwrap();
        let mut read_sock = BufReader::new(read_sock);
        loop {
            line_buf.clear();
            match read_sock.read_line(&mut line_buf).await {
                Ok(0) => {
                    eprintln!("stdin closed");
                    continue 'read_loop;
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!("stdin broken: {err}");
                    continue 'read_loop;
                }
            }
            let line = line_buf.trim_ascii();
            if line.is_empty() {
                continue;
            }

            let MainResponse { callid, outcome } = match serde_json::from_str(line) {
                Ok(outcome) => outcome,
                Err(err) => {
                    let err = format!("could not deserialize response: {err}");
                    return Err(std::io::Error::new(ErrorKind::InvalidData, err));
                }
            };
            let request_response = pending_responses.lock().await.remove(&callid);
            let result = if let Outcome::Success(data) = outcome {
                Ok(data.into_owned())
            } else {
                Err(Error::custom(outcome.to_string()))
            };
            if let Some(request_response) = request_response {
                let _: Result<(), _> = request_response.send(result);
            }
        }
    }
}

async fn respawn_loop(
    stdout_tx: mpsc::Sender<OwnedReadHalf>,
    pending_responses: PendingResponses,
) -> Result<Infallible, std::io::Error> {
    let exe_path = current_exe().map_err(|err| {
        std::io::Error::new(
            ErrorKind::NotFound,
            format!("could not find current executable's path: {err}"),
        )
    })?;

    loop {
        let exe_time = match exe_path.metadata().and_then(|m| m.modified()) {
            Ok(exe_hash) => exe_hash,
            Err(err) => {
                eprintln!(
                    "could not get modification timestamp of current exe file, retrying: {err}"
                );
                sleep(Duration::from_millis(1000)).await;
                continue;
            }
        };

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let addr = serde_json::to_string(&addr)?;

        let (ev_tx, mut ev_rx) = mpsc::channel(1);
        let ev_tx = Arc::new(ev_tx);
        let handle = Handle::current();
        let on_event = move |_| {
            let ev_tx = Arc::clone(&ev_tx);
            handle.spawn(async move {
                let _ = ev_tx.send(()).await;
            });
        };
        let _watcher = match recommended_watcher(on_event).and_then(|mut watcher| {
            watcher.watch(&exe_path, notify::RecursiveMode::NonRecursive)?;
            Ok(watcher)
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                eprintln!("cannot monitor subprocess exe path, retrying: {err}");
                sleep(Duration::from_millis(1000)).await;
                continue;
            }
        };
        let notify_loop = async {
            loop {
                if ev_rx.recv().await.is_none() {
                    break;
                } else if let Ok(ts) = exe_path.metadata().and_then(|m| m.modified()) {
                    if ts != exe_time {
                        break;
                    }
                } else {
                    break;
                }
            }
        };

        let child = Command::new(&exe_path)
            .arg("--__rinja_dynamic")
            .arg(addr)
            .env(DYNAMIC_ENVIRON_KEY, DYNAMIC_ENVIRON_VALUE)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn();
        pending_responses.lock().await.clear();
        let mut child = match child {
            Ok(child) => child,
            Err(err) => {
                eprintln!("could not start child process, retrying: {err}");
                sleep(Duration::from_millis(1000)).await;
                continue;
            }
        };

        let sock = match timeout(Duration::from_secs(5), listener.accept()).await {
            Ok(Ok((sock, _))) => Ok(sock),
            Ok(Err(err)) => {
                eprintln!("could not accept connection of subprocess: {err}");
                Err(())
            }
            Err(_) => {
                eprintln!("timeout while waiting for subprocess connection");
                Err(())
            }
        };
        if let Ok(sock) = sock {
            let sock = sock;
            let _: Result<(), std::io::Error> = sock.set_linger(None);
            let _: Result<(), std::io::Error> = sock.set_nodelay(true);
            let (sock_read, mut sock_write) = sock.into_split();
            let _: Result<(), _> = stdout_tx.send(sock_read).await;

            let pending_responses_ = Arc::clone(&pending_responses);
            let read_loop = async move {
                let pending_responses = pending_responses_;
                loop {
                    let Request {
                        callid,
                        request,
                        response,
                        ..
                    } = *QUEUE
                        .get_channel()
                        .await
                        .1
                        .lock()
                        .await
                        .recv()
                        .await
                        .unwrap();
                    pending_responses.lock().await.insert(callid, response);

                    if let Err(err) = sock_write.write_all(request.as_bytes()).await {
                        return err;
                    }
                    if let Err(err) = sock_write.flush().await {
                        return err;
                    }
                }
            };

            eprintln!("subprocess ready");
            select! {
                biased;
                exit_code = child.wait() => match exit_code {
                    Ok(exit_status) => {
                        eprintln!("subprocess died with {exit_status}, restarting");
                    },
                    Err(err) => {
                        eprintln!("could not query subprocess exit status, restarting: {err}");
                    }
                },
                err = read_loop => {
                    eprintln!("could not read from subprocess, restarting: {err}");
                },
                _ = notify_loop => {
                    eprintln!("subprocess exe was changed, restarting");
                }
            }
        };

        eprintln!("stopping subprocess");
        let deadline = Instant::now() + Duration::from_millis(250);
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.start_kill();
            if !matches!(timeout_at(deadline, child.wait()).await, Ok(Ok(_))) {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }
        sleep_until(deadline).await;
    }
}

type RequestChannel = Mutex<
    Option<
        Arc<(
            Mutex<mpsc::Sender<Box<Request>>>,
            Mutex<mpsc::Receiver<Box<Request>>>,
        )>,
    >,
>;

struct Queue {
    callid: AtomicU64,
    channel: RequestChannel,
}

struct Request {
    callid: u64,
    request: String,
    response: RequestResponse,
}

type RequestResponse = oneshot::Sender<Result<String, Error>>;

impl Queue {
    const fn new() -> Self {
        Self {
            callid: AtomicU64::new(0),
            channel: Mutex::const_new(None),
        }
    }

    async fn get_channel(
        &self,
    ) -> Arc<(
        Mutex<mpsc::Sender<Box<Request>>>,
        Mutex<mpsc::Receiver<Box<Request>>>,
    )> {
        Arc::clone(match &mut *self.channel.lock().await {
            Some(channel) => channel,
            channel @ None => {
                let (tx, rx) = mpsc::channel(QUEUE_SIZE);
                *channel = Some(Arc::new((Mutex::new(tx), Mutex::new(rx))));
                channel.as_ref().unwrap()
            }
        })
    }
}
