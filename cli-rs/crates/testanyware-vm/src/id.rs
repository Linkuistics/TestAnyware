//! VM instance identifiers: `testanyware-<8 hex digits>` (contract §6).

/// Generate a fresh `testanyware-<hex8>` identifier. Ports
/// `VMStartOptions.generateID()` — 4 random bytes rendered lowercase hex.
pub fn generate_id() -> String {
    let mut bytes = [0u8; 4];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("testanyware-{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_has_expected_shape() {
        let id = generate_id();
        assert!(id.starts_with("testanyware-"), "got {id}");
        let hex = id.strip_prefix("testanyware-").unwrap();
        assert_eq!(hex.len(), 8, "8 hex chars: {id}");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }

    #[test]
    fn ids_are_distinct() {
        let a = generate_id();
        let b = generate_id();
        assert_ne!(a, b, "two ids collided (1-in-4-billion fluke, re-run)");
    }
}
