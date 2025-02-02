pub(crate) mod child;
pub(crate) mod parent;

use std::borrow::Cow;
use std::env::var_os;
use std::process::exit;
use std::sync::OnceLock;
use std::{eprintln, fmt};

use serde::{Deserialize, Serialize};

const DYNAMIC_ENVIRON_KEY: &str = "__rinja_dynamic";
const DYNAMIC_ENVIRON_VALUE: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize)]
struct MainRequest<'a> {
    callid: u64,
    #[serde(borrow)]
    name: Cow<'a, str>,
    data: Cow<'a, str>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MainResponse<'a> {
    callid: u64,
    #[serde(borrow, flatten)]
    outcome: Outcome<'a>,
}

/// The outcome of a dynamic template call.
#[derive(Debug, Serialize, Deserialize)]
pub enum Outcome<'a> {
    /// The template was rendered correctly.
    #[serde(borrow)]
    Success(Cow<'a, str>),
    /// The JSON serialized template could not be deserialized.
    #[serde(borrow)]
    Deserialize(Cow<'a, str>),
    /// The template was not rendered correctly.
    #[serde(borrow)]
    Render(Cow<'a, str>),
    /// The template's type name was not known to the subprocess.
    NotFound,
    /// An error occurred but the error could not be printed.
    Fmt,
}

impl std::error::Error for Outcome<'_> {}

impl fmt::Display for Outcome<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Outcome::Success(_) => write!(f, "not an error"),
            Outcome::Deserialize(err) => write!(f, "could not deserialize: {err}"),
            Outcome::Render(err) => write!(f, "could not render: {err}"),
            Outcome::NotFound => write!(f, "template not found"),
            Outcome::Fmt => write!(f, "could not format error message"),
        }
    }
}

/// True if the current process is a dynamic subprocess.
#[inline]
#[track_caller]
fn am_dynamic_child() -> bool {
    #[inline(never)]
    #[cold]
    #[track_caller]
    fn uninitialized() -> bool {
        unreachable!("init_am_dynamic_child() was never called");
    }

    *AM_DYNAMIC_CHILD.get_or_init(uninitialized)
}

pub(crate) fn init_am_dynamic_child() -> bool {
    let value = if let Some(var) = var_os(DYNAMIC_ENVIRON_KEY) {
        let Some(var) = var.to_str() else {
            eprintln!("Environment variable {DYNAMIC_ENVIRON_KEY} does not contain UTF-8 data");
            exit(1);
        };
        match var {
            DYNAMIC_ENVIRON_VALUE => true,
            "" => false,
            var => {
                eprintln!(
                    "\
                    Environment variable {DYNAMIC_ENVIRON_KEY} contains wrong value. \
                    Expected: {DYNAMIC_ENVIRON_VALUE:?}, actual: {var:?}"
                );
                exit(1);
            }
        }
    } else {
        false
    };

    AM_DYNAMIC_CHILD.set(value).unwrap();
    value
}

static AM_DYNAMIC_CHILD: OnceLock<bool> = OnceLock::new();
