//! # indra_db
//!
//! A content-addressed graph database for versioned thoughts.
//!
//! indra_db combines git-like content-addressed storage with graph database
//! semantics, enabling agents to maintain evolving knowledge graphs with
//! full history and semantic search capabilities.
//!
//! ## Core Concepts
//!
//! - **Thoughts**: Content-addressed nodes with embeddings
//! - **Relations**: Typed edges between thoughts (float to latest version)
//! - **Commits**: Immutable snapshots of graph state
//! - **Branches**: Named refs to commits, enabling parallel hypotheses
//!
//! ## Example
//!
//! ```ignore
//! use indra_db::Database;
//!
//! let mut db = Database::open_or_create("thoughts.indra")?;
//! let thought = db.create_thought("The cat sat on the mat")?;
//! db.commit("Initial thought")?;
//! ```

pub mod embedding;
pub mod graph;
pub mod model;
pub mod ops;
pub mod search;
pub mod store;
pub mod trie;

mod database;
mod error;

pub use database::Database;
pub use embedding::{Embedder, MockEmbedder};
pub use error::{Error, Result};
pub use graph::TraversalDirection;
pub use model::{Commit, Edge, EdgeType, Hash, Thought, ThoughtId};
pub use search::SearchResult;
pub use store::ObjectStore;

/// Database version for format compatibility
pub const VERSION: u32 = 1;

/// Magic bytes for file identification
pub const MAGIC: &[u8; 8] = b"INDRA_DB";
