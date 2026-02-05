//! Pluggable embedding system

mod mock;
mod traits;

#[cfg(feature = "hf-embeddings")]
mod hf;

#[cfg(feature = "api-embeddings")]
mod api;

pub use mock::MockEmbedder;
pub use traits::{cosine_similarity, euclidean_distance, Embedder};

#[cfg(feature = "hf-embeddings")]
pub use hf::HFEmbedder;

#[cfg(feature = "api-embeddings")]
pub use api::{ApiEmbedder, ApiProvider};
