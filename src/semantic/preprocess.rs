//! Content preprocessing for embedding generation.
//!
//! Prepares bookmark content for embedding:
//! 1. Trim whitespace
//! 2. Skip if both title and description are empty
//! 3. Concatenate title, description, tags, and URL domain
//! 4. Truncate to max length with ellipsis
//!
//! Output format: `"{title} | {description} | tags: {tags} | site: {domain}"`
//! Sections are omitted when empty (no "tags:" if no tags, no "site:" if URL invalid)

/// Maximum content length for embedding input (characters, not tokens)
const MAX_CONTENT_LENGTH: usize = 512;

/// Ellipsis suffix when content is truncated
const TRUNCATION_SUFFIX: &str = "...";

/// Preprocess bookmark content for embedding generation.
///
/// Combines title, description, tags, and URL domain into a single string
/// optimized for semantic search. Returns `None` if both title and description
/// are empty after trimming (tags/URL alone don't constitute searchable content).
///
/// # Arguments
/// * `title` - Bookmark title
/// * `description` - Bookmark description
/// * `tags` - Bookmark tags (will be joined with ", ")
/// * `url` - Bookmark URL (domain will be extracted)
///
/// # Format
/// `"{title} | {description} | tags: {tags} | site: {domain}"`
/// - Sections are omitted when empty
/// - Domain extraction: `https://docs.github.com/page` → `github`
pub fn preprocess_content(title: &str, description: &str, tags: &[String], url: &str) -> Option<String> {
    let title = title.trim();
    let description = description.trim();

    // Tags and URL alone don't constitute searchable content
    if title.is_empty() && description.is_empty() {
        return None;
    }

    let mut parts: Vec<&str> = Vec::with_capacity(4);

    // Add title if present
    if !title.is_empty() {
        parts.push(title);
    }

    // Add description if present
    if !description.is_empty() {
        parts.push(description);
    }

    let mut content = parts.join(" | ");

    // Add tags section if tags present
    if !tags.is_empty() {
        let tags_str = tags.join(", ");
        content.push_str(" | tags: ");
        content.push_str(&tags_str);
    }

    // Add site domain if URL is valid
    if let Some(domain) = extract_domain(url) {
        content.push_str(" | site: ");
        content.push_str(&domain);
    }

    Some(truncate_content(&content))
}

/// Extract the main domain name from a URL.
///
/// Returns the second-level domain (e.g., "github" from "docs.github.com").
/// Returns `None` for invalid URLs or URLs without a proper host.
fn extract_domain(url: &str) -> Option<String> {
    // Parse URL
    let parsed = url::Url::parse(url).ok()?;

    // Get host (returns None for non-http(s) URLs without host)
    let host = parsed.host_str()?;

    // Split by dots and extract second-level domain
    let parts: Vec<&str> = host.split('.').collect();

    // Handle different cases:
    // - "github.com" -> ["github", "com"] -> "github"
    // - "docs.github.com" -> ["docs", "github", "com"] -> "github"
    // - "localhost" -> ["localhost"] -> "localhost"
    // - "192.168.1.1" -> skip (IP address)

    // Skip IP addresses (all parts are numeric)
    if parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        return None;
    }

    match parts.len() {
        0 => None,
        1 => Some(parts[0].to_string()), // localhost
        _ => {
            // Get second-to-last part (second-level domain)
            // For "docs.github.com", this is "github"
            let sld = parts[parts.len() - 2];
            Some(sld.to_string())
        }
    }
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
///
/// Includes title, description, tags, and URL to detect any changes
/// that would affect the embedding.
pub fn content_hash(title: &str, description: &str, tags: &[String], url: &str) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    title.trim().hash(&mut hasher);
    description.trim().hash(&mut hasher);
    tags.hash(&mut hasher);
    url.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_content_returns_none() {
        assert!(preprocess_content("", "", &[], "").is_none());
        assert!(preprocess_content("   ", "   ", &[], "").is_none());
        assert!(preprocess_content("\n\t", "  \r\n", &[], "").is_none());
    }

    #[test]
    fn test_title_only() {
        let result = preprocess_content("Hello World", "", &[], "");
        assert_eq!(result, Some("Hello World".to_string()));
    }

    #[test]
    fn test_description_only() {
        let result = preprocess_content("", "A description", &[], "");
        assert_eq!(result, Some("A description".to_string()));
    }

    #[test]
    fn test_both_title_and_description() {
        let result = preprocess_content("Title", "Description", &[], "");
        assert_eq!(result, Some("Title | Description".to_string()));
    }

    #[test]
    fn test_trims_whitespace() {
        let result = preprocess_content("  Title  ", "  Description  ", &[], "");
        assert_eq!(result, Some("Title | Description".to_string()));
    }

    #[test]
    fn test_truncation() {
        let long_content = "x".repeat(600);
        let result = preprocess_content(&long_content, "", &[], "");

        assert!(result.is_some());
        let content = result.unwrap();
        assert!(content.len() <= MAX_CONTENT_LENGTH);
        assert!(content.ends_with(TRUNCATION_SUFFIX));
    }

    #[test]
    fn test_no_truncation_for_short_content() {
        let short = "Short title";
        let result = preprocess_content(short, "", &[], "");
        assert_eq!(result, Some(short.to_string()));
    }

    // Tests for new tags and URL functionality
    #[test]
    fn test_with_tags() {
        let tags = vec!["rust".to_string(), "cli".to_string()];
        let result = preprocess_content("Title", "Description", &tags, "");
        assert_eq!(result, Some("Title | Description | tags: rust, cli".to_string()));
    }

    #[test]
    fn test_with_empty_tags() {
        let result = preprocess_content("Title", "Description", &[], "");
        // Empty tags should not add "tags:" section
        assert_eq!(result, Some("Title | Description".to_string()));
    }

    #[test]
    fn test_with_url_domain() {
        let result = preprocess_content("Title", "Description", &[], "https://github.com/user/repo");
        assert_eq!(result, Some("Title | Description | site: github".to_string()));
    }

    #[test]
    fn test_with_url_subdomain() {
        // Should extract main domain, not subdomain
        let result = preprocess_content("Title", "Description", &[], "https://docs.github.com/page");
        assert_eq!(result, Some("Title | Description | site: github".to_string()));
    }

    #[test]
    fn test_with_invalid_url() {
        // Invalid URLs should not add "site:" section
        let result = preprocess_content("Title", "Description", &[], "not-a-url");
        assert_eq!(result, Some("Title | Description".to_string()));
    }

    #[test]
    fn test_with_tags_and_url() {
        let tags = vec!["rust".to_string()];
        let result = preprocess_content("Title", "Description", &tags, "https://crates.io/crate");
        assert_eq!(result, Some("Title | Description | tags: rust | site: crates".to_string()));
    }

    #[test]
    fn test_tags_only_no_title_description() {
        let tags = vec!["tag1".to_string()];
        let result = preprocess_content("", "", &tags, "");
        // Tags alone should not produce content (need title or description)
        assert!(result.is_none());
    }

    #[test]
    fn test_content_hash_consistency() {
        let hash1 = content_hash("Title", "Description", &[], "");
        let hash2 = content_hash("Title", "Description", &[], "");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_different_for_different_content() {
        let hash1 = content_hash("Title A", "Description", &[], "");
        let hash2 = content_hash("Title B", "Description", &[], "");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_trims() {
        let hash1 = content_hash("  Title  ", "  Description  ", &[], "");
        let hash2 = content_hash("Title", "Description", &[], "");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_includes_tags() {
        let tags1 = vec!["rust".to_string()];
        let tags2 = vec!["python".to_string()];
        let hash1 = content_hash("Title", "Description", &tags1, "");
        let hash2 = content_hash("Title", "Description", &tags2, "");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_includes_url() {
        let hash1 = content_hash("Title", "Description", &[], "https://a.com");
        let hash2 = content_hash("Title", "Description", &[], "https://b.com");
        assert_ne!(hash1, hash2);
    }
}
