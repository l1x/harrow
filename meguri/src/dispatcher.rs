//! Dispatcher: maps user_data to wakers and reclaim actions.
//!
//! When an operation is submitted, its `user_data` is registered here with
//! the associated `Waker`. When the completion arrives, the dispatcher
//! wakes the task. If the operation was dropped before completion, the
//! dispatcher routes the buffer to the reclaim pool.

use std::task::Waker;

use slab::Slab;

use crate::reclaim::ReclaimPool;

/// State for a pending operation.
pub enum OpState {
    /// Operation is pending completion. Waker will be called on completion.
    Pending { waker: Option<Waker> },
    /// Operation was dropped before completion. The buffer must be reclaimed.
    Dropped { reclaim_key: u64 },
}

/// Maps user_data (slab key) to pending operation state.
///
/// Uses a `Slab` for O(1) register/complete/lookup — no hashing overhead
/// on the hot completion path.
pub struct Dispatcher {
    pending: Slab<OpState>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            pending: Slab::with_capacity(256),
        }
    }

    /// Allocate a new user_data and register the operation as pending.
    /// Returns the user_data (slab key) to attach to the SQE.
    pub fn register(&mut self, waker: Option<Waker>) -> u64 {
        self.pending.insert(OpState::Pending { waker }) as u64
    }

    /// Mark an operation as dropped (future dropped before completion).
    /// The buffer will be reclaimed when the completion arrives.
    pub fn mark_dropped(&mut self, user_data: u64, reclaim_key: u64) {
        let key = user_data as usize;
        if let Some(state) = self.pending.get_mut(key) {
            *state = OpState::Dropped { reclaim_key };
        }
    }

    /// Process a completion. Wakes the task if pending, or reclaims the
    /// buffer if the operation was dropped.
    pub fn complete(&mut self, user_data: u64, res: i32, _flags: u32, reclaim: &mut ReclaimPool) {
        let key = user_data as usize;
        if !self.pending.contains(key) {
            // Completion for an unknown user_data — can happen for
            // internal operations (eventfd, timeout).
            return;
        }
        match self.pending.remove(key) {
            OpState::Pending { waker } => {
                if let Some(w) = waker {
                    w.wake();
                }
            }
            OpState::Dropped { reclaim_key } => {
                reclaim.reclaim(reclaim_key, res);
            }
        }
    }

    /// Update the waker for a pending operation (called from Future::poll).
    pub fn update_waker(&mut self, user_data: u64, waker: &Waker) {
        let key = user_data as usize;
        if let Some(OpState::Pending { waker: stored }) = self.pending.get_mut(key) {
            match stored {
                Some(existing) if existing.will_wake(waker) => {}
                _ => *stored = Some(waker.clone()),
            }
        }
    }

    /// Check if an operation is still pending.
    pub fn is_pending(&self, user_data: u64) -> bool {
        self.pending.contains(user_data as usize)
    }
}
