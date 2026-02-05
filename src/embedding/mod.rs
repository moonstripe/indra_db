//! Pluggable embedding system

mod mock;
mod traits;

pub use mock::MockEmbedder;
pub use traits::{cosine_similarity, euclidean_distance, Embedder};
