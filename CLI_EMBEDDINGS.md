# CLI Embedding Configuration

The `indra` CLI supports multiple embedding providers via command-line flags.

## Quick Start

```bash
# Default (MockEmbedder)
indra create "My thought"

# HuggingFace local model
indra --embedder hf create "My thought"

# OpenAI API
indra --embedder openai create "My thought"

# Custom model
indra --embedder hf --model sentence-transformers/all-mpnet-base-v2 create "Better embeddings"
```

## Options

### `--embedder <PROVIDER>`

Choose embedding provider (default: `mock`):
- `mock` - Fast, deterministic, no dependencies
- `hf` - Local HuggingFace models (requires `--features hf-embeddings`)
- `openai` - OpenAI API (requires `--features api-embeddings`)
- `cohere` - Cohere API (requires `--features api-embeddings`)
- `voyage` - Voyage AI API (requires `--features api-embeddings`)

### `--model <MODEL>`

Model name for the embedder:
- **HF**: `sentence-transformers/all-MiniLM-L6-v2` (default)
- **OpenAI**: `text-embedding-3-small` (default), `text-embedding-3-large`
- **Cohere**: `embed-english-v3.0` (default)
- **Voyage**: `voyage-3` (default), `voyage-code-3`

### `--dimension <DIM>`

Embedding dimension (required for API embedders):
- OpenAI: 1536 (small), 3072 (large)
- Cohere: 1024
- Voyage: 1024

## Examples

### MockEmbedder (Default)
```bash
indra init
indra create "Test thought"
```

### HuggingFace Local
```bash
# Requires: cargo install indra_db --features hf-embeddings
indra --embedder hf create "Rust is fast"
indra --embedder hf search "programming" -l 5
```

### OpenAI
```bash
# Requires: 
#   cargo install indra_db --features api-embeddings
#   export OPENAI_API_KEY=sk-...

indra --embedder openai create "AI memory"
indra --embedder openai --model text-embedding-3-large --dimension 3072 create "High quality"
```

### Cohere
```bash
# Requires: export COHERE_API_KEY=...
indra --embedder cohere create "Multilingual text"
```

### Voyage AI
```bash
# Requires: export VOYAGE_API_KEY=...
indra --embedder voyage create "General text"
indra --embedder voyage --model voyage-code-3 create "function main() {}"
```

## Environment Variables

- `HF_HOME` - HuggingFace cache directory
- `HF_TOKEN` - HuggingFace API token
- `OPENAI_API_KEY` - OpenAI API key
- `COHERE_API_KEY` - Cohere API key
- `VOYAGE_API_KEY` - Voyage AI API key

## Consistency

**Important:** Use the same embedder for all operations on a database:

```bash
# ✅ Good
indra -d db.indra --embedder hf init
indra -d db.indra --embedder hf create "thought"

# ❌ Bad - dimension mismatch
indra -d db.indra --embedder hf init       # 384 dim
indra -d db.indra --embedder openai create # 1536 dim
```

## Complete Documentation

See [EMBEDDINGS.md](EMBEDDINGS.md) for full embedding documentation.
