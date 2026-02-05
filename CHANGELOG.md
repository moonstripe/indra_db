# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2026-02-05

### Added
- **CLI Embedding Configuration**: New flags for choosing embedding providers
  - `--embedder`: Choose provider (mock, hf, openai, cohere, voyage)
  - `--model`: Specify model name for the embedder
  - `--dimension`: Set embedding dimension (required for API providers)
  - Examples: `indra --embedder hf create "thought"`, `indra --embedder openai --model text-embedding-3-small create "thought"`
  
- **Documentation**: `CLI_EMBEDDINGS.md` with complete CLI embedding guide
  - Quick start examples for all providers
  - Environment variable requirements
  - Consistency guidelines
  - Troubleshooting tips

### Changed
- Updated `open_db()` function to accept embedder configuration parameters
- All CLI commands now support embedder selection

## [0.1.2] - 2026-02-05

### Added
- **HFEmbedder**: Local transformer model support via Candle
  - Downloads models from HuggingFace Hub with caching
  - Supports sentence-transformers and BERT-compatible models
  - Respects `HF_HOME` and `HF_TOKEN` environment variables
  - Mean pooling and normalization for quality embeddings
  - Examples: all-MiniLM-L6-v2, all-mpnet-base-v2, bge-small-en-v1.5
  
- **ApiEmbedder**: External embedding API support
  - OpenAI (text-embedding-3-small, text-embedding-3-large)
  - Cohere (embed-english-v3.0, embed-multilingual-v3.0)
  - Voyage AI (voyage-3, voyage-3-lite, voyage-code-3)
  - Custom OpenAI-compatible endpoints
  - Efficient batch embedding operations
  
- **Optional Features**: Keep binary small with opt-in features
  - `hf-embeddings`: Enable HuggingFace local models
  - `api-embeddings`: Enable external API providers
  - Default build has zero embedding dependencies

- **Comprehensive Documentation**
  - `EMBEDDINGS.md`: Complete guide to all embedding options
  - Comparison tables, setup instructions, performance tips
  - `examples/embeddings.rs`: Working examples for all embedders
  - `tests/README.md`: Integration test guide

- **Integration Test Suite**
  - Auto-skips in CI (no network downloads, no timeouts)
  - Local tests run with cached models only
  - Explicit download tests available
  - Model cache detection and listing utilities

### Changed
- Updated README with embedding examples and feature flags
- Split CI test runs for better clarity (lib + integration tests)

### Fixed
- All existing tests continue to pass (59 unit tests)
- No breaking changes to public API

## [0.1.1] - 2026-02-04

### Added
- GitHub Actions workflows for CI/CD
  - Continuous Integration: test, clippy, format checks
  - Release workflow: cross-platform binary builds (9 targets)
  - Automated releases on version tags

### Changed
- Updated README with binary installation instructions
- Added prebuilt binary download examples

## [0.1.0] - 2026-02-04

### Added
- Initial release of indra_db
- Content-addressed graph database with git-like versioning
- Thought nodes with embeddings
- Typed/weighted edges
- Commit history and branching
- MockEmbedder for deterministic testing
- CLI tool (`indra`) with JSON output
- Single-file database format with BLAKE3 hashing
- Merkle trie for structural sharing
- Vector similarity search
- Graph traversal operations (neighbors, BFS, shortest path)
- Complete test suite (54 tests)

[0.1.3]: https://github.com/moonstripe/indra_db/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/moonstripe/indra_db/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/moonstripe/indra_db/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/moonstripe/indra_db/releases/tag/v0.1.0
