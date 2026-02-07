//! Visualization support for Indra knowledge bases
//!
//! This module provides dimensionality reduction (PCA) to project
//! high-dimensional embeddings into 3D space for visualization.

#[cfg(feature = "viz")]
mod pca;

#[cfg(feature = "viz")]
pub use pca::*;

use serde::{Deserialize, Serialize};

/// A thought with its 3D visualization coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizThought {
    /// The thought ID
    pub id: String,
    /// The thought content
    pub content: String,
    /// Optional thought type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_type: Option<String>,
    /// 3D coordinates for visualization [x, y, z]
    pub position: [f32; 3],
    /// Whether this thought has an embedding
    pub has_embedding: bool,
    /// Creation timestamp (unix millis)
    pub created_at: u64,
}

/// A commit in visualization export format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizCommit {
    /// The commit hash
    pub hash: String,
    /// Commit message
    pub message: String,
    /// Author identifier
    pub author: String,
    /// Timestamp (unix millis)
    pub timestamp: u64,
    /// Parent commit hashes
    pub parents: Vec<String>,
}

/// Export format for visualization data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizExport {
    /// The thoughts with 3D positions
    pub thoughts: Vec<VizThought>,
    /// Commit history (newest first)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<VizCommit>,
    /// Metadata about the export
    pub meta: VizMeta,
}

/// Metadata about the visualization export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizMeta {
    /// Number of thoughts total
    pub total_thoughts: usize,
    /// Number of thoughts with embeddings (and thus 3D positions)
    pub embedded_thoughts: usize,
    /// The dimensionality reduction method used
    pub reduction_method: String,
    /// Original embedding dimension (e.g., 384)
    pub original_dim: usize,
    /// Variance explained by the 3 principal components (if PCA)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variance_explained: Option<[f64; 3]>,
}
