//! Sync client for IndraNet API
//!
//! Handles push/pull operations with the remote API.

use crate::remote::{CredentialStore, Remote};
use crate::{Error, Result};
use std::path::Path;

/// Default API base URL (production)
pub const DEFAULT_API_URL: &str = "https://api.indradb.net";

/// Authentication credentials
#[derive(Clone, Debug)]
pub enum Auth {
    /// Access token authentication (from OAuth flow)
    AccessToken(String),
    /// API key authentication (legacy, for programmatic access)
    ApiKey(String),
    /// No authentication
    None,
}

/// Sync client configuration
#[derive(Clone, Debug)]
pub struct SyncConfig {
    /// Base URL for the API
    pub api_url: String,
    /// Authentication credentials
    pub auth: Auth,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            api_url: DEFAULT_API_URL.to_string(),
            auth: Auth::None,
            timeout_secs: 60,
        }
    }
}

impl SyncConfig {
    /// Create config from environment variables and stored credentials
    /// Priority: INDRA_API_KEY env var > stored credentials > none
    pub fn from_env() -> Self {
        let api_url =
            std::env::var("INDRA_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());

        // Check for legacy API key first
        if let Ok(key) = std::env::var("INDRA_API_KEY") {
            return SyncConfig {
                api_url,
                auth: Auth::ApiKey(key),
                ..Default::default()
            };
        }

        // Try to load stored credentials
        if let Ok(store) = CredentialStore::new() {
            if let Ok(Some(creds)) = store.load(&api_url) {
                return SyncConfig {
                    api_url,
                    auth: Auth::AccessToken(creds.access_token),
                    ..Default::default()
                };
            }
        }

        SyncConfig {
            api_url,
            auth: Auth::None,
            ..Default::default()
        }
    }

    /// Create config with stored credentials, refreshing if needed
    #[cfg(feature = "sync")]
    pub fn from_env_with_refresh() -> Result<Self> {
        let api_url =
            std::env::var("INDRA_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());

        // Check for legacy API key first
        if let Ok(key) = std::env::var("INDRA_API_KEY") {
            return Ok(SyncConfig {
                api_url,
                auth: Auth::ApiKey(key),
                ..Default::default()
            });
        }

        // Try to load and potentially refresh credentials
        let store = CredentialStore::new()?;
        if let Some(mut creds) = store.load(&api_url)? {
            if creds.is_expired() {
                // Refresh the token
                let client = reqwest::blocking::Client::new();
                creds = super::refresh_access_token(&client, &api_url, &creds.refresh_token)?;
                store.save(creds.clone())?;
            }
            return Ok(SyncConfig {
                api_url,
                auth: Auth::AccessToken(creds.access_token),
                ..Default::default()
            });
        }

        Ok(SyncConfig {
            api_url,
            auth: Auth::None,
            ..Default::default()
        })
    }
}

/// Response from push operation
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PushResponse {
    pub success: bool,
    pub size_bytes: Option<u64>,
    pub error: Option<String>,
}

/// Response from pull operation (status check)
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RemoteStatus {
    pub head_hash: Option<String>,
    pub size_bytes: u64,
    pub thought_count: u64,
    pub commit_count: u64,
    pub updated_at: String,
    #[serde(default)]
    pub visibility: Option<String>,
}

/// Sync state between local and remote
#[derive(Debug, Clone, serde::Serialize)]
pub enum SyncState {
    /// Local and remote are in sync
    InSync,
    /// Local is ahead of remote (can push)
    LocalAhead {
        local_head: String,
        remote_head: Option<String>,
    },
    /// Remote is ahead of local (can pull)
    RemoteAhead {
        local_head: Option<String>,
        remote_head: String,
    },
    /// Local and remote have diverged (conflict)
    Diverged {
        local_head: String,
        remote_head: String,
    },
    /// Remote doesn't exist yet
    RemoteEmpty,
    /// Local doesn't exist yet  
    LocalEmpty { remote_head: String },
    /// Unknown state (couldn't determine)
    Unknown { reason: String },
}

impl SyncState {
    pub fn can_push(&self) -> bool {
        matches!(
            self,
            SyncState::InSync | SyncState::LocalAhead { .. } | SyncState::RemoteEmpty
        )
    }

    pub fn can_pull(&self) -> bool {
        matches!(
            self,
            SyncState::InSync | SyncState::RemoteAhead { .. } | SyncState::LocalEmpty { .. }
        )
    }

    pub fn has_conflict(&self) -> bool {
        matches!(self, SyncState::Diverged { .. })
    }
}

/// Sync client for IndraNet API
#[cfg(feature = "sync")]
pub struct SyncClient {
    config: SyncConfig,
    client: reqwest::blocking::Client,
}

#[cfg(feature = "sync")]
impl SyncClient {
    /// Create a new sync client
    pub fn new(config: SyncConfig) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| Error::Http(e.to_string()))?;

        Ok(SyncClient { config, client })
    }

    /// Create a sync client from environment
    pub fn from_env() -> Result<Self> {
        Self::new(SyncConfig::from_env())
    }

    /// Build the full URL for an endpoint
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.api_url.trim_end_matches('/'), path)
    }

    /// Add auth headers to a request
    fn add_auth(
        &self,
        builder: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match &self.config.auth {
            Auth::AccessToken(token) => {
                builder.header("Authorization", format!("Bearer {}", token))
            }
            Auth::ApiKey(key) => builder.header("Authorization", format!("Bearer {}", key)),
            Auth::None => builder,
        }
    }

    /// Resolve a remote URL to owner/repo
    fn parse_remote(remote: &Remote) -> Result<(String, String)> {
        remote
            .parse_url()
            .ok_or_else(|| Error::Remote(format!("Invalid remote URL: {}", remote.url)))
    }

    /// Get an existing base by owner/name (returns None if not found)
    pub fn get_base(&self, remote: &Remote) -> Result<Option<String>> {
        let (owner, name) = Self::parse_remote(remote)?;

        let url = self.url(&format!("/bases/by-name/{}/{}", owner, name));
        let response = self.add_auth(self.client.get(&url)).send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                #[derive(serde::Deserialize)]
                struct BaseResponse {
                    base: Base,
                }
                #[derive(serde::Deserialize)]
                struct Base {
                    id: String,
                }
                let data: BaseResponse = resp.json().map_err(|e| Error::Http(e.to_string()))?;
                Ok(Some(data.base.id))
            }
            Ok(resp) if resp.status().as_u16() == 404 => Ok(None),
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().unwrap_or_default();
                Err(Error::Http(format!("API error {}: {}", status, text)))
            }
            Err(e) => Err(Error::Http(e.to_string())),
        }
    }

    /// Get or create a base ID for the remote
    /// First tries to find existing base, then creates if not found
    pub fn ensure_base(&self, remote: &Remote) -> Result<String> {
        let (_, name) = Self::parse_remote(remote)?;

        // Try to get existing base
        if let Some(id) = self.get_base(remote)? {
            return Ok(id);
        }

        // Create new base
        self.create_base(&name, None)
    }

    /// Create a new base on the remote
    pub fn create_base(&self, name: &str, description: Option<&str>) -> Result<String> {
        let url = self.url("/bases");

        #[derive(serde::Serialize)]
        struct CreateRequest<'a> {
            name: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<&'a str>,
            visibility: &'a str,
        }

        let body = CreateRequest {
            name,
            description,
            visibility: "private",
        };

        let response = self
            .add_auth(self.client.post(&url))
            .json(&body)
            .send()
            .map_err(|e| Error::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(Error::Http(format!(
                "Failed to create base: {} - {}",
                status, text
            )));
        }

        #[derive(serde::Deserialize)]
        struct CreateResponse {
            base: Base,
        }
        #[derive(serde::Deserialize)]
        struct Base {
            id: String,
        }

        let data: CreateResponse = response.json().map_err(|e| Error::Http(e.to_string()))?;

        Ok(data.base.id)
    }

    /// Get remote status without downloading
    pub fn status(&self, remote: &Remote) -> Result<Option<RemoteStatus>> {
        let base_id = match self.get_base(remote)? {
            Some(id) => id,
            None => return Ok(None), // Remote doesn't exist
        };

        let url = self.url(&format!("/bases/{}/status", base_id));

        let response = self
            .add_auth(self.client.get(&url))
            .send()
            .map_err(|e| Error::Http(e.to_string()))?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(Error::Http(format!(
                "Status check failed: {} - {}",
                status, text
            )));
        }

        let result: RemoteStatus = response.json().map_err(|e| Error::Http(e.to_string()))?;

        Ok(Some(result))
    }

    /// Compare local and remote state to determine sync state
    pub fn compare(&self, db_path: &Path, remote: &Remote) -> Result<SyncState> {
        // Get local head hash
        let local_head = self.get_local_head(db_path)?;

        // Get remote status
        let remote_status = self.status(remote)?;

        match (local_head, remote_status) {
            // Both empty or both don't exist
            (None, None) => Ok(SyncState::RemoteEmpty),

            // Local has commits, remote is empty/new
            (Some(local), None) => Ok(SyncState::LocalAhead {
                local_head: local,
                remote_head: None,
            }),

            // Local is empty, remote has data
            (None, Some(remote)) => {
                if let Some(ref remote_head) = remote.head_hash {
                    Ok(SyncState::LocalEmpty {
                        remote_head: remote_head.clone(),
                    })
                } else {
                    // Remote exists but has no commits
                    Ok(SyncState::InSync)
                }
            }

            // Both have data - compare hashes
            (Some(local), Some(remote)) => {
                match &remote.head_hash {
                    None => {
                        // Remote has no commits yet
                        Ok(SyncState::LocalAhead {
                            local_head: local,
                            remote_head: None,
                        })
                    }
                    Some(remote_head) if remote_head == &local => {
                        // Hashes match - in sync
                        Ok(SyncState::InSync)
                    }
                    Some(remote_head) => {
                        // Hashes differ - need to determine direction
                        // For now, we can't know if one is ancestor of other without
                        // downloading and checking. Assume diverged and let user decide.
                        // TODO: Implement ancestry check by downloading commit DAG
                        Ok(SyncState::Diverged {
                            local_head: local,
                            remote_head: remote_head.clone(),
                        })
                    }
                }
            }
        }
    }

    /// Get local HEAD hash from a database file
    fn get_local_head(&self, db_path: &Path) -> Result<Option<String>> {
        if !db_path.exists() {
            return Ok(None);
        }

        // Open the database and get HEAD
        let db = crate::Database::open(db_path)?;
        let log = db.log(Some(1))?;

        Ok(log.first().map(|(h, _)| h.to_hex()))
    }

    /// Push a database file to the remote
    /// Returns error if there's a conflict (unless force=true)
    pub fn push(&self, db_path: &Path, remote: &Remote, force: bool) -> Result<PushResponse> {
        // Check for conflicts unless force pushing
        if !force {
            let sync_state = self.compare(db_path, remote)?;

            if sync_state.has_conflict() {
                return Ok(PushResponse {
                    success: false,
                    size_bytes: None,
                    error: Some(
                        "Conflict detected: local and remote have diverged. Use --force to overwrite, or pull first.".to_string()
                    ),
                });
            }

            if !sync_state.can_push() {
                if let SyncState::RemoteAhead { remote_head, .. } = sync_state {
                    return Ok(PushResponse {
                        success: false,
                        size_bytes: None,
                        error: Some(format!(
                            "Remote is ahead (head: {}). Pull first or use --force to overwrite.",
                            remote_head
                        )),
                    });
                }
            }
        }

        // Get local head hash to send with request
        let local_head = self.get_local_head(db_path)?;

        // Read the database file
        let data = std::fs::read(db_path).map_err(Error::Io)?;

        // Ensure the base exists (or create it)
        let base_id = self.ensure_base(remote)?;

        // Upload to push endpoint with head hash header
        let url = self.url(&format!("/bases/{}/push", base_id));

        let mut request = self
            .add_auth(self.client.post(&url))
            .header("Content-Type", "application/octet-stream");

        if let Some(ref head) = local_head {
            request = request.header("X-Indra-Head-Hash", head);
        }

        let response = request
            .body(data)
            .send()
            .map_err(|e| Error::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Ok(PushResponse {
                success: false,
                size_bytes: None,
                error: Some(format!("Push failed: {} - {}", status, text)),
            });
        }

        let result: PushResponse = response.json().map_err(|e| Error::Http(e.to_string()))?;

        Ok(result)
    }

    /// Pull a database file from the remote
    /// Returns error if local has uncommitted changes (unless force=true)
    pub fn pull(&self, db_path: &Path, remote: &Remote) -> Result<u64> {
        // Get base ID (must exist, don't create)
        let base_id = match self.get_base(remote)? {
            Some(id) => id,
            None => {
                return Err(Error::Remote(format!(
                    "Remote base not found: {}",
                    remote.url
                )))
            }
        };

        // Download from pull endpoint
        let url = self.url(&format!("/bases/{}/pull", base_id));

        let response = self
            .add_auth(self.client.get(&url))
            .send()
            .map_err(|e| Error::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(Error::Http(format!("Pull failed: {} - {}", status, text)));
        }

        let bytes = response.bytes().map_err(|e| Error::Http(e.to_string()))?;

        let size = bytes.len() as u64;

        // Write to database path
        std::fs::write(db_path, &bytes).map_err(Error::Io)?;

        Ok(size)
    }

    /// Smart pull: check for conflicts first, offer options
    pub fn pull_smart(&self, db_path: &Path, remote: &Remote, force: bool) -> Result<PullResult> {
        let sync_state = self.compare(db_path, remote)?;

        match sync_state {
            SyncState::InSync => Ok(PullResult::AlreadyUpToDate),
            SyncState::RemoteAhead { .. } | SyncState::LocalEmpty { .. } => {
                let size = self.pull(db_path, remote)?;
                Ok(PullResult::Updated { size_bytes: size })
            }
            SyncState::LocalAhead { .. } => Ok(PullResult::LocalAhead),
            SyncState::RemoteEmpty => Ok(PullResult::RemoteEmpty),
            SyncState::Diverged {
                local_head,
                remote_head,
            } => {
                if force {
                    let size = self.pull(db_path, remote)?;
                    Ok(PullResult::ForcePulled {
                        size_bytes: size,
                        discarded_head: local_head,
                    })
                } else {
                    Ok(PullResult::Conflict {
                        local_head,
                        remote_head,
                    })
                }
            }
            SyncState::Unknown { reason } => Err(Error::Remote(format!(
                "Cannot determine sync state: {}",
                reason
            ))),
        }
    }
}

/// Result of a smart pull operation
#[derive(Debug, serde::Serialize)]
pub enum PullResult {
    /// Already up to date
    AlreadyUpToDate,
    /// Successfully pulled updates
    Updated { size_bytes: u64 },
    /// Local is ahead of remote (nothing to pull)
    LocalAhead,
    /// Remote is empty/doesn't exist
    RemoteEmpty,
    /// Conflict: local and remote have diverged
    Conflict {
        local_head: String,
        remote_head: String,
    },
    /// Force pulled, discarding local changes
    ForcePulled {
        size_bytes: u64,
        discarded_head: String,
    },
}

/// Stub implementation when sync feature is disabled
#[cfg(not(feature = "sync"))]
pub struct SyncClient;

#[cfg(not(feature = "sync"))]
impl SyncClient {
    pub fn new(_config: SyncConfig) -> Result<Self> {
        Err(Error::Remote(
            "Sync feature not enabled. Compile with --features sync".into(),
        ))
    }

    pub fn from_env() -> Result<Self> {
        Self::new(SyncConfig::default())
    }
}
