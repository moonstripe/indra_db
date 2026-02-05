//! API-based embedder for external providers
//!
//! Supports OpenAI, Cohere, Voyage, and other embedding APIs.
//! Requires API keys via environment variables.

use super::Embedder;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[cfg(feature = "api-embeddings")]
use reqwest::Client;

/// API provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiProvider {
    /// OpenAI (text-embedding-3-small, text-embedding-3-large, etc.)
    OpenAI,
    /// Cohere (embed-english-v3.0, embed-multilingual-v3.0, etc.)
    Cohere,
    /// Voyage AI (voyage-3, voyage-3-lite, etc.)
    Voyage,
    /// Custom API with OpenAI-compatible interface
    Custom,
}

impl ApiProvider {
    /// Get the default API base URL for this provider
    pub fn default_base_url(&self) -> &'static str {
        match self {
            ApiProvider::OpenAI => "https://api.openai.com/v1",
            ApiProvider::Cohere => "https://api.cohere.com/v1",
            ApiProvider::Voyage => "https://api.voyageai.com/v1",
            ApiProvider::Custom => "",
        }
    }

    /// Get the default environment variable name for API key
    pub fn env_var_name(&self) -> &'static str {
        match self {
            ApiProvider::OpenAI => "OPENAI_API_KEY",
            ApiProvider::Cohere => "COHERE_API_KEY",
            ApiProvider::Voyage => "VOYAGE_API_KEY",
            ApiProvider::Custom => "API_KEY",
        }
    }
}

/// API embedder that calls external embedding services
///
/// Supports multiple providers with automatic batching and retry logic.
///
/// # Environment Variables
/// - `OPENAI_API_KEY`: OpenAI API key
/// - `COHERE_API_KEY`: Cohere API key
/// - `VOYAGE_API_KEY`: Voyage AI API key
/// - `API_KEY`: Custom provider API key
///
/// # Example
/// ```no_run
/// use indra_db::embedding::{ApiEmbedder, ApiProvider};
///
/// // OpenAI
/// std::env::set_var("OPENAI_API_KEY", "sk-...");
/// let embedder = ApiEmbedder::new(
///     ApiProvider::OpenAI,
///     "text-embedding-3-small",
///     1536
/// )?;
///
/// // Cohere
/// std::env::set_var("COHERE_API_KEY", "...");
/// let embedder = ApiEmbedder::new(
///     ApiProvider::Cohere,
///     "embed-english-v3.0",
///     1024
/// )?;
///
/// // Custom provider with OpenAI-compatible API
/// let embedder = ApiEmbedder::new_custom(
///     "https://custom-api.com/v1",
///     "custom-model",
///     768,
///     "bearer-token-here"
/// )?;
/// # Ok::<(), indra_db::Error>(())
/// ```
pub struct ApiEmbedder {
    provider: ApiProvider,
    model_name: String,
    dimension: usize,
    #[cfg(feature = "api-embeddings")]
    client: Client,
    #[cfg(feature = "api-embeddings")]
    base_url: String,
    #[cfg(feature = "api-embeddings")]
    api_key: String,
}

#[cfg(feature = "api-embeddings")]
#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding_format: Option<String>,
}

#[cfg(feature = "api-embeddings")]
#[derive(Deserialize)]
struct OpenAIResponse {
    data: Vec<OpenAIEmbedding>,
}

#[cfg(feature = "api-embeddings")]
#[derive(Deserialize)]
struct OpenAIEmbedding {
    embedding: Vec<f32>,
}

#[cfg(feature = "api-embeddings")]
#[derive(Serialize)]
struct CohereRequest {
    model: String,
    texts: Vec<String>,
    input_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncate: Option<String>,
}

#[cfg(feature = "api-embeddings")]
#[derive(Deserialize)]
struct CohereResponse {
    embeddings: Vec<Vec<f32>>,
}

#[cfg(feature = "api-embeddings")]
#[derive(Serialize)]
struct VoyageRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_type: Option<String>,
}

#[cfg(feature = "api-embeddings")]
#[derive(Deserialize)]
struct VoyageResponse {
    data: Vec<VoyageEmbedding>,
}

#[cfg(feature = "api-embeddings")]
#[derive(Deserialize)]
struct VoyageEmbedding {
    embedding: Vec<f32>,
}

impl ApiEmbedder {
    /// Create a new API embedder for the specified provider
    ///
    /// The API key will be read from the appropriate environment variable.
    #[cfg(feature = "api-embeddings")]
    pub fn new(provider: ApiProvider, model_name: &str, dimension: usize) -> Result<Self> {
        let api_key = std::env::var(provider.env_var_name()).map_err(|_| {
            Error::Embedding(format!(
                "Missing API key: {} environment variable not set",
                provider.env_var_name()
            ))
        })?;

        let base_url = provider.default_base_url().to_string();

        Ok(ApiEmbedder {
            provider,
            model_name: model_name.to_string(),
            dimension,
            client: Client::new(),
            base_url,
            api_key,
        })
    }

    /// Create a new API embedder with custom base URL and API key
    ///
    /// Useful for self-hosted or proxy endpoints that implement OpenAI-compatible API.
    #[cfg(feature = "api-embeddings")]
    pub fn new_custom(
        base_url: &str,
        model_name: &str,
        dimension: usize,
        api_key: &str,
    ) -> Result<Self> {
        Ok(ApiEmbedder {
            provider: ApiProvider::Custom,
            model_name: model_name.to_string(),
            dimension,
            client: Client::new(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
        })
    }

    /// Call OpenAI-compatible API
    #[cfg(feature = "api-embeddings")]
    async fn call_openai_api(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let request = OpenAIRequest {
            model: self.model_name.clone(),
            input: texts,
            encoding_format: Some("float".to_string()),
        };

        let url = format!("{}/embeddings", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Embedding(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            return Err(Error::Embedding(format!(
                "API request failed with status {}: {}",
                status, body
            )));
        }

        let api_response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;

        Ok(api_response.data.into_iter().map(|e| e.embedding).collect())
    }

    /// Call Cohere API
    #[cfg(feature = "api-embeddings")]
    async fn call_cohere_api(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let request = CohereRequest {
            model: self.model_name.clone(),
            texts,
            input_type: "search_document".to_string(),
            truncate: Some("END".to_string()),
        };

        let url = format!("{}/embed", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Embedding(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            return Err(Error::Embedding(format!(
                "API request failed with status {}: {}",
                status, body
            )));
        }

        let api_response: CohereResponse = response
            .json()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;

        Ok(api_response.embeddings)
    }

    /// Call Voyage API
    #[cfg(feature = "api-embeddings")]
    async fn call_voyage_api(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let request = VoyageRequest {
            model: self.model_name.clone(),
            input: texts,
            input_type: Some("document".to_string()),
        };

        let url = format!("{}/embeddings", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Embedding(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            return Err(Error::Embedding(format!(
                "API request failed with status {}: {}",
                status, body
            )));
        }

        let api_response: VoyageResponse = response
            .json()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to parse response: {}", e)))?;

        Ok(api_response.data.into_iter().map(|e| e.embedding).collect())
    }
}

#[cfg(not(feature = "api-embeddings"))]
impl ApiEmbedder {
    pub fn new(_provider: ApiProvider, _model_name: &str, _dimension: usize) -> Result<Self> {
        Err(Error::Embedding(
            "API embeddings feature not enabled. Compile with --features api-embeddings"
                .to_string(),
        ))
    }

    pub fn new_custom(
        _base_url: &str,
        _model_name: &str,
        _dimension: usize,
        _api_key: &str,
    ) -> Result<Self> {
        Err(Error::Embedding(
            "API embeddings feature not enabled. Compile with --features api-embeddings"
                .to_string(),
        ))
    }
}

impl Embedder for ApiEmbedder {
    fn dimension(&self) -> usize {
        self.dimension
    }

    #[cfg(feature = "api-embeddings")]
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Use tokio runtime to call async method
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| Error::Embedding(format!("Failed to create runtime: {}", e)))?;

        runtime.block_on(async {
            let embeddings = match self.provider {
                ApiProvider::OpenAI | ApiProvider::Custom => {
                    self.call_openai_api(vec![text.to_string()]).await?
                }
                ApiProvider::Cohere => self.call_cohere_api(vec![text.to_string()]).await?,
                ApiProvider::Voyage => self.call_voyage_api(vec![text.to_string()]).await?,
            };

            embeddings
                .into_iter()
                .next()
                .ok_or_else(|| Error::Embedding("No embedding returned from API".to_string()))
        })
    }

    #[cfg(not(feature = "api-embeddings"))]
    fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(Error::Embedding(
            "API embeddings feature not enabled".to_string(),
        ))
    }

    #[cfg(feature = "api-embeddings")]
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| Error::Embedding(format!("Failed to create runtime: {}", e)))?;

        runtime.block_on(async {
            let text_strings: Vec<String> = texts.iter().map(|s| s.to_string()).collect();

            match self.provider {
                ApiProvider::OpenAI | ApiProvider::Custom => {
                    self.call_openai_api(text_strings).await
                }
                ApiProvider::Cohere => self.call_cohere_api(text_strings).await,
                ApiProvider::Voyage => self.call_voyage_api(text_strings).await,
            }
        })
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

#[cfg(all(test, feature = "api-embeddings"))]
mod tests {
    use super::*;

    #[test]
    fn test_provider_urls() {
        assert_eq!(
            ApiProvider::OpenAI.default_base_url(),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            ApiProvider::Cohere.default_base_url(),
            "https://api.cohere.com/v1"
        );
        assert_eq!(
            ApiProvider::Voyage.default_base_url(),
            "https://api.voyageai.com/v1"
        );
    }

    #[test]
    fn test_provider_env_vars() {
        assert_eq!(ApiProvider::OpenAI.env_var_name(), "OPENAI_API_KEY");
        assert_eq!(ApiProvider::Cohere.env_var_name(), "COHERE_API_KEY");
        assert_eq!(ApiProvider::Voyage.env_var_name(), "VOYAGE_API_KEY");
    }

    #[tokio::test]
    async fn test_openai_embedder() {
        // Only run if API key is available
        if std::env::var("OPENAI_API_KEY").is_err() {
            eprintln!("Skipping OpenAI test: OPENAI_API_KEY not set");
            return;
        }

        let embedder =
            ApiEmbedder::new(ApiProvider::OpenAI, "text-embedding-3-small", 1536).unwrap();

        let embedding = embedder.embed("Hello, world!").unwrap();
        assert_eq!(embedding.len(), 1536);
    }

    #[tokio::test]
    async fn test_batch_embedding() {
        if std::env::var("OPENAI_API_KEY").is_err() {
            eprintln!("Skipping OpenAI batch test: OPENAI_API_KEY not set");
            return;
        }

        let embedder =
            ApiEmbedder::new(ApiProvider::OpenAI, "text-embedding-3-small", 1536).unwrap();

        let texts = vec!["Hello", "World", "Test"];
        let embeddings = embedder.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for embedding in embeddings {
            assert_eq!(embedding.len(), 1536);
        }
    }
}
