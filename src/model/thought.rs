//! Thought (node) type - the fundamental unit of knowledge

use super::Hash;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Wrapper type for storing JSON values that is compatible with bincode.
///
/// bincode doesn't support `serde_json::Value` because it uses `deserialize_any`,
/// so we serialize JSON values to strings for storage.
#[derive(Clone, Debug, PartialEq)]
pub struct JsonValue(pub serde_json::Value);

impl JsonValue {
    pub fn new(value: serde_json::Value) -> Self {
        JsonValue(value)
    }
}

impl From<serde_json::Value> for JsonValue {
    fn from(v: serde_json::Value) -> Self {
        JsonValue(v)
    }
}

impl From<JsonValue> for serde_json::Value {
    fn from(v: JsonValue) -> Self {
        v.0
    }
}

impl Serialize for JsonValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize the JSON value as a string
        let json_string = serde_json::to_string(&self.0).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&json_string)
    }
}

impl<'de> Deserialize<'de> for JsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize as string, then parse as JSON
        let json_string = String::deserialize(deserializer)?;
        let value: serde_json::Value =
            serde_json::from_str(&json_string).map_err(serde::de::Error::custom)?;
        Ok(JsonValue(value))
    }
}

/// Unique identifier for a thought (semantic ID, not content hash)
/// This allows thoughts to evolve while maintaining identity
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThoughtId(pub String);

impl ThoughtId {
    /// Create a new thought ID
    pub fn new(id: impl Into<String>) -> Self {
        ThoughtId(id.into())
    }

    /// Generate a unique thought ID using timestamp + random suffix
    pub fn generate() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Use hash of timestamp for uniqueness
        let hash = Hash::digest(&timestamp.to_le_bytes());
        ThoughtId(hash.short())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ThoughtId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ThoughtId {
    fn from(s: &str) -> Self {
        ThoughtId(s.to_string())
    }
}

impl From<String> for ThoughtId {
    fn from(s: String) -> Self {
        ThoughtId(s)
    }
}

impl From<&ThoughtId> for ThoughtId {
    fn from(id: &ThoughtId) -> Self {
        id.clone()
    }
}

impl From<&String> for ThoughtId {
    fn from(s: &String) -> Self {
        ThoughtId(s.clone())
    }
}

/// A thought - the fundamental unit of knowledge in indra_db
///
/// Thoughts are content-addressed: their hash is derived from their content.
/// The ThoughtId provides stable identity across versions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thought {
    /// Stable identity across versions
    pub id: ThoughtId,

    /// The actual content/text of the thought
    pub content: String,

    /// Optional type classification (e.g., "fact", "hypothesis", "question")
    pub thought_type: Option<String>,

    /// Embedding vector (stored with the thought for content-addressing)
    /// Dimension is configurable at database level
    pub embedding: Option<Vec<f32>>,

    /// Arbitrary metadata (stored as JSON strings for bincode compatibility)
    pub attrs: HashMap<String, JsonValue>,

    /// Creation timestamp (unix millis)
    pub created_at: u64,

    /// Last modified timestamp (unix millis)
    pub modified_at: u64,
}

impl Thought {
    /// Create a new thought with the given content
    pub fn new(content: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Thought {
            id: ThoughtId::generate(),
            content: content.into(),
            thought_type: None,
            embedding: None,
            attrs: HashMap::new(),
            created_at: now,
            modified_at: now,
        }
    }

    /// Create a thought with a specific ID
    pub fn with_id(id: impl Into<ThoughtId>, content: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Thought {
            id: id.into(),
            content: content.into(),
            thought_type: None,
            embedding: None,
            attrs: HashMap::new(),
            created_at: now,
            modified_at: now,
        }
    }

    /// Set the thought type
    pub fn with_type(mut self, thought_type: impl Into<String>) -> Self {
        self.thought_type = Some(thought_type.into());
        self
    }

    /// Set the embedding
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Add a metadata attribute
    pub fn with_attr(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.attrs.insert(key.into(), JsonValue::new(value.into()));
        self
    }

    /// Get a metadata attribute
    pub fn get_attr(&self, key: &str) -> Option<&serde_json::Value> {
        self.attrs.get(key).map(|v| &v.0)
    }

    /// Compute the content hash of this thought
    /// This determines the blob's address in content-addressed storage
    pub fn content_hash(&self) -> Hash {
        // Hash the serialized content - this makes the hash deterministic
        // and includes all fields that define the thought's "content"
        let data = bincode::serialize(self).expect("serialization should not fail");
        Hash::digest(&data)
    }

    /// Update the thought's content (creates a new version)
    pub fn update_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
        self.embedding = None; // Invalidate embedding on content change
        self.modified_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thought_creation() {
        let thought = Thought::new("Hello, world!");
        assert_eq!(thought.content, "Hello, world!");
        assert!(thought.embedding.is_none());
    }

    #[test]
    fn test_thought_content_hash_deterministic() {
        // Same content, same timestamps should produce same hash
        let mut t1 = Thought::with_id("test", "content");
        let mut t2 = Thought::with_id("test", "content");

        // Force same timestamps for determinism
        t1.created_at = 1000;
        t1.modified_at = 1000;
        t2.created_at = 1000;
        t2.modified_at = 1000;

        assert_eq!(t1.content_hash(), t2.content_hash());

        // Different content = different hash
        t2.content = "different".to_string();
        assert_ne!(t1.content_hash(), t2.content_hash());
    }

    #[test]
    fn test_thought_builder() {
        let thought = Thought::new("Test thought")
            .with_type("hypothesis")
            .with_attr("confidence", serde_json::json!(0.8));

        assert_eq!(thought.thought_type, Some("hypothesis".to_string()));
        assert_eq!(
            thought.get_attr("confidence"),
            Some(&serde_json::json!(0.8))
        );
    }
}
