//! indra CLI - Command line interface for indra_db
//!
//! Provides commands for managing a thought graph from the command line.
//! Designed to be wrapped by MCP servers in other languages (e.g., TypeScript/Bun).

use clap::{Parser, Subcommand};
use indra_db::{Database, EdgeType, MockEmbedder, TraversalDirection};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "indra")]
#[command(about = "A content-addressed graph database for versioned thoughts")]
#[command(version)]
struct Cli {
    /// Path to the database file
    #[arg(short, long, default_value = "thoughts.indra")]
    database: PathBuf,

    /// Output format (json or text)
    #[arg(short, long, default_value = "json")]
    format: OutputFormat,

    /// Disable auto-commit (by default, mutating commands auto-commit)
    #[arg(long)]
    no_auto_commit: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Text,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new database
    Init,

    // === Thought Commands ===
    /// Create a new thought
    Create {
        /// The content of the thought
        content: String,
        /// Optional ID for the thought
        #[arg(short, long)]
        id: Option<String>,
    },

    /// Get a thought by ID
    Get {
        /// The thought ID
        id: String,
    },

    /// Update a thought's content
    Update {
        /// The thought ID
        id: String,
        /// The new content
        content: String,
    },

    /// Delete a thought
    Delete {
        /// The thought ID
        id: String,
    },

    /// List all thoughts
    List {
        /// Maximum number of thoughts to return
        #[arg(short, long)]
        limit: Option<usize>,
    },

    // === Relationship Commands ===
    /// Create a relationship between thoughts
    Relate {
        /// Source thought ID
        source: String,
        /// Target thought ID
        target: String,
        /// Relationship type
        #[arg(short = 't', long, default_value = "relates_to")]
        edge_type: String,
        /// Relationship weight (0.0 to 1.0)
        #[arg(short, long)]
        weight: Option<f32>,
    },

    /// Remove a relationship
    Unrelate {
        /// Source thought ID
        source: String,
        /// Target thought ID
        target: String,
        /// Relationship type
        #[arg(short = 't', long, default_value = "relates_to")]
        edge_type: String,
    },

    /// Get neighbors of a thought
    Neighbors {
        /// The thought ID
        id: String,
        /// Direction: outgoing, incoming, or both
        #[arg(short, long, default_value = "both")]
        direction: String,
    },

    // === Search Commands ===
    /// Search thoughts by semantic similarity
    Search {
        /// The search query
        query: String,
        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Minimum similarity threshold (0.0 to 1.0)
        #[arg(short, long)]
        threshold: Option<f32>,
    },

    // === Version Control Commands ===
    /// Commit current changes
    Commit {
        /// Commit message
        message: String,
        /// Author name
        #[arg(short, long, default_value = "indra-cli")]
        author: String,
    },

    /// Show commit history
    Log {
        /// Maximum number of commits to show
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Create a new branch
    Branch {
        /// Branch name
        name: String,
    },

    /// Switch to a branch
    Checkout {
        /// Branch name
        name: String,
    },

    /// List all branches
    Branches,

    /// Show diff between commits
    Diff {
        /// First commit hash (or "HEAD" for current)
        #[arg(default_value = "HEAD~1")]
        from: String,
        /// Second commit hash (or "HEAD" for current)
        #[arg(default_value = "HEAD")]
        to: String,
    },

    /// Show database status
    Status,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            let db = Database::create(&cli.database)?;
            db.sync()?;
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "message": format!("Created database at {}", cli.database.display())
                }),
            );
        }

        Commands::Create { content, id } => {
            let mut db = open_db(&cli.database)?;
            let thought_id = if let Some(id) = id {
                db.create_thought_with_id(id, &content)?
            } else {
                db.create_thought(&content)?
            };
            // Always commit for CLI (each invocation is separate process)
            if !cli.no_auto_commit {
                db.commit_with_author("Auto-commit: create thought", "indra-cli")?;
            }
            db.sync()?;
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "id": thought_id.to_string()
                }),
            );
        }

        Commands::Get { id } => {
            let db = open_db(&cli.database)?;
            let thought_id = indra_db::ThoughtId::new(&id);
            match db.get_thought(&thought_id)? {
                Some(thought) => {
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "id": thought.id.to_string(),
                            "content": thought.content,
                            "type": thought.thought_type,
                            "created_at": thought.created_at,
                            "modified_at": thought.modified_at,
                            "attrs": thought.attrs,
                            "has_embedding": thought.embedding.is_some()
                        }),
                    );
                }
                None => {
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "status": "error",
                            "message": format!("Thought not found: {}", id)
                        }),
                    );
                    std::process::exit(1);
                }
            }
        }

        Commands::Update { id, content } => {
            let mut db = open_db(&cli.database)?;
            let thought_id = indra_db::ThoughtId::new(&id);
            db.update_thought(&thought_id, &content)?;
            if !cli.no_auto_commit {
                db.commit_with_author("Auto-commit: update thought", "indra-cli")?;
            }
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "id": id
                }),
            );
        }

        Commands::Delete { id } => {
            let mut db = open_db(&cli.database)?;
            let thought_id = indra_db::ThoughtId::new(&id);
            db.delete_thought(&thought_id)?;
            if !cli.no_auto_commit {
                db.commit_with_author("Auto-commit: delete thought", "indra-cli")?;
            }
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "id": id
                }),
            );
        }

        Commands::List { limit } => {
            let db = open_db(&cli.database)?;
            let mut thoughts = db.list_thoughts()?;
            if let Some(limit) = limit {
                thoughts.truncate(limit);
            }
            let items: Vec<_> = thoughts
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id.to_string(),
                        "content": t.content,
                        "type": t.thought_type,
                        "has_embedding": t.embedding.is_some()
                    })
                })
                .collect();
            output(
                &cli.format,
                &serde_json::json!({
                    "count": items.len(),
                    "thoughts": items
                }),
            );
        }

        Commands::Relate {
            source,
            target,
            edge_type,
            weight,
        } => {
            let mut db = open_db(&cli.database)?;
            if let Some(w) = weight {
                db.relate_weighted(&source, &target, EdgeType::new(&edge_type), w)?;
            } else {
                db.relate(&source, &target, EdgeType::new(&edge_type))?;
            }
            if !cli.no_auto_commit {
                db.commit_with_author("Auto-commit: create relation", "indra-cli")?;
            }
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "source": source,
                    "target": target,
                    "type": edge_type
                }),
            );
        }

        Commands::Unrelate {
            source,
            target,
            edge_type,
        } => {
            let mut db = open_db(&cli.database)?;
            db.unrelate(&source, &target, EdgeType::new(&edge_type))?;
            if !cli.no_auto_commit {
                db.commit_with_author("Auto-commit: remove relation", "indra-cli")?;
            }
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "source": source,
                    "target": target,
                    "type": edge_type
                }),
            );
        }

        Commands::Neighbors { id, direction } => {
            let db = open_db(&cli.database)?;
            let thought_id = indra_db::ThoughtId::new(&id);
            let dir = match direction.as_str() {
                "outgoing" | "out" => TraversalDirection::Outgoing,
                "incoming" | "in" => TraversalDirection::Incoming,
                _ => TraversalDirection::Both,
            };
            let neighbors = db.neighbors(&thought_id, dir)?;
            let items: Vec<_> = neighbors
                .iter()
                .map(|(thought, edge)| {
                    serde_json::json!({
                        "thought": {
                            "id": thought.id.to_string(),
                            "content": thought.content
                        },
                        "edge": {
                            "source": edge.source.to_string(),
                            "target": edge.target.to_string(),
                            "type": edge.edge_type.to_string(),
                            "weight": edge.weight
                        }
                    })
                })
                .collect();
            output(
                &cli.format,
                &serde_json::json!({
                    "count": items.len(),
                    "neighbors": items
                }),
            );
        }

        Commands::Search {
            query,
            limit,
            threshold,
        } => {
            let db = open_db(&cli.database)?;
            let results = if let Some(t) = threshold {
                db.search_with_threshold(&query, t, limit)?
            } else {
                db.search(&query, limit)?
            };
            let items: Vec<_> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.thought.id.to_string(),
                        "content": r.thought.content,
                        "score": r.score
                    })
                })
                .collect();
            output(
                &cli.format,
                &serde_json::json!({
                    "query": query,
                    "count": items.len(),
                    "results": items
                }),
            );
        }

        Commands::Commit { message, author } => {
            let mut db = open_db(&cli.database)?;
            let hash = db.commit_with_author(&message, &author)?;
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "hash": hash.to_hex(),
                    "message": message
                }),
            );
        }

        Commands::Log { limit } => {
            let db = open_db(&cli.database)?;
            let log = db.log(limit)?;
            let items: Vec<_> = log
                .iter()
                .map(|(hash, commit)| {
                    serde_json::json!({
                        "hash": hash.to_hex(),
                        "message": commit.message,
                        "author": commit.author,
                        "timestamp": commit.timestamp,
                        "parents": commit.parents.iter().map(|p| p.to_hex()).collect::<Vec<_>>()
                    })
                })
                .collect();
            output(
                &cli.format,
                &serde_json::json!({
                    "count": items.len(),
                    "commits": items
                }),
            );
        }

        Commands::Branch { name } => {
            let db = open_db(&cli.database)?;
            db.create_branch(&name)?;
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "branch": name
                }),
            );
        }

        Commands::Checkout { name } => {
            let mut db = open_db(&cli.database)?;
            db.checkout(&name)?;
            output(
                &cli.format,
                &serde_json::json!({
                    "status": "ok",
                    "branch": name
                }),
            );
        }

        Commands::Branches => {
            let db = open_db(&cli.database)?;
            let current = db.current_branch();
            let branches = db.list_branches();
            let items: Vec<_> = branches
                .iter()
                .map(|(name, hash)| {
                    serde_json::json!({
                        "name": name,
                        "hash": if hash.is_zero() { "".to_string() } else { hash.to_hex() },
                        "current": name == &current
                    })
                })
                .collect();
            output(
                &cli.format,
                &serde_json::json!({
                    "current": current,
                    "branches": items
                }),
            );
        }

        Commands::Diff { from, to } => {
            let db = open_db(&cli.database)?;
            let log = db.log(None)?;

            let from_hash = resolve_ref(&from, &log)?;
            let to_hash = resolve_ref(&to, &log)?;

            let diff = db.diff(from_hash, to_hash)?;
            let entries: Vec<_> = diff
                .entries
                .iter()
                .map(|e| match e {
                    indra_db::ops::DiffEntry::Added { key, new_hash } => {
                        serde_json::json!({
                            "type": "added",
                            "key": String::from_utf8_lossy(key),
                            "hash": new_hash.to_hex()
                        })
                    }
                    indra_db::ops::DiffEntry::Removed { key, old_hash } => {
                        serde_json::json!({
                            "type": "removed",
                            "key": String::from_utf8_lossy(key),
                            "hash": old_hash.to_hex()
                        })
                    }
                    indra_db::ops::DiffEntry::Modified {
                        key,
                        old_hash,
                        new_hash,
                    } => {
                        serde_json::json!({
                            "type": "modified",
                            "key": String::from_utf8_lossy(key),
                            "old_hash": old_hash.to_hex(),
                            "new_hash": new_hash.to_hex()
                        })
                    }
                })
                .collect();
            output(
                &cli.format,
                &serde_json::json!({
                    "from": from_hash.to_hex(),
                    "to": to_hash.to_hex(),
                    "added": diff.added_count(),
                    "removed": diff.removed_count(),
                    "modified": diff.modified_count(),
                    "entries": entries
                }),
            );
        }

        Commands::Status => {
            let db = open_db(&cli.database)?;
            output(
                &cli.format,
                &serde_json::json!({
                    "database": cli.database.display().to_string(),
                    "branch": db.current_branch(),
                    "dirty": db.is_dirty()
                }),
            );
        }
    }

    Ok(())
}

fn open_db(path: &PathBuf) -> anyhow::Result<Database> {
    let db = Database::open_or_create(path)?.with_embedder(MockEmbedder::default());
    Ok(db)
}

fn output(format: &OutputFormat, value: &serde_json::Value) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(value).unwrap());
        }
        OutputFormat::Text => {
            println!("{}", serde_json::to_string_pretty(value).unwrap());
        }
    }
}

fn resolve_ref(
    reference: &str,
    log: &[(indra_db::Hash, indra_db::Commit)],
) -> anyhow::Result<indra_db::Hash> {
    if reference == "HEAD" {
        return log
            .first()
            .map(|(h, _)| *h)
            .ok_or_else(|| anyhow::anyhow!("No commits yet"));
    }

    if reference.starts_with("HEAD~") {
        let n: usize = reference[5..].parse()?;
        return log
            .get(n)
            .map(|(h, _)| *h)
            .ok_or_else(|| anyhow::anyhow!("Not enough commits in history"));
    }

    // Try as hash
    indra_db::Hash::from_hex(reference)
        .map_err(|_| anyhow::anyhow!("Invalid reference: {}", reference))
}
