//! High-level Database API
//!
//! This module provides the main entry point for interacting with indra_db.

use crate::embedding::Embedder;
use crate::graph::GraphView;
use crate::model::{Commit, Edge, EdgeType, Hash, JsonValue, Thought, ThoughtId};
use crate::ops::{diff_trees, BranchManager, Diff};
use crate::search::{SearchResult, VectorSearch};
use crate::store::ObjectStore;
use crate::trie::MerkleTrie;
use crate::Result;
use std::path::Path;
use std::sync::Arc;

/// The main database interface
///
/// Provides a convenient API for:
/// - Creating and managing thoughts
/// - Creating relationships between thoughts  
/// - Semantic search
/// - Version control (branches, commits, history)
pub struct Database {
    store: ObjectStore,
    embedder: Option<Arc<dyn Embedder>>,
    /// Current working state (uncommitted changes)
    working_tree: WorkingTree,
}

/// Tracks uncommitted changes
struct WorkingTree {
    /// Thoughts by ID (includes modifications)
    thoughts: std::collections::HashMap<ThoughtId, Thought>,
    /// Edges (canonical key → Edge)
    edges: std::collections::HashMap<String, Edge>,
    /// IDs of thoughts that were removed
    removed_thoughts: std::collections::HashSet<ThoughtId>,
    /// Keys of edges that were removed  
    removed_edges: std::collections::HashSet<String>,
    /// Whether there are uncommitted changes
    dirty: bool,
}

impl WorkingTree {
    fn new() -> Self {
        WorkingTree {
            thoughts: std::collections::HashMap::new(),
            edges: std::collections::HashMap::new(),
            removed_thoughts: std::collections::HashSet::new(),
            removed_edges: std::collections::HashSet::new(),
            dirty: false,
        }
    }

    fn clear(&mut self) {
        self.thoughts.clear();
        self.edges.clear();
        self.removed_thoughts.clear();
        self.removed_edges.clear();
        self.dirty = false;
    }
}

impl Database {
    /// Create a new database at the given path
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let store = ObjectStore::create(path)?;
        Ok(Database {
            store,
            embedder: None,
            working_tree: WorkingTree::new(),
        })
    }

    /// Open an existing database
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let store = ObjectStore::open(path)?;
        Ok(Database {
            store,
            embedder: None,
            working_tree: WorkingTree::new(),
        })
    }

    /// Open or create a database
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self> {
        let store = ObjectStore::open_or_create(path)?;
        Ok(Database {
            store,
            embedder: None,
            working_tree: WorkingTree::new(),
        })
    }

    /// Set the embedder to use for semantic search
    pub fn with_embedder(mut self, embedder: impl Embedder + 'static) -> Self {
        self.embedder = Some(Arc::new(embedder));
        self
    }

    /// Set the embedder (mutable)
    pub fn set_embedder(&mut self, embedder: impl Embedder + 'static) {
        self.embedder = Some(Arc::new(embedder));
    }

    // === Thought Operations ===

    /// Create a new thought
    pub fn create_thought(&mut self, content: impl Into<String>) -> Result<ThoughtId> {
        let mut thought = Thought::new(content);

        // Generate embedding if we have an embedder
        if let Some(ref embedder) = self.embedder {
            thought.embedding = Some(embedder.embed(&thought.content)?);
            thought.attrs.insert(
                "embedder_model".to_string(),
                JsonValue::new(serde_json::Value::String(embedder.model_name().to_string())),
            );
        }

        let id = thought.id.clone();
        self.working_tree.thoughts.insert(id.clone(), thought);
        self.working_tree.removed_thoughts.remove(&id);
        self.working_tree.dirty = true;

        Ok(id)
    }

    /// Create a thought with a specific ID
    pub fn create_thought_with_id(
        &mut self,
        id: impl Into<ThoughtId>,
        content: impl Into<String>,
    ) -> Result<ThoughtId> {
        let id = id.into();
        let mut thought = Thought::with_id(id.clone(), content);

        if let Some(ref embedder) = self.embedder {
            thought.embedding = Some(embedder.embed(&thought.content)?);
            thought.attrs.insert(
                "embedder_model".to_string(),
                JsonValue::new(serde_json::Value::String(embedder.model_name().to_string())),
            );
        }

        self.working_tree.thoughts.insert(id.clone(), thought);
        self.working_tree.removed_thoughts.remove(&id);
        self.working_tree.dirty = true;

        Ok(id)
    }

    /// Get a thought by ID (checks working tree first, then committed state)
    pub fn get_thought(&self, id: &ThoughtId) -> Result<Option<Thought>> {
        // Check if removed
        if self.working_tree.removed_thoughts.contains(id) {
            return Ok(None);
        }

        // Check working tree
        if let Some(thought) = self.working_tree.thoughts.get(id) {
            return Ok(Some(thought.clone()));
        }

        // Check committed state
        let tree_hash = self.head_tree()?;
        if tree_hash.is_zero() {
            return Ok(None);
        }

        let view = GraphView::new(&self.store, tree_hash)?;
        view.get_thought(id)
    }

    /// Update a thought's content
    pub fn update_thought(&mut self, id: &ThoughtId, content: impl Into<String>) -> Result<()> {
        let mut thought = self
            .get_thought(id)?
            .ok_or_else(|| crate::Error::NotFound(id.to_string()))?;

        thought.update_content(content);

        // Re-embed if we have an embedder
        if let Some(ref embedder) = self.embedder {
            thought.embedding = Some(embedder.embed(&thought.content)?);
            thought.attrs.insert(
                "embedder_model".to_string(),
                JsonValue::new(serde_json::Value::String(embedder.model_name().to_string())),
            );
        }

        self.working_tree.thoughts.insert(id.clone(), thought);
        self.working_tree.dirty = true;

        Ok(())
    }

    /// Delete a thought
    pub fn delete_thought(&mut self, id: &ThoughtId) -> Result<()> {
        self.working_tree.thoughts.remove(id);
        self.working_tree.removed_thoughts.insert(id.clone());
        self.working_tree.dirty = true;
        Ok(())
    }

    /// List all thoughts (committed + working tree changes)
    pub fn list_thoughts(&self) -> Result<Vec<Thought>> {
        let tree_hash = self.head_tree()?;
        let mut thoughts: std::collections::HashMap<ThoughtId, Thought> = if tree_hash.is_zero() {
            std::collections::HashMap::new()
        } else {
            let view = GraphView::new(&self.store, tree_hash)?;
            view.all_thoughts()?
                .into_iter()
                .map(|t| (t.id.clone(), t))
                .collect()
        };

        // Apply working tree changes
        for (id, thought) in &self.working_tree.thoughts {
            thoughts.insert(id.clone(), thought.clone());
        }
        for id in &self.working_tree.removed_thoughts {
            thoughts.remove(id);
        }

        Ok(thoughts.into_values().collect())
    }

    // === Edge Operations ===

    /// Create an edge between two thoughts
    pub fn relate(
        &mut self,
        source: impl Into<ThoughtId>,
        target: impl Into<ThoughtId>,
        edge_type: impl Into<EdgeType>,
    ) -> Result<()> {
        let edge = Edge::new(source, target, edge_type);
        let key = edge_key(&edge);

        self.working_tree.edges.insert(key.clone(), edge);
        self.working_tree.removed_edges.remove(&key);
        self.working_tree.dirty = true;

        Ok(())
    }

    /// Create a weighted edge
    pub fn relate_weighted(
        &mut self,
        source: impl Into<ThoughtId>,
        target: impl Into<ThoughtId>,
        edge_type: impl Into<EdgeType>,
        weight: f32,
    ) -> Result<()> {
        let edge = Edge::new(source, target, edge_type).with_weight(weight);
        let key = edge_key(&edge);

        self.working_tree.edges.insert(key.clone(), edge);
        self.working_tree.removed_edges.remove(&key);
        self.working_tree.dirty = true;

        Ok(())
    }

    /// Remove an edge
    pub fn unrelate(
        &mut self,
        source: impl Into<ThoughtId>,
        target: impl Into<ThoughtId>,
        edge_type: impl Into<EdgeType>,
    ) -> Result<()> {
        let source = source.into();
        let target = target.into();
        let edge_type = edge_type.into();
        let key = format!("{}:{}:{}", source.0, target.0, edge_type.0);

        self.working_tree.edges.remove(&key);
        self.working_tree.removed_edges.insert(key);
        self.working_tree.dirty = true;

        Ok(())
    }

    /// Get neighbors of a thought
    pub fn neighbors(
        &self,
        id: &ThoughtId,
        direction: crate::graph::TraversalDirection,
    ) -> Result<Vec<(Thought, Edge)>> {
        let tree_hash = self.head_tree()?;
        if tree_hash.is_zero() {
            return Ok(vec![]);
        }

        let view = GraphView::new(&self.store, tree_hash)?;
        view.neighbors(id, direction, None)
    }

    // === Search Operations ===

    /// Semantic search for thoughts similar to the query
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let embedder = self
            .embedder
            .as_ref()
            .ok_or_else(|| crate::Error::Embedding("No embedder configured".into()))?;

        let query_embedding = embedder.embed(query)?;

        let tree_hash = self.head_tree()?;
        if tree_hash.is_zero() {
            return Ok(vec![]);
        }

        let view = GraphView::new(&self.store, tree_hash)?;
        let search = VectorSearch::new(&view);
        search.search(&query_embedding, limit)
    }

    /// Search with a minimum similarity threshold
    pub fn search_with_threshold(
        &self,
        query: &str,
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let embedder = self
            .embedder
            .as_ref()
            .ok_or_else(|| crate::Error::Embedding("No embedder configured".into()))?;

        let query_embedding = embedder.embed(query)?;

        let tree_hash = self.head_tree()?;
        if tree_hash.is_zero() {
            return Ok(vec![]);
        }

        let view = GraphView::new(&self.store, tree_hash)?;
        let search = VectorSearch::new(&view);
        search.search_with_threshold(&query_embedding, threshold, limit)
    }

    // === Version Control Operations ===

    /// Commit current changes
    pub fn commit(&mut self, message: &str) -> Result<Hash> {
        self.commit_with_author(message, "indra_db")
    }

    /// Commit with a specific author
    pub fn commit_with_author(&mut self, message: &str, author: &str) -> Result<Hash> {
        // Build tree from current state
        let base_tree = self.head_tree()?;
        let mut trie = MerkleTrie::from_root(&self.store, base_tree)?;

        // Check if there are any changes to commit
        let has_changes = !self.working_tree.thoughts.is_empty()
            || !self.working_tree.edges.is_empty()
            || !self.working_tree.removed_thoughts.is_empty()
            || !self.working_tree.removed_edges.is_empty();

        if !has_changes {
            return Err(crate::Error::NotFound("Nothing to commit".into()));
        }

        // Apply thought changes
        for (id, thought) in &self.working_tree.thoughts {
            let hash = self.store.put_thought(thought)?;
            let key = format!("t:{}", id.0);
            trie.insert(key.as_bytes(), hash)?;
        }
        for id in &self.working_tree.removed_thoughts {
            let key = format!("t:{}", id.0);
            trie.remove(key.as_bytes())?;
        }

        // Apply edge changes
        for (key, edge) in &self.working_tree.edges {
            let hash = self.store.put_edge(edge)?;
            let full_key = format!("e:{}", key);
            trie.insert(full_key.as_bytes(), hash)?;
        }
        for key in &self.working_tree.removed_edges {
            let full_key = format!("e:{}", key);
            trie.remove(full_key.as_bytes())?;
        }

        // Commit the tree
        let tree_hash = trie.commit()?;

        let manager = BranchManager::new(&self.store);
        let commit_hash = manager.commit(tree_hash, message, author)?;

        // Clear working tree
        self.working_tree.clear();

        Ok(commit_hash)
    }

    /// Check if there are uncommitted changes
    pub fn is_dirty(&self) -> bool {
        self.working_tree.dirty
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> String {
        self.store.head()
    }

    /// Create a new branch at current HEAD
    pub fn create_branch(&self, name: &str) -> Result<()> {
        let manager = BranchManager::new(&self.store);
        manager.create_branch(name)
    }

    /// Switch to a branch
    pub fn checkout(&mut self, branch: &str) -> Result<()> {
        if self.working_tree.dirty {
            return Err(crate::Error::BranchNotFound(
                "Cannot checkout with uncommitted changes".into(),
            ));
        }

        let manager = BranchManager::new(&self.store);
        manager.switch_branch(branch)
    }

    /// List all branches
    pub fn list_branches(&self) -> Vec<(String, Hash)> {
        self.store.list_refs()
    }

    /// Get commit history
    pub fn log(&self, limit: Option<usize>) -> Result<Vec<(Hash, Commit)>> {
        let manager = BranchManager::new(&self.store);
        manager.log(limit)
    }

    /// Diff between two commits
    pub fn diff(&self, from: Hash, to: Hash) -> Result<Diff> {
        let from_tree = if from.is_zero() {
            Hash::ZERO
        } else {
            self.store.get_commit(&from)?.tree
        };

        let to_tree = if to.is_zero() {
            Hash::ZERO
        } else {
            self.store.get_commit(&to)?.tree
        };

        diff_trees(&self.store, from_tree, to_tree)
    }

    /// Get the current HEAD tree hash
    fn head_tree(&self) -> Result<Hash> {
        if let Some(commit_hash) = self.store.head_commit() {
            let commit = self.store.get_commit(&commit_hash)?;
            Ok(commit.tree)
        } else {
            Ok(Hash::ZERO)
        }
    }

    /// Sync to disk
    pub fn sync(&self) -> Result<()> {
        self.store.sync()
    }
}

fn edge_key(edge: &Edge) -> String {
    format!("{}:{}:{}", edge.source.0, edge.target.0, edge.edge_type.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::MockEmbedder;
    use tempfile::tempdir;

    #[test]
    fn test_database_create_and_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");

        // Create
        {
            let _db = Database::create(&path).unwrap();
        }

        // Open
        {
            let _db = Database::open(&path).unwrap();
        }
    }

    #[test]
    fn test_thought_crud() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let mut db = Database::create(&path).unwrap();

        // Create
        let id = db.create_thought("Hello, world!").unwrap();

        // Read
        let thought = db.get_thought(&id).unwrap().unwrap();
        assert_eq!(thought.content, "Hello, world!");

        // Update
        db.update_thought(&id, "Hello, Indra!").unwrap();
        let thought = db.get_thought(&id).unwrap().unwrap();
        assert_eq!(thought.content, "Hello, Indra!");

        // Commit
        db.commit("Add and update thought").unwrap();

        // Verify persistence
        let thought = db.get_thought(&id).unwrap().unwrap();
        assert_eq!(thought.content, "Hello, Indra!");
    }

    #[test]
    fn test_relationships() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let mut db = Database::create(&path).unwrap();

        let id1 = db.create_thought_with_id("cat", "Cat").unwrap();
        let id2 = db.create_thought_with_id("animal", "Animal").unwrap();

        db.relate(&id1, &id2, EdgeType::PART_OF).unwrap();
        db.commit("Add cat and animal").unwrap();

        let neighbors = db
            .neighbors(&id1, crate::graph::TraversalDirection::Outgoing)
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0.id.0, "animal");
    }

    #[test]
    fn test_search_with_embedder() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let mut db = Database::create(&path)
            .unwrap()
            .with_embedder(MockEmbedder::default());

        db.create_thought("The cat sat on the mat").unwrap();
        db.create_thought("A dog ran in the park").unwrap();
        db.create_thought("The bird flew over the tree").unwrap();
        db.commit("Add thoughts").unwrap();

        let results = db.search("cat sitting", 10).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_branching() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let mut db = Database::create(&path).unwrap();

        // Initial commit on main
        db.create_thought("Initial thought").unwrap();
        db.commit("Initial commit").unwrap();

        // Create and switch to feature branch
        db.create_branch("feature").unwrap();
        db.checkout("feature").unwrap();
        assert_eq!(db.current_branch(), "feature");

        // Add thought on feature
        db.create_thought("Feature thought").unwrap();
        db.commit("Feature commit").unwrap();

        // Switch back to main
        db.checkout("main").unwrap();
        assert_eq!(db.current_branch(), "main");
    }

    #[test]
    fn test_commit_log() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let mut db = Database::create(&path).unwrap();

        db.create_thought("First").unwrap();
        db.commit("First commit").unwrap();

        db.create_thought("Second").unwrap();
        db.commit("Second commit").unwrap();

        let log = db.log(None).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].1.message, "Second commit");
        assert_eq!(log[1].1.message, "First commit");
    }
}
