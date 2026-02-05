//! Content-addressed object store
//!
//! This module implements the core storage layer using content-addressed blobs.
//! Objects are stored by their BLAKE3 hash and compressed with zstd.

mod blob;
mod file_store;

pub use blob::{Blob, BlobType};
pub use file_store::ObjectStore;
