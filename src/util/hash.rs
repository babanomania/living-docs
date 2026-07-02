/// Stable content hash used for symbol ids and (later) managed-block
/// hashes: hex-encoded blake3, never derived from line numbers or mtimes,
/// so it only changes when what it names actually changes.
pub fn stable_id(input: &str) -> String {
    blake3::hash(input.as_bytes()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_produces_same_id() {
        assert_eq!(
            stable_id("src/user.ts#UserService"),
            stable_id("src/user.ts#UserService")
        );
    }

    #[test]
    fn different_input_produces_different_id() {
        assert_ne!(
            stable_id("src/user.ts#UserService"),
            stable_id("src/user.ts#PolicyService")
        );
    }
}
