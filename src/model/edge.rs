//! Edge (relationship) type between thoughts

use super::{Hash, ThoughtId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Edge type classification
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeType(pub String);

impl EdgeType {
    // Common edge types
    pub const RELATES_TO: &'static str = "relates_to";
    pub const SUPPORTS: &'static str = "supports";
    pub const CONTRADICTS: &'static str = "contradicts";
    pub const DERIVES_FROM: &'static str = "derives_from";
    pub const PART_OF: &'static str = "part_of";
    pub const SIMILAR_TO: &'static str = "similar_to";
    pub const CAUSES: &'static str = "causes";
    pub const PRECEDES: &'static str = "precedes";

    pub fn new(edge_type: impl Into<String>) -> Self {
        EdgeType(edge_type.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for EdgeType {
    fn from(s: &str) -> Self {
        EdgeType(s.to_string())
    }
}

/// An edge connecting two thoughts
///
/// Edges reference ThoughtIds, not content hashes. This means edges
/// "float" to the latest version of a thought rather than being pinned
/// to a specific version.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    /// Source thought ID
    pub source: ThoughtId,

    /// Target thought ID  
    pub target: ThoughtId,

    /// Type of relationship
    pub edge_type: EdgeType,

    /// Weight/strength of the relationship (0.0 to 1.0)
    pub weight: f32,

    /// Whether this edge is directed
    pub directed: bool,

    /// Arbitrary metadata
    pub attrs: HashMap<String, serde_json::Value>,

    /// Creation timestamp (unix millis)
    pub created_at: u64,
}

impl Edge {
    /// Create a new directed edge
    pub fn new(
        source: impl Into<ThoughtId>,
        target: impl Into<ThoughtId>,
        edge_type: impl Into<EdgeType>,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        Edge {
            source: source.into(),
            target: target.into(),
            edge_type: edge_type.into(),
            weight: 1.0,
            directed: true,
            attrs: HashMap::new(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    /// Create an undirected edge
    pub fn undirected(
        source: impl Into<ThoughtId>,
        target: impl Into<ThoughtId>,
        edge_type: impl Into<EdgeType>,
    ) -> Self {
        let mut edge = Self::new(source, target, edge_type);
        edge.directed = false;
        edge
    }

    /// Set the weight
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Add a metadata attribute
    pub fn with_attr(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.attrs.insert(key.into(), value.into());
        self
    }

    /// Compute the content hash of this edge
    pub fn content_hash(&self) -> Hash {
        let data = bincode::serialize(self).expect("serialization should not fail");
        Hash::digest(&data)
    }

    /// Get a canonical key for this edge (for deduplication)
    /// For undirected edges, this normalizes the order
    pub fn canonical_key(&self) -> (ThoughtId, ThoughtId, EdgeType) {
        if self.directed {
            (
                self.source.clone(),
                self.target.clone(),
                self.edge_type.clone(),
            )
        } else {
            // For undirected, use lexicographic ordering for consistency
            if self.source.0 <= self.target.0 {
                (
                    self.source.clone(),
                    self.target.clone(),
                    self.edge_type.clone(),
                )
            } else {
                (
                    self.target.clone(),
                    self.source.clone(),
                    self.edge_type.clone(),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_creation() {
        let edge = Edge::new("thought1", "thought2", EdgeType::RELATES_TO);
        assert_eq!(edge.source.0, "thought1");
        assert_eq!(edge.target.0, "thought2");
        assert_eq!(edge.weight, 1.0);
        assert!(edge.directed);
    }

    #[test]
    fn test_undirected_canonical_key() {
        let e1 = Edge::undirected("a", "b", "relates");
        let e2 = Edge::undirected("b", "a", "relates");

        assert_eq!(e1.canonical_key(), e2.canonical_key());
    }

    #[test]
    fn test_directed_canonical_key() {
        let e1 = Edge::new("a", "b", "causes");
        let e2 = Edge::new("b", "a", "causes");

        assert_ne!(e1.canonical_key(), e2.canonical_key());
    }
}
