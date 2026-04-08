//! Timeout operation.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::error::MeguriResult;

/// A pending timeout operation.
pub struct Timeout {
    duration: Duration,
    user_data: Option<u64>,
    result: Option<MeguriResult<()>>,
}

impl Timeout {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            user_data: None,
            result: None,
        }
    }
}

impl Future for Timeout {
    type Output = MeguriResult<()>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }
        Poll::Pending
    }
}
