//! Merkle trie implementation for the graph state

use super::TrieNode;
use crate::model::Hash;
use crate::store::{Blob, BlobType, ObjectStore};
use crate::Result;
use std::collections::HashMap;

/// A merkle trie that stores thoughts and edges
///
/// The trie has two namespaces:
/// - "t:" prefix for thoughts (keyed by ThoughtId)
/// - "e:" prefix for edges (keyed by canonical edge key)
pub struct MerkleTrie<'a> {
    store: &'a ObjectStore,
    /// Root node (cached in memory)
    root: TrieNode,
    /// Cache of loaded nodes
    cache: HashMap<Hash, TrieNode>,
}

impl<'a> MerkleTrie<'a> {
    /// Create a new empty trie
    pub fn new(store: &'a ObjectStore) -> Self {
        MerkleTrie {
            store,
            root: TrieNode::empty(),
            cache: HashMap::new(),
        }
    }

    /// Load a trie from a root hash
    pub fn from_root(store: &'a ObjectStore, root_hash: Hash) -> Result<Self> {
        let mut trie = MerkleTrie {
            store,
            root: TrieNode::empty(),
            cache: HashMap::new(),
        };

        if !root_hash.is_zero() {
            trie.root = trie.load_node(&root_hash)?;
        }

        Ok(trie)
    }

    /// Get the root hash
    pub fn root_hash(&self) -> Hash {
        if self.root.is_empty() {
            Hash::ZERO
        } else {
            self.root.hash()
        }
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, key: &[u8], value: Hash) -> Result<()> {
        self.root = self.insert_recursive(&self.root.clone(), key, 0, value)?;
        Ok(())
    }

    /// Get a value by key
    pub fn get(&self, key: &[u8]) -> Result<Option<Hash>> {
        self.get_recursive(&self.root, key, 0)
    }

    /// Remove a key
    pub fn remove(&mut self, key: &[u8]) -> Result<Option<Hash>> {
        let (new_root, removed) = self.remove_recursive(&self.root.clone(), key, 0)?;
        self.root = new_root;
        Ok(removed)
    }

    /// Persist all nodes to the store and return the root hash
    pub fn commit(&self) -> Result<Hash> {
        if self.root.is_empty() {
            return Ok(Hash::ZERO);
        }
        self.persist_node(&self.root)
    }

    /// List all keys with a given prefix
    pub fn list_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Hash)>> {
        let mut results = Vec::new();
        self.collect_prefix(&self.root, prefix, 0, Vec::new(), &mut results)?;
        Ok(results)
    }

    // === Internal helpers ===

    fn load_node(&mut self, hash: &Hash) -> Result<TrieNode> {
        if let Some(node) = self.cache.get(hash) {
            return Ok(node.clone());
        }

        let blob = self.store.get(hash)?;
        if blob.blob_type != BlobType::Tree {
            return Err(crate::Error::Corruption(format!(
                "Expected Tree, got {:?}",
                blob.blob_type
            )));
        }

        let node: TrieNode = bincode::deserialize(&blob.data)?;
        self.cache.insert(*hash, node.clone());
        Ok(node)
    }

    fn persist_node(&self, node: &TrieNode) -> Result<Hash> {
        let data = bincode::serialize(node)?;
        let blob = Blob::new(BlobType::Tree, data);
        self.store.put(&blob)
    }

    fn insert_recursive(
        &mut self,
        node: &TrieNode,
        key: &[u8],
        depth: usize,
        value: Hash,
    ) -> Result<TrieNode> {
        let remaining = &key[depth..];

        match node {
            TrieNode::Empty => {
                // Create a new leaf
                Ok(TrieNode::leaf(remaining.to_vec(), value))
            }
            TrieNode::Leaf {
                key_suffix,
                value: existing_value,
            } => {
                if key_suffix == remaining {
                    // Same key, update value
                    Ok(TrieNode::leaf(remaining.to_vec(), value))
                } else {
                    // Need to split into a branch
                    let common_len = common_prefix_len(remaining, key_suffix);
                    let common = remaining[..common_len].to_vec();

                    let mut children = std::collections::BTreeMap::new();

                    // Add existing leaf
                    if common_len < key_suffix.len() {
                        let existing_suffix = key_suffix[common_len..].to_vec();
                        let existing_first = existing_suffix[0];
                        let existing_node =
                            TrieNode::leaf(existing_suffix[1..].to_vec(), *existing_value);
                        let existing_hash = self.persist_node(&existing_node)?;
                        children.insert(existing_first, existing_hash);
                    }

                    // Add new leaf
                    if common_len < remaining.len() {
                        let new_suffix = remaining[common_len..].to_vec();
                        let new_first = new_suffix[0];
                        let new_node = TrieNode::leaf(new_suffix[1..].to_vec(), value);
                        let new_hash = self.persist_node(&new_node)?;
                        children.insert(new_first, new_hash);
                    }

                    // Handle case where one key is prefix of another
                    let branch_value = if common_len == remaining.len() {
                        Some(value)
                    } else if common_len == key_suffix.len() {
                        Some(*existing_value)
                    } else {
                        None
                    };

                    Ok(TrieNode::Branch {
                        prefix: common,
                        children,
                        value: branch_value,
                    })
                }
            }
            TrieNode::Branch {
                prefix,
                children,
                value: branch_value,
            } => {
                let common_len = common_prefix_len(remaining, prefix);

                if common_len < prefix.len() {
                    // Need to split the branch
                    let common = remaining[..common_len].to_vec();
                    let old_suffix = prefix[common_len..].to_vec();
                    let old_first = old_suffix[0];

                    // Create new branch for the old subtree
                    let old_branch = TrieNode::Branch {
                        prefix: old_suffix[1..].to_vec(),
                        children: children.clone(),
                        value: *branch_value,
                    };
                    let old_hash = self.persist_node(&old_branch)?;

                    let mut new_children = std::collections::BTreeMap::new();
                    new_children.insert(old_first, old_hash);

                    // Add the new key
                    let new_value = if common_len == remaining.len() {
                        Some(value)
                    } else {
                        let new_suffix = remaining[common_len..].to_vec();
                        let new_first = new_suffix[0];
                        let new_node = TrieNode::leaf(new_suffix[1..].to_vec(), value);
                        let new_hash = self.persist_node(&new_node)?;
                        new_children.insert(new_first, new_hash);
                        None
                    };

                    Ok(TrieNode::Branch {
                        prefix: common,
                        children: new_children,
                        value: new_value,
                    })
                } else {
                    // Prefix matches, continue down
                    let after_prefix = &remaining[prefix.len()..];

                    if after_prefix.is_empty() {
                        // Key ends at this branch
                        Ok(TrieNode::Branch {
                            prefix: prefix.clone(),
                            children: children.clone(),
                            value: Some(value),
                        })
                    } else {
                        let next_byte = after_prefix[0];
                        let mut new_children = children.clone();

                        if let Some(child_hash) = children.get(&next_byte) {
                            // Recurse into existing child
                            let child = self.load_node(child_hash)?;
                            let new_child =
                                self.insert_recursive(&child, &after_prefix[1..], 0, value)?;
                            let new_hash = self.persist_node(&new_child)?;
                            new_children.insert(next_byte, new_hash);
                        } else {
                            // Create new child
                            let new_node = TrieNode::leaf(after_prefix[1..].to_vec(), value);
                            let new_hash = self.persist_node(&new_node)?;
                            new_children.insert(next_byte, new_hash);
                        }

                        Ok(TrieNode::Branch {
                            prefix: prefix.clone(),
                            children: new_children,
                            value: *branch_value,
                        })
                    }
                }
            }
        }
    }

    fn get_recursive(&self, node: &TrieNode, key: &[u8], depth: usize) -> Result<Option<Hash>> {
        let remaining = &key[depth..];

        match node {
            TrieNode::Empty => Ok(None),
            TrieNode::Leaf { key_suffix, value } => {
                if key_suffix == remaining {
                    Ok(Some(*value))
                } else {
                    Ok(None)
                }
            }
            TrieNode::Branch {
                prefix,
                children,
                value,
            } => {
                if !remaining.starts_with(prefix) {
                    return Ok(None);
                }

                let after_prefix = &remaining[prefix.len()..];

                if after_prefix.is_empty() {
                    return Ok(*value);
                }

                let next_byte = after_prefix[0];
                if let Some(child_hash) = children.get(&next_byte) {
                    // Need to load child - but we don't have mutable access
                    // For now, load directly (inefficient but correct)
                    let blob = self.store.get(child_hash)?;
                    let child: TrieNode = bincode::deserialize(&blob.data)?;
                    self.get_recursive(&child, &after_prefix[1..], 0)
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn remove_recursive(
        &mut self,
        node: &TrieNode,
        key: &[u8],
        depth: usize,
    ) -> Result<(TrieNode, Option<Hash>)> {
        let remaining = &key[depth..];

        match node {
            TrieNode::Empty => Ok((TrieNode::Empty, None)),
            TrieNode::Leaf { key_suffix, value } => {
                if key_suffix == remaining {
                    Ok((TrieNode::Empty, Some(*value)))
                } else {
                    Ok((node.clone(), None))
                }
            }
            TrieNode::Branch {
                prefix,
                children,
                value,
            } => {
                if !remaining.starts_with(prefix) {
                    return Ok((node.clone(), None));
                }

                let after_prefix = &remaining[prefix.len()..];

                if after_prefix.is_empty() {
                    // Remove value at this branch
                    if children.is_empty() {
                        return Ok((TrieNode::Empty, *value));
                    }
                    return Ok((
                        TrieNode::Branch {
                            prefix: prefix.clone(),
                            children: children.clone(),
                            value: None,
                        },
                        *value,
                    ));
                }

                let next_byte = after_prefix[0];
                if let Some(child_hash) = children.get(&next_byte) {
                    let child = self.load_node(child_hash)?;
                    let (new_child, removed) =
                        self.remove_recursive(&child, &after_prefix[1..], 0)?;

                    if removed.is_some() {
                        let mut new_children = children.clone();
                        if new_child.is_empty() {
                            new_children.remove(&next_byte);
                        } else {
                            let new_hash = self.persist_node(&new_child)?;
                            new_children.insert(next_byte, new_hash);
                        }

                        // Collapse if only one child and no value
                        if new_children.len() == 1 && value.is_none() {
                            // Could collapse here for efficiency
                        }

                        if new_children.is_empty() && value.is_none() {
                            return Ok((TrieNode::Empty, removed));
                        }

                        return Ok((
                            TrieNode::Branch {
                                prefix: prefix.clone(),
                                children: new_children,
                                value: *value,
                            },
                            removed,
                        ));
                    }
                }

                Ok((node.clone(), None))
            }
        }
    }

    fn collect_prefix(
        &self,
        node: &TrieNode,
        prefix: &[u8],
        _depth: usize,
        current_key: Vec<u8>,
        results: &mut Vec<(Vec<u8>, Hash)>,
    ) -> Result<()> {
        match node {
            TrieNode::Empty => {}
            TrieNode::Leaf { key_suffix, value } => {
                let mut full_key = current_key;
                full_key.extend(key_suffix);
                if full_key.starts_with(prefix) {
                    results.push((full_key, *value));
                }
            }
            TrieNode::Branch {
                prefix: node_prefix,
                children,
                value,
            } => {
                let mut current = current_key;
                current.extend(node_prefix);

                if let Some(v) = value {
                    if current.starts_with(prefix) {
                        results.push((current.clone(), *v));
                    }
                }

                // Only recurse if we're still matching the prefix
                if current.starts_with(prefix) || prefix.starts_with(&current) {
                    for (byte, child_hash) in children {
                        let mut child_key = current.clone();
                        child_key.push(*byte);

                        let blob = self.store.get(child_hash)?;
                        let child: TrieNode = bincode::deserialize(&blob.data)?;
                        self.collect_prefix(&child, prefix, 0, child_key, results)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Find the length of the common prefix between two byte slices
fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, ObjectStore) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();
        (dir, store)
    }

    #[test]
    fn test_trie_insert_get() {
        let (_dir, store) = setup();
        let mut trie = MerkleTrie::new(&store);

        let value = Hash::digest(b"value1");
        trie.insert(b"key1", value).unwrap();

        assert_eq!(trie.get(b"key1").unwrap(), Some(value));
        assert_eq!(trie.get(b"key2").unwrap(), None);
    }

    #[test]
    fn test_trie_multiple_keys() {
        let (_dir, store) = setup();
        let mut trie = MerkleTrie::new(&store);

        let v1 = Hash::digest(b"v1");
        let v2 = Hash::digest(b"v2");
        let v3 = Hash::digest(b"v3");

        trie.insert(b"apple", v1).unwrap();
        trie.insert(b"application", v2).unwrap();
        trie.insert(b"banana", v3).unwrap();

        assert_eq!(trie.get(b"apple").unwrap(), Some(v1));
        assert_eq!(trie.get(b"application").unwrap(), Some(v2));
        assert_eq!(trie.get(b"banana").unwrap(), Some(v3));
        assert_eq!(trie.get(b"app").unwrap(), None);
    }

    #[test]
    fn test_trie_remove() {
        let (_dir, store) = setup();
        let mut trie = MerkleTrie::new(&store);

        let value = Hash::digest(b"value");
        trie.insert(b"key", value).unwrap();

        let removed = trie.remove(b"key").unwrap();
        assert_eq!(removed, Some(value));
        assert_eq!(trie.get(b"key").unwrap(), None);
    }

    #[test]
    fn test_trie_list_prefix() {
        let (_dir, store) = setup();
        let mut trie = MerkleTrie::new(&store);

        trie.insert(b"t:thought1", Hash::digest(b"t1")).unwrap();
        trie.insert(b"t:thought2", Hash::digest(b"t2")).unwrap();
        trie.insert(b"e:edge1", Hash::digest(b"e1")).unwrap();

        let thoughts = trie.list_prefix(b"t:").unwrap();
        assert_eq!(thoughts.len(), 2);

        let edges = trie.list_prefix(b"e:").unwrap();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_trie_root_hash_changes() {
        let (_dir, store) = setup();
        let mut trie = MerkleTrie::new(&store);

        let h1 = trie.root_hash();

        trie.insert(b"key", Hash::digest(b"value")).unwrap();
        let h2 = trie.root_hash();

        assert_ne!(h1, h2);

        trie.insert(b"key2", Hash::digest(b"value2")).unwrap();
        let h3 = trie.root_hash();

        assert_ne!(h2, h3);
    }
}
