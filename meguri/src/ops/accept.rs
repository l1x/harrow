//! Accept operation.

use std::future::Future;
use std::net::SocketAddr;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::error::MeguriResult;

/// A pending accept operation.
pub struct Accept {
    fd: RawFd,
    user_data: Option<u64>,
    result: Option<MeguriResult<(RawFd, SocketAddr)>>,
}

impl Accept {
    pub fn new(fd: RawFd) -> Self {
        Self {
            fd,
            user_data: None,
            result: None,
        }
    }
}

impl Future for Accept {
    type Output = MeguriResult<(RawFd, SocketAddr)>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }
        Poll::Pending
    }
}
