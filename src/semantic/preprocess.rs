//! Content preprocessing for embedding generation.
//!
//! Prepares bookmark content for embedding:
//! 1. Trim whitespace
//! 2. Skip if both title and description are empty
//! 3. Concatenate title (repeated for emphasis), description, tags, and URL keywords
//! 4. Truncate to max length with ellipsis
//!
//! Output format: `"{title}. {title}. {description}. {tags}. {url_keywords}"`
//! - Title repeated to emphasize primary signal
//! - Tags as plain words (no "tags:" prefix)
//! - URL keywords extracted from domain and path segments
//! - Sections omitted when empty

/// Maximum content length for embedding input (characters, not tokens)
const MAX_CONTENT_LENGTH: usize = 512;

/// Ellipsis suffix when content is truncated
const TRUNCATION_SUFFIX: &str = "...";

/// Common HTML entities to decode
const HTML_ENTITIES: &[(&str, &str)] = &[
    ("&amp;", "&"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&apos;", "'"),
    ("&#39;", "'"),
    ("&nbsp;", " "),
    ("&#160;", " "),
    ("&ndash;", "-"),
    ("&mdash;", "-"),
    ("&hellip;", "..."),
];

/// Preprocess bookmark content for embedding generation.
///
/// Combines title, description, tags, and URL keywords into a single string
/// optimized for semantic search. Returns `None` if both title and description
/// are empty after trimming (tags/URL alone don't constitute searchable content).
///
/// # Arguments
/// * `title` - Bookmark title
/// * `description` - Bookmark description
/// * `tags` - Bookmark tags
/// * `url` - Bookmark URL (keywords extracted from domain and path)
///
/// # Format
/// `"{title}. {title}. {description}. {tags}. {url_keywords}"`
/// - Title repeated to emphasize primary signal
/// - Tags as space-separated words (cleaner for embeddings)
/// - URL keywords from domain + path segments
/// - Sections omitted when empty
pub fn preprocess_content(title: &str, description: &str, tags: &[String], url: &str) -> Option<String> {
    let title = sanitize_text(title);
    let description = sanitize_text(description);

    // Tags and URL alone don't constitute searchable content
    if title.is_empty() && description.is_empty() {
        return None;
    }

    let mut parts: Vec<String> = Vec::with_capacity(5);

    // Add title twice for emphasis (title is the strongest signal)
    if !title.is_empty() {
        parts.push(title.clone());
        parts.push(title);
    }

    // Add description if present
    if !description.is_empty() {
        parts.push(description);
    }

    // Add tags as plain words (no "tags:" prefix noise)
    if !tags.is_empty() {
        parts.push(tags.join(" "));
    }

    // Add URL keywords (domain + path segments)
    let url_keywords = extract_url_keywords(url);
    if !url_keywords.is_empty() {
        parts.push(url_keywords);
    }

    let content = parts.join(". ");
    Some(truncate_content(&content))
}

/// Words to filter out from URL extraction (noise).
const URL_STOP_WORDS: &[&str] = &[
    // TLDs and common URL parts
    "com", "org", "net", "io", "dev", "co", "uk", "de", "fr", "jp", "cn",
    "www", "http", "https", "html", "htm", "php", "asp", "jsp", "cgi",
    // Common URL path noise
    "en", "us", "index", "home", "page", "pages", "post", "posts",
    "article", "articles", "blog", "docs", "doc", "documentation",
    "wiki", "help", "faq", "about", "contact", "login", "signin",
    "signup", "register", "search", "tag", "tags", "category",
    "categories", "archive", "archives", "feed", "rss", "api", "v1", "v2", "v3",
    // Tracking/technical params
    "utm", "ref", "src", "id", "amp",
];

/// Extract keywords from URL (domain + path segments).
///
/// Extracts meaningful words from:
/// - Subdomain (e.g., "docs" from "docs.github.com")
/// - Domain name (e.g., "github" from "github.com")
/// - Path segments (e.g., "tokio", "async" from "/tokio-rs/tokio-async")
///
/// Filters out TLDs, common noise words, numbers, and short words (<3 chars).
/// Returns deduplicated, space-separated keywords.
fn extract_url_keywords(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return String::new(),
    };

    let mut keywords: Vec<String> = Vec::new();

    // Extract from host (subdomain + domain)
    // Split by '.' first, then by '-' to handle domains like "rust-lang.github.io"
    if let Some(host) = parsed.host_str() {
        for part in host.split('.') {
            for word in part.split('-') {
                if is_meaningful_word(word) {
                    keywords.push(word.to_lowercase());
                }
            }
        }
    }

    // Extract from path segments
    let path = parsed.path();
    for segment in path.split('/') {
        // Strip common file extensions before processing
        let segment = segment
            .trim_end_matches(".html")
            .trim_end_matches(".htm")
            .trim_end_matches(".php")
            .trim_end_matches(".asp")
            .trim_end_matches(".jsp");

        // Keep the full segment if it's meaningful (e.g., "rust-by-example")
        if is_meaningful_word(segment) {
            keywords.push(segment.to_lowercase());
        }

        // Also split by common separators and keep individual words
        for word in segment.split(|c| c == '-' || c == '_' || c == '.') {
            if is_meaningful_word(word) {
                keywords.push(word.to_lowercase());
            }
        }
    }

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    keywords.retain(|w| seen.insert(w.clone()));

    keywords.join(" ")
}

/// Check if a word is meaningful for embedding.
/// Filters out noise: short words, numbers, stop words.
fn is_meaningful_word(word: &str) -> bool {
    // Too short
    if word.len() < 3 {
        return false;
    }

    // Pure numbers (but allow alphanumeric like "rust2024")
    if word.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // Stop words
    let lower = word.to_lowercase();
    if URL_STOP_WORDS.contains(&lower.as_str()) {
        return false;
    }

    true
}

/// Sanitize text for embedding: decode HTML entities, remove formatting noise, normalize whitespace.
fn sanitize_text(text: &str) -> String {
    let mut result = text.to_string();

    // Decode HTML entities
    for (entity, replacement) in HTML_ENTITIES {
        result = result.replace(entity, replacement);
    }

    // Remove formatting/noise characters (markdown, code fences, etc.)
    // Keep meaningful punctuation: . , : ; ? ! - ' "
    let result: String = result
        .chars()
        .map(|c| match c {
            // Remove: backticks, asterisks, hash, pipes, brackets for links
            '`' | '*' | '#' | '|' | '[' | ']' => ' ',
            // Remove angle brackets (HTML tags)
            '<' | '>' => ' ',
            // Remove curly braces (templates, code)
            '{' | '}' => ' ',
            // Remove tilde (strikethrough), backslash (escapes)
            '~' | '\\' => ' ',
            // Keep everything else
            _ => c,
        })
        .collect();

    // Normalize whitespace: collapse multiple spaces/newlines to single space
    result.split_whitespace().collect::<Vec<_>>().join(" ")
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
        // Title repeated for emphasis
        assert_eq!(result, Some("Hello World. Hello World".to_string()));
    }

    #[test]
    fn test_description_only() {
        let result = preprocess_content("", "A description", &[], "");
        assert_eq!(result, Some("A description".to_string()));
    }

    #[test]
    fn test_both_title_and_description() {
        let result = preprocess_content("Title", "Description", &[], "");
        // Title repeated, then description
        assert_eq!(result, Some("Title. Title. Description".to_string()));
    }

    #[test]
    fn test_trims_whitespace() {
        let result = preprocess_content("  Title  ", "  Description  ", &[], "");
        assert_eq!(result, Some("Title. Title. Description".to_string()));
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
        // Title repeated
        assert_eq!(result, Some("Short title. Short title".to_string()));
    }

    // Tests for tags and URL functionality
    #[test]
    fn test_with_tags() {
        let tags = vec!["rust".to_string(), "cli".to_string()];
        let result = preprocess_content("Title", "Description", &tags, "");
        // Tags as plain words, no "tags:" prefix
        assert_eq!(result, Some("Title. Title. Description. rust cli".to_string()));
    }

    #[test]
    fn test_with_empty_tags() {
        let result = preprocess_content("Title", "Description", &[], "");
        assert_eq!(result, Some("Title. Title. Description".to_string()));
    }

    #[test]
    fn test_with_url_keywords() {
        let result = preprocess_content("Title", "Description", &[], "https://github.com/user/repo");
        // Extracts "github", "user", "repo" from URL
        assert_eq!(result, Some("Title. Title. Description. github user repo".to_string()));
    }

    #[test]
    fn test_with_url_subdomain_and_path() {
        // Extracts subdomain + domain + path segments
        let result = preprocess_content("Title", "Description", &[], "https://docs.github.com/en/actions");
        // "docs" from subdomain, "github" from domain, "actions" from path ("en" filtered as noise)
        assert_eq!(result, Some("Title. Title. Description. github actions".to_string()));
    }

    #[test]
    fn test_with_invalid_url() {
        let result = preprocess_content("Title", "Description", &[], "not-a-url");
        assert_eq!(result, Some("Title. Title. Description".to_string()));
    }

    #[test]
    fn test_with_tags_and_url() {
        let tags = vec!["rust".to_string()];
        let result = preprocess_content("Title", "Description", &tags, "https://crates.io/crates/tokio");
        // Tags + URL keywords
        assert_eq!(result, Some("Title. Title. Description. rust. crates tokio".to_string()));
    }

    #[test]
    fn test_tags_only_no_title_description() {
        let tags = vec!["tag1".to_string()];
        let result = preprocess_content("", "", &tags, "");
        // Tags alone should not produce content (need title or description)
        assert!(result.is_none());
    }

    // URL keyword extraction tests
    #[test]
    fn test_url_keywords_filters_noise() {
        // Numbers, short words, TLDs filtered
        let keywords = extract_url_keywords("https://www.example.com/v1/api/123/rust-guide");
        // Contains compound "rust-guide" plus individual "rust" and "guide"
        assert!(keywords.contains("example"));
        assert!(keywords.contains("rust-guide"));
        assert!(keywords.contains("rust"));
        assert!(keywords.contains("guide"));
        // Noise filtered
        assert!(!keywords.contains("www"));
        assert!(!keywords.contains("com"));
        assert!(!keywords.contains("123"));
        assert!(!keywords.contains("api"));
    }

    #[test]
    fn test_url_keywords_deduplicates() {
        let keywords = extract_url_keywords("https://rust-lang.github.io/rust-by-example/rust");
        // Should contain: rust, lang, github, rust-by-example, example
        // "rust" appears multiple times but individual occurrences deduplicated
        assert!(keywords.contains("rust"));
        assert!(keywords.contains("rust-by-example")); // compound kept!
        assert!(keywords.contains("example"));
    }

    #[test]
    fn test_url_keywords_keeps_compounds_and_splits() {
        let keywords = extract_url_keywords("https://example.com/machine-learning_tutorial");
        // Keeps compound AND individual words
        assert!(keywords.contains("machine-learning_tutorial")); // full segment
        assert!(keywords.contains("machine"));
        assert!(keywords.contains("learning"));
        assert!(keywords.contains("tutorial"));
    }

    #[test]
    fn test_url_keywords_splits_segments() {
        let keywords = extract_url_keywords("https://example.com/rust-guide.html");
        assert!(keywords.contains("rust"));
        assert!(keywords.contains("guide"));
        // "html" filtered as noise
        assert!(!keywords.contains("html"));
    }

    // Sanitization tests
    #[test]
    fn test_sanitize_removes_backticks() {
        let result = sanitize_text("Use `println!` for output");
        assert_eq!(result, "Use println! for output");
    }

    #[test]
    fn test_sanitize_removes_markdown() {
        let result = sanitize_text("**bold** and *italic* and # header");
        assert_eq!(result, "bold and italic and header");
    }

    #[test]
    fn test_sanitize_removes_brackets_pipes() {
        let result = sanitize_text("[link](url) | table | cell");
        assert_eq!(result, "link (url) table cell");
    }

    #[test]
    fn test_sanitize_decodes_html_entities() {
        // Note: &lt; becomes < which then gets removed as noise
        let result = sanitize_text("Tom &amp; Jerry &lt;3 &nbsp; spaces");
        assert_eq!(result, "Tom & Jerry 3 spaces");
    }

    #[test]
    fn test_sanitize_normalizes_whitespace() {
        let result = sanitize_text("multiple   spaces\n\nnewlines\ttabs");
        assert_eq!(result, "multiple spaces newlines tabs");
    }

    #[test]
    fn test_sanitize_preserves_meaningful_punctuation() {
        let result = sanitize_text("Hello, world! How are you? It's fine: yes.");
        assert_eq!(result, "Hello, world! How are you? It's fine: yes.");
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
