//! HuggingFace local model embedder using Candle
//!
//! This embedder loads models from HuggingFace Hub and runs them locally using Candle.
//! It respects the HF_HOME cache directory and HF_TOKEN environment variable.

use super::Embedder;
use crate::{Error, Result};
use std::path::PathBuf;

#[cfg(feature = "hf-embeddings")]
use {
    candle_core::{Device, Tensor},
    candle_nn::VarBuilder,
    candle_transformers::models::bert::{BertModel, Config, DTYPE},
    hf_hub::{api::tokio::Api, Repo, RepoType},
    std::sync::Arc,
    tokenizers::Tokenizer,
};

/// HuggingFace embedder that runs models locally using Candle
///
/// Supports any BERT-compatible model from HuggingFace Hub.
/// Common models:
/// - `sentence-transformers/all-MiniLM-L6-v2` (384 dim, fast)
/// - `sentence-transformers/all-mpnet-base-v2` (768 dim, higher quality)
/// - `BAAI/bge-small-en-v1.5` (384 dim, good for retrieval)
///
/// Environment variables:
/// - `HF_HOME`: Cache directory for models (default: ~/.cache/huggingface)
/// - `HF_TOKEN`: HuggingFace API token for private models (optional)
pub struct HFEmbedder {
    #[cfg(feature = "hf-embeddings")]
    model: Arc<BertModel>,
    #[cfg(feature = "hf-embeddings")]
    tokenizer: Arc<Tokenizer>,
    #[cfg(feature = "hf-embeddings")]
    device: Device,
    model_name: String,
    dimension: usize,
}

impl HFEmbedder {
    /// Create a new HF embedder with the specified model
    ///
    /// This will download the model if not cached, using HF_HOME for storage.
    /// If HF_TOKEN is set, it will be used for authentication.
    ///
    /// # Example
    /// ```no_run
    /// use indra_db::embedding::HFEmbedder;
    ///
    /// // Will use ~/.cache/huggingface by default
    /// let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2").await?;
    ///
    /// // Or set custom cache location
    /// std::env::set_var("HF_HOME", "/custom/cache/path");
    /// let embedder = HFEmbedder::new("BAAI/bge-small-en-v1.5").await?;
    /// # Ok::<(), indra_db::Error>(())
    /// ```
    #[cfg(feature = "hf-embeddings")]
    pub async fn new(model_name: &str) -> Result<Self> {
        Self::new_with_options(model_name, None).await
    }

    /// Create a new HF embedder with custom cache directory
    ///
    /// If `cache_dir` is None, uses HF_HOME env var or default (~/.cache/huggingface)
    #[cfg(feature = "hf-embeddings")]
    pub async fn new_with_options(model_name: &str, cache_dir: Option<PathBuf>) -> Result<Self> {
        use tokio::runtime::Handle;

        // Ensure we're in a tokio runtime
        let runtime = Handle::try_current()
            .map_err(|_| Error::Embedding("No tokio runtime found".to_string()))?;

        // Set cache directory if provided
        if let Some(dir) = cache_dir {
            std::env::set_var("HF_HOME", dir.to_str().unwrap_or_default());
        }

        // Check if HF_TOKEN is set
        let token = std::env::var("HF_TOKEN").ok();
        if token.is_some() {
            eprintln!("✓ Using HF_TOKEN for authentication");
        }

        // Initialize HF Hub API
        let api = Api::new()
            .map_err(|e| Error::Embedding(format!("Failed to initialize HF Hub API: {}", e)))?;

        let repo = api.repo(Repo::new(model_name.to_string(), RepoType::Model));

        // Download model files
        eprintln!("Downloading model files for {}...", model_name);
        let config_path = repo
            .get("config.json")
            .await
            .map_err(|e| Error::Embedding(format!("Failed to download config.json: {}", e)))?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .await
            .map_err(|e| Error::Embedding(format!("Failed to download tokenizer.json: {}", e)))?;
        let weights_path = repo
            .get("model.safetensors")
            .await
            .or_else(|_| {
                // Fallback to pytorch_model.bin
                runtime.block_on(repo.get("pytorch_model.bin"))
            })
            .map_err(|e| Error::Embedding(format!("Failed to download model weights: {}", e)))?;

        eprintln!("✓ Model files cached locally");

        // Load config
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| Error::Embedding(format!("Failed to read config: {}", e)))?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| Error::Embedding(format!("Failed to parse config: {}", e)))?;

        let dimension = config.hidden_size;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::Embedding(format!("Failed to load tokenizer: {}", e)))?;

        // Setup device (CPU for now, GPU support can be added later)
        let device = Device::Cpu;

        // Load model weights
        let vb = if weights_path.extension().and_then(|s| s.to_str()) == Some("safetensors") {
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device) }
                .map_err(|e| Error::Embedding(format!("Failed to load weights: {}", e)))?
        } else {
            return Err(Error::Embedding(
                "Only safetensors format is supported".to_string(),
            ));
        };

        // Create model
        let model = BertModel::load(vb, &config)
            .map_err(|e| Error::Embedding(format!("Failed to create model: {}", e)))?;

        eprintln!("✓ Model loaded successfully");

        Ok(HFEmbedder {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
            model_name: model_name.to_string(),
            dimension,
        })
    }

    /// Get the cache directory being used
    pub fn cache_dir() -> PathBuf {
        std::env::var("HF_HOME")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".cache").join("huggingface")
            })
    }

    /// Mean pooling implementation
    #[cfg(feature = "hf-embeddings")]
    fn mean_pooling(
        last_hidden_state: &Tensor,
        attention_mask: &Tensor,
    ) -> Result<Tensor> {
        // Expand attention mask to match hidden state dimensions
        let expanded_mask = attention_mask
            .unsqueeze(2)
            .map_err(|e| Error::Embedding(format!("Failed to expand mask: {}", e)))?
            .expand(last_hidden_state.shape())
            .map_err(|e| Error::Embedding(format!("Failed to expand mask shape: {}", e)))?
            .to_dtype(last_hidden_state.dtype())
            .map_err(|e| Error::Embedding(format!("Failed to convert dtype: {}", e)))?;

        // Apply mask and sum
        let masked = (last_hidden_state * &expanded_mask)
            .map_err(|e| Error::Embedding(format!("Failed to apply mask: {}", e)))?;
        let sum_embeddings = masked
            .sum(1)
            .map_err(|e| Error::Embedding(format!("Failed to sum embeddings: {}", e)))?;

        // Sum mask for averaging
        let sum_mask = expanded_mask
            .sum(1)
            .map_err(|e| Error::Embedding(format!("Failed to sum mask: {}", e)))?;

        // Clamp to avoid division by zero
        let sum_mask = sum_mask
            .clamp(1e-9, f32::MAX)
            .map_err(|e| Error::Embedding(format!("Failed to clamp: {}", e)))?;

        // Mean pooling
        let pooled = sum_embeddings
            .broadcast_div(&sum_mask)
            .map_err(|e| Error::Embedding(format!("Failed to divide: {}", e)))?;

        Ok(pooled)
    }

    /// Normalize embeddings to unit length
    #[cfg(feature = "hf-embeddings")]
    fn normalize(tensor: &Tensor) -> Result<Tensor> {
        let norm = tensor
            .sqr()
            .map_err(|e| Error::Embedding(format!("Failed to square: {}", e)))?
            .sum_keepdim(1)
            .map_err(|e| Error::Embedding(format!("Failed to sum: {}", e)))?
            .sqrt()
            .map_err(|e| Error::Embedding(format!("Failed to sqrt: {}", e)))?
            .clamp(1e-12, f32::MAX)
            .map_err(|e| Error::Embedding(format!("Failed to clamp: {}", e)))?;

        tensor
            .broadcast_div(&norm)
            .map_err(|e| Error::Embedding(format!("Failed to normalize: {}", e)))
    }
}

#[cfg(not(feature = "hf-embeddings"))]
impl HFEmbedder {
    pub async fn new(_model_name: &str) -> Result<Self> {
        Err(Error::Embedding(
            "HF embeddings feature not enabled. Compile with --features hf-embeddings".to_string(),
        ))
    }

    pub async fn new_with_options(_model_name: &str, _cache_dir: Option<PathBuf>) -> Result<Self> {
        Err(Error::Embedding(
            "HF embeddings feature not enabled. Compile with --features hf-embeddings".to_string(),
        ))
    }

    pub fn cache_dir() -> PathBuf {
        PathBuf::from(".")
    }
}

impl Embedder for HFEmbedder {
    fn dimension(&self) -> usize {
        self.dimension
    }

    #[cfg(feature = "hf-embeddings")]
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Tokenize
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| Error::Embedding(format!("Tokenization failed: {}", e)))?;

        let token_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        // Convert to tensors
        let token_ids = Tensor::new(token_ids, &self.device)
            .map_err(|e| Error::Embedding(format!("Failed to create token tensor: {}", e)))?
            .unsqueeze(0)
            .map_err(|e| Error::Embedding(format!("Failed to unsqueeze tokens: {}", e)))?;

        let attention_mask = Tensor::new(attention_mask, &self.device)
            .map_err(|e| Error::Embedding(format!("Failed to create mask tensor: {}", e)))?
            .unsqueeze(0)
            .map_err(|e| Error::Embedding(format!("Failed to unsqueeze mask: {}", e)))?;

        // Run model (forward takes token_ids, attention_mask, and optional token_type_ids)
        let outputs = self
            .model
            .forward(&token_ids, &attention_mask, None)
            .map_err(|e| Error::Embedding(format!("Model forward failed: {}", e)))?;

        // Mean pooling
        let pooled = Self::mean_pooling(&outputs, &attention_mask)?;

        // Normalize
        let normalized = Self::normalize(&pooled)?;

        // Convert to Vec<f32>
        let embedding = normalized
            .squeeze(0)
            .map_err(|e| Error::Embedding(format!("Failed to squeeze: {}", e)))?
            .to_vec1::<f32>()
            .map_err(|e| Error::Embedding(format!("Failed to convert to vec: {}", e)))?;

        Ok(embedding)
    }

    #[cfg(not(feature = "hf-embeddings"))]
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(Error::Embedding(
            "HF embeddings feature not enabled".to_string(),
        ))
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

#[cfg(all(test, feature = "hf-embeddings"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hf_embedder_creation() {
        // This test requires network access and will download the model
        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap();

        assert_eq!(embedder.dimension(), 384);
        assert_eq!(embedder.model_name(), "sentence-transformers/all-MiniLM-L6-v2");
    }

    #[tokio::test]
    async fn test_hf_embedder_embed() {
        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap();

        let embedding = embedder.embed("Hello, world!").unwrap();
        assert_eq!(embedding.len(), 384);

        // Check normalized
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[tokio::test]
    async fn test_hf_embedder_deterministic() {
        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap();

        let e1 = embedder.embed("test text").unwrap();
        let e2 = embedder.embed("test text").unwrap();

        // Should be identical
        for (a, b) in e1.iter().zip(e2.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[tokio::test]
    async fn test_hf_cache_dir() {
        std::env::set_var("HF_HOME", "/custom/cache");
        assert_eq!(HFEmbedder::cache_dir(), PathBuf::from("/custom/cache"));
        std::env::remove_var("HF_HOME");
    }
}
