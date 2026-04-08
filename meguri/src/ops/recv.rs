//! Recv operation.

use std::future::Future;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;

use crate::error::MeguriResult;

/// A pending recv operation.
pub struct Recv {
    fd: RawFd,
    buf: BytesMut,
    flags: u32,
    user_data: Option<u64>,
    result: Option<MeguriResult<(BytesMut, usize)>>,
}

impl Recv {
    pub fn new(fd: RawFd, buf: BytesMut, flags: u32) -> Self {
        Self {
            fd,
            buf,
            flags,
            user_data: None,
            result: None,
        }
    }
}

impl Future for Recv {
    type Output = MeguriResult<(BytesMut, usize)>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }
        Poll::Pending
    }
}
