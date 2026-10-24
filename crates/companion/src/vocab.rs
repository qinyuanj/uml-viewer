//! Companion domain vocabulary: the error type, the creation identity of a
//! process and the outcome of a bounded command. No I/O happens here; the
//! POSIX boundary produces these values and the pure-logic modules consume
//! them without importing it.

use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, Failure>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub message: String,
    pub reason: Option<&'static str>,
}

impl Failure {
    pub fn new(message: impl Into<String>) -> Failure {
        Failure {
            message: message.into(),
            reason: None,
        }
    }

    pub fn with_reason(message: impl Into<String>, reason: &'static str) -> Failure {
        Failure {
            message: message.into(),
            reason: Some(reason),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Failure {
        Failure::new(error.to_string())
    }
}

impl From<edn::Error> for Failure {
    fn from(error: edn::Error) -> Failure {
        Failure::new(error.to_string())
    }
}

/// Exact creation identity of a live process; `None` when absent or a zombie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub pid: i32,
    pub start: String,
    pub name: String,
}

/// Result of a bounded synchronous command (port of `Process.run`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOut {
    pub exit: Option<i32>,
    pub out: String,
    pub err: String,
    pub timeout: bool,
}
