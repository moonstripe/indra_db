//! Mock embedder for testing

use super::Embedder;
use crate::Result;

/// A mock embedder that generates deterministic embeddings based on text hash
///
/// Useful for testing without requiring an actual embedding model.
/// The embeddings are deterministic: same text → same embedding.
pub struct MockEmbedder {
    dimension: usize,
}

impl MockEmbedder {
    /// Create a new mock embedder with the specified dimension
    pub fn new(dimension: usize) -> Self {
        MockEmbedder { dimension }
    }

    /// Create a mock embedder with default dimension (384)
    pub fn default_dimension() -> Self {
        MockEmbedder { dimension: 384 }
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::default_dimension()
    }
}

impl Embedder for MockEmbedder {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Use BLAKE3 to generate deterministic pseudo-random values
        let hash = blake3::hash(text.as_bytes());
        let hash_bytes = hash.as_bytes();

        // Generate embedding by repeatedly hashing
        let mut embedding = Vec::with_capacity(self.dimension);
        let mut current_hash = *hash_bytes;

        for i in 0..self.dimension {
            // Use bytes from hash, rehash when exhausted
            let byte_index = i % 32;
            if byte_index == 0 && i > 0 {
                let next = blake3::hash(&current_hash);
                current_hash = *next.as_bytes();
            }

            // Convert byte to float in [-1, 1]
            let byte = current_hash[byte_index];
            let value = (byte as f32 / 127.5) - 1.0;
            embedding.push(value);
        }

        // Normalize to unit length
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
            }
        }

        Ok(embedding)
    }

    fn model_name(&self) -> &str {
        "mock-embedder"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::traits::cosine_similarity;

    #[test]
    fn test_mock_embedder_dimension() {
        let embedder = MockEmbedder::new(128);
        assert_eq!(embedder.dimension(), 128);

        let embedding = embedder.embed("test").unwrap();
        assert_eq!(embedding.len(), 128);
    }

    #[test]
    fn test_mock_embedder_deterministic() {
        let embedder = MockEmbedder::default();

        let e1 = embedder.embed("hello world").unwrap();
        let e2 = embedder.embed("hello world").unwrap();

        assert_eq!(e1, e2);
    }

    #[test]
    fn test_mock_embedder_different_texts() {
        let embedder = MockEmbedder::default();

        let e1 = embedder.embed("hello").unwrap();
        let e2 = embedder.embed("world").unwrap();

        assert_ne!(e1, e2);
    }

    #[test]
    fn test_mock_embedder_normalized() {
        let embedder = MockEmbedder::default();
        let embedding = embedder.embed("test").unwrap();

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_mock_embedder_similar_texts() {
        let embedder = MockEmbedder::default();

        // Similar texts won't necessarily have similar embeddings with mock
        // But we can verify the similarity calculation works
        let e1 = embedder.embed("the cat sat").unwrap();
        let e2 = embedder.embed("the cat sat").unwrap();

        let sim = cosine_similarity(&e1, &e2);
        assert!((sim - 1.0).abs() < 1e-5);
    }
}
