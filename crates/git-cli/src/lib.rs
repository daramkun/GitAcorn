//! System Git infrastructure shared by application use cases.

const REDACTED: &str = "***";

/// Removes user information from HTTP(S) remote URLs before diagnostics are emitted.
pub fn redact_remote(remote: &str) -> String {
    let Some(scheme_end) = remote.find("://") else {
        return remote.to_owned();
    };
    let authority_start = scheme_end + 3;
    let Some(relative_at) = remote[authority_start..].find('@') else {
        return remote.to_owned();
    };
    let at = authority_start + relative_at;

    format!(
        "{}{}{}",
        &remote[..authority_start],
        REDACTED,
        &remote[at..]
    )
}

#[cfg(test)]
mod tests {
    use super::redact_remote;

    #[test]
    fn masks_credentials_in_https_remote() {
        assert_eq!(
            redact_remote("https://alice:secret@example.com/org/repo.git"),
            "https://***@example.com/org/repo.git"
        );
    }

    #[test]
    fn preserves_remote_without_inline_credentials() {
        assert_eq!(
            redact_remote("https://example.com/org/repo.git"),
            "https://example.com/org/repo.git"
        );
    }
}
