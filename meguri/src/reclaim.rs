//! Buffer reclaim mechanism.
//!
//! When an operation's Future is dropped before the kernel completes,
//! the buffer must not be freed until the kernel is done. The reclaim
//! pool holds these buffers until they can be safely returned.

use slab::Slab;

/// A reclaimable resource (typically a buffer).
pub trait Reclaimable: Send + 'static {
    /// Called when the kernel has finished with this resource.
    fn reclaim(self: Box<Self>, result: i32);
}

/// Holds buffers/resources for operations that were dropped before completion.
///
/// When the completion arrives, the resource is reclaimed (either returned
/// to a pool or dropped safely). Uses a `Slab` for O(1) keyed access.
pub struct ReclaimPool {
    pending: Slab<Box<dyn Reclaimable>>,
}

impl Default for ReclaimPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ReclaimPool {
    pub fn new() -> Self {
        Self {
            pending: Slab::with_capacity(64),
        }
    }

    /// Register a resource for reclaim.
    /// Returns the key to associate with the operation's user_data.
    pub fn register(&mut self, resource: impl Reclaimable) -> u64 {
        self.pending.insert(Box::new(resource)) as u64
    }

    /// Reclaim a resource by key. Called when the kernel completes.
    pub fn reclaim(&mut self, key: u64, result: i32) {
        let k = key as usize;
        if self.pending.contains(k) {
            let resource = self.pending.remove(k);
            resource.reclaim(result);
        }
    }

    /// Number of pending reclamations.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}
