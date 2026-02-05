//! Blob type - the unit of content-addressed storage

use crate::model::Hash;
use serde::{Deserialize, Serialize};

/// Type tag for blobs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlobType {
    /// A thought (node)
    Thought,
    /// An edge (relationship)
    Edge,
    /// A commit
    Commit,
    /// A tree (merkle trie node)
    Tree,
}

impl BlobType {
    pub fn as_byte(&self) -> u8 {
        match self {
            BlobType::Thought => 0,
            BlobType::Edge => 1,
            BlobType::Commit => 2,
            BlobType::Tree => 3,
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(BlobType::Thought),
            1 => Some(BlobType::Edge),
            2 => Some(BlobType::Commit),
            3 => Some(BlobType::Tree),
            _ => None,
        }
    }
}

/// A blob is a typed, compressed chunk of data
#[derive(Clone, Debug)]
pub struct Blob {
    /// Type of content
    pub blob_type: BlobType,
    /// Raw data (uncompressed)
    pub data: Vec<u8>,
}

impl Blob {
    /// Create a new blob
    pub fn new(blob_type: BlobType, data: Vec<u8>) -> Self {
        Blob { blob_type, data }
    }

    /// Compute the content hash
    pub fn hash(&self) -> Hash {
        // Include type in hash for safety
        Hash::digest_many(&[&[self.blob_type.as_byte()], &self.data])
    }

    /// Compress the blob for storage
    pub fn compress(&self) -> crate::Result<Vec<u8>> {
        let mut output = Vec::new();
        // Type byte prefix
        output.push(self.blob_type.as_byte());
        // Compressed data
        let compressed = zstd::encode_all(self.data.as_slice(), 3)?;
        output.extend(compressed);
        Ok(output)
    }

    /// Decompress a blob from storage
    pub fn decompress(data: &[u8]) -> crate::Result<Self> {
        if data.is_empty() {
            return Err(crate::Error::Corruption("Empty blob data".into()));
        }

        let blob_type = BlobType::from_byte(data[0])
            .ok_or_else(|| crate::Error::Corruption(format!("Invalid blob type: {}", data[0])))?;

        let decompressed = zstd::decode_all(&data[1..])?;

        Ok(Blob {
            blob_type,
            data: decompressed,
        })
    }

    /// Get the size of the uncompressed data
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_roundtrip() {
        let original = Blob::new(BlobType::Thought, b"hello world".to_vec());
        let compressed = original.compress().unwrap();
        let restored = Blob::decompress(&compressed).unwrap();

        assert_eq!(original.blob_type, restored.blob_type);
        assert_eq!(original.data, restored.data);
    }

    #[test]
    fn test_blob_hash_includes_type() {
        let thought = Blob::new(BlobType::Thought, b"data".to_vec());
        let edge = Blob::new(BlobType::Edge, b"data".to_vec());

        // Same data, different types → different hashes
        assert_ne!(thought.hash(), edge.hash());
    }
}
