//! Branch and checkout operations

use crate::model::{Commit, Hash};
use crate::store::ObjectStore;
use crate::Result;

/// Manages branches and refs
pub struct BranchManager<'a> {
    store: &'a ObjectStore,
}

impl<'a> BranchManager<'a> {
    pub fn new(store: &'a ObjectStore) -> Self {
        BranchManager { store }
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> String {
        self.store.head()
    }

    /// List all branches
    pub fn list_branches(&self) -> Vec<(String, Hash)> {
        self.store.list_refs()
    }

    /// Create a new branch at the current HEAD
    pub fn create_branch(&self, name: &str) -> Result<()> {
        let head_commit = self.store.head_commit().unwrap_or(Hash::ZERO);
        self.store.create_branch(name, head_commit)
    }

    /// Create a new branch at a specific commit
    pub fn create_branch_at(&self, name: &str, commit_hash: Hash) -> Result<()> {
        self.store.create_branch(name, commit_hash)
    }

    /// Delete a branch
    pub fn delete_branch(&self, name: &str) -> Result<()> {
        self.store.delete_branch(name)
    }

    /// Switch to a branch
    pub fn switch_branch(&self, name: &str) -> Result<()> {
        self.store.set_head(name)
    }

    /// Get the commit at a branch
    pub fn branch_commit(&self, name: &str) -> Option<Hash> {
        self.store.get_ref(name)
    }

    /// Get the tree hash at the current HEAD
    pub fn head_tree(&self) -> Result<Hash> {
        if let Some(commit_hash) = self.store.head_commit() {
            let commit = self.store.get_commit(&commit_hash)?;
            Ok(commit.tree)
        } else {
            Ok(Hash::ZERO)
        }
    }

    /// Commit the current state
    pub fn commit(&self, tree_hash: Hash, message: &str, author: &str) -> Result<Hash> {
        let parent = self.store.head_commit();

        let commit = if let Some(parent_hash) = parent {
            Commit::child(tree_hash, parent_hash, message, author)
        } else {
            Commit::initial(tree_hash, message, author)
        };

        let commit_hash = self.store.put_commit(&commit)?;

        // Update the current branch ref
        let branch = self.current_branch();
        self.store.set_ref(&branch, commit_hash);

        Ok(commit_hash)
    }

    /// Get commit history from HEAD
    pub fn log(&self, limit: Option<usize>) -> Result<Vec<(Hash, Commit)>> {
        let mut result = Vec::new();
        let mut current = self.store.head_commit();
        let limit = limit.unwrap_or(usize::MAX);

        while let Some(hash) = current {
            if result.len() >= limit {
                break;
            }

            let commit = self.store.get_commit(&hash)?;
            let parent = commit.parents.first().copied();
            result.push((hash, commit));
            current = parent;
        }

        Ok(result)
    }
}

/// Checkout a specific commit or branch
pub fn checkout(store: &ObjectStore, target: &str) -> Result<Hash> {
    // Try as branch first
    if let Some(commit_hash) = store.get_ref(target) {
        store.set_head(target)?;
        if commit_hash.is_zero() {
            return Ok(Hash::ZERO);
        }
        let commit = store.get_commit(&commit_hash)?;
        return Ok(commit.tree);
    }

    // Try as commit hash
    if let Ok(hash) = Hash::from_hex(target) {
        if store.contains(&hash) {
            let commit = store.get_commit(&hash)?;
            // Create a detached HEAD (we'll use a special ref name)
            store.set_ref("HEAD_DETACHED", hash);
            return Ok(commit.tree);
        }
    }

    Err(crate::Error::RefNotFound(target.to_string()))
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
    fn test_branch_operations() {
        let (_dir, store) = setup();
        let manager = BranchManager::new(&store);

        // Default branch is main
        assert_eq!(manager.current_branch(), "main");

        // Create a new branch
        manager.create_branch("feature").unwrap();

        // Switch to it
        manager.switch_branch("feature").unwrap();
        assert_eq!(manager.current_branch(), "feature");

        // List branches
        let branches = manager.list_branches();
        assert_eq!(branches.len(), 2);
    }

    #[test]
    fn test_commit() {
        let (_dir, store) = setup();
        let manager = BranchManager::new(&store);

        let tree = Hash::digest(b"tree1");
        let c1 = manager.commit(tree, "First commit", "test").unwrap();

        let tree2 = Hash::digest(b"tree2");
        let c2 = manager.commit(tree2, "Second commit", "test").unwrap();

        // Check log
        let log = manager.log(None).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].0, c2);
        assert_eq!(log[1].0, c1);
    }
}
