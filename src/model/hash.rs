//! Content-addressed hash type using BLAKE3

use serde::{Deserialize, Serialize};
use std::fmt;

/// A 32-byte BLAKE3 hash used for content addressing
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    /// The zero hash (used as a sentinel/null value)
    pub const ZERO: Hash = Hash([0u8; 32]);

    /// Create a hash from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Hash(bytes)
    }

    /// Hash arbitrary data
    pub fn digest(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Hash(*hash.as_bytes())
    }

    /// Hash multiple pieces of data
    pub fn digest_many(parts: &[&[u8]]) -> Self {
        let mut hasher = blake3::Hasher::new();
        for part in parts {
            hasher.update(part);
        }
        Hash(*hasher.finalize().as_bytes())
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Hash(arr))
    }

    /// Get a short prefix for display (first 7 chars, like git)
    pub fn short(&self) -> String {
        self.to_hex()[..7].to_string()
    }

    /// Check if this is the zero hash
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.short())
    }
}

impl Default for Hash {
    fn default() -> Self {
        Hash::ZERO
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_digest() {
        let h1 = Hash::digest(b"hello");
        let h2 = Hash::digest(b"hello");
        let h3 = Hash::digest(b"world");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_hash_hex_roundtrip() {
        let h1 = Hash::digest(b"test data");
        let hex = h1.to_hex();
        let h2 = Hash::from_hex(&hex).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_short() {
        let h = Hash::digest(b"test");
        assert_eq!(h.short().len(), 7);
    }
}
