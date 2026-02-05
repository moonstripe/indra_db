//! Merkle trie for structural sharing of graph state
//!
//! This implements a content-addressed trie where:
//! - Each node's hash is derived from its children's hashes
//! - Unchanged subtrees share storage across commits
//! - The root hash uniquely identifies the entire graph state

mod node;
mod tree;

pub use node::TrieNode;
pub use tree::MerkleTrie;
