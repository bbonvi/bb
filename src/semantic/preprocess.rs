//! Content preprocessing for embedding generation.
//!
//! Prepares bookmark title and description for embedding:
//! 1. Trim whitespace
//! 2. Skip if both empty
//! 3. Concatenate with separator
//! 4. Truncate to max length with ellipsis

/// Maximum content length for embedding input (characters, not tokens)
const MAX_CONTENT_LENGTH: usize = 512;

/// Ellipsis suffix when content is truncated
const TRUNCATION_SUFFIX: &str = "...";

/// Preprocess title and description for embedding generation.
///
/// Returns `None` if both title and description are empty after trimming.
/// Otherwise, concatenates them and truncates to `MAX_CONTENT_LENGTH`.
pub fn preprocess_content(title: &str, description: &str) -> Option<String> {
    let title = title.trim();
    let description = description.trim();

    if title.is_empty() && description.is_empty() {
        return None;
    }

    let content = if title.is_empty() {
        description.to_string()
    } else if description.is_empty() {
        title.to_string()
    } else {
        format!("{} - {}", title, description)
    };

    Some(truncate_content(&content))
}

/// Truncate content to MAX_CONTENT_LENGTH, adding ellipsis if truncated.
fn truncate_content(content: &str) -> String {
    if content.len() <= MAX_CONTENT_LENGTH {
        return content.to_string();
    }

    // Find a safe truncation point (don't break UTF-8 sequences)
    let max_chars = MAX_CONTENT_LENGTH - TRUNCATION_SUFFIX.len();
    let truncated: String = content.chars().take(max_chars).collect();

    format!("{}{}", truncated, TRUNCATION_SUFFIX)
}

/// Compute a hash of the content for change detection.
/// Used to determine if a bookmark needs re-embedding.
pub fn content_hash(title: &str, description: &str) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    title.trim().hash(&mut hasher);
    description.trim().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_content_returns_none() {
        assert!(preprocess_content("", "").is_none());
        assert!(preprocess_content("   ", "   ").is_none());
        assert!(preprocess_content("\n\t", "  \r\n").is_none());
    }

    #[test]
    fn test_title_only() {
        let result = preprocess_content("Hello World", "");
        assert_eq!(result, Some("Hello World".to_string()));
    }

    #[test]
    fn test_description_only() {
        let result = preprocess_content("", "A description");
        assert_eq!(result, Some("A description".to_string()));
    }

    #[test]
    fn test_both_title_and_description() {
        let result = preprocess_content("Title", "Description");
        assert_eq!(result, Some("Title - Description".to_string()));
    }

    #[test]
    fn test_trims_whitespace() {
        let result = preprocess_content("  Title  ", "  Description  ");
        assert_eq!(result, Some("Title - Description".to_string()));
    }

    #[test]
    fn test_truncation() {
        let long_content = "x".repeat(600);
        let result = preprocess_content(&long_content, "");

        assert!(result.is_some());
        let content = result.unwrap();
        assert!(content.len() <= MAX_CONTENT_LENGTH);
        assert!(content.ends_with(TRUNCATION_SUFFIX));
    }

    #[test]
    fn test_no_truncation_for_short_content() {
        let short = "Short title";
        let result = preprocess_content(short, "");
        assert_eq!(result, Some(short.to_string()));
    }

    #[test]
    fn test_content_hash_consistency() {
        let hash1 = content_hash("Title", "Description");
        let hash2 = content_hash("Title", "Description");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_different_for_different_content() {
        let hash1 = content_hash("Title A", "Description");
        let hash2 = content_hash("Title B", "Description");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_trims() {
        let hash1 = content_hash("  Title  ", "  Description  ");
        let hash2 = content_hash("Title", "Description");
        assert_eq!(hash1, hash2);
    }
}
