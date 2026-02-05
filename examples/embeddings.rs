//! Example: Using different embedding providers with indra_db
//!
//! This example demonstrates:
//! 1. MockEmbedder (default, no feature required)
//! 2. HFEmbedder (requires --features hf-embeddings)
//! 3. ApiEmbedder (requires --features api-embeddings)
//!
//! Run examples:
//! ```bash
//! # MockEmbedder (always works)
//! cargo run --example embeddings
//!
//! # HFEmbedder (local models)
//! cargo run --example embeddings --features hf-embeddings
//!
//! # ApiEmbedder (requires OPENAI_API_KEY)
//! export OPENAI_API_KEY=sk-...
//! cargo run --example embeddings --features api-embeddings
//!
//! # All embedders
//! cargo run --example embeddings --all-features
//! ```

use indra_db::{Database, Result};

#[cfg(feature = "hf-embeddings")]
use indra_db::embedding::HFEmbedder;

#[cfg(feature = "api-embeddings")]
use indra_db::embedding::{ApiEmbedder, ApiProvider};

use indra_db::embedding::MockEmbedder;

fn example_mock_embedder() -> Result<()> {
    println!("\n=== MockEmbedder Example ===");
    println!("Fast, deterministic, zero dependencies\n");

    let embedder = MockEmbedder::new(384);
    let mut db = Database::open_or_create("example_mock.indra")?.with_embedder(embedder);

    // Create thoughts
    let _rust_id = db.create_thought_with_id("rust", "Rust is a systems programming language")?;
    let _python_id = db.create_thought_with_id("python", "Python is great for data science")?;
    let _js_id = db.create_thought_with_id("javascript", "JavaScript runs in browsers")?;

    db.commit("Add programming languages")?;

    // Search (MockEmbedder doesn't understand semantics, but it works!)
    println!("Searching for 'programming'...");
    let results = db.search("programming", 3)?;
    for (i, r) in results.iter().enumerate() {
        println!("  {}. {} (score: {:.3})", i + 1, r.thought.id, r.score);
    }

    println!("\n✓ MockEmbedder is great for testing and development!");

    // Clean up
    std::fs::remove_file("example_mock.indra").ok();
    Ok(())
}

#[cfg(feature = "hf-embeddings")]
async fn example_hf_embedder() -> Result<()> {
    println!("\n=== HFEmbedder Example ===");
    println!("Local transformer models, privacy-first\n");

    // This will download the model on first run
    println!("Loading model: sentence-transformers/all-MiniLM-L6-v2");
    println!("(First run will download ~90MB model to ~/.cache/huggingface)\n");

    let embedder = HFEmbedder::new("sentence-transformers/all-MiniLM-L6-v2").await?;
    let mut db = Database::open_or_create("example_hf.indra")?.with_embedder(embedder);

    // Create thoughts about programming
    db.create_thought_with_id(
        "rust",
        "Rust is a systems programming language focused on safety and performance",
    )?;
    db.create_thought_with_id(
        "python",
        "Python is a high-level language popular for data science and AI",
    )?;
    db.create_thought_with_id(
        "javascript",
        "JavaScript is the language of the web, running in all browsers",
    )?;
    db.create_thought_with_id(
        "go",
        "Go is a compiled language designed for simplicity and concurrency",
    )?;

    db.commit("Add programming languages with real embeddings")?;

    // Semantic search (this actually understands meaning!)
    println!("Searching for 'fast compiled language'...");
    let results = db.search("fast compiled language", 3)?;
    for (i, r) in results.iter().enumerate() {
        println!("  {}. {} (score: {:.3})", i + 1, r.thought.id, r.score);
    }

    println!("\nSearching for 'web development'...");
    let results = db.search("web development", 3)?;
    for (i, r) in results.iter().enumerate() {
        println!("  {}. {} (score: {:.3})", i + 1, r.thought.id, r.score);
    }

    println!("\nSearching for 'machine learning'...");
    let results = db.search("machine learning", 3)?;
    for (i, r) in results.iter().enumerate() {
        println!("  {}. {} (score: {:.3})", i + 1, r.thought.id, r.score);
    }

    println!("\n✓ HFEmbedder understands semantic meaning!");
    println!("✓ Everything runs locally - no API calls needed");
    println!("✓ Model is cached, subsequent runs are instant");

    // Clean up
    std::fs::remove_file("example_hf.indra").ok();
    Ok(())
}

#[cfg(feature = "api-embeddings")]
fn example_api_embedder() -> Result<()> {
    println!("\n=== ApiEmbedder Example ===");
    println!("Production-grade embeddings via OpenAI API\n");

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        println!("⚠️  OPENAI_API_KEY not set. Skipping API example.");
        println!("   Set it with: export OPENAI_API_KEY=sk-...");
        return Ok(());
    }

    println!("Using OpenAI text-embedding-3-small (1536 dimensions)\n");

    let embedder = ApiEmbedder::new(ApiProvider::OpenAI, "text-embedding-3-small", 1536)?;

    let mut db = Database::open_or_create("example_api.indra")?.with_embedder(embedder);

    // Create thoughts
    db.create_thought_with_id(
        "quantum",
        "Quantum computing uses qubits to perform calculations exponentially faster",
    )?;
    db.create_thought_with_id(
        "blockchain",
        "Blockchain is a distributed ledger technology that ensures tamper-proof records",
    )?;
    db.create_thought_with_id(
        "llm",
        "Large Language Models like GPT use transformers to understand and generate text",
    )?;
    db.create_thought_with_id(
        "crispr",
        "CRISPR gene editing allows precise modifications to DNA sequences",
    )?;

    db.commit("Add cutting-edge technologies")?;

    // Semantic search
    println!("Searching for 'artificial intelligence'...");
    let results = db.search("artificial intelligence", 3)?;
    for (i, r) in results.iter().enumerate() {
        println!("  {}. {} (score: {:.3})", i + 1, r.thought.id, r.score);
    }

    println!("\nSearching for 'advanced computing'...");
    let results = db.search("advanced computing", 3)?;
    for (i, r) in results.iter().enumerate() {
        println!("  {}. {} (score: {:.3})", i + 1, r.thought.id, r.score);
    }

    println!("\n✓ ApiEmbedder provides production-grade embeddings");
    println!("✓ No local model storage needed");
    println!("✓ Latest models automatically available");

    // Clean up
    std::fs::remove_file("example_api.indra").ok();
    Ok(())
}

#[cfg(feature = "api-embeddings")]
fn example_batch_embeddings() -> Result<()> {
    println!("\n=== Batch Embedding Example ===");
    println!("Efficiently embed multiple texts at once\n");

    if std::env::var("OPENAI_API_KEY").is_err() {
        println!("⚠️  OPENAI_API_KEY not set. Skipping batch example.");
        return Ok(());
    }

    let embedder = ApiEmbedder::new(ApiProvider::OpenAI, "text-embedding-3-small", 1536)?;

    println!("Embedding 10 thoughts in a single API call...");

    let thoughts = vec![
        "Machine learning",
        "Deep learning",
        "Neural networks",
        "Reinforcement learning",
        "Natural language processing",
        "Computer vision",
        "Generative AI",
        "Transfer learning",
        "Supervised learning",
        "Unsupervised learning",
    ];

    // This makes a single API call instead of 10!
    let start = std::time::Instant::now();
    let embeddings = embedder.embed_batch(&thoughts)?;
    let duration = start.elapsed();

    println!("✓ Embedded {} thoughts in {:?}", embeddings.len(), duration);
    println!(
        "✓ Single API call instead of {} separate calls",
        thoughts.len()
    );
    println!("✓ Significant cost savings for bulk operations");

    Ok(())
}

fn print_comparison_table() {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                   Embedding Comparison                        ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Feature        │ MockEmbedder │ HFEmbedder   │ ApiEmbedder   ║");
    println!("╠════════════════╪══════════════╪══════════════╪═══════════════╣");
    println!("║ Setup          │ ✓ Zero       │ ⚠ Download   │ ✓ API key     ║");
    println!("║ Speed          │ ✓✓✓ Instant  │ ✓✓ Fast      │ ✓ Network     ║");
    println!("║ Cost           │ ✓ Free       │ ✓ Free       │ ⚠ Pay/token   ║");
    println!("║ Privacy        │ ✓ Local      │ ✓ Local      │ ✗ Cloud       ║");
    println!("║ Offline        │ ✓ Yes        │ ✓ Yes        │ ✗ No          ║");
    println!("║ Quality        │ ✗ No meaning │ ✓✓✓✓         │ ✓✓✓✓✓         ║");
    println!("║ Storage        │ ✓ None       │ ⚠ 100-500MB  │ ✓ None        ║");
    println!("╚════════════════╧══════════════╧══════════════╧═══════════════╝");
}

fn main() -> Result<()> {
    println!("🧠 Indra DB Embedding Examples");
    println!("================================\n");

    // Always run MockEmbedder example
    example_mock_embedder()?;

    // Run HF example if feature is enabled
    #[cfg(feature = "hf-embeddings")]
    {
        // Need tokio runtime for async HF embedder
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(example_hf_embedder())?;
    }

    #[cfg(not(feature = "hf-embeddings"))]
    {
        println!("\n💡 HFEmbedder example skipped (compile with --features hf-embeddings)");
    }

    // Run API examples if feature is enabled
    #[cfg(feature = "api-embeddings")]
    {
        example_api_embedder()?;
        example_batch_embeddings()?;
    }

    #[cfg(not(feature = "api-embeddings"))]
    {
        println!("\n💡 ApiEmbedder examples skipped (compile with --features api-embeddings)");
    }

    print_comparison_table();

    println!("\n📚 For more details, see EMBEDDINGS.md");
    println!("   https://github.com/moonstripe/indra_db/blob/main/EMBEDDINGS.md");

    Ok(())
}
