use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique request ID.
/// Uses a monotonic counter combined with a process-unique prefix.
/// Not globally unique across processes — use UUIDs if you need that.
pub fn generate() -> String {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("hrw-{id:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_ids() {
        let a = generate();
        let b = generate();
        assert_ne!(a, b);
        assert!(a.starts_with("hrw-"));
    }
}
