//! In-memory vector index with cosine similarity search.
//!
//! Stores bookmark embeddings and provides fast similarity search.

use std::collections::HashMap;

/// An entry in the vector index.
#[derive(Debug, Clone)]
pub struct VectorEntry {
    /// Hash of the content that was embedded
    pub content_hash: u64,
    /// The embedding vector
    pub embedding: Vec<f32>,
}

/// In-memory vector index for semantic search.
///
/// Stores embeddings keyed by bookmark ID, supporting:
/// - Insert/update/remove operations
/// - Cosine similarity search with threshold filtering
pub struct VectorIndex {
    /// Bookmark ID -> (content_hash, embedding)
    entries: HashMap<u64, VectorEntry>,
    /// Expected embedding dimensions
    dimensions: usize,
}

/// Search result from the vector index.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Bookmark ID
    pub id: u64,
    /// Cosine similarity score (0.0 to 1.0)
    pub score: f32,
}

impl VectorIndex {
    /// Create a new empty vector index with specified dimensions.
    pub fn new(dimensions: usize) -> Self {
        Self {
            entries: HashMap::new(),
            dimensions,
        }
    }

    /// Create an index with pre-allocated capacity.
    pub fn with_capacity(dimensions: usize, capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            dimensions,
        }
    }

    /// Get the expected embedding dimensions.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Get the number of entries in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert or update an entry in the index.
    ///
    /// Returns an error if the embedding has zero norm (cannot be normalized).
    pub fn insert(
        &mut self,
        id: u64,
        content_hash: u64,
        embedding: Vec<f32>,
    ) -> Result<(), IndexError> {
        if embedding.len() != self.dimensions {
            return Err(IndexError::DimensionMismatch {
                expected: self.dimensions,
                got: embedding.len(),
            });
        }

        // Check for zero norm
        let norm = Self::l2_norm(&embedding);
        if norm < f32::EPSILON {
            return Err(IndexError::ZeroNormVector);
        }

        self.entries.insert(
            id,
            VectorEntry {
                content_hash,
                embedding,
            },
        );

        Ok(())
    }

    /// Remove an entry by bookmark ID.
    pub fn remove(&mut self, id: u64) -> Option<VectorEntry> {
        self.entries.remove(&id)
    }

    /// Get an entry by bookmark ID.
    pub fn get(&self, id: u64) -> Option<&VectorEntry> {
        self.entries.get(&id)
    }

    /// Check if an entry exists for the given ID.
    pub fn contains(&self, id: u64) -> bool {
        self.entries.contains_key(&id)
    }

    /// Get all bookmark IDs in the index.
    pub fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.keys().copied()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (u64, &VectorEntry)> {
        self.entries.iter().map(|(k, v)| (*k, v))
    }

    /// Search for similar vectors using cosine similarity.
    ///
    /// # Arguments
    /// * `query` - The query embedding vector
    /// * `candidate_ids` - Optional set of IDs to search within (filters results)
    /// * `threshold` - Minimum similarity score (0.0 to 1.0)
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// Results sorted by similarity score (highest first).
    pub fn search(
        &self,
        query: &[f32],
        candidate_ids: Option<&[u64]>,
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<SearchResult>, IndexError> {
        if query.len() != self.dimensions {
            return Err(IndexError::DimensionMismatch {
                expected: self.dimensions,
                got: query.len(),
            });
        }

        let query_norm = Self::l2_norm(query);
        if query_norm < f32::EPSILON {
            return Err(IndexError::ZeroNormVector);
        }

        let mut results: Vec<SearchResult> = self
            .entries
            .iter()
            .filter(|(id, _)| {
                candidate_ids
                    .map(|ids| ids.contains(id))
                    .unwrap_or(true)
            })
            .filter_map(|(id, entry)| {
                let score = Self::cosine_similarity(query, &entry.embedding, query_norm);
                if score >= threshold {
                    Some(SearchResult { id: *id, score })
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Apply limit
        results.truncate(limit);

        Ok(results)
    }

    /// Compute L2 norm of a vector.
    fn l2_norm(v: &[f32]) -> f32 {
        v.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Compute cosine similarity between two vectors.
    /// Assumes query_norm is precomputed for efficiency.
    fn cosine_similarity(query: &[f32], target: &[f32], query_norm: f32) -> f32 {
        let target_norm = Self::l2_norm(target);
        if target_norm < f32::EPSILON {
            return 0.0;
        }

        let dot_product: f32 = query.iter().zip(target.iter()).map(|(a, b)| a * b).sum();
        dot_product / (query_norm * target_norm)
    }

    /// Bulk load entries into the index.
    /// Used when loading from storage.
    pub fn bulk_load(&mut self, entries: Vec<(u64, u64, Vec<f32>)>) -> Result<(), IndexError> {
        for (id, content_hash, embedding) in entries {
            self.insert(id, content_hash, embedding)?;
        }
        Ok(())
    }

    /// Clear all entries from the index.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Errors that can occur during index operations.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("Cannot store or search with zero-norm vector")]
    ZeroNormVector,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_embedding(dimensions: usize, value: f32) -> Vec<f32> {
        vec![value; dimensions]
    }

    #[test]
    fn test_new_index() {
        let index = VectorIndex::new(384);
        assert_eq!(index.dimensions(), 384);
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_insert_and_get() {
        let mut index = VectorIndex::new(3);
        let embedding = vec![1.0, 0.0, 0.0];

        index.insert(1, 12345, embedding.clone()).unwrap();

        assert_eq!(index.len(), 1);
        assert!(index.contains(1));

        let entry = index.get(1).unwrap();
        assert_eq!(entry.content_hash, 12345);
        assert_eq!(entry.embedding, embedding);
    }

    #[test]
    fn test_insert_dimension_mismatch() {
        let mut index = VectorIndex::new(3);
        let wrong_dims = vec![1.0, 0.0, 0.0, 0.0]; // 4 dims

        let result = index.insert(1, 12345, wrong_dims);
        assert!(matches!(result, Err(IndexError::DimensionMismatch { .. })));
    }

    #[test]
    fn test_insert_zero_norm_rejected() {
        let mut index = VectorIndex::new(3);
        let zero_vec = vec![0.0, 0.0, 0.0];

        let result = index.insert(1, 12345, zero_vec);
        assert!(matches!(result, Err(IndexError::ZeroNormVector)));
    }

    #[test]
    fn test_remove() {
        let mut index = VectorIndex::new(3);
        index.insert(1, 12345, vec![1.0, 0.0, 0.0]).unwrap();

        let removed = index.remove(1);
        assert!(removed.is_some());
        assert!(!index.contains(1));
        assert!(index.is_empty());
    }

    #[test]
    fn test_search_basic() {
        let mut index = VectorIndex::new(3);

        // Insert two orthogonal vectors
        index.insert(1, 100, vec![1.0, 0.0, 0.0]).unwrap();
        index.insert(2, 200, vec![0.0, 1.0, 0.0]).unwrap();

        // Query similar to first vector
        let query = vec![1.0, 0.1, 0.0];
        let results = index.search(&query, None, 0.0, 10).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 1); // Should be most similar
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_search_with_threshold() {
        let mut index = VectorIndex::new(3);

        index.insert(1, 100, vec![1.0, 0.0, 0.0]).unwrap();
        index.insert(2, 200, vec![0.0, 1.0, 0.0]).unwrap();

        // Query for first vector with high threshold
        let query = vec![1.0, 0.0, 0.0];
        let results = index.search(&query, None, 0.9, 10).unwrap();

        // Should only return exact match
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
        assert!((results[0].score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_search_with_limit() {
        let mut index = VectorIndex::new(3);

        for i in 0..10 {
            index
                .insert(i, i * 100, vec![1.0, i as f32 * 0.1, 0.0])
                .unwrap();
        }

        let query = vec![1.0, 0.0, 0.0];
        let results = index.search(&query, None, 0.0, 3).unwrap();

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_with_candidate_filter() {
        let mut index = VectorIndex::new(3);

        index.insert(1, 100, vec![1.0, 0.0, 0.0]).unwrap();
        index.insert(2, 200, vec![0.9, 0.1, 0.0]).unwrap();
        index.insert(3, 300, vec![0.8, 0.2, 0.0]).unwrap();

        // Search only within subset
        let candidates = vec![2, 3];
        let query = vec![1.0, 0.0, 0.0];
        let results = index.search(&query, Some(&candidates), 0.0, 10).unwrap();

        // Should not include ID 1
        assert!(!results.iter().any(|r| r.id == 1));
        assert!(results.iter().any(|r| r.id == 2));
        assert!(results.iter().any(|r| r.id == 3));
    }

    #[test]
    fn test_bulk_load() {
        let mut index = VectorIndex::new(3);

        let entries = vec![
            (1, 100, vec![1.0, 0.0, 0.0]),
            (2, 200, vec![0.0, 1.0, 0.0]),
            (3, 300, vec![0.0, 0.0, 1.0]),
        ];

        index.bulk_load(entries).unwrap();
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_ids_iterator() {
        let mut index = VectorIndex::new(3);
        index.insert(1, 100, vec![1.0, 0.0, 0.0]).unwrap();
        index.insert(5, 500, vec![0.0, 1.0, 0.0]).unwrap();

        let ids: Vec<u64> = index.ids().collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&5));
    }
}
