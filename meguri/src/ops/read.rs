//! Read operation.
//!
//! Owns the read buffer. On completion, returns the buffer and bytes read.

use std::future::Future;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;

use crate::dispatcher::Dispatcher;
use crate::error::MeguriResult;
use crate::ring::Ring;

/// A pending read operation.
///
/// # Buffer ownership
/// The operation owns the read buffer. If the future is dropped before
/// completion, the buffer is registered with the reclaim pool and will
/// not be freed until the kernel finishes.
pub struct Read {
    fd: RawFd,
    buf: BytesMut,
    offset: u64,
    user_data: Option<u64>,
    submitted: bool,
    result: Option<MeguriResult<(BytesMut, usize)>>,
}

impl Read {
    /// Create a new read operation.
    pub fn new(fd: RawFd, buf: BytesMut, offset: u64) -> Self {
        Self {
            fd,
            buf,
            offset,
            user_data: None,
            submitted: false,
            result: None,
        }
    }

    /// Submit the read to the ring.
    pub fn submit(&mut self, ring: &mut Ring) -> MeguriResult<()> {
        let ud = ring.dispatcher().register(None);
        self.user_data = Some(ud);

        let buf_ptr = self.buf.as_mut_ptr();
        let buf_len = self.buf.len() as u32;

        if !ring
            .sq()
            .push_read(ud, self.fd, buf_ptr, buf_len, self.offset)
        {
            return Err(crate::error::MeguriError::SqFull);
        }

        self.submitted = true;
        Ok(())
    }
}

impl Future for Read {
    type Output = MeguriResult<(BytesMut, usize)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Check if result is already available (set by dispatcher).
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }

        // Register or update waker.
        if let Some(ud) = this.user_data {
            this.dispatcher_mut().update_waker(ud, cx.waker());
        }

        Poll::Pending
    }
}

// Helper to access dispatcher from within the future.
// In a full implementation, this would be handled via a shared reference.
impl Read {
    fn dispatcher_mut(&mut self) -> &mut Dispatcher {
        // Placeholder: in production, this holds a reference to the ring's
        // dispatcher. The skeleton uses a thread-local for now.
        unimplemented!("dispatcher access — requires ring reference")
    }
}
