//! Trie node types

use crate::model::Hash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A node in the merkle trie
///
/// We use a radix trie structure where:
/// - Keys are thought/edge IDs converted to bytes
/// - Values are content hashes (for leaves) or child node hashes (for branches)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TrieNode {
    /// A branch node with children indexed by key prefix
    Branch {
        /// Prefix bytes shared by all children
        prefix: Vec<u8>,
        /// Children indexed by the next byte after prefix
        children: BTreeMap<u8, Hash>,
        /// Optional value if this exact key exists
        value: Option<Hash>,
    },
    /// A leaf node with a value
    Leaf {
        /// Remaining key suffix
        key_suffix: Vec<u8>,
        /// The content hash
        value: Hash,
    },
    /// An empty node
    Empty,
}

impl TrieNode {
    /// Create an empty node
    pub fn empty() -> Self {
        TrieNode::Empty
    }

    /// Create a leaf node
    pub fn leaf(key_suffix: Vec<u8>, value: Hash) -> Self {
        TrieNode::Leaf { key_suffix, value }
    }

    /// Create a branch node
    pub fn branch(prefix: Vec<u8>) -> Self {
        TrieNode::Branch {
            prefix,
            children: BTreeMap::new(),
            value: None,
        }
    }

    /// Compute the hash of this node
    pub fn hash(&self) -> Hash {
        let data = bincode::serialize(self).expect("serialization should not fail");
        Hash::digest(&data)
    }

    /// Check if this node is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, TrieNode::Empty)
    }

    /// Get the value at this exact node (if any)
    pub fn value(&self) -> Option<Hash> {
        match self {
            TrieNode::Leaf { value, .. } => Some(*value),
            TrieNode::Branch { value, .. } => *value,
            TrieNode::Empty => None,
        }
    }
}

impl Default for TrieNode {
    fn default() -> Self {
        TrieNode::Empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_hash_deterministic() {
        let node = TrieNode::leaf(b"key".to_vec(), Hash::digest(b"value"));
        let h1 = node.hash();
        let h2 = node.hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_different_nodes_different_hashes() {
        let n1 = TrieNode::leaf(b"key1".to_vec(), Hash::digest(b"value"));
        let n2 = TrieNode::leaf(b"key2".to_vec(), Hash::digest(b"value"));
        assert_ne!(n1.hash(), n2.hash());
    }
}
