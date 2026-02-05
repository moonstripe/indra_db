//! Single-file object store with content-addressed storage
//!
//! File format:
//! ```text
//! [HEADER: 64 bytes]
//!   - magic: 8 bytes ("INDRA_DB")
//!   - version: 4 bytes (u32 LE)
//!   - flags: 4 bytes
//!   - object_count: 8 bytes (u64 LE)
//!   - index_offset: 8 bytes (u64 LE)
//!   - reserved: 32 bytes
//!
//! [OBJECTS: variable]
//!   - blob data, concatenated
//!
//! [INDEX: variable]
//!   - sorted array of (hash, offset, size) entries
//!
//! [REFS: variable]
//!   - branch names → commit hashes
//! ```

use crate::model::Hash;
use crate::store::blob::{Blob, BlobType};
use crate::{Error, Result, MAGIC, VERSION};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const HEADER_SIZE: u64 = 64;

/// Index entry for an object
#[derive(Clone, Debug)]
struct IndexEntry {
    offset: u64,
    size: u32,
}

/// In-memory index for fast lookups
struct Index {
    entries: HashMap<Hash, IndexEntry>,
}

impl Index {
    fn new() -> Self {
        Index {
            entries: HashMap::new(),
        }
    }
}

/// A content-addressed object store backed by a single file
pub struct ObjectStore {
    /// Path to the database file
    path: std::path::PathBuf,
    /// The file handle
    file: RwLock<File>,
    /// In-memory index
    index: RwLock<Index>,
    /// Refs (branch name → commit hash)
    refs: RwLock<HashMap<String, Hash>>,
    /// Current HEAD ref name
    head: RwLock<String>,
    /// Current append position
    write_offset: RwLock<u64>,
}

impl ObjectStore {
    /// Create a new database file
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        // Write header
        let mut header = [0u8; HEADER_SIZE as usize];
        header[0..8].copy_from_slice(MAGIC);
        header[8..12].copy_from_slice(&VERSION.to_le_bytes());
        // flags: 0
        // object_count: 0
        // index_offset: 0 (will be updated)
        file.write_all(&header)?;
        file.sync_all()?;

        let mut refs = HashMap::new();
        refs.insert("main".to_string(), Hash::ZERO);

        Ok(ObjectStore {
            path,
            file: RwLock::new(file),
            index: RwLock::new(Index::new()),
            refs: RwLock::new(refs),
            head: RwLock::new("main".to_string()),
            write_offset: RwLock::new(HEADER_SIZE),
        })
    }

    /// Open an existing database file
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let mut file = OpenOptions::new().read(true).write(true).open(&path)?;

        // Read and validate header
        let mut header = [0u8; HEADER_SIZE as usize];
        file.read_exact(&mut header)?;

        if &header[0..8] != MAGIC {
            return Err(Error::InvalidFile("Invalid magic bytes".into()));
        }

        let version = u32::from_le_bytes(header[8..12].try_into().unwrap());
        if version != VERSION {
            return Err(Error::VersionMismatch {
                expected: VERSION,
                found: version,
            });
        }

        let object_count = u64::from_le_bytes(header[16..24].try_into().unwrap());
        let index_offset = u64::from_le_bytes(header[24..32].try_into().unwrap());
        let refs_offset = u64::from_le_bytes(header[32..40].try_into().unwrap());
        let refs_count = u64::from_le_bytes(header[40..48].try_into().unwrap());
        let head_len = u16::from_le_bytes(header[48..50].try_into().unwrap()) as usize;
        let head_name = if head_len > 0 && head_len <= 14 {
            String::from_utf8_lossy(&header[50..50 + head_len]).to_string()
        } else {
            "main".to_string()
        };

        // Load index if it exists
        let mut index = Index::new();
        if index_offset > 0 && object_count > 0 {
            file.seek(SeekFrom::Start(index_offset))?;
            // Each index entry: 32 (hash) + 8 (offset) + 4 (size) = 44 bytes
            for _ in 0..object_count {
                let mut entry_buf = [0u8; 44];
                file.read_exact(&mut entry_buf)?;

                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(&entry_buf[0..32]);
                let hash = Hash::from_bytes(hash_bytes);

                let offset = u64::from_le_bytes(entry_buf[32..40].try_into().unwrap());
                let size = u32::from_le_bytes(entry_buf[40..44].try_into().unwrap());

                index.entries.insert(hash, IndexEntry { offset, size });
            }
        }

        // Load refs
        let mut refs = HashMap::new();
        if refs_offset > 0 && refs_count > 0 {
            file.seek(SeekFrom::Start(refs_offset))?;
            for _ in 0..refs_count {
                let mut len_buf = [0u8; 2];
                file.read_exact(&mut len_buf)?;
                let name_len = u16::from_le_bytes(len_buf) as usize;

                let mut name_buf = vec![0u8; name_len];
                file.read_exact(&mut name_buf)?;
                let name = String::from_utf8_lossy(&name_buf).to_string();

                let mut hash_buf = [0u8; 32];
                file.read_exact(&mut hash_buf)?;
                let hash = Hash::from_bytes(hash_buf);

                refs.insert(name, hash);
            }
        }

        // Ensure main branch exists
        if refs.is_empty() {
            refs.insert("main".to_string(), Hash::ZERO);
        }

        // Calculate write offset (end of objects, before index)
        let write_offset = if index_offset > 0 {
            index_offset
        } else {
            file.seek(SeekFrom::End(0))?
        };

        Ok(ObjectStore {
            path,
            file: RwLock::new(file),
            index: RwLock::new(index),
            refs: RwLock::new(refs),
            head: RwLock::new(head_name),
            write_offset: RwLock::new(write_offset),
        })
    }

    /// Open or create a database file
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            Self::open(path)
        } else {
            Self::create(path)
        }
    }

    /// Store a blob, returns its hash
    pub fn put(&self, blob: &Blob) -> Result<Hash> {
        let hash = blob.hash();

        // Check if already exists
        {
            let index = self.index.read();
            if index.entries.contains_key(&hash) {
                return Ok(hash);
            }
        }

        // Compress and write
        let compressed = blob.compress()?;
        let size = compressed.len() as u32;

        let offset = {
            let mut write_offset = self.write_offset.write();
            let offset = *write_offset;

            let mut file = self.file.write();
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(&compressed)?;

            *write_offset = offset + size as u64;
            offset
        };

        // Update index
        {
            let mut index = self.index.write();
            index.entries.insert(hash, IndexEntry { offset, size });
        }

        Ok(hash)
    }

    /// Retrieve a blob by hash
    pub fn get(&self, hash: &Hash) -> Result<Blob> {
        let entry = {
            let index = self.index.read();
            index.entries.get(hash).cloned()
        };

        let entry = entry.ok_or_else(|| Error::NotFound(hash.to_hex()))?;

        let mut file = self.file.write();
        file.seek(SeekFrom::Start(entry.offset))?;

        let mut data = vec![0u8; entry.size as usize];
        file.read_exact(&mut data)?;

        Blob::decompress(&data)
    }

    /// Check if a hash exists
    pub fn contains(&self, hash: &Hash) -> bool {
        let index = self.index.read();
        index.entries.contains_key(hash)
    }

    /// Store a thought and return its hash
    pub fn put_thought(&self, thought: &crate::model::Thought) -> Result<Hash> {
        let data = bincode::serialize(thought)?;
        let blob = Blob::new(BlobType::Thought, data);
        self.put(&blob)
    }

    /// Retrieve a thought by hash
    pub fn get_thought(&self, hash: &Hash) -> Result<crate::model::Thought> {
        let blob = self.get(hash)?;
        if blob.blob_type != BlobType::Thought {
            return Err(Error::Corruption(format!(
                "Expected Thought, got {:?}",
                blob.blob_type
            )));
        }
        Ok(bincode::deserialize(&blob.data)?)
    }

    /// Store an edge and return its hash
    pub fn put_edge(&self, edge: &crate::model::Edge) -> Result<Hash> {
        let data = bincode::serialize(edge)?;
        let blob = Blob::new(BlobType::Edge, data);
        self.put(&blob)
    }

    /// Retrieve an edge by hash
    pub fn get_edge(&self, hash: &Hash) -> Result<crate::model::Edge> {
        let blob = self.get(hash)?;
        if blob.blob_type != BlobType::Edge {
            return Err(Error::Corruption(format!(
                "Expected Edge, got {:?}",
                blob.blob_type
            )));
        }
        Ok(bincode::deserialize(&blob.data)?)
    }

    /// Store a commit and return its hash
    pub fn put_commit(&self, commit: &crate::model::Commit) -> Result<Hash> {
        let data = bincode::serialize(commit)?;
        let blob = Blob::new(BlobType::Commit, data);
        self.put(&blob)
    }

    /// Retrieve a commit by hash
    pub fn get_commit(&self, hash: &Hash) -> Result<crate::model::Commit> {
        let blob = self.get(hash)?;
        if blob.blob_type != BlobType::Commit {
            return Err(Error::Corruption(format!(
                "Expected Commit, got {:?}",
                blob.blob_type
            )));
        }
        Ok(bincode::deserialize(&blob.data)?)
    }

    // === Ref Management ===

    /// Get the current HEAD ref name
    pub fn head(&self) -> String {
        self.head.read().clone()
    }

    /// Set HEAD to point to a ref
    pub fn set_head(&self, ref_name: &str) -> Result<()> {
        let refs = self.refs.read();
        if !refs.contains_key(ref_name) {
            return Err(Error::RefNotFound(ref_name.to_string()));
        }
        drop(refs);

        *self.head.write() = ref_name.to_string();
        Ok(())
    }

    /// Get the commit hash for a ref
    pub fn get_ref(&self, ref_name: &str) -> Option<Hash> {
        let refs = self.refs.read();
        refs.get(ref_name).copied()
    }

    /// Set a ref to point to a commit
    pub fn set_ref(&self, ref_name: &str, commit_hash: Hash) {
        let mut refs = self.refs.write();
        refs.insert(ref_name.to_string(), commit_hash);
    }

    /// Get the current HEAD commit hash
    pub fn head_commit(&self) -> Option<Hash> {
        let head = self.head.read();
        let refs = self.refs.read();
        refs.get(head.as_str()).copied().filter(|h| !h.is_zero())
    }

    /// List all refs
    pub fn list_refs(&self) -> Vec<(String, Hash)> {
        let refs = self.refs.read();
        refs.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    /// Create a new branch at the given commit
    pub fn create_branch(&self, name: &str, commit_hash: Hash) -> Result<()> {
        let mut refs = self.refs.write();
        if refs.contains_key(name) {
            return Err(Error::BranchNotFound(format!(
                "Branch '{}' already exists",
                name
            )));
        }
        refs.insert(name.to_string(), commit_hash);
        Ok(())
    }

    /// Delete a branch
    pub fn delete_branch(&self, name: &str) -> Result<()> {
        let head = self.head.read();
        if head.as_str() == name {
            return Err(Error::BranchNotFound(
                "Cannot delete current branch".to_string(),
            ));
        }
        drop(head);

        let mut refs = self.refs.write();
        refs.remove(name)
            .ok_or_else(|| Error::BranchNotFound(name.to_string()))?;
        Ok(())
    }

    /// Get the number of objects in the store
    pub fn object_count(&self) -> usize {
        let index = self.index.read();
        index.entries.len()
    }

    /// Flush changes and write index to disk
    pub fn sync(&self) -> Result<()> {
        let index = self.index.read();
        let refs = self.refs.read();
        let head = self.head.read();
        let write_offset = *self.write_offset.read();
        let mut file = self.file.write();

        // Calculate where refs will be written (after index)
        let index_size = index.entries.len() * 44; // 32 hash + 8 offset + 4 size
        let refs_offset = write_offset + index_size as u64;

        // Update header
        file.seek(SeekFrom::Start(16))?;
        file.write_all(&(index.entries.len() as u64).to_le_bytes())?;
        file.write_all(&write_offset.to_le_bytes())?;
        // Write refs offset at byte 32
        file.seek(SeekFrom::Start(32))?;
        file.write_all(&refs_offset.to_le_bytes())?;
        file.write_all(&(refs.len() as u64).to_le_bytes())?;
        // Write HEAD ref name length and content at byte 48
        let head_bytes = head.as_bytes();
        file.seek(SeekFrom::Start(48))?;
        file.write_all(&(head_bytes.len() as u16).to_le_bytes())?;
        // Head name (up to 14 bytes to fit in header)
        let head_slice = &head_bytes[..head_bytes.len().min(14)];
        file.write_all(head_slice)?;

        // Write index at write_offset
        file.seek(SeekFrom::Start(write_offset))?;

        // Sort by hash for determinism
        let mut entries: Vec<_> = index.entries.iter().collect();
        entries.sort_by_key(|(h, _)| h.as_bytes());

        for (hash, entry) in entries {
            file.write_all(hash.as_bytes())?;
            file.write_all(&entry.offset.to_le_bytes())?;
            file.write_all(&entry.size.to_le_bytes())?;
        }

        // Write refs after index
        // Format: for each ref: name_len (u16) + name + hash (32 bytes)
        let mut ref_list: Vec<_> = refs.iter().collect();
        ref_list.sort_by_key(|(name, _)| *name);

        for (name, hash) in ref_list {
            let name_bytes = name.as_bytes();
            file.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
            file.write_all(name_bytes)?;
            file.write_all(hash.as_bytes())?;
        }

        file.sync_all()?;
        Ok(())
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ObjectStore {
    fn drop(&mut self) {
        // Best-effort sync on drop
        let _ = self.sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Commit, Edge, EdgeType, Thought};
    use tempfile::tempdir;

    #[test]
    fn test_create_and_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");

        // Create
        {
            let store = ObjectStore::create(&path).unwrap();
            assert_eq!(store.object_count(), 0);
        }

        // Reopen
        {
            let store = ObjectStore::open(&path).unwrap();
            assert_eq!(store.object_count(), 0);
        }
    }

    #[test]
    fn test_thought_storage() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();

        let thought = Thought::new("The cat sat on the mat");
        let hash = store.put_thought(&thought).unwrap();

        let retrieved = store.get_thought(&hash).unwrap();
        assert_eq!(thought.content, retrieved.content);
    }

    #[test]
    fn test_edge_storage() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();

        let edge = Edge::new("thought1", "thought2", EdgeType::RELATES_TO);
        let hash = store.put_edge(&edge).unwrap();

        let retrieved = store.get_edge(&hash).unwrap();
        assert_eq!(edge.source.0, retrieved.source.0);
        assert_eq!(edge.target.0, retrieved.target.0);
    }

    #[test]
    fn test_commit_storage() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();

        let tree = Hash::digest(b"tree");
        let commit = Commit::initial(tree, "Initial commit", "test");
        let hash = store.put_commit(&commit).unwrap();

        let retrieved = store.get_commit(&hash).unwrap();
        assert_eq!(commit.message, retrieved.message);
    }

    #[test]
    fn test_deduplication() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();

        let blob = Blob::new(BlobType::Thought, b"duplicate data".to_vec());
        let hash1 = store.put(&blob).unwrap();
        let hash2 = store.put(&blob).unwrap();

        assert_eq!(hash1, hash2);
        assert_eq!(store.object_count(), 1);
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");

        let hash;
        {
            let store = ObjectStore::create(&path).unwrap();
            let thought = Thought::new("Persistent thought");
            hash = store.put_thought(&thought).unwrap();
            store.sync().unwrap();
        }

        {
            let store = ObjectStore::open(&path).unwrap();
            let retrieved = store.get_thought(&hash).unwrap();
            assert_eq!(retrieved.content, "Persistent thought");
        }
    }

    #[test]
    fn test_refs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.indra");
        let store = ObjectStore::create(&path).unwrap();

        let commit_hash = Hash::digest(b"commit");

        // Default HEAD is main
        assert_eq!(store.head(), "main");

        // Set main to point to a commit
        store.set_ref("main", commit_hash);
        assert_eq!(store.get_ref("main"), Some(commit_hash));

        // Create a new branch
        store.create_branch("feature", commit_hash).unwrap();
        assert_eq!(store.get_ref("feature"), Some(commit_hash));

        // Switch HEAD
        store.set_head("feature").unwrap();
        assert_eq!(store.head(), "feature");
    }
}
