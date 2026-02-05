//! Commit type - a snapshot of the graph state

use super::Hash;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// A commit represents an immutable snapshot of the graph state
///
/// Like git commits, these form a DAG that tracks the evolution
/// of understanding over time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commit {
    /// Hash of the tree (merkle root of the graph state)
    pub tree: Hash,

    /// Parent commit hashes (empty for initial commit, multiple for merges)
    pub parents: Vec<Hash>,

    /// Human-readable message describing this commit
    pub message: String,

    /// Author/agent identifier
    pub author: String,

    /// Timestamp (unix millis)
    pub timestamp: u64,

    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

impl Commit {
    /// Create a new commit
    pub fn new(
        tree: Hash,
        parents: Vec<Hash>,
        message: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Commit {
            tree,
            parents,
            message: message.into(),
            author: author.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            metadata: None,
        }
    }

    /// Create the initial commit (no parents)
    pub fn initial(tree: Hash, message: impl Into<String>, author: impl Into<String>) -> Self {
        Self::new(tree, vec![], message, author)
    }

    /// Create a commit with a single parent
    pub fn child(
        tree: Hash,
        parent: Hash,
        message: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self::new(tree, vec![parent], message, author)
    }

    /// Create a merge commit
    pub fn merge(
        tree: Hash,
        parents: Vec<Hash>,
        message: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        assert!(
            parents.len() >= 2,
            "merge commit requires at least 2 parents"
        );
        Self::new(tree, parents, message, author)
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Compute the commit hash
    pub fn hash(&self) -> Hash {
        let data = bincode::serialize(self).expect("serialization should not fail");
        Hash::digest(&data)
    }

    /// Check if this is the initial commit
    pub fn is_initial(&self) -> bool {
        self.parents.is_empty()
    }

    /// Check if this is a merge commit
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_commit() {
        let tree = Hash::digest(b"tree");
        let commit = Commit::initial(tree, "Initial commit", "agent-1");

        assert!(commit.is_initial());
        assert!(!commit.is_merge());
        assert_eq!(commit.parents.len(), 0);
    }

    #[test]
    fn test_child_commit() {
        let tree = Hash::digest(b"tree");
        let parent = Hash::digest(b"parent");
        let commit = Commit::child(tree, parent, "Add new thought", "agent-1");

        assert!(!commit.is_initial());
        assert!(!commit.is_merge());
        assert_eq!(commit.parents.len(), 1);
    }

    #[test]
    fn test_merge_commit() {
        let tree = Hash::digest(b"tree");
        let p1 = Hash::digest(b"parent1");
        let p2 = Hash::digest(b"parent2");
        let commit = Commit::merge(tree, vec![p1, p2], "Merge branches", "agent-1");

        assert!(!commit.is_initial());
        assert!(commit.is_merge());
        assert_eq!(commit.parents.len(), 2);
    }

    #[test]
    fn test_commit_hash_deterministic() {
        let tree = Hash::digest(b"tree");
        let commit = Commit::initial(tree, "test", "author");
        let hash1 = commit.hash();
        let hash2 = commit.hash();

        assert_eq!(hash1, hash2);
    }
}
