//! Shared temporary repository fixtures begin in M1.

/// Returns a deterministic identity for Git integration-test commits.
pub const fn fixture_identity() -> (&'static str, &'static str) {
    ("GitAcorn Test", "test@gitacorn.local")
}

#[cfg(test)]
mod tests {
    use super::fixture_identity;

    #[test]
    fn fixture_identity_is_stable() {
        assert_eq!(fixture_identity(), ("GitAcorn Test", "test@gitacorn.local"));
    }
}
