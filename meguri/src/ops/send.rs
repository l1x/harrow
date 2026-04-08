//! Send operation.

use std::future::Future;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;

use crate::error::MeguriResult;

/// A pending send operation.
pub struct Send {
    fd: RawFd,
    buf: Bytes,
    flags: u32,
    user_data: Option<u64>,
    result: Option<MeguriResult<usize>>,
}

impl Send {
    pub fn new(fd: RawFd, buf: Bytes, flags: u32) -> Self {
        Self {
            fd,
            buf,
            flags,
            user_data: None,
            result: None,
        }
    }
}

impl Future for Send {
    type Output = MeguriResult<usize>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }
        Poll::Pending
    }
}
