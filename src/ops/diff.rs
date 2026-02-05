//! Diff operations between tree states

use crate::model::Hash;
use crate::store::ObjectStore;
use crate::trie::MerkleTrie;
use crate::Result;
use std::collections::{HashMap, HashSet};

/// Type of change in a diff
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffEntry {
    /// Key was added
    Added { key: Vec<u8>, new_hash: Hash },
    /// Key was removed
    Removed { key: Vec<u8>, old_hash: Hash },
    /// Key was modified
    Modified {
        key: Vec<u8>,
        old_hash: Hash,
        new_hash: Hash,
    },
}

impl DiffEntry {
    pub fn key(&self) -> &[u8] {
        match self {
            DiffEntry::Added { key, .. } => key,
            DiffEntry::Removed { key, .. } => key,
            DiffEntry::Modified { key, .. } => key,
        }
    }

    pub fn is_thought(&self) -> bool {
        self.key().starts_with(b"t:")
    }

    pub fn is_edge(&self) -> bool {
        self.key().starts_with(b"e:")
    }
}

/// A diff between two tree states
#[derive(Clone, Debug)]
pub struct Diff {
    pub entries: Vec<DiffEntry>,
}

impl Diff {
    pub fn new(entries: Vec<DiffEntry>) -> Self {
        Diff { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn added_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, DiffEntry::Added { .. }))
            .count()
    }

    pub fn removed_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, DiffEntry::Removed { .. }))
            .count()
    }

    pub fn modified_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, DiffEntry::Modified { .. }))
            .count()
    }

    pub fn thought_changes(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.is_thought())
    }

    pub fn edge_changes(&self) -> impl Iterator<Item = &DiffEntry> {
        self.entries.iter().filter(|e| e.is_edge())
    }
}

/// Compute the diff between two tree states
pub fn diff_trees(store: &ObjectStore, old_root: Hash, new_root: Hash) -> Result<Diff> {
    if old_root == new_root {
        return Ok(Diff::new(vec![]));
    }

    // Get all entries from both trees
    let old_entries = if old_root.is_zero() {
        HashMap::new()
    } else {
        let trie = MerkleTrie::from_root(store, old_root)?;
        collect_all_entries(&trie)?
    };

    let new_entries = if new_root.is_zero() {
        HashMap::new()
    } else {
        let trie = MerkleTrie::from_root(store, new_root)?;
        collect_all_entries(&trie)?
    };

    let mut diff_entries = Vec::new();

    // Find all keys
    let all_keys: HashSet<_> = old_entries.keys().chain(new_entries.keys()).collect();

    for key in all_keys {
        match (old_entries.get(key), new_entries.get(key)) {
            (None, Some(&new_hash)) => {
                diff_entries.push(DiffEntry::Added {
                    key: key.clone(),
                    new_hash,
                });
            }
            (Some(&old_hash), None) => {
                diff_entries.push(DiffEntry::Removed {
                    key: key.clone(),
                    old_hash,
                });
            }
            (Some(&old_hash), Some(&new_hash)) if old_hash != new_hash => {
                diff_entries.push(DiffEntry::Modified {
                    key: key.clone(),
                    old_hash,
                    new_hash,
                });
            }
            _ => {} // Unchanged
        }
    }

    // Sort entries for determinism
    diff_entries.sort_by(|a, b| a.key().cmp(b.key()));

    Ok(Diff::new(diff_entries))
}

fn collect_all_entries(trie: &MerkleTrie) -> Result<HashMap<Vec<u8>, Hash>> {
    let mut entries = HashMap::new();

    // Collect thoughts
    for (key, hash) in trie.list_prefix(b"t:")? {
        entries.insert(key, hash);
    }

    // Collect edges
    for (key, hash) in trie.list_prefix(b"e:")? {
        entries.insert(key, hash);
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Thought;
    use crate::trie::MerkleTrie;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, ObjectStore) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();
        (dir, store)
    }

    #[test]
    fn test_diff_empty_to_non_empty() {
        let (_dir, store) = setup();

        let thought = Thought::with_id("t1", "Hello");
        let hash = store.put_thought(&thought).unwrap();

        let mut trie = MerkleTrie::new(&store);
        trie.insert(b"t:t1", hash).unwrap();
        let new_root = trie.commit().unwrap();

        let diff = diff_trees(&store, Hash::ZERO, new_root).unwrap();

        assert_eq!(diff.added_count(), 1);
        assert_eq!(diff.removed_count(), 0);
        assert_eq!(diff.modified_count(), 0);
    }

    #[test]
    fn test_diff_modification() {
        let (_dir, store) = setup();

        // Create initial state
        let t1 = Thought::with_id("t1", "Hello");
        let h1 = store.put_thought(&t1).unwrap();

        let mut trie1 = MerkleTrie::new(&store);
        trie1.insert(b"t:t1", h1).unwrap();
        let root1 = trie1.commit().unwrap();

        // Create modified state
        let t1_modified = Thought::with_id("t1", "Hello, World!");
        let h2 = store.put_thought(&t1_modified).unwrap();

        let mut trie2 = MerkleTrie::new(&store);
        trie2.insert(b"t:t1", h2).unwrap();
        let root2 = trie2.commit().unwrap();

        let diff = diff_trees(&store, root1, root2).unwrap();

        assert_eq!(diff.added_count(), 0);
        assert_eq!(diff.removed_count(), 0);
        assert_eq!(diff.modified_count(), 1);
    }

    #[test]
    fn test_diff_same_trees() {
        let (_dir, store) = setup();

        let thought = Thought::with_id("t1", "Hello");
        let hash = store.put_thought(&thought).unwrap();

        let mut trie = MerkleTrie::new(&store);
        trie.insert(b"t:t1", hash).unwrap();
        let root = trie.commit().unwrap();

        let diff = diff_trees(&store, root, root).unwrap();

        assert!(diff.is_empty());
    }
}
