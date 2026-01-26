//! Lexical (keyword) scoring for hybrid search.
//!
//! Provides simple keyword matching against bookmark content.
//! Used alongside semantic search for RRF fusion.
//!
//! Length normalization: Description matches are weighted inversely to description
//! length to prevent long descriptions from having unfair advantage due to more
//! surface area for substring matches.

/// Result of lexical scoring.
#[derive(Debug, Clone)]
pub struct LexicalResult {
    /// Bookmark ID
    pub id: u64,
    /// Number of query terms matched
    pub matched_terms: usize,
    /// Weighted score across all fields (length-normalized)
    pub total_hits: f32,
}

/// Score a set of bookmarks against a query using keyword matching.
///
/// # Arguments
/// * `query` - The search query (will be tokenized)
/// * `bookmarks` - Slice of (id, title, description, tags) tuples
///
/// # Returns
/// Results sorted by relevance (matched_terms DESC, total_hits DESC).
/// Only bookmarks with at least one match are returned.
pub fn score_lexical(
    query: &str,
    bookmarks: &[(u64, &str, &str, &[String])],
) -> Vec<LexicalResult> {
    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return vec![];
    }

    let mut results: Vec<LexicalResult> = bookmarks
        .iter()
        .filter_map(|(id, title, description, tags)| {
            let (matched_terms, total_hits) =
                count_matches(&query_terms, title, description, tags);

            if matched_terms > 0 {
                Some(LexicalResult {
                    id: *id,
                    matched_terms,
                    total_hits,
                })
            } else {
                None
            }
        })
        .collect();

    // Sort by matched_terms DESC, then total_hits DESC
    results.sort_by(|a, b| {
        b.matched_terms
            .cmp(&a.matched_terms)
            .then_with(|| b.total_hits.partial_cmp(&a.total_hits).unwrap_or(std::cmp::Ordering::Equal))
    });

    results
}

/// Tokenize query into lowercase terms.
/// Filters out very short terms (1 char) and common stop words.
fn tokenize(query: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
        "in", "on", "at", "to", "for", "of", "with", "by", "from", "as",
        "and", "or", "but", "not", "no", "so", "if", "then",
    ];

    query
        .split(|c: char| !c.is_alphanumeric())
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() > 1 && !STOP_WORDS.contains(&s.as_str()))
        .collect()
}

/// Baseline description length for normalization (characters).
/// Descriptions at or below this length get full weight.
const DESC_LENGTH_BASELINE: f32 = 100.0;

/// Compute description match weight based on length.
///
/// Longer descriptions have more surface area for substring matches,
/// giving them an unfair advantage. This function applies logarithmic
/// decay to normalize for length.
///
/// - 100 chars or less: 1.0 (full weight)
/// - 270 chars: ~0.5
/// - 730 chars: ~0.33
fn description_length_weight(len: usize) -> f32 {
    if len <= DESC_LENGTH_BASELINE as usize {
        return 1.0;
    }
    // Logarithmic decay: 1 / (1 + ln(len / baseline))
    1.0 / (1.0 + (len as f32 / DESC_LENGTH_BASELINE).ln())
}

/// Count term matches across all bookmark fields.
/// Returns (unique_terms_matched, total_occurrences as f32 for weighted scoring).
fn count_matches(
    query_terms: &[String],
    title: &str,
    description: &str,
    tags: &[String],
) -> (usize, f32) {
    let title_lower = title.to_lowercase();
    let description_lower = description.to_lowercase();
    let tags_lower: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    // Compute description weight once
    let desc_weight = description_length_weight(description.len());

    let mut matched_terms = 0;
    let mut total_hits: f32 = 0.0;

    for term in query_terms {
        let mut term_hits: f32 = 0.0;

        // Title match (weighted higher implicitly via occurrence count)
        if title_lower.contains(term) {
            term_hits += 2.0; // Title matches worth more
        }

        // Description match (length-normalized)
        if description_lower.contains(term) {
            term_hits += desc_weight; // Was: term_hits += 1
        }

        // Tag match (exact match or prefix)
        for tag in &tags_lower {
            if tag == term || tag.starts_with(&format!("{}/", term)) {
                term_hits += 3.0; // Tag matches are highly relevant
            }
        }

        if term_hits > 0.0 {
            matched_terms += 1;
            total_hits += term_hits;
        }
    }

    (matched_terms, total_hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("machine learning guide");
        assert_eq!(tokens, vec!["machine", "learning", "guide"]);
    }

    #[test]
    fn test_tokenize_filters_stop_words() {
        let tokens = tokenize("the quick brown fox");
        assert_eq!(tokens, vec!["quick", "brown", "fox"]);
    }

    #[test]
    fn test_tokenize_filters_short_words() {
        let tokens = tokenize("I am a person");
        assert_eq!(tokens, vec!["am", "person"]);
    }

    #[test]
    fn test_tokenize_handles_punctuation() {
        let tokens = tokenize("rust-lang, python/django");
        assert_eq!(tokens, vec!["rust", "lang", "python", "django"]);
    }

    #[test]
    fn test_tokenize_lowercase() {
        let tokens = tokenize("RUST Lang Python");
        assert_eq!(tokens, vec!["rust", "lang", "python"]);
    }

    #[test]
    fn test_score_lexical_empty_query() {
        let bookmarks: Vec<(u64, &str, &str, &[String])> = vec![];
        let results = score_lexical("", &bookmarks);
        assert!(results.is_empty());
    }

    #[test]
    fn test_score_lexical_no_matches() {
        let tags: Vec<String> = vec![];
        let bookmarks = vec![(1, "Cooking Recipes", "Food and meals", tags.as_slice())];

        let results = score_lexical("programming rust", &bookmarks);
        assert!(results.is_empty());
    }

    #[test]
    fn test_score_lexical_title_match() {
        let tags: Vec<String> = vec![];
        let bookmarks = vec![
            (1, "Rust Programming Guide", "Learn rust basics", tags.as_slice()),
            (2, "Python Tutorial", "Python for beginners", tags.as_slice()),
        ];

        let results = score_lexical("rust", &bookmarks);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
        assert!(results[0].matched_terms > 0);
    }

    #[test]
    fn test_score_lexical_description_match() {
        let tags: Vec<String> = vec![];
        let bookmarks = vec![(
            1,
            "Programming Guide",
            "A comprehensive rust tutorial",
            tags.as_slice(),
        )];

        let results = score_lexical("rust", &bookmarks);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
    }

    #[test]
    fn test_score_lexical_tag_match() {
        let tags = vec!["rust".to_string(), "programming".to_string()];
        let bookmarks = vec![(1, "Untitled", "No description", tags.as_slice())];

        let results = score_lexical("rust", &bookmarks);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
        // Tag match should give higher score (tag=3.0 + desc=~1.0 for short desc)
        assert!(results[0].total_hits >= 3.0);
    }

    #[test]
    fn test_score_lexical_tag_hierarchy_match() {
        let tags = vec!["programming/rust".to_string()];
        let bookmarks = vec![(1, "Untitled", "No description", tags.as_slice())];

        let results = score_lexical("programming", &bookmarks);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
    }

    #[test]
    fn test_score_lexical_sorted_by_relevance() {
        let tags1: Vec<String> = vec![];
        let tags2 = vec!["rust".to_string()];
        let tags3: Vec<String> = vec![];

        let bookmarks = vec![
            (1, "Python Guide", "Also mentions rust once", tags1.as_slice()),
            (2, "Rust Tutorial", "Learn rust programming", tags2.as_slice()),
            (3, "Cooking Recipes", "No programming here", tags3.as_slice()),
        ];

        let results = score_lexical("rust", &bookmarks);

        // Should only have 2 results (not cooking)
        assert_eq!(results.len(), 2);

        // Bookmark 2 should rank first (title + description + tag)
        assert_eq!(results[0].id, 2);
        // Bookmark 1 should rank second (description only)
        assert_eq!(results[1].id, 1);
    }

    #[test]
    fn test_score_lexical_multi_term_query() {
        let tags1: Vec<String> = vec![];
        let tags2: Vec<String> = vec![];

        let bookmarks = vec![
            (1, "Rust Guide", "Programming language", tags1.as_slice()),
            (2, "Machine Learning with Rust", "Programming ML", tags2.as_slice()),
        ];

        let results = score_lexical("rust machine learning", &bookmarks);

        // Both should match, but bookmark 2 matches more terms
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 2); // Matches rust + machine + learning
        assert!(results[0].matched_terms > results[1].matched_terms);
    }

    #[test]
    fn test_score_lexical_case_insensitive() {
        let tags: Vec<String> = vec![];
        let bookmarks = vec![(1, "RUST PROGRAMMING", "Description", tags.as_slice())];

        let results = score_lexical("rust", &bookmarks);
        assert_eq!(results.len(), 1);
    }

    // Length normalization tests
    #[test]
    fn test_description_length_weight_short() {
        // Short descriptions get full weight
        assert_eq!(description_length_weight(50), 1.0);
        assert_eq!(description_length_weight(100), 1.0);
    }

    #[test]
    fn test_description_length_weight_decay() {
        // Longer descriptions get reduced weight
        let w200 = description_length_weight(200);
        let w400 = description_length_weight(400);
        let w800 = description_length_weight(800);

        // Verify decay ordering
        assert!(w200 < 1.0);
        assert!(w400 < w200);
        assert!(w800 < w400);

        // Verify approximate values (logarithmic decay)
        assert!((w200 - 0.59).abs() < 0.1); // ~0.59 at 200 chars
        assert!((w400 - 0.42).abs() < 0.1); // ~0.42 at 400 chars
    }

    #[test]
    fn test_long_description_penalized() {
        // This is the key behavioral test: a short focused description
        // should score higher than a long description with the same keyword
        let tags: Vec<String> = vec![];

        let short_desc = "Learn rust basics"; // ~17 chars
        let long_desc = "This comprehensive guide covers many programming topics \
            including JavaScript, Python, databases, web development, DevOps, \
            cloud computing, and also briefly mentions rust somewhere in here \
            along with many other technologies and frameworks"; // ~250 chars

        let bookmarks = vec![
            (1, "Short", short_desc, tags.as_slice()),
            (2, "Long", long_desc, tags.as_slice()),
        ];

        let results = score_lexical("rust", &bookmarks);

        // Both match "rust" in description only
        assert_eq!(results.len(), 2);

        // Short description should have higher total_hits due to length penalty on long
        let short_result = results.iter().find(|r| r.id == 1).unwrap();
        let long_result = results.iter().find(|r| r.id == 2).unwrap();

        assert!(
            short_result.total_hits > long_result.total_hits,
            "Short desc ({}) should score higher than long desc ({})",
            short_result.total_hits,
            long_result.total_hits
        );
    }
}
