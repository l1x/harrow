/// Generate a random request ID.
/// Uses OS randomness via `getrandom` for unpredictable, non-sequential IDs.
/// Format: `hrw-` followed by 16 lowercase hex digits (64 bits of entropy).
pub fn generate() -> String {
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("getrandom failed");
    let id = u64::from_ne_bytes(buf);
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

    #[test]
    fn correct_length() {
        let id = generate();
        // "hrw-" (4) + 16 hex digits = 20 chars
        assert_eq!(id.len(), 20);
    }
}
