//! Async cancel operation.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::error::MeguriResult;

/// Cancel result.
#[derive(Debug, PartialEq)]
pub enum CancelResult {
    Cancelled,
    AlreadyStarted,
    NotFound,
}

/// A pending cancel operation.
pub struct Cancel {
    target_user_data: u64,
    result: Option<MeguriResult<CancelResult>>,
}

impl Cancel {
    pub fn new(target_user_data: u64) -> Self {
        Self {
            target_user_data,
            result: None,
        }
    }
}

impl Future for Cancel {
    type Output = MeguriResult<CancelResult>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this.result.take() {
            return Poll::Ready(result);
        }
        Poll::Pending
    }
}
