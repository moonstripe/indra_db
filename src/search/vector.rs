//! Brute-force vector search

use crate::embedding::cosine_similarity;
use crate::graph::GraphView;
use crate::model::{Thought, ThoughtId};
use crate::Result;
use std::cmp::Ordering;

/// A search result with score
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub thought: Thought,
    pub score: f32,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.thought.id == other.thought.id && (self.score - other.score).abs() < f32::EPSILON
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher scores first
        other
            .score
            .partial_cmp(&self.score)
            .unwrap_or(Ordering::Equal)
    }
}

/// Brute-force vector search over a graph view
pub struct VectorSearch<'a> {
    view: &'a GraphView<'a>,
}

impl<'a> VectorSearch<'a> {
    /// Create a new vector search over the given graph view
    pub fn new(view: &'a GraphView<'a>) -> Self {
        VectorSearch { view }
    }

    /// Search for thoughts similar to the query embedding
    ///
    /// Returns results sorted by similarity score (highest first)
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<SearchResult>> {
        let thoughts = self.view.all_thoughts()?;

        let mut results: Vec<SearchResult> = thoughts
            .into_iter()
            .filter_map(|thought| {
                let emb = thought.embedding.clone()?;
                let score = cosine_similarity(query_embedding, &emb);
                Some(SearchResult { thought, score })
            })
            .collect();

        // Sort by score descending
        results.sort();

        // Take top K
        results.truncate(limit);

        Ok(results)
    }

    /// Search with a minimum similarity threshold
    pub fn search_with_threshold(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let thoughts = self.view.all_thoughts()?;

        let mut results: Vec<SearchResult> = thoughts
            .into_iter()
            .filter_map(|thought| {
                let emb = thought.embedding.clone()?;
                let score = cosine_similarity(query_embedding, &emb);
                if score >= threshold {
                    Some(SearchResult { thought, score })
                } else {
                    None
                }
            })
            .collect();

        results.sort();
        results.truncate(limit);

        Ok(results)
    }

    /// Find the K nearest neighbors to a thought
    pub fn nearest_neighbors(&self, id: &ThoughtId, k: usize) -> Result<Vec<SearchResult>> {
        let source = self.view.get_thought(id)?;

        let source = match source {
            Some(t) => t,
            None => return Ok(vec![]),
        };

        let embedding = match &source.embedding {
            Some(e) => e,
            None => return Ok(vec![]),
        };

        let thoughts = self.view.all_thoughts()?;

        let mut results: Vec<SearchResult> = thoughts
            .into_iter()
            .filter(|t| t.id != *id) // Exclude self
            .filter_map(|thought| {
                let emb = thought.embedding.clone()?;
                let score = cosine_similarity(embedding, &emb);
                Some(SearchResult { thought, score })
            })
            .collect();

        results.sort();
        results.truncate(k);

        Ok(results)
    }
}

/// Simple keyword search (substring matching)
pub fn keyword_search(view: &GraphView, query: &str) -> Result<Vec<Thought>> {
    let query_lower = query.to_lowercase();
    let thoughts = view.all_thoughts()?;

    Ok(thoughts
        .into_iter()
        .filter(|t| t.content.to_lowercase().contains(&query_lower))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::Embedder;
    use crate::embedding::MockEmbedder;
    use crate::model::Thought;
    use crate::store::ObjectStore;
    use crate::trie::MerkleTrie;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, ObjectStore) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();
        (dir, store)
    }

    #[test]
    fn test_vector_search() {
        let (_dir, store) = setup();
        let embedder = MockEmbedder::default();

        // Create thoughts with embeddings
        let mut t1 = Thought::with_id("t1", "The cat sat on the mat");
        t1.embedding = Some(embedder.embed(&t1.content).unwrap());

        let mut t2 = Thought::with_id("t2", "A dog ran in the park");
        t2.embedding = Some(embedder.embed(&t2.content).unwrap());

        let mut t3 = Thought::with_id("t3", "The cat played with yarn");
        t3.embedding = Some(embedder.embed(&t3.content).unwrap());

        let h1 = store.put_thought(&t1).unwrap();
        let h2 = store.put_thought(&t2).unwrap();
        let h3 = store.put_thought(&t3).unwrap();

        // Build trie
        let mut trie = MerkleTrie::new(&store);
        trie.insert(b"t:t1", h1).unwrap();
        trie.insert(b"t:t2", h2).unwrap();
        trie.insert(b"t:t3", h3).unwrap();
        let root = trie.commit().unwrap();

        // Search
        let view = GraphView::new(&store, root).unwrap();
        let search = VectorSearch::new(&view);

        let query = embedder.embed("cat sitting").unwrap();
        let results = search.search(&query, 10).unwrap();

        assert_eq!(results.len(), 3);
        // Results should be sorted by score
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[test]
    fn test_keyword_search() {
        let (_dir, store) = setup();

        let t1 = Thought::with_id("t1", "The quick brown fox");
        let t2 = Thought::with_id("t2", "The lazy dog");
        let t3 = Thought::with_id("t3", "Quick thinking");

        let h1 = store.put_thought(&t1).unwrap();
        let h2 = store.put_thought(&t2).unwrap();
        let h3 = store.put_thought(&t3).unwrap();

        let mut trie = MerkleTrie::new(&store);
        trie.insert(b"t:t1", h1).unwrap();
        trie.insert(b"t:t2", h2).unwrap();
        trie.insert(b"t:t3", h3).unwrap();
        let root = trie.commit().unwrap();

        let view = GraphView::new(&store, root).unwrap();

        let results = keyword_search(&view, "quick").unwrap();
        assert_eq!(results.len(), 2);
    }
}
