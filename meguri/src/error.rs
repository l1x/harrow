//! Error types.

use std::fmt;
use std::io;

/// A meguri error.
#[derive(Debug)]
pub enum MeguriError {
    /// An I/O error from a syscall.
    Io(io::Error),
    /// The submission queue is full.
    SqFull,
    /// An operation was cancelled by the kernel.
    Cancelled,
    /// Invalid argument.
    InvalidArgument(&'static str),
}

impl fmt::Display for MeguriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeguriError::Io(e) => write!(f, "io_uring error: {e}"),
            MeguriError::SqFull => write!(f, "submission queue is full"),
            MeguriError::Cancelled => write!(f, "operation was cancelled"),
            MeguriError::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
        }
    }
}

impl std::error::Error for MeguriError {}

impl From<io::Error> for MeguriError {
    fn from(e: io::Error) -> Self {
        MeguriError::Io(e)
    }
}

/// Result type for meguri operations.
pub type MeguriResult<T> = Result<T, MeguriError>;
