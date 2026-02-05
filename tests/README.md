# HFEmbedder Integration Tests

This directory contains integration tests for the HuggingFace embedder that are designed to:
1. **Skip automatically in CI** (no network downloads, no timeouts)
2. **Run locally with cached models** (fast, no downloads)
3. **Support explicit model downloads** (when requested)

## Test Categories

### 1. Utility Tests (Always Run)

```bash
cargo test --features hf-embeddings test_cache_dir_detection
cargo test --features hf-embeddings test_list_cached_models -- --nocapture
```

These tests don't require models and always run:
- `test_cache_dir_detection` - Verifies HF_HOME env var handling
- `test_list_cached_models` - Shows what models you have cached locally

### 2. Local Tests (Run with Cached Models)

```bash
cargo test --features hf-embeddings -- --ignored --nocapture
```

These tests automatically skip if:
- Running in CI (`CI` or `GITHUB_ACTIONS` env var is set)
- Model is not in local cache

Tests:
- `test_local_minilm_basic` - Basic embedding test
- `test_local_minilm_semantic_similarity` - Semantic understanding
- `test_local_minilm_deterministic` - Reproducibility
- `test_local_minilm_batch` - Batch embedding
- `test_local_mpnet_if_cached` - MPNet model (if cached)
- `test_local_bge_if_cached` - BGE model (if cached)

Example output:
```
⏭️  Skipping: sentence-transformers/all-MiniLM-L6-v2 not in cache
✓ Found cached model: sentence-transformers/all-MiniLM-L6-v2
✓ Basic embedding test passed
✓ Semantic similarity test passed
```

### 3. Download Tests (Explicit Only)

```bash
# Download a specific model (only run one at a time to avoid rate limits)
cargo test --features hf-embeddings test_download_minilm -- --ignored --nocapture --test-threads=1

# Download MPNet (larger model, ~400MB)
cargo test --features hf-embeddings test_download_mpnet -- --ignored --nocapture --test-threads=1

# Download BGE (~130MB)
cargo test --features hf-embeddings test_download_bge_small -- --ignored --nocapture --test-threads=1
```

These tests will:
- Download the model from HuggingFace Hub (first time only)
- Cache it in `~/.cache/huggingface` (or `$HF_HOME`)
- Test embedding functionality
- Skip automatically in CI

## Supported Models

The tests use BERT-compatible sentence-transformer models:

| Model | Dimension | Size | Speed | Quality |
|-------|-----------|------|-------|---------|
| `sentence-transformers/all-MiniLM-L6-v2` | 384 | ~90MB | ⚡⚡⚡ | ⭐⭐⭐ |
| `sentence-transformers/all-mpnet-base-v2` | 768 | ~400MB | ⚡⚡ | ⭐⭐⭐⭐ |
| `BAAI/bge-small-en-v1.5` | 384 | ~130MB | ⚡⚡⚡ | ⭐⭐⭐⭐ |

## Why Not EmbeddingGemma?

You have `google/embeddinggemma-300m` in your cache, but it's incomplete and uses a different architecture:

- **EmbeddingGemma** uses Gemma architecture (not BERT-compatible)
- Requires different model loading code
- Your cached version appears incomplete (no model weights)

Supporting EmbeddingGemma would require:
1. Implementing Gemma model support in Candle
2. Different tokenizer handling
3. Different pooling strategy

For now, we focus on sentence-transformers models which:
- Work out of the box with Candle's BERT implementation
- Are well-documented and widely used
- Have consistent APIs across models

## CI Behavior

In GitHub Actions:
- **Unit tests run** (no `#[ignore]` attribute)
- **Integration tests are ignored** (marked with `#[ignore]`)
- **All HF tests detect CI and skip** (via `is_ci()` check)

This ensures:
- ✅ Fast CI runs (<2 minutes)
- ✅ No network downloads
- ✅ No HF_TOKEN required
- ✅ No timeout issues

## Environment Variables

- `HF_HOME` - Cache directory (default: `~/.cache/huggingface`)
- `HF_TOKEN` - HuggingFace API token (optional, for private models)
- `CI` or `GITHUB_ACTIONS` - Automatically set by CI systems

## Example Workflow

```bash
# 1. Check what you have cached
cargo test --features hf-embeddings test_list_cached_models -- --nocapture

# Output:
# 📦 Cached models in "/Users/you/.cache/huggingface/hub":
#     ⚠️ google/embeddinggemma-300m (incomplete)
#     ✓ microsoft/trocr-base-handwritten

# 2. Download a model
cargo test --features hf-embeddings test_download_minilm -- --ignored --nocapture --test-threads=1

# Output:
# 📥 Downloading sentence-transformers/all-MiniLM-L6-v2...
#     (This may take a few minutes on first run)
# ✓ Model downloaded and working!
#     Cached at: "/Users/you/.cache/huggingface"

# 3. Run all tests with cached models
cargo test --features hf-embeddings -- --ignored --nocapture

# Output:
# ✓ Found cached model: sentence-transformers/all-MiniLM-L6-v2
# ✓ Basic embedding test passed
# ✓ Semantic similarity test passed
# Similar sentences similarity: 0.847
# Different sentences similarity: 0.123
# ✓ Determinism test passed
# ✓ Batch embedding test passed
```

## Adding New Model Tests

To add support for a new model:

```rust
#[tokio::test]
#[ignore]
async fn test_local_your_model() {
    if is_ci() || !model_cached("your-org/your-model") {
        eprintln!("⏭️  Skipping: your-org/your-model not in cache");
        return;
    }

    let embedder = HFEmbedder::new("your-org/your-model")
        .await
        .expect("Failed to load model");

    let embedding = embedder.embed("test text").unwrap();
    assert_eq!(embedding.len(), YOUR_EXPECTED_DIM);

    println!("✓ Your model test passed");
}
```

## Troubleshooting

### "relative URL without a base" error
This happens when `hf-hub` API has issues. The tests are marked `#[ignore]` so they won't break CI. Try:
```bash
# Check your HF token
echo $HF_TOKEN

# Try downloading manually
huggingface-cli download sentence-transformers/all-MiniLM-L6-v2
```

### Tests always skip
Make sure:
1. You're using `--ignored` flag
2. Model is actually cached (check with `test_list_cached_models`)
3. Model files are complete (has `model.safetensors` or `pytorch_model.bin`)

### Slow tests
Download tests can take 1-5 minutes depending on model size and network speed. Use:
```bash
--test-threads=1  # Run one at a time
--nocapture       # See progress messages
```
