# indra_db

A content-addressed graph database for versioned thoughts. Think **git for knowledge graphs**.

[![Crates.io](https://img.shields.io/crates/v/indra_db.svg)](https://crates.io/crates/indra_db)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Why?

Most agent memory systems are **state-based**: here's what I know *now*. But understanding isn't a snapshot—it's a trajectory. When an agent rewrites a note, it loses:

- Why the understanding changed
- What the previous understanding was
- The branching paths not taken
- The confidence evolution

**indra_db** solves this by combining:
- **Git-like versioning**: Content-addressed storage, commits, branches
- **Graph semantics**: Thoughts as nodes, typed/weighted relationships as edges
- **Semantic search**: Embeddings stored with nodes, vector similarity queries

## Installation

### As a Rust library

```toml
[dependencies]
indra_db = "0.1"
```

### As a CLI

**Via cargo:**
```bash
cargo install indra_db
```

**Via prebuilt binary:**

Download the latest release for your platform from [GitHub Releases](https://github.com/moonstripe/indra_db/releases):

```bash
# macOS (Apple Silicon)
curl -L https://github.com/moonstripe/indra_db/releases/latest/download/indra-aarch64-apple-darwin.tar.gz | tar xz
chmod +x indra
sudo mv indra /usr/local/bin/

# Linux x86_64
curl -L https://github.com/moonstripe/indra_db/releases/latest/download/indra-x86_64-unknown-linux-gnu.tar.gz | tar xz
chmod +x indra
sudo mv indra /usr/local/bin/

# Windows (PowerShell)
# Download from releases page and add to PATH
```

Binaries are available for:
- macOS (Intel + Apple Silicon)
- Linux (x86_64, ARM64, ARMv7, musl variants)
- Windows (x86_64 + ARM64)

## Quick Start

### CLI Usage

```bash
# Initialize a new database
indra init

# Create thoughts
indra create "Cats are furry animals" --id cats
indra create "Dogs are loyal companions" --id dogs
indra create "Animals need food and water" --id animals

# Create relationships
indra relate cats animals -t part_of
indra relate dogs animals -t part_of

# Search semantically
indra search "fluffy pet"

# View neighbors
indra neighbors cats

# View history
indra log

# Branch for experimentation
indra branch experiment
indra checkout experiment
indra create "Cats are actually liquid" --id cats-liquid
indra checkout main  # original "cats" still intact
```

### Library Usage

```rust
use indra_db::{Database, MockEmbedder, TraversalDirection};

fn main() -> anyhow::Result<()> {
    // Open or create database with embedder
    let mut db = Database::open_or_create("thoughts.indra")?
        .with_embedder(MockEmbedder::default());

    // Create thoughts (auto-generates embeddings)
    let cat = db.create_thought_with_id("cat", "Cats are furry animals")?;
    let animal = db.create_thought_with_id("animal", "Animals need care")?;

    // Create relationship
    db.relate(&cat, &animal, "part_of")?;

    // Commit changes
    db.commit("Add cat and animal")?;

    // Semantic search
    let results = db.search("fluffy pet", 5)?;
    for r in results {
        println!("{}: {} (score: {:.3})", r.thought.id, r.thought.content, r.score);
    }

    // Graph traversal
    let neighbors = db.neighbors(&cat, TraversalDirection::Outgoing)?;
    for (thought, edge) in neighbors {
        println!("{} --[{}]--> {}", cat, edge.edge_type, thought.id);
    }

    Ok(())
}
```

## CLI Reference

```
indra [OPTIONS] <COMMAND>

Commands:
  init       Initialize a new database
  create     Create a new thought
  get        Get a thought by ID
  update     Update a thought's content
  delete     Delete a thought
  list       List all thoughts
  relate     Create a relationship between thoughts
  unrelate   Remove a relationship
  neighbors  Get neighbors of a thought
  search     Search thoughts by semantic similarity
  commit     Commit current changes
  log        Show commit history
  branch     Create a new branch
  checkout   Switch to a branch
  branches   List all branches
  diff       Show diff between commits
  status     Show database status

Options:
  -d, --database <DATABASE>  Path to database file [default: thoughts.indra]
  -f, --format <FORMAT>      Output format: json or text [default: json]
  --no-auto-commit           Disable auto-commit (for batch operations)
  -h, --help                 Print help
  -V, --version              Print version
```

### Examples

```bash
# JSON output (default) - great for scripting
indra list
# {"count":3,"thoughts":[{"id":"cats","content":"Cats are furry",...}]}

# Pretty-printed output
indra -f text list

# Custom database path
indra -d my_knowledge.indra create "New thought"

# Batch operations without auto-commit
indra --no-auto-commit create "Thought 1"
indra --no-auto-commit create "Thought 2"
indra commit "Add multiple thoughts"
```

## Using with MCP (Model Context Protocol)

The CLI outputs JSON by default, making it easy to wrap as an MCP server. A typical TypeScript wrapper would look like:

```typescript
import { Server } from "@modelcontextprotocol/server";
import { spawn } from "child_process";

async function indra(args: string[]): Promise<any> {
  return new Promise((resolve, reject) => {
    const proc = spawn("indra", ["-d", "agent.indra", ...args]);
    let stdout = "";
    proc.stdout.on("data", (d) => stdout += d);
    proc.on("close", (code) => {
      if (code === 0) resolve(JSON.parse(stdout));
      else reject(new Error(`indra exited with ${code}`));
    });
  });
}

const server = new Server({ name: "indra-mcp", version: "0.1.0" });

server.tool("create_thought", { content: "string", id: "string?" }, async ({ content, id }) => {
  const args = ["create", content];
  if (id) args.push("--id", id);
  return await indra(args);
});

server.tool("search_thoughts", { query: "string", limit: "number?" }, async ({ query, limit }) => {
  return await indra(["search", query, "-l", String(limit ?? 10)]);
});

// ... more tools
```

An MCP server implementation is planned as a separate npm package.

## Architecture

```
thoughts.indra (single file)
├── Header (64 bytes)
│   ├── Magic: "INDRA_DB"
│   ├── Version, flags
│   ├── Object count, index offset
│   └── Refs offset, HEAD
├── Objects (content-addressed, zstd compressed)
│   ├── Thoughts (id, content, embedding, metadata)
│   ├── Edges (source, target, type, weight)
│   ├── Commits (tree hash, parents, message)
│   └── Tree nodes (merkle trie)
├── Index (hash → offset mapping)
└── Refs (branch names → commit hashes)
```

**Key design decisions:**
- **BLAKE3** for content hashing (fast, secure)
- **Merkle trie** for structural sharing across commits
- **Edges float** to latest node version (not pinned to hashes)
- **Embeddings stored with nodes** (content-addressed, deduplicated)
- **Pluggable embedder trait** (bring your own model)

## Edge Types

Built-in edge type constants:
- `relates_to` - General relationship
- `supports` - Evidence/support
- `contradicts` - Contradiction
- `derives_from` - Derivation
- `part_of` - Hierarchy
- `similar_to` - Similarity
- `causes` - Causation
- `precedes` - Temporal ordering

Custom types are strings—use whatever makes sense for your domain.

## Embeddings

indra_db uses a pluggable embedding system. The built-in `MockEmbedder` generates deterministic embeddings from text hashes (good for testing). For production, implement the `Embedder` trait:

```rust
use indra_db::{Embedder, Result};

struct OpenAIEmbedder { /* ... */ }

impl Embedder for OpenAIEmbedder {
    fn dimension(&self) -> usize { 1536 }
    
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Call OpenAI API
    }
    
    fn model_name(&self) -> &str { "text-embedding-3-small" }
}
```

## Performance

Current implementation uses brute-force vector search, which is fine for <10k thoughts (~10-50ms). For larger graphs, HNSW indexing is on the roadmap.

| Operation | ~1k thoughts | ~10k thoughts |
|-----------|-------------|---------------|
| Create | <1ms | <1ms |
| Search | ~5ms | ~50ms |
| Commit | ~10ms | ~50ms |
| Get by ID | <1ms | <1ms |

## Roadmap

- [ ] HNSW index for HEAD (faster search at scale)
- [ ] Merge operations (three-way merge for branches)
- [ ] Export/import (JSON, GEXF)
- [ ] Python bindings (PyO3)
- [ ] Remote embedder support (OpenAI, Cohere, etc.)

## License

MIT

## Etymology

Named after [Indra's net](https://en.wikipedia.org/wiki/Indra%27s_net), a Buddhist metaphor for the interconnectedness of all phenomena—a net of jewels where each jewel reflects all others.
