//! Credential storage and automatic token refresh
//!
//! Stores credentials in ~/.config/indra/credentials.json
//! Supports automatic refresh of access tokens.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stored credentials with refresh capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    /// API base URL these credentials are for
    pub api_url: String,
    /// Access token (short-lived)
    pub access_token: String,
    /// Refresh token (long-lived)
    pub refresh_token: String,
    /// When the access token expires (unix timestamp)
    pub expires_at: u64,
    /// User info
    pub user: Option<UserInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub github_username: Option<String>,
}

impl Credentials {
    /// Check if access token is expired (with 60s buffer)
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.expires_at <= now + 60
    }
}

/// Credentials store - manages persistence and refresh
pub struct CredentialStore {
    path: PathBuf,
}

impl CredentialStore {
    /// Create store using default path (~/.config/indra/credentials.json)
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Config("Could not find config directory".into()))?
            .join("indra");

        std::fs::create_dir_all(&config_dir)
            .map_err(|e| Error::Config(format!("Failed to create config dir: {}", e)))?;

        Ok(Self {
            path: config_dir.join("credentials.json"),
        })
    }

    /// Load credentials for a given API URL
    pub fn load(&self, api_url: &str) -> Result<Option<Credentials>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| Error::Config(format!("Failed to read credentials: {}", e)))?;

        let all_creds: Vec<Credentials> = serde_json::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse credentials: {}", e)))?;

        Ok(all_creds.into_iter().find(|c| c.api_url == api_url))
    }

    /// Save credentials (updates existing or adds new)
    pub fn save(&self, creds: Credentials) -> Result<()> {
        let mut all_creds: Vec<Credentials> = if self.path.exists() {
            let content = std::fs::read_to_string(&self.path).unwrap_or_else(|_| "[]".into());
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            vec![]
        };

        // Update existing or add new
        if let Some(existing) = all_creds.iter_mut().find(|c| c.api_url == creds.api_url) {
            *existing = creds;
        } else {
            all_creds.push(creds);
        }

        let content = serde_json::to_string_pretty(&all_creds)
            .map_err(|e| Error::Config(format!("Failed to serialize credentials: {}", e)))?;

        std::fs::write(&self.path, content)
            .map_err(|e| Error::Config(format!("Failed to write credentials: {}", e)))?;

        Ok(())
    }

    /// Remove credentials for a given API URL
    pub fn remove(&self, api_url: &str) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| Error::Config(format!("Failed to read credentials: {}", e)))?;

        let mut all_creds: Vec<Credentials> = serde_json::from_str(&content).unwrap_or_default();

        all_creds.retain(|c| c.api_url != api_url);

        let content = serde_json::to_string_pretty(&all_creds)
            .map_err(|e| Error::Config(format!("Failed to serialize credentials: {}", e)))?;

        std::fs::write(&self.path, content)
            .map_err(|e| Error::Config(format!("Failed to write credentials: {}", e)))?;

        Ok(())
    }

    /// Get the credentials file path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new().expect("Failed to create credential store")
    }
}

#[cfg(feature = "sync")]
use reqwest::blocking::Client;

#[cfg(feature = "sync")]
/// Refresh an expired access token using the refresh token
pub fn refresh_access_token(
    client: &Client,
    api_url: &str,
    refresh_token: &str,
) -> Result<Credentials> {
    let url = format!("{}/auth/refresh", api_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .map_err(|e| Error::Http(format!("Refresh request failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(Error::Http(format!(
            "Token refresh failed ({}): {}",
            status, text
        )));
    }

    #[derive(Deserialize)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: String,
        expires_in: u64,
        user: Option<UserInfo>,
    }

    let data: RefreshResponse = resp
        .json()
        .map_err(|e| Error::Http(format!("Invalid refresh response: {}", e)))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Ok(Credentials {
        api_url: api_url.to_string(),
        access_token: data.access_token,
        refresh_token: data.refresh_token,
        expires_at: now + data.expires_in,
        user: data.user,
    })
}
