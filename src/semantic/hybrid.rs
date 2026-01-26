//! Hybrid search combining semantic and lexical rankings.
//!
//! Uses Weighted Reciprocal Rank Fusion (RRF) to merge results from both
//! semantic (embedding-based) and lexical (keyword-based) search.
//!
//! The semantic weight (α) controls the balance:
//! - α=0.5: Equal weight to both (classic RRF)
//! - α=0.6: 60% semantic, 40% lexical (default)
//! - α=1.0: Pure semantic ranking

use std::collections::HashMap;

/// RRF constant (standard value from literature).
/// Higher k reduces the impact of high-ranking items.
const RRF_K: f32 = 60.0;

/// Default semantic weight for hybrid search.
pub const DEFAULT_SEMANTIC_WEIGHT: f32 = 0.6;

/// Result from hybrid search with combined score.
#[derive(Debug, Clone)]
pub struct HybridResult {
    /// Bookmark ID
    pub id: u64,
    /// RRF fusion score
    pub score: f32,
    /// Rank from semantic search (None if not in semantic results)
    pub semantic_rank: Option<usize>,
    /// Rank from lexical search (None if not in lexical results)
    pub lexical_rank: Option<usize>,
}

/// Fuse semantic and lexical rankings using Weighted Reciprocal Rank Fusion (RRF).
///
/// Weighted RRF formula:
///   score(d) = α * 1/(k + rank_semantic) + (1-α) * 1/(k + rank_lexical)
///
/// Where α (semantic_weight) controls the balance between semantic and lexical ranking.
///
/// # Arguments
/// * `semantic_ids` - IDs from semantic search, ordered by similarity (best first)
/// * `lexical_ids` - IDs from lexical search, ordered by relevance (best first)
/// * `semantic_weight` - Weight for semantic ranking [0.0, 1.0]. Default: 0.6
///
/// # Returns
/// Combined results sorted by RRF score (highest first).
pub fn rrf_fusion(
    semantic_ids: &[u64],
    lexical_ids: &[u64],
    semantic_weight: f32,
) -> Vec<HybridResult> {
    let mut scores: HashMap<u64, HybridResult> = HashMap::new();

    let sem_weight = semantic_weight.clamp(0.0, 1.0);
    let lex_weight = 1.0 - sem_weight;

    // Process semantic results
    for (rank, &id) in semantic_ids.iter().enumerate() {
        let rrf_score = sem_weight / (RRF_K + rank as f32 + 1.0); // rank is 0-indexed, add 1
        scores.insert(
            id,
            HybridResult {
                id,
                score: rrf_score,
                semantic_rank: Some(rank + 1),
                lexical_rank: None,
            },
        );
    }

    // Process lexical results (add to existing or create new)
    for (rank, &id) in lexical_ids.iter().enumerate() {
        let rrf_score = lex_weight / (RRF_K + rank as f32 + 1.0);

        scores
            .entry(id)
            .and_modify(|result| {
                result.score += rrf_score;
                result.lexical_rank = Some(rank + 1);
            })
            .or_insert(HybridResult {
                id,
                score: rrf_score,
                semantic_rank: None,
                lexical_rank: Some(rank + 1),
            });
    }

    // Sort by score descending
    let mut results: Vec<HybridResult> = scores.into_values().collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_empty_inputs() {
        let results = rrf_fusion(&[], &[], 0.5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_rrf_semantic_only() {
        let semantic = vec![1, 2, 3];
        let results = rrf_fusion(&semantic, &[], 0.5);

        assert_eq!(results.len(), 3);

        // First item should have highest score
        assert_eq!(results[0].id, 1);
        assert!(results[0].score > results[1].score);

        // Should have semantic rank but no lexical rank
        assert_eq!(results[0].semantic_rank, Some(1));
        assert_eq!(results[0].lexical_rank, None);
    }

    #[test]
    fn test_rrf_lexical_only() {
        let lexical = vec![1, 2, 3];
        let results = rrf_fusion(&[], &lexical, 0.5);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, 1);

        // Should have lexical rank but no semantic rank
        assert_eq!(results[0].semantic_rank, None);
        assert_eq!(results[0].lexical_rank, Some(1));
    }

    #[test]
    fn test_rrf_both_rankings_boost_equal_weight() {
        // Item 1 is in both rankings (using equal weight 0.5)
        let semantic = vec![1, 2];
        let lexical = vec![1, 3];

        let results = rrf_fusion(&semantic, &lexical, 0.5);

        // Item 1 should be boosted to the top
        assert_eq!(results[0].id, 1);
        assert_eq!(results[0].semantic_rank, Some(1));
        assert_eq!(results[0].lexical_rank, Some(1));

        // Score should be weighted sum: 0.5/61 + 0.5/61 = 1.0/61
        let expected_score = 1.0 / (RRF_K + 1.0);
        assert!((results[0].score - expected_score).abs() < 0.001);
    }

    #[test]
    fn test_rrf_different_ranks_in_each() {
        // Item 1: semantic rank 1, lexical rank 3
        // Item 2: semantic rank 2, not in lexical
        // Item 3: not in semantic, lexical rank 1
        let semantic = vec![1, 2];
        let lexical = vec![3, 4, 1];

        let results = rrf_fusion(&semantic, &lexical, 0.5);

        // Find result for item 1
        let item1 = results.iter().find(|r| r.id == 1).unwrap();
        assert_eq!(item1.semantic_rank, Some(1));
        assert_eq!(item1.lexical_rank, Some(3));

        // Item 1 should score highest (appears in both)
        assert_eq!(results[0].id, 1);
    }

    #[test]
    fn test_rrf_preserves_ordering_equal_weight() {
        // Semantic: 1 > 2 > 3 > 4
        // Lexical:  4 > 3 > 2 > 1
        // With equal weights, items 1 and 4 should tie (same ranks swapped)
        let semantic = vec![1, 2, 3, 4];
        let lexical = vec![4, 3, 2, 1];

        let results = rrf_fusion(&semantic, &lexical, 0.5);

        // Items 1 and 4 should have same score
        let item1 = results.iter().find(|r| r.id == 1).unwrap();
        let item4 = results.iter().find(|r| r.id == 4).unwrap();
        assert!((item1.score - item4.score).abs() < 0.0001);

        // Items 2 and 3 should also have same score
        let item2 = results.iter().find(|r| r.id == 2).unwrap();
        let item3 = results.iter().find(|r| r.id == 3).unwrap();
        assert!((item2.score - item3.score).abs() < 0.0001);
    }

    #[test]
    fn test_rrf_k_constant_with_weight() {
        // With k=60 and weight=0.6, semantic rank 1 gives 0.6/61
        let semantic = vec![1];
        let results = rrf_fusion(&semantic, &[], 0.6);

        let expected = 0.6 / 61.0;
        assert!((results[0].score - expected).abs() < 0.0001);
    }

    #[test]
    fn test_rrf_many_items() {
        let semantic: Vec<u64> = (1..=100).collect();
        let lexical: Vec<u64> = (50..=150).collect();

        let results = rrf_fusion(&semantic, &lexical, 0.6);

        // Items 50-100 appear in both
        let item50 = results.iter().find(|r| r.id == 50).unwrap();
        assert!(item50.semantic_rank.is_some());
        assert!(item50.lexical_rank.is_some());

        // Total unique items: 1-100 + 101-150 = 150 items
        assert_eq!(results.len(), 150);
    }

    #[test]
    fn test_rrf_semantic_weight_favors_semantic() {
        // Semantic: 1 > 2
        // Lexical:  2 > 1
        // With sem_weight=0.8, semantic ranking should dominate
        let semantic = vec![1, 2];
        let lexical = vec![2, 1];

        let results = rrf_fusion(&semantic, &lexical, 0.8);

        // Item 1 should win due to higher semantic weight
        // Item 1: sem=1, lex=2 -> 0.8/61 + 0.2/62
        // Item 2: sem=2, lex=1 -> 0.8/62 + 0.2/61
        assert_eq!(results[0].id, 1);
    }

    #[test]
    fn test_rrf_lexical_weight_favors_lexical() {
        // Semantic: 1 > 2
        // Lexical:  2 > 1
        // With sem_weight=0.2, lexical ranking should dominate
        let semantic = vec![1, 2];
        let lexical = vec![2, 1];

        let results = rrf_fusion(&semantic, &lexical, 0.2);

        // Item 2 should win due to higher lexical weight
        assert_eq!(results[0].id, 2);
    }

    #[test]
    fn test_rrf_weight_clamping() {
        // Test that out-of-range weights are clamped
        let semantic = vec![1];
        let lexical = vec![2];

        // Weight > 1.0 should be clamped to 1.0
        let results = rrf_fusion(&semantic, &lexical, 1.5);
        let expected_sem_score = 1.0 / 61.0; // Full weight on semantic
        assert!((results[0].score - expected_sem_score).abs() < 0.0001);
        assert_eq!(results[0].id, 1); // Semantic result with weight=1.0

        // Weight < 0.0 should be clamped to 0.0
        let results = rrf_fusion(&semantic, &lexical, -0.5);
        let expected_lex_score = 1.0 / 61.0; // Full weight on lexical
        assert!((results[0].score - expected_lex_score).abs() < 0.0001);
        assert_eq!(results[0].id, 2); // Lexical result with sem_weight=0.0
    }
}
