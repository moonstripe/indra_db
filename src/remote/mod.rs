//! Remote repository management
//!
//! Handles configuration and synchronization with remote IndraNet repositories.

mod credentials;
mod sync;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "sync")]
pub use credentials::refresh_access_token;
pub use credentials::{CredentialStore, Credentials, UserInfo};
pub use sync::{
    Auth, PullResult, PushResponse, RemoteStatus, SyncClient, SyncConfig, SyncState,
    DEFAULT_API_URL,
};

/// A remote repository configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Remote {
    /// Name of the remote (e.g., "origin")
    pub name: String,
    /// URL of the remote (e.g., "username/repo" or "https://indradb.net/username/repo")
    pub url: String,
    /// Last known HEAD commit hash on the remote
    #[serde(default)]
    pub last_known_head: Option<String>,
    /// When we last synced with this remote
    #[serde(default)]
    pub last_sync: Option<u64>,
}

impl Remote {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Remote {
            name: name.into(),
            url: url.into(),
            last_known_head: None,
            last_sync: None,
        }
    }

    /// Parse the URL to extract owner and repo name
    pub fn parse_url(&self) -> Option<(String, String)> {
        let url = self.url.trim();

        // Handle full URLs: https://indradb.net/username/repo
        if url.starts_with("https://") || url.starts_with("http://") {
            let path = url
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_start_matches("indradb.net/")
                .trim_start_matches("api.indradb.net/")
                // Legacy domain support
                .trim_start_matches("indra.net/")
                .trim_start_matches("api.indra.net/")
                .trim_start_matches("indra.dev/")
                .trim_start_matches("api.indra.dev/");
            return Self::parse_path(path);
        }

        // Handle short form: username/repo
        Self::parse_path(url)
    }

    fn parse_path(path: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    /// Get the full API URL for this remote
    pub fn api_url(&self, base_url: &str) -> String {
        if let Some((owner, repo)) = self.parse_url() {
            format!(
                "{}/bases/{}/{}",
                base_url.trim_end_matches('/'),
                owner,
                repo
            )
        } else {
            // Assume it's already a full URL
            self.url.clone()
        }
    }
}

/// Configuration for all remotes, stored alongside the database
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RemoteConfig {
    /// Map of remote name to remote configuration
    pub remotes: HashMap<String, Remote>,
    /// Default remote for push/pull operations
    #[serde(default)]
    pub default_remote: Option<String>,
}

impl RemoteConfig {
    /// Load remote config from a file path
    pub fn load(db_path: &Path) -> crate::Result<Self> {
        let config_path = Self::config_path(db_path);
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save remote config to a file path
    pub fn save(&self, db_path: &Path) -> crate::Result<()> {
        let config_path = Self::config_path(db_path);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    /// Get the config file path for a database
    fn config_path(db_path: &Path) -> std::path::PathBuf {
        let mut config_path = db_path.to_path_buf();
        let file_name = db_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| ".indra".to_string());
        config_path.set_file_name(format!("{}.remotes", file_name));
        config_path
    }

    /// Add a new remote
    pub fn add(&mut self, name: impl Into<String>, url: impl Into<String>) -> crate::Result<()> {
        let name = name.into();
        if self.remotes.contains_key(&name) {
            return Err(crate::Error::Remote(format!(
                "Remote '{}' already exists",
                name
            )));
        }
        let remote = Remote::new(name.clone(), url);
        self.remotes.insert(name.clone(), remote);

        // Set as default if it's the first remote
        if self.default_remote.is_none() {
            self.default_remote = Some(name);
        }

        Ok(())
    }

    /// Remove a remote
    pub fn remove(&mut self, name: &str) -> crate::Result<()> {
        if self.remotes.remove(name).is_none() {
            return Err(crate::Error::Remote(format!("Remote '{}' not found", name)));
        }

        // Clear default if we removed it
        if self.default_remote.as_deref() == Some(name) {
            self.default_remote = self.remotes.keys().next().cloned();
        }

        Ok(())
    }

    /// Get a remote by name
    pub fn get(&self, name: &str) -> Option<&Remote> {
        self.remotes.get(name)
    }

    /// Get a mutable remote by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Remote> {
        self.remotes.get_mut(name)
    }

    /// Set the URL for an existing remote
    pub fn set_url(&mut self, name: &str, url: impl Into<String>) -> crate::Result<()> {
        let remote = self
            .remotes
            .get_mut(name)
            .ok_or_else(|| crate::Error::Remote(format!("Remote '{}' not found", name)))?;
        remote.url = url.into();
        Ok(())
    }

    /// List all remotes
    pub fn list(&self) -> Vec<&Remote> {
        self.remotes.values().collect()
    }

    /// Set the default remote
    pub fn set_default(&mut self, name: &str) {
        if self.remotes.contains_key(name) {
            self.default_remote = Some(name.to_string());
        }
    }

    /// Update the last_sync timestamp for a remote
    pub fn update_last_sync(&mut self, name: &str) -> crate::Result<()> {
        let remote = self
            .remotes
            .get_mut(name)
            .ok_or_else(|| crate::Error::Remote(format!("Remote '{}' not found", name)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        remote.last_sync = Some(now);
        Ok(())
    }

    /// Update the last known HEAD hash for a remote
    pub fn update_last_known_head(&mut self, name: &str, hash: &str) -> crate::Result<()> {
        let remote = self
            .remotes
            .get_mut(name)
            .ok_or_else(|| crate::Error::Remote(format!("Remote '{}' not found", name)))?;

        remote.last_known_head = Some(hash.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_url_parsing() {
        let remote = Remote::new("origin", "username/repo");
        assert_eq!(
            remote.parse_url(),
            Some(("username".to_string(), "repo".to_string()))
        );

        let remote = Remote::new("origin", "https://indradb.net/username/repo");
        assert_eq!(
            remote.parse_url(),
            Some(("username".to_string(), "repo".to_string()))
        );

        let remote = Remote::new("origin", "https://api.indradb.net/username/repo");
        assert_eq!(
            remote.parse_url(),
            Some(("username".to_string(), "repo".to_string()))
        );

        // Legacy domains should still work
        let remote = Remote::new("origin", "https://indra.dev/username/repo");
        assert_eq!(
            remote.parse_url(),
            Some(("username".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_remote_config() {
        let mut config = RemoteConfig::default();

        config.add("origin", "user/repo").unwrap();
        assert_eq!(config.default_remote, Some("origin".to_string()));

        config.add("upstream", "other/repo").unwrap();
        assert!(config.get("upstream").is_some());

        config.set_url("origin", "newuser/newrepo").unwrap();
        assert_eq!(config.get("origin").unwrap().url, "newuser/newrepo");

        config.remove("origin").unwrap();
        assert!(config.get("origin").is_none());
        assert_eq!(config.default_remote, Some("upstream".to_string()));
    }
}
