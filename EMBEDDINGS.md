# Embedding Providers

Indra DB supports multiple embedding backends through the `Embedder` trait. This allows you to choose the best option for your use case.

## Available Embedders

### 1. MockEmbedder (Default)

**Always available** - No features required

A deterministic embedder that generates embeddings based on text hash using BLAKE3. Useful for:
- Testing and development
- When you don't need semantic similarity
- Reproducible results without external dependencies

```rust
use indra_db::embedding::MockEmbedder;

let embedder = MockEmbedder::new(384);
let embedding = embedder.embed("hello world")?;
```

**Pros:**
- Zero dependencies
- Deterministic (same text → same embedding)
- Fast
- No network/disk I/O

**Cons:**
- No semantic understanding
- Not useful for similarity search based on meaning

---

### 2. HFEmbedder - Local HuggingFace Models

**Feature:** `hf-embeddings`

Runs transformer models locally using [Candle](https://github.com/huggingface/candle). Models are downloaded from HuggingFace Hub and cached locally.

#### Setup

```bash
# Enable the feature
cargo build --features hf-embeddings

# Optional: Set cache directory (defaults to ~/.cache/huggingface)
export HF_HOME=/path/to/cache

# Optional: Set API token for private models
export HF_TOKEN=hf_xxxxxxxxxxxxx
```

#### Usage

```rust
use indra_db::embedding::HFEmbedder;

// Download and cache model (only once)
let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2").await?;

// Generate embeddings
let embedding = embedder.embed("This is a test")?;
```

#### Recommended Models

| Model | Dimension | Speed | Quality | Use Case |
|-------|-----------|-------|---------|----------|
| `sentence-transformers/all-MiniLM-L6-v2` | 384 | ⚡⚡⚡ | ⭐⭐⭐ | General purpose, fast |
| `sentence-transformers/all-mpnet-base-v2` | 768 | ⚡⚡ | ⭐⭐⭐⭐ | Higher quality, slower |
| `BAAI/bge-small-en-v1.5` | 384 | ⚡⚡⚡ | ⭐⭐⭐⭐ | Retrieval optimized |
| `BAAI/bge-base-en-v1.5` | 768 | ⚡⚡ | ⭐⭐⭐⭐⭐ | Best quality |

#### Custom Cache Directory

```rust
use std::path::PathBuf;
use indra_db::embedding::HFEmbedder;

let embedder = HFEmbedder::new_with_options(
    "sentence-transformers/all-MiniLM-L6-v2",
    Some(PathBuf::from("/custom/cache"))
).await?;
```

#### Pros:
- 🔒 **Privacy**: Everything runs locally
- 🚀 **Fast**: No network latency after download
- 💰 **Free**: No API costs
- 🎯 **Quality**: State-of-the-art models

#### Cons:
- 💾 **Storage**: Models are 100MB-500MB each
- 🐌 **First run**: Download takes time
- 🖥️ **CPU-bound**: Currently CPU-only (GPU support planned)

---

### 3. ApiEmbedder - External API Providers

**Feature:** `api-embeddings`

Call external embedding APIs like OpenAI, Cohere, or Voyage. Best for production applications where you want:
- Latest models without local updates
- GPU-accelerated inference
- No local compute overhead

#### Setup

```bash
# Enable the feature
cargo build --features api-embeddings

# Set API keys
export OPENAI_API_KEY=sk-...
export COHERE_API_KEY=...
export VOYAGE_API_KEY=...
```

#### Supported Providers

##### OpenAI

```rust
use indra_db::embedding::{ApiEmbedder, ApiProvider};

let embedder = ApiEmbedder::new(
    ApiProvider::OpenAI,
    "text-embedding-3-small",
    1536
)?;

let embedding = embedder.embed("Hello, world!")?;
```

**Models:**
- `text-embedding-3-small` (1536 dim, $0.02/1M tokens)
- `text-embedding-3-large` (3072 dim, $0.13/1M tokens)
- `text-embedding-ada-002` (1536 dim, legacy)

##### Cohere

```rust
let embedder = ApiEmbedder::new(
    ApiProvider::Cohere,
    "embed-english-v3.0",
    1024
)?;
```

**Models:**
- `embed-english-v3.0` (1024 dim)
- `embed-multilingual-v3.0` (1024 dim)
- `embed-english-light-v3.0` (384 dim, faster/cheaper)

##### Voyage AI

```rust
let embedder = ApiEmbedder::new(
    ApiProvider::Voyage,
    "voyage-3",
    1024
)?;
```

**Models:**
- `voyage-3` (1024 dim)
- `voyage-3-lite` (512 dim)
- `voyage-code-3` (1024 dim, optimized for code)

##### Custom OpenAI-Compatible API

For self-hosted or proxy endpoints:

```rust
let embedder = ApiEmbedder::new_custom(
    "https://api.example.com/v1",
    "custom-model-name",
    768,
    "your-api-key"
)?;
```

#### Batch Operations

API providers support efficient batching:

```rust
let texts = vec!["first text", "second text", "third text"];
let embeddings = embedder.embed_batch(&texts)?;
// Single API call instead of 3!
```

#### Pros:
- 🚀 **Fast setup**: No model downloads
- 🎯 **Latest models**: Always up-to-date
- 💪 **Powerful**: GPU-accelerated
- 📦 **Small binary**: No model weights in build

#### Cons:
- 💰 **Cost**: Pay per token
- 🌐 **Network required**: No offline operation
- 🔓 **Privacy**: Data sent to third party
- ⏱️ **Latency**: Network round-trip time

---

## Comparison Table

| Feature | MockEmbedder | HFEmbedder | ApiEmbedder |
|---------|--------------|------------|-------------|
| **Setup** | ✅ Zero | ⚠️ Download models | ✅ API key only |
| **Runtime** | ✅ Instant | ⚡ Fast (CPU) | ⏱️ Network latency |
| **Cost** | 💚 Free | 💚 Free | 💰 Pay per token |
| **Privacy** | 🔒 Local | 🔒 Local | 🌐 Cloud |
| **Offline** | ✅ Yes | ✅ Yes | ❌ No |
| **Quality** | ❌ No semantics | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **Binary size** | 📦 Small | 📦 Small | 📦 Small |
| **Storage** | 💾 None | 💾 100-500MB per model | 💾 None |

---

## Integration Examples

### Using in Database

```rust
use indra_db::{Database, embedding::MockEmbedder};

// With MockEmbedder (default)
let db = Database::open_with_embedder("thoughts.indra", MockEmbedder::default())?;

// With HFEmbedder
#[cfg(feature = "hf-embeddings")]
{
    use indra_db::embedding::HFEmbedder;
    let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2").await?;
    let db = Database::open_with_embedder("thoughts.indra", embedder)?;
}

// With ApiEmbedder
#[cfg(feature = "api-embeddings")]
{
    use indra_db::embedding::{ApiEmbedder, ApiProvider};
    let embedder = ApiEmbedder::new(
        ApiProvider::OpenAI,
        "text-embedding-3-small",
        1536
    )?;
    let db = Database::open_with_embedder("thoughts.indra", embedder)?;
}
```

### Custom Embedder

Implement the `Embedder` trait for your own backend:

```rust
use indra_db::embedding::Embedder;
use indra_db::Result;

struct MyEmbedder {
    dimension: usize,
}

impl Embedder for MyEmbedder {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Your implementation here
        todo!()
    }

    fn model_name(&self) -> &str {
        "my-custom-embedder"
    }
}
```

---

## Choosing the Right Embedder

### Development & Testing
→ **MockEmbedder** - Fast, deterministic, zero setup

### Personal Projects (Local)
→ **HFEmbedder** with `all-MiniLM-L6-v2` - Good balance of quality and speed

### Production (High Volume)
→ **HFEmbedder** with `bge-base-en-v1.5` - Best quality, no per-token costs

### Production (Low Volume, Latest Models)
→ **ApiEmbedder** with OpenAI or Cohere - Minimal setup, always updated

### Production (Privacy Critical)
→ **HFEmbedder** - Everything stays local

### Code Embeddings
→ **ApiEmbedder** with Voyage `voyage-code-3` - Specialized for code

---

## Performance Tips

### HFEmbedder

1. **Reuse the embedder instance** - Model loading is expensive
2. **Batch when possible** - Use `embed_batch()` for multiple texts
3. **Choose dimension wisely** - Higher ≠ always better
4. **Cache directory on SSD** - Faster model loading

### ApiEmbedder

1. **Always use batching** - Reduces API calls and cost
2. **Implement retry logic** - Handle rate limits gracefully
3. **Consider response time** - Network latency adds up
4. **Monitor costs** - Track token usage

---

## Troubleshooting

### HFEmbedder: Model download fails

```bash
# Check cache directory
echo $HF_HOME

# Verify token (for private models)
echo $HF_TOKEN

# Try manual download
curl -H "Authorization: Bearer $HF_TOKEN" \
  https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/model.safetensors
```

### ApiEmbedder: Authentication errors

```bash
# Verify API key is set
echo $OPENAI_API_KEY

# Test with curl
curl https://api.openai.com/v1/embeddings \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"text-embedding-3-small","input":"test"}'
```

### "Feature not enabled" errors

```bash
# Check enabled features
cargo build --features hf-embeddings,api-embeddings

# Or enable both
cargo build --all-features
```

---

## Future Embedders

Planned for future releases:

- 🎯 **ONNX Runtime** - Cross-platform optimized models
- 🚀 **GPU Support** - CUDA/Metal acceleration for HFEmbedder
- 🌐 **More providers** - Azure, AWS Bedrock, Google Vertex
- 🔢 **Quantized models** - Smaller, faster local models
- 🐍 **Python bindings** - Use any Python embedding library
