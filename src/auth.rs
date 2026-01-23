//! Authentication module for bearer token validation.
//!
//! Provides constant-time token comparison and bearer header extraction.

/// Validates a provided token against the expected token using constant-time comparison.
///
/// This prevents timing attacks by ensuring the comparison takes the same amount
/// of time regardless of where (or if) tokens differ.
///
/// Returns `false` if either token is empty.
pub fn validate_token(provided: &str, expected: &str) -> bool {
    let provided = provided.as_bytes();
    let expected = expected.as_bytes();

    // Empty tokens are never valid
    if provided.is_empty() || expected.is_empty() {
        return false;
    }

    // Length mismatch - still compare to maintain constant time
    // We compare all bytes of the shorter string, then account for length diff
    let len_match = provided.len() == expected.len();

    // XOR accumulator: if any byte differs, result will be non-zero
    let mut diff: u8 = 0;
    for (a, b) in provided.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }

    // Both conditions must be true: same length AND all bytes match
    len_match && diff == 0
}

/// Extracts the bearer token from an Authorization header value.
///
/// Expected format: "Bearer <token>"
/// Returns `None` if the header doesn't match the expected format.
pub fn extract_bearer_token(header: &str) -> Option<&str> {
    let header = header.trim();

    // Case-insensitive "Bearer " prefix check (RFC 6750 allows case-insensitive)
    if header.len() < 7 {
        return None;
    }

    let (prefix, token) = header.split_at(7);
    if prefix.eq_ignore_ascii_case("Bearer ") {
        let token = token.trim();
        if token.is_empty() {
            None
        } else {
            Some(token)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_token_matching() {
        assert!(validate_token("secret123", "secret123"));
        assert!(validate_token("a", "a"));
        assert!(validate_token(
            "very-long-token-with-special-chars!@#$%",
            "very-long-token-with-special-chars!@#$%"
        ));
    }

    #[test]
    fn test_validate_token_mismatch() {
        assert!(!validate_token("secret123", "secret124"));
        assert!(!validate_token("secret123", "SECRET123"));
        assert!(!validate_token("short", "longer"));
        assert!(!validate_token("longer", "short"));
    }

    #[test]
    fn test_validate_token_empty() {
        assert!(!validate_token("", ""));
        assert!(!validate_token("", "secret"));
        assert!(!validate_token("secret", ""));
    }

    #[test]
    fn test_extract_bearer_token_valid() {
        assert_eq!(extract_bearer_token("Bearer secret123"), Some("secret123"));
        assert_eq!(extract_bearer_token("bearer secret123"), Some("secret123"));
        assert_eq!(extract_bearer_token("BEARER secret123"), Some("secret123"));
        assert_eq!(extract_bearer_token("  Bearer secret123  "), Some("secret123"));
        assert_eq!(extract_bearer_token("Bearer   token-with-spaces  "), Some("token-with-spaces"));
    }

    #[test]
    fn test_extract_bearer_token_invalid() {
        assert_eq!(extract_bearer_token(""), None);
        assert_eq!(extract_bearer_token("Basic secret123"), None);
        assert_eq!(extract_bearer_token("Bearer"), None);
        assert_eq!(extract_bearer_token("Bearer "), None);
        assert_eq!(extract_bearer_token("Bearersecret123"), None);
        assert_eq!(extract_bearer_token("secret123"), None);
    }
}
