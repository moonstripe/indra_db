//! Graph traversal operations

use crate::model::{Edge, EdgeType, Hash, Thought, ThoughtId};
use crate::store::ObjectStore;
use crate::trie::MerkleTrie;
use crate::Result;
use std::collections::{HashMap, HashSet, VecDeque};

/// Direction for traversing edges
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraversalDirection {
    /// Follow outgoing edges (source → target)
    Outgoing,
    /// Follow incoming edges (target → source)
    Incoming,
    /// Follow edges in both directions
    Both,
}

/// A view into the graph at a specific commit
///
/// This provides read-only access to the graph state at a point in time.
pub struct GraphView<'a> {
    store: &'a ObjectStore,
    trie: MerkleTrie<'a>,
    /// Cache: ThoughtId → content hash
    thought_index: HashMap<ThoughtId, Hash>,
    /// Cache: source ThoughtId → edge hashes
    edges_from: HashMap<ThoughtId, Vec<Hash>>,
    /// Cache: target ThoughtId → edge hashes  
    edges_to: HashMap<ThoughtId, Vec<Hash>>,
}

impl<'a> GraphView<'a> {
    /// Create a view at the given tree root
    pub fn new(store: &'a ObjectStore, root_hash: Hash) -> Result<Self> {
        let trie = MerkleTrie::from_root(store, root_hash)?;

        // Build indices from trie
        let mut thought_index = HashMap::new();
        let mut edges_from: HashMap<ThoughtId, Vec<Hash>> = HashMap::new();
        let mut edges_to: HashMap<ThoughtId, Vec<Hash>> = HashMap::new();

        // Load all thoughts
        for (key, hash) in trie.list_prefix(b"t:")? {
            let id_bytes = &key[2..]; // Skip "t:" prefix
            if let Ok(id_str) = std::str::from_utf8(id_bytes) {
                thought_index.insert(ThoughtId::new(id_str), hash);
            }
        }

        // Load all edges and build indices
        for (_, hash) in trie.list_prefix(b"e:")? {
            let edge = store.get_edge(&hash)?;
            edges_from
                .entry(edge.source.clone())
                .or_default()
                .push(hash);
            edges_to.entry(edge.target.clone()).or_default().push(hash);
        }

        Ok(GraphView {
            store,
            trie,
            thought_index,
            edges_from,
            edges_to,
        })
    }

    /// Create an empty view
    pub fn empty(store: &'a ObjectStore) -> Result<Self> {
        Self::new(store, Hash::ZERO)
    }

    /// Get a thought by ID
    pub fn get_thought(&self, id: &ThoughtId) -> Result<Option<Thought>> {
        if let Some(hash) = self.thought_index.get(id) {
            Ok(Some(self.store.get_thought(hash)?))
        } else {
            Ok(None)
        }
    }

    /// Check if a thought exists
    pub fn has_thought(&self, id: &ThoughtId) -> bool {
        self.thought_index.contains_key(id)
    }

    /// Get all thoughts
    pub fn all_thoughts(&self) -> Result<Vec<Thought>> {
        self.thought_index
            .values()
            .map(|hash| self.store.get_thought(hash))
            .collect()
    }

    /// Count thoughts
    pub fn thought_count(&self) -> usize {
        self.thought_index.len()
    }

    /// Get neighbors of a thought
    pub fn neighbors(
        &self,
        id: &ThoughtId,
        direction: TraversalDirection,
        edge_type: Option<&EdgeType>,
    ) -> Result<Vec<(Thought, Edge)>> {
        let mut results = Vec::new();

        let edge_hashes: Vec<Hash> = match direction {
            TraversalDirection::Outgoing => self.edges_from.get(id).cloned().unwrap_or_default(),
            TraversalDirection::Incoming => self.edges_to.get(id).cloned().unwrap_or_default(),
            TraversalDirection::Both => {
                let mut edges = self.edges_from.get(id).cloned().unwrap_or_default();
                edges.extend(self.edges_to.get(id).cloned().unwrap_or_default());
                edges
            }
        };

        for hash in edge_hashes {
            let edge = self.store.get_edge(&hash)?;

            // Filter by edge type if specified
            if let Some(et) = edge_type {
                if &edge.edge_type != et {
                    continue;
                }
            }

            // Get the neighbor thought
            let neighbor_id = match direction {
                TraversalDirection::Outgoing => &edge.target,
                TraversalDirection::Incoming => &edge.source,
                TraversalDirection::Both => {
                    if &edge.source == id {
                        &edge.target
                    } else {
                        &edge.source
                    }
                }
            };

            if let Some(thought) = self.get_thought(neighbor_id)? {
                results.push((thought, edge));
            }
        }

        Ok(results)
    }

    /// Get edges between two thoughts
    pub fn edges_between(&self, source: &ThoughtId, target: &ThoughtId) -> Result<Vec<Edge>> {
        let mut results = Vec::new();

        if let Some(edge_hashes) = self.edges_from.get(source) {
            for hash in edge_hashes {
                let edge = self.store.get_edge(hash)?;
                if &edge.target == target {
                    results.push(edge);
                }
            }
        }

        Ok(results)
    }

    /// Breadth-first traversal from a starting thought
    pub fn bfs(
        &self,
        start: &ThoughtId,
        direction: TraversalDirection,
        max_depth: Option<usize>,
    ) -> Result<Vec<(Thought, usize)>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        // Start with the initial thought
        if let Some(thought) = self.get_thought(start)? {
            visited.insert(start.clone());
            results.push((thought, 0));
            queue.push_back((start.clone(), 0));
        }

        while let Some((current_id, depth)) = queue.pop_front() {
            // Check depth limit
            if let Some(max) = max_depth {
                if depth >= max {
                    continue;
                }
            }

            // Get neighbors
            for (thought, _edge) in self.neighbors(&current_id, direction, None)? {
                if !visited.contains(&thought.id) {
                    visited.insert(thought.id.clone());
                    results.push((thought.clone(), depth + 1));
                    queue.push_back((thought.id.clone(), depth + 1));
                }
            }
        }

        Ok(results)
    }

    /// Find shortest path between two thoughts
    pub fn shortest_path(
        &self,
        from: &ThoughtId,
        to: &ThoughtId,
    ) -> Result<Option<Vec<ThoughtId>>> {
        if from == to {
            return Ok(Some(vec![from.clone()]));
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut predecessors: HashMap<ThoughtId, ThoughtId> = HashMap::new();

        visited.insert(from.clone());
        queue.push_back(from.clone());

        while let Some(current) = queue.pop_front() {
            for (thought, _edge) in self.neighbors(&current, TraversalDirection::Both, None)? {
                if !visited.contains(&thought.id) {
                    visited.insert(thought.id.clone());
                    predecessors.insert(thought.id.clone(), current.clone());

                    if &thought.id == to {
                        // Reconstruct path
                        let mut path = vec![to.clone()];
                        let mut curr = to;
                        while let Some(pred) = predecessors.get(curr) {
                            path.push(pred.clone());
                            curr = pred;
                        }
                        path.reverse();
                        return Ok(Some(path));
                    }

                    queue.push_back(thought.id.clone());
                }
            }
        }

        Ok(None)
    }

    /// Get the underlying trie root hash
    pub fn root_hash(&self) -> Hash {
        self.trie.root_hash()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EdgeType;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, ObjectStore) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();
        (dir, store)
    }

    #[test]
    fn test_empty_graph() {
        let (_dir, store) = setup();
        let view = GraphView::empty(&store).unwrap();

        assert_eq!(view.thought_count(), 0);
        assert!(view
            .get_thought(&ThoughtId::new("nonexistent"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_graph_with_thoughts() {
        let (_dir, store) = setup();

        // Create some thoughts
        let t1 = Thought::with_id("t1", "First thought");
        let t2 = Thought::with_id("t2", "Second thought");

        let h1 = store.put_thought(&t1).unwrap();
        let h2 = store.put_thought(&t2).unwrap();

        // Build a trie
        let mut trie = MerkleTrie::new(&store);
        trie.insert(b"t:t1", h1).unwrap();
        trie.insert(b"t:t2", h2).unwrap();
        let root = trie.commit().unwrap();

        // Create view
        let view = GraphView::new(&store, root).unwrap();

        assert_eq!(view.thought_count(), 2);
        assert!(view.has_thought(&ThoughtId::new("t1")));
        assert!(view.has_thought(&ThoughtId::new("t2")));

        let retrieved = view.get_thought(&ThoughtId::new("t1")).unwrap().unwrap();
        assert_eq!(retrieved.content, "First thought");
    }

    #[test]
    fn test_graph_with_edges() {
        let (_dir, store) = setup();

        // Create thoughts and edges
        let t1 = Thought::with_id("t1", "Cat");
        let t2 = Thought::with_id("t2", "Animal");
        let edge = Edge::new("t1", "t2", EdgeType::PART_OF);

        let h1 = store.put_thought(&t1).unwrap();
        let h2 = store.put_thought(&t2).unwrap();
        let eh = store.put_edge(&edge).unwrap();

        // Build trie
        let mut trie = MerkleTrie::new(&store);
        trie.insert(b"t:t1", h1).unwrap();
        trie.insert(b"t:t2", h2).unwrap();
        trie.insert(
            format!("e:{}:{}:{}", edge.source.0, edge.target.0, edge.edge_type.0).as_bytes(),
            eh,
        )
        .unwrap();
        let root = trie.commit().unwrap();

        // Create view and test neighbors
        let view = GraphView::new(&store, root).unwrap();

        let neighbors = view
            .neighbors(&ThoughtId::new("t1"), TraversalDirection::Outgoing, None)
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0.id.0, "t2");
    }
}
