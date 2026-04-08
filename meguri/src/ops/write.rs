//! Write operation.
//!
//! Owns the write buffer until completion.

use std::future::Future;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;

use crate::error::MeguriResult;

/// A pending write operation.
pub struct Write {
    fd: RawFd,
    buf: Bytes,
    offset: u64,
    user_data: Option<u64>,
    submitted: bool,
    result: Option<MeguriResult<usize>>,
}

impl Write {
    pub fn new(fd: RawFd, buf: Bytes, offset: u64) -> Self {
        Self {
            fd,
            buf,
            offset,
            user_data: None,
            submitted: false,
            result: None,
        }
    }

    pub fn submit(&mut self, _ring: &mut crate::ring::Ring) -> MeguriResult<()> {
        // Placeholder: push to SQ in production.
        self.submitted = true;
        Ok(())
    }
}

impl Future for Write {
    type Output = MeguriResult<usize>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }
        Poll::Pending
    }
}
