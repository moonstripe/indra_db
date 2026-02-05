//! Integration tests for HFEmbedder
//!
//! These tests are designed to run locally when you have models cached,
//! but are automatically skipped in CI environments.
//!
//! Run all tests (downloads model if needed):
//! ```bash
//! cargo test --features hf-embeddings -- --ignored --test-threads=1
//! ```
//!
//! Run only cached model tests:
//! ```bash
//! cargo test --features hf-embeddings test_local -- --ignored
//! ```
//!
//! The tests use sentence-transformers models which are BERT-compatible.
//! Note: embeddinggemma uses Gemma architecture and would require separate implementation.

#[cfg(feature = "hf-embeddings")]
mod hf_integration_tests {
    use indra_db::embedding::{cosine_similarity, Embedder, HFEmbedder};
    use std::path::PathBuf;

    /// Check if we're in CI (skip downloads)
    fn is_ci() -> bool {
        std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok()
    }

    /// Check if model exists in local cache
    fn model_cached(model_name: &str) -> bool {
        let cache_dir = HFEmbedder::cache_dir();
        let model_dir = format!("models--{}", model_name.replace('/', "--"));
        let model_path = cache_dir.join("hub").join(&model_dir).join("snapshots");

        if !model_path.exists() {
            return false;
        }

        // Check for actual model files in any snapshot
        if let Ok(entries) = std::fs::read_dir(model_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("model.safetensors").exists()
                    || path.join("pytorch_model.bin").exists()
                {
                    return true;
                }
            }
        }

        false
    }

    // ============================================================================
    // Tests that run locally with cached models
    // ============================================================================

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_local_minilm_basic() {
        if is_ci() || !model_cached("sentence-transformers/all-MiniLM-L6-v2") {
            eprintln!("⏭️  Skipping: sentence-transformers/all-MiniLM-L6-v2 not in cache");
            return;
        }

        println!("✓ Found cached model: sentence-transformers/all-MiniLM-L6-v2");

        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model");

        assert_eq!(embedder.dimension(), 384);
        assert_eq!(
            embedder.model_name(),
            "sentence-transformers/all-MiniLM-L6-v2"
        );

        // Test embedding
        let embedding = embedder.embed("Hello world").expect("Failed to embed");
        assert_eq!(embedding.len(), 384);

        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "Embedding not normalized");

        println!("✓ Basic embedding test passed");
    }

    #[tokio::test]
    #[ignore]
    async fn test_local_minilm_semantic_similarity() {
        if is_ci() || !model_cached("sentence-transformers/all-MiniLM-L6-v2") {
            eprintln!("⏭️  Skipping semantic similarity test");
            return;
        }

        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to load model");

        // Test semantic understanding
        let e1 = embedder.embed("The cat sits on the mat").unwrap();
        let e2 = embedder.embed("A cat is sitting on a mat").unwrap();
        let e3 = embedder.embed("Python is a programming language").unwrap();

        let sim_similar = cosine_similarity(&e1, &e2);
        let sim_different = cosine_similarity(&e1, &e3);

        println!("Similar sentences similarity: {:.3}", sim_similar);
        println!("Different sentences similarity: {:.3}", sim_different);

        assert!(
            sim_similar > sim_different,
            "Similar sentences should have higher similarity"
        );
        assert!(sim_similar > 0.7, "Similar sentences should be > 0.7");

        println!("✓ Semantic similarity test passed");
    }

    #[tokio::test]
    #[ignore]
    async fn test_local_minilm_deterministic() {
        if is_ci() || !model_cached("sentence-transformers/all-MiniLM-L6-v2") {
            eprintln!("⏭️  Skipping deterministic test");
            return;
        }

        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap();

        let text = "This is a test sentence for determinism";
        let e1 = embedder.embed(text).unwrap();
        let e2 = embedder.embed(text).unwrap();

        // Embeddings should be exactly identical
        for (a, b) in e1.iter().zip(e2.iter()) {
            assert_eq!(a, b, "Embeddings are not deterministic");
        }

        println!("✓ Determinism test passed");
    }

    #[tokio::test]
    #[ignore]
    async fn test_local_minilm_batch() {
        if is_ci() || !model_cached("sentence-transformers/all-MiniLM-L6-v2") {
            eprintln!("⏭️  Skipping batch test");
            return;
        }

        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .unwrap();

        let texts = vec![
            "First sentence about cats",
            "Second sentence about dogs",
            "Third sentence about programming",
            "Fourth sentence about weather",
        ];

        let embeddings = embedder.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 4);

        for (i, embedding) in embeddings.iter().enumerate() {
            assert_eq!(embedding.len(), 384);

            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-4, "Embedding {} not normalized", i);
        }

        println!("✓ Batch embedding test passed");
    }

    #[tokio::test]
    #[ignore]
    async fn test_local_mpnet_if_cached() {
        if is_ci() || !model_cached("sentence-transformers/all-mpnet-base-v2") {
            eprintln!("⏭️  Skipping: all-mpnet-base-v2 not in cache");
            return;
        }

        println!("✓ Found cached model: sentence-transformers/all-mpnet-base-v2");

        let embedder = HFEmbedder::new("sentence-transformers/all-mpnet-base-v2")
            .await
            .expect("Failed to load mpnet model");

        assert_eq!(embedder.dimension(), 768);

        let embedding = embedder.embed("Test with MPNet model").unwrap();
        assert_eq!(embedding.len(), 768);

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);

        println!("✓ MPNet model test passed");
    }

    #[tokio::test]
    #[ignore]
    async fn test_local_bge_if_cached() {
        if is_ci() || !model_cached("BAAI/bge-small-en-v1.5") {
            eprintln!("⏭️  Skipping: BAAI/bge-small-en-v1.5 not in cache");
            return;
        }

        println!("✓ Found cached model: BAAI/bge-small-en-v1.5");

        let embedder = HFEmbedder::new("BAAI/bge-small-en-v1.5")
            .await
            .expect("Failed to load BGE model");

        assert_eq!(embedder.dimension(), 384);

        let embedding = embedder.embed("Test with BGE model").unwrap();
        assert_eq!(embedding.len(), 384);

        println!("✓ BGE model test passed");
    }

    // ============================================================================
    // Download tests - only run when explicitly requested
    // ============================================================================

    #[tokio::test]
    #[ignore] // Run with: cargo test test_download_minilm -- --ignored --test-threads=1
    async fn test_download_minilm() {
        if is_ci() {
            eprintln!("⏭️  Skipping download test in CI");
            return;
        }

        println!("📥 Downloading sentence-transformers/all-MiniLM-L6-v2...");
        println!("    (This may take a few minutes on first run)");

        let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
            .await
            .expect("Failed to download/load model");

        assert_eq!(embedder.dimension(), 384);

        let embedding = embedder.embed("Hello from fresh download").unwrap();
        assert_eq!(embedding.len(), 384);

        println!("✓ Model downloaded and working!");
        println!("    Cached at: {:?}", HFEmbedder::cache_dir());
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_mpnet() {
        if is_ci() {
            eprintln!("⏭️  Skipping download test in CI");
            return;
        }

        println!("📥 Downloading sentence-transformers/all-mpnet-base-v2...");
        println!("    (This is ~400MB, may take several minutes)");

        let embedder = HFEmbedder::new("sentence-transformers/all-mpnet-base-v2")
            .await
            .expect("Failed to download/load MPNet model");

        assert_eq!(embedder.dimension(), 768);

        let embedding = embedder
            .embed("Testing MPNet after download")
            .expect("Failed to embed");
        assert_eq!(embedding.len(), 768);

        println!("✓ MPNet model downloaded and working!");
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_bge_small() {
        if is_ci() {
            eprintln!("⏭️  Skipping download test in CI");
            return;
        }

        println!("📥 Downloading BAAI/bge-small-en-v1.5...");
        println!("    (This is ~130MB)");

        let embedder = HFEmbedder::new("BAAI/bge-small-en-v1.5")
            .await
            .expect("Failed to download/load BGE model");

        assert_eq!(embedder.dimension(), 384);

        let embedding = embedder.embed("Testing BGE after download").unwrap();
        assert_eq!(embedding.len(), 384);

        println!("✓ BGE model downloaded and working!");
    }

    // ============================================================================
    // Utility tests
    // ============================================================================

    #[test]
    fn test_cache_dir_detection() {
        // Test default cache dir
        let default_cache = HFEmbedder::cache_dir();
        assert!(default_cache.to_str().unwrap().contains("huggingface"));

        // Test custom cache dir
        std::env::set_var("HF_HOME", "/tmp/test_cache");
        let custom_cache = HFEmbedder::cache_dir();
        assert_eq!(custom_cache, PathBuf::from("/tmp/test_cache"));
        std::env::remove_var("HF_HOME");

        println!("✓ Cache directory detection works");
    }

    #[test]
    fn test_list_cached_models() {
        let cache_dir = HFEmbedder::cache_dir();
        let hub_dir = cache_dir.join("hub");

        if !hub_dir.exists() {
            println!("ℹ️  No HF cache directory found at {:?}", hub_dir);
            return;
        }

        println!("📦 Cached models in {:?}:", hub_dir);

        if let Ok(entries) = std::fs::read_dir(&hub_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.starts_with("models--") {
                    let model_name = name_str
                        .strip_prefix("models--")
                        .unwrap()
                        .replace("--", "/");

                    let is_complete = model_cached(&model_name);
                    let status = if is_complete { "✓" } else { "⚠️" };

                    println!("    {} {}", status, model_name);
                }
            }
        }
    }
}
