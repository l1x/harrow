//! Connect operation.

use std::future::Future;
use std::net::SocketAddr;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::error::MeguriResult;

/// A pending connect operation.
pub struct Connect {
    fd: RawFd,
    addr: SocketAddr,
    user_data: Option<u64>,
    result: Option<MeguriResult<()>>,
}

impl Connect {
    pub fn new(fd: RawFd, addr: SocketAddr) -> Self {
        Self {
            fd,
            addr,
            user_data: None,
            result: None,
        }
    }
}

impl Future for Connect {
    type Output = MeguriResult<()>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }
        Poll::Pending
    }
}
