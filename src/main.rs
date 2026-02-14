//! indra CLI - Command line interface for indra_db
//!
//! Provides commands for managing a thought graph from the command line.
//! Designed to be wrapped by MCP servers in other languages (e.g., TypeScript/Bun).

use clap::{Parser, Subcommand};
use indra_db::{Database, EdgeType, TraversalDirection};
use std::path::PathBuf;

#[cfg(feature = "hf-embeddings")]
use indra_db::embedding::HFEmbedder;

#[cfg(feature = "api-embeddings")]
use indra_db::embedding::{ApiEmbedder, ApiProvider};

use indra_db::embedding::MockEmbedder;

#[derive(Parser)]
#[command(name = "indra")]
#[command(about = "A content-addressed graph database for versioned thoughts")]
#[command(version)]
struct Cli {
    /// Path to the database file
    #[arg(short, long, default_value = ".indra")]
    database: PathBuf,

    /// Output format (json or text)
    #[arg(short, long, default_value = "json")]
    format: OutputFormat,

    /// Disable auto-commit (by default, mutating commands auto-commit)
    #[arg(long)]
    no_auto_commit: bool,

    /// Embedding provider: hf (default), mock, openai, cohere, voyage
    #[arg(long, default_value = "hf")]
    embedder: String,

    /// Model name for embedder (e.g., "sentence-transformers/all-MiniLM-L6-v2" for HF, "text-embedding-3-small" for OpenAI)
    #[arg(long)]
    model: Option<String>,

    /// Embedding dimension (required for API embedders)
    #[arg(long)]
    dimension: Option<usize>,

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

    // === Remote Commands ===
    /// Manage remote repositories
    #[command(subcommand)]
    Remote(RemoteCommands),

    /// Push to a remote repository
    Push {
        /// Remote name (default: origin)
        #[arg(default_value = "origin")]
        remote: String,
        /// Force push even if remote is ahead
        #[arg(short, long)]
        force: bool,
    },

    /// Pull from a remote repository
    Pull {
        /// Remote name (default: origin)
        #[arg(default_value = "origin")]
        remote: String,
        /// Force pull even if there are conflicts (discards local changes)
        #[arg(short, long)]
        force: bool,
    },

    /// Check sync status with remote
    SyncStatus {
        /// Remote name (default: origin)
        #[arg(default_value = "origin")]
        remote: String,
    },

    /// Clone a remote repository
    Clone {
        /// Remote URL (e.g., username/repo or full URL)
        url: String,
        /// Local path (default: repo name)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Login to IndraNet (opens browser for GitHub OAuth)
    Login,

    /// Logout from IndraNet (removes stored credentials)
    Logout,

    /// Show current authentication status
    Whoami,

    /// Export database in various formats
    Export {
        /// Export format: json (raw thoughts and commits)
        #[arg(short, long, default_value = "json")]
        format: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RemoteCommands {
    /// Add a new remote
    Add {
        /// Name for the remote (e.g., origin)
        name: String,
        /// URL of the remote (e.g., username/repo or https://indradb.net/username/repo)
        url: String,
    },

    /// Remove a remote
    Remove {
        /// Name of the remote to remove
        name: String,
    },

    /// List all remotes
    List,

    /// Show details of a remote
    Show {
        /// Name of the remote
        name: String,
    },

    /// Set the URL of an existing remote
    SetUrl {
        /// Name of the remote
        name: String,
        /// New URL
        url: String,
    },
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
            let mut db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let mut db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let mut db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let mut db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let mut db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let mut db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let mut db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;
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
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;

            // Load remote config for status display
            let remote_config = indra_db::RemoteConfig::load(&cli.database).unwrap_or_default();
            let remotes: Vec<_> = remote_config
                .list()
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "name": r.name,
                        "url": r.url
                    })
                })
                .collect();

            output(
                &cli.format,
                &serde_json::json!({
                    "database": cli.database.display().to_string(),
                    "branch": db.current_branch(),
                    "dirty": db.is_dirty(),
                    "remotes": remotes
                }),
            );
        }

        // === Remote Commands ===
        Commands::Remote(remote_cmd) => {
            let mut remote_config = indra_db::RemoteConfig::load(&cli.database)?;

            match remote_cmd {
                RemoteCommands::Add { name, url } => {
                    remote_config.add(&name, &url)?;
                    remote_config.save(&cli.database)?;
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "status": "ok",
                            "message": format!("Added remote '{}' -> {}", name, url)
                        }),
                    );
                }

                RemoteCommands::Remove { name } => {
                    remote_config.remove(&name)?;
                    remote_config.save(&cli.database)?;
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "status": "ok",
                            "message": format!("Removed remote '{}'", name)
                        }),
                    );
                }

                RemoteCommands::List => {
                    let remotes: Vec<_> = remote_config
                        .list()
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "name": r.name,
                                "url": r.url,
                                "last_sync": r.last_sync,
                                "last_known_head": r.last_known_head
                            })
                        })
                        .collect();
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "count": remotes.len(),
                            "default": remote_config.default_remote,
                            "remotes": remotes
                        }),
                    );
                }

                RemoteCommands::Show { name } => {
                    let remote = remote_config
                        .get(&name)
                        .ok_or_else(|| anyhow::anyhow!("Remote '{}' not found", name))?;
                    let parsed = remote.parse_url();
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "name": remote.name,
                            "url": remote.url,
                            "owner": parsed.as_ref().map(|(o, _)| o),
                            "repo": parsed.as_ref().map(|(_, r)| r),
                            "last_sync": remote.last_sync,
                            "last_known_head": remote.last_known_head
                        }),
                    );
                }

                RemoteCommands::SetUrl { name, url } => {
                    remote_config.set_url(&name, &url)?;
                    remote_config.save(&cli.database)?;
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "status": "ok",
                            "message": format!("Updated URL for remote '{}' -> {}", name, url)
                        }),
                    );
                }
            }
        }

        Commands::Push { remote, force } => {
            let mut remote_config = indra_db::RemoteConfig::load(&cli.database)?;
            let remote_info = remote_config
                .get(&remote)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Remote '{}' not found. Add it with: indra remote add {} <url>",
                        remote,
                        remote
                    )
                })?
                .clone();

            // Get local head for reporting
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;

            let log = db.log(Some(1))?;
            let head_hash = log.first().map(|(h, _)| h.to_hex()).unwrap_or_default();

            drop(db); // Close database before push

            // Create sync client and push
            #[cfg(feature = "sync")]
            {
                let sync_config = indra_db::SyncConfig::from_env();
                let client = indra_db::SyncClient::new(sync_config)?;

                match client.push(&cli.database, &remote_info, force) {
                    Ok(result) => {
                        if result.success {
                            // Update last_sync time
                            remote_config.update_last_sync(&remote)?;
                            remote_config.save(&cli.database)?;

                            output(
                                &cli.format,
                                &serde_json::json!({
                                    "status": "ok",
                                    "message": "Push completed successfully",
                                    "remote": remote_info.name,
                                    "url": remote_info.url,
                                    "local_head": head_hash,
                                    "size_bytes": result.size_bytes
                                }),
                            );
                        } else {
                            output(
                                &cli.format,
                                &serde_json::json!({
                                    "status": "error",
                                    "message": result.error.unwrap_or_else(|| "Push failed".to_string()),
                                    "remote": remote_info.name,
                                    "url": remote_info.url
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "error",
                                "message": format!("Push failed: {}", e),
                                "remote": remote_info.name,
                                "url": remote_info.url
                            }),
                        );
                    }
                }
            }

            #[cfg(not(feature = "sync"))]
            {
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": "Sync feature not enabled. Rebuild with --features sync",
                        "remote": remote_info.name,
                        "url": remote_info.url,
                        "local_head": head_hash
                    }),
                );
            }
        }

        Commands::Pull { remote, force } => {
            let mut remote_config = indra_db::RemoteConfig::load(&cli.database)?;
            let remote_info = remote_config
                .get(&remote)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Remote '{}' not found. Add it with: indra remote add {} <url>",
                        remote,
                        remote
                    )
                })?
                .clone();

            #[cfg(feature = "sync")]
            {
                let sync_config = indra_db::SyncConfig::from_env();
                let client = indra_db::SyncClient::new(sync_config)?;

                match client.pull_smart(&cli.database, &remote_info, force) {
                    Ok(result) => {
                        match result {
                            indra_db::PullResult::AlreadyUpToDate => {
                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "ok",
                                        "message": "Already up to date",
                                        "remote": remote_info.name,
                                        "url": remote_info.url
                                    }),
                                );
                            }
                            indra_db::PullResult::Updated { size_bytes } => {
                                // Update last_sync time
                                remote_config.update_last_sync(&remote)?;
                                remote_config.save(&cli.database)?;

                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "ok",
                                        "message": "Pull completed successfully",
                                        "remote": remote_info.name,
                                        "url": remote_info.url,
                                        "size_bytes": size_bytes
                                    }),
                                );
                            }
                            indra_db::PullResult::LocalAhead => {
                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "ok",
                                        "message": "Local is ahead of remote. Nothing to pull.",
                                        "remote": remote_info.name,
                                        "url": remote_info.url
                                    }),
                                );
                            }
                            indra_db::PullResult::RemoteEmpty => {
                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "ok",
                                        "message": "Remote is empty. Nothing to pull.",
                                        "remote": remote_info.name,
                                        "url": remote_info.url
                                    }),
                                );
                            }
                            indra_db::PullResult::Conflict {
                                local_head,
                                remote_head,
                            } => {
                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "conflict",
                                        "message": "Local and remote have diverged. Use --force to discard local changes.",
                                        "remote": remote_info.name,
                                        "url": remote_info.url,
                                        "local_head": local_head,
                                        "remote_head": remote_head
                                    }),
                                );
                            }
                            indra_db::PullResult::ForcePulled {
                                size_bytes,
                                discarded_head,
                            } => {
                                remote_config.update_last_sync(&remote)?;
                                remote_config.save(&cli.database)?;

                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "ok",
                                        "message": "Force pulled, discarding local changes",
                                        "remote": remote_info.name,
                                        "url": remote_info.url,
                                        "size_bytes": size_bytes,
                                        "discarded_head": discarded_head
                                    }),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "error",
                                "message": format!("Pull failed: {}", e),
                                "remote": remote_info.name,
                                "url": remote_info.url
                            }),
                        );
                    }
                }
            }

            #[cfg(not(feature = "sync"))]
            {
                let _ = force; // Suppress unused warning
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": "Sync feature not enabled. Rebuild with --features sync",
                        "remote": remote_info.name,
                        "url": remote_info.url
                    }),
                );
            }
        }

        Commands::SyncStatus { remote } => {
            let remote_config = indra_db::RemoteConfig::load(&cli.database)?;
            let remote_info = remote_config
                .get(&remote)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Remote '{}' not found. Add it with: indra remote add {} <url>",
                        remote,
                        remote
                    )
                })?
                .clone();

            #[cfg(feature = "sync")]
            {
                let sync_config = indra_db::SyncConfig::from_env();
                let client = indra_db::SyncClient::new(sync_config)?;

                match client.compare(&cli.database, &remote_info) {
                    Ok(state) => {
                        let (status, message) = match &state {
                            indra_db::SyncState::InSync => {
                                ("in_sync", "Local and remote are in sync".to_string())
                            }
                            indra_db::SyncState::LocalAhead {
                                local_head,
                                remote_head,
                            } => (
                                "local_ahead",
                                format!(
                                    "Local is ahead. Local: {}, Remote: {:?}",
                                    local_head, remote_head
                                ),
                            ),
                            indra_db::SyncState::RemoteAhead {
                                local_head,
                                remote_head,
                            } => (
                                "remote_ahead",
                                format!(
                                    "Remote is ahead. Local: {:?}, Remote: {}",
                                    local_head, remote_head
                                ),
                            ),
                            indra_db::SyncState::Diverged {
                                local_head,
                                remote_head,
                            } => (
                                "diverged",
                                format!(
                                    "Local and remote have diverged. Local: {}, Remote: {}",
                                    local_head, remote_head
                                ),
                            ),
                            indra_db::SyncState::RemoteEmpty => (
                                "remote_empty",
                                "Remote doesn't exist or is empty".to_string(),
                            ),
                            indra_db::SyncState::LocalEmpty { remote_head } => (
                                "local_empty",
                                format!("Local is empty. Remote head: {}", remote_head),
                            ),
                            indra_db::SyncState::Unknown { reason } => {
                                ("unknown", format!("Could not determine state: {}", reason))
                            }
                        };

                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": status,
                                "message": message,
                                "remote": remote_info.name,
                                "url": remote_info.url,
                                "can_push": state.can_push(),
                                "can_pull": state.can_pull(),
                                "has_conflict": state.has_conflict()
                            }),
                        );
                    }
                    Err(e) => {
                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "error",
                                "message": format!("Failed to check sync status: {}", e),
                                "remote": remote_info.name,
                                "url": remote_info.url
                            }),
                        );
                    }
                }
            }

            #[cfg(not(feature = "sync"))]
            {
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": "Sync feature not enabled. Rebuild with --features sync",
                        "remote": remote_info.name,
                        "url": remote_info.url
                    }),
                );
            }
        }

        Commands::Clone { url, path } => {
            // Determine local path from URL if not specified
            let local_path = path.unwrap_or_else(|| {
                let remote = indra_db::Remote::new("origin", &url);
                if let Some((_, repo)) = remote.parse_url() {
                    PathBuf::from(format!("{}.indra", repo))
                } else {
                    PathBuf::from(".indra")
                }
            });

            // Check if local path already exists
            if local_path.exists() {
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": format!("Path already exists: {}", local_path.display()),
                        "url": url,
                        "local_path": local_path.display().to_string()
                    }),
                );
                return Ok(());
            }

            #[cfg(feature = "sync")]
            {
                let remote = indra_db::Remote::new("origin", &url);
                let sync_config = indra_db::SyncConfig::from_env();
                let client = indra_db::SyncClient::new(sync_config)?;

                match client.pull(&local_path, &remote) {
                    Ok(size_bytes) => {
                        // Set up origin remote in the new database
                        let mut remote_config = indra_db::RemoteConfig::load(&local_path)?;
                        remote_config.add("origin".to_string(), url.clone())?;
                        remote_config.set_default("origin");
                        remote_config.update_last_sync("origin")?;
                        remote_config.save(&local_path)?;

                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "ok",
                                "message": "Clone completed successfully",
                                "url": url,
                                "local_path": local_path.display().to_string(),
                                "size_bytes": size_bytes
                            }),
                        );
                    }
                    Err(e) => {
                        // Clean up partial file if it exists
                        let _ = std::fs::remove_file(&local_path);

                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "error",
                                "message": format!("Clone failed: {}", e),
                                "url": url,
                                "local_path": local_path.display().to_string()
                            }),
                        );
                    }
                }
            }

            #[cfg(not(feature = "sync"))]
            {
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": "Sync feature not enabled. Rebuild with --features sync",
                        "url": url,
                        "local_path": local_path.display().to_string()
                    }),
                );
            }
        }

        Commands::Login => {
            #[cfg(feature = "sync")]
            {
                let api_url = std::env::var("INDRA_API_URL")
                    .unwrap_or_else(|_| indra_db::DEFAULT_API_URL.to_string());

                // Get login URL from API
                let client = reqwest::blocking::Client::new();
                let resp = client
                    .get(format!("{}/auth/cli/start", api_url))
                    .send()
                    .map_err(|e| anyhow::anyhow!("Failed to start login: {}", e))?;

                if !resp.status().is_success() {
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "status": "error",
                            "message": "Failed to start login flow"
                        }),
                    );
                    return Ok(());
                }

                #[derive(serde::Deserialize)]
                struct LoginStart {
                    url: String,
                    #[allow(dead_code)]
                    state: String,
                    poll_url: String,
                }

                let login_data: LoginStart = resp
                    .json()
                    .map_err(|e| anyhow::anyhow!("Invalid response: {}", e))?;

                // Open browser
                eprintln!("Opening browser for authentication...");
                if let Err(e) = open::that(&login_data.url) {
                    eprintln!("Failed to open browser: {}", e);
                    eprintln!("Please open this URL manually:");
                    eprintln!("{}", login_data.url);
                }

                // Poll for completion
                eprintln!("Waiting for authentication...");

                #[derive(serde::Deserialize)]
                struct PollResponse {
                    status: String,
                    access_token: Option<String>,
                    refresh_token: Option<String>,
                    expires_in: Option<u64>,
                    user: Option<indra_db::UserInfo>,
                }

                let mut attempts = 0;
                let max_attempts = 60; // 2 minutes with 2s intervals

                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    attempts += 1;

                    if attempts > max_attempts {
                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "error",
                                "message": "Login timed out. Please try again."
                            }),
                        );
                        return Ok(());
                    }

                    let poll_resp = client.get(&login_data.poll_url).send();

                    match poll_resp {
                        Ok(resp) if resp.status().is_success() => {
                            let poll_data: PollResponse = resp
                                .json()
                                .map_err(|e| anyhow::anyhow!("Invalid poll response: {}", e))?;

                            match poll_data.status.as_str() {
                                "pending" => continue,
                                "complete" => {
                                    // Save credentials
                                    let store = indra_db::CredentialStore::new()?;

                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs();

                                    let creds = indra_db::Credentials {
                                        api_url: api_url.clone(),
                                        access_token: poll_data.access_token.unwrap_or_default(),
                                        refresh_token: poll_data.refresh_token.unwrap_or_default(),
                                        expires_at: now + poll_data.expires_in.unwrap_or(3600),
                                        user: poll_data.user,
                                    };

                                    store.save(creds.clone())?;

                                    let user_name = creds
                                        .user
                                        .as_ref()
                                        .map(|u| u.name.clone())
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    output(
                                        &cli.format,
                                        &serde_json::json!({
                                            "status": "ok",
                                            "message": format!("Logged in as {}", user_name),
                                            "credentials_path": store.path().display().to_string()
                                        }),
                                    );
                                    return Ok(());
                                }
                                _ => {
                                    output(
                                        &cli.format,
                                        &serde_json::json!({
                                            "status": "error",
                                            "message": "Authentication failed"
                                        }),
                                    );
                                    return Ok(());
                                }
                            }
                        }
                        Ok(resp) if resp.status().as_u16() == 404 => {
                            output(
                                &cli.format,
                                &serde_json::json!({
                                    "status": "error",
                                    "message": "Login session expired. Please try again."
                                }),
                            );
                            return Ok(());
                        }
                        _ => continue,
                    }
                }
            }

            #[cfg(not(feature = "sync"))]
            {
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": "Sync feature not enabled. Rebuild with --features sync"
                    }),
                );
            }
        }

        Commands::Logout => {
            #[cfg(feature = "sync")]
            {
                let api_url = std::env::var("INDRA_API_URL")
                    .unwrap_or_else(|_| indra_db::DEFAULT_API_URL.to_string());

                // Remove stored credentials
                if let Ok(store) = indra_db::CredentialStore::new() {
                    if store.remove(&api_url).is_ok() {
                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "ok",
                                "message": "Logged out successfully",
                                "api_url": api_url
                            }),
                        );
                        return Ok(());
                    }
                }

                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "ok",
                        "message": "No stored credentials found",
                        "note": "If using INDRA_API_KEY, unset it: unset INDRA_API_KEY"
                    }),
                );
            }

            #[cfg(not(feature = "sync"))]
            {
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": "Sync feature not enabled"
                    }),
                );
            }
        }

        Commands::Whoami => {
            #[cfg(feature = "sync")]
            {
                let api_url = std::env::var("INDRA_API_URL")
                    .unwrap_or_else(|_| indra_db::DEFAULT_API_URL.to_string());

                // Check for stored credentials first
                if let Ok(store) = indra_db::CredentialStore::new() {
                    if let Ok(Some(creds)) = store.load(&api_url) {
                        if let Some(ref user) = creds.user {
                            output(
                                &cli.format,
                                &serde_json::json!({
                                    "status": "ok",
                                    "authenticated": true,
                                    "user": {
                                        "id": user.id,
                                        "name": user.name,
                                        "github_username": user.github_username
                                    },
                                    "api_url": api_url,
                                    "token_expires_at": creds.expires_at,
                                    "token_expired": creds.is_expired()
                                }),
                            );
                            return Ok(());
                        }
                    }
                }

                // Check for legacy API key
                if let Ok(key) = std::env::var("INDRA_API_KEY") {
                    let client = reqwest::blocking::Client::new();
                    let resp = client
                        .get(format!("{}/auth/me", api_url))
                        .header("Authorization", format!("Bearer {}", key))
                        .send();

                    match resp {
                        Ok(r) if r.status().is_success() => {
                            let data: serde_json::Value = r.json().unwrap_or_default();
                            if let Some(user) = data.get("user") {
                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "ok",
                                        "authenticated": true,
                                        "auth_method": "api_key",
                                        "user": user
                                    }),
                                );
                            } else {
                                output(
                                    &cli.format,
                                    &serde_json::json!({
                                        "status": "ok",
                                        "authenticated": true,
                                        "auth_method": "api_key",
                                        "message": "Authenticated via API key"
                                    }),
                                );
                            }
                        }
                        _ => {
                            output(
                                &cli.format,
                                &serde_json::json!({
                                    "status": "ok",
                                    "authenticated": true,
                                    "auth_method": "api_key",
                                    "message": "API key set but could not verify with server"
                                }),
                            );
                        }
                    }
                    return Ok(());
                }

                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "ok",
                        "authenticated": false,
                        "message": "Not logged in. Run 'indra login' to authenticate.",
                        "api_url": api_url
                    }),
                );
            }

            #[cfg(not(feature = "sync"))]
            {
                output(
                    &cli.format,
                    &serde_json::json!({
                        "status": "error",
                        "message": "Sync feature not enabled"
                    }),
                );
            }
        }

        Commands::Export {
            format,
            output: output_path,
        } => {
            let db = open_db(
                &cli.database,
                &cli.embedder,
                cli.model.clone(),
                cli.dimension,
            )?;

            match format.as_str() {
                "json" => {
                    let thoughts = db.list_thoughts()?;
                    let commits = db.log(None)?;

                    // Build export data
                    let export_data = serde_json::json!({
                        "thoughts": thoughts.iter().map(|t| {
                            serde_json::json!({
                                "id": t.id.0,
                                "content": t.content,
                                "thought_type": t.thought_type,
                                "created_at": t.created_at,
                                "embedding_dim": t.embedding.as_ref().map(|e| e.len()),
                            })
                        }).collect::<Vec<_>>(),
                        "commits": commits.iter().map(|(hash, commit)| {
                            serde_json::json!({
                                "hash": hash.to_hex(),
                                "message": commit.message,
                                "author": commit.author,
                                "timestamp": commit.timestamp,
                                "parents": commit.parents.iter().map(|p| p.to_hex()).collect::<Vec<_>>(),
                            })
                        }).collect::<Vec<_>>(),
                        "meta": {
                            "total_thoughts": thoughts.len(),
                            "total_commits": commits.len(),
                        }
                    });

                    let json = serde_json::to_string_pretty(&export_data)?;

                    if let Some(path) = output_path {
                        std::fs::write(&path, &json)?;
                        output(
                            &cli.format,
                            &serde_json::json!({
                                "status": "ok",
                                "message": format!("Exported to {}", path.display()),
                                "thoughts": thoughts.len(),
                                "commits": commits.len()
                            }),
                        );
                    } else {
                        // Output raw JSON to stdout
                        println!("{}", json);
                    }
                }
                _ => {
                    output(
                        &cli.format,
                        &serde_json::json!({
                            "status": "error",
                            "message": format!("Unknown export format: {}. Supported: json", format)
                        }),
                    );
                }
            }
        }
    }

    Ok(())
}

fn open_db(
    path: &PathBuf,
    embedder_type: &str,
    model: Option<String>,
    _dimension: Option<usize>,
) -> anyhow::Result<Database> {
    let db = Database::open_or_create(path)?;

    match embedder_type {
        "mock" => Ok(db.with_embedder(MockEmbedder::default())),

        #[cfg(feature = "hf-embeddings")]
        "hf" => {
            let model_name =
                model.unwrap_or_else(|| "sentence-transformers/all-MiniLM-L6-v2".to_string());
            eprintln!("Loading HF model: {}", model_name);
            let embedder =
                tokio::runtime::Runtime::new()?.block_on(HFEmbedder::new(&model_name))?;
            Ok(db.with_embedder(embedder))
        }

        #[cfg(feature = "api-embeddings")]
        "openai" => {
            let model_name = model.unwrap_or_else(|| "text-embedding-3-small".to_string());
            let dim = _dimension.unwrap_or(1536);
            let embedder = ApiEmbedder::new(ApiProvider::OpenAI, &model_name, dim)?;
            Ok(db.with_embedder(embedder))
        }

        #[cfg(feature = "api-embeddings")]
        "cohere" => {
            let model_name = model.unwrap_or_else(|| "embed-english-v3.0".to_string());
            let dim = _dimension.unwrap_or(1024);
            let embedder = ApiEmbedder::new(ApiProvider::Cohere, &model_name, dim)?;
            Ok(db.with_embedder(embedder))
        }

        #[cfg(feature = "api-embeddings")]
        "voyage" => {
            let model_name = model.unwrap_or_else(|| "voyage-3".to_string());
            let dim = _dimension.unwrap_or(1024);
            let embedder = ApiEmbedder::new(ApiProvider::Voyage, &model_name, dim)?;
            Ok(db.with_embedder(embedder))
        }

        _ => {
            #[cfg(not(feature = "hf-embeddings"))]
            if embedder_type == "hf" {
                anyhow::bail!("HF embedder not available. Compile with --features hf-embeddings");
            }

            #[cfg(not(feature = "api-embeddings"))]
            if ["openai", "cohere", "voyage"].contains(&embedder_type) {
                anyhow::bail!("API embedder not available. Compile with --features api-embeddings");
            }

            anyhow::bail!(
                "Unknown embedder: {}. Use: mock, hf, openai, cohere, or voyage",
                embedder_type
            )
        }
    }
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

    if let Some(suffix) = reference.strip_prefix("HEAD~") {
        let n: usize = suffix.parse()?;
        return log
            .get(n)
            .map(|(h, _)| *h)
            .ok_or_else(|| anyhow::anyhow!("Not enough commits in history"));
    }

    // Try as hash
    indra_db::Hash::from_hex(reference)
        .map_err(|_| anyhow::anyhow!("Invalid reference: {}", reference))
}
