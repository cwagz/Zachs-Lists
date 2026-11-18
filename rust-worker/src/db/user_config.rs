use anyhow::{anyhow, Result};
use bson::doc;
use mongodb::{Collection, Database};
use serde::Deserialize;

use crate::build_stamp::{should_apply_whitelist, AllowlistIdentity};

/// One allowlist catalog entry stored on a user or the default config.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AllowlistSourceConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub project_url: String,
}

/// Inputs that decide blocklist filtering and the published allowlist.
#[derive(Debug, Clone)]
pub struct BuildInputs {
    pub blocklists: String,
    pub whitelist: String,
    pub allowlist_sources: Vec<AllowlistSourceConfig>,
    pub apply_whitelist_to_blocklists: bool,
    pub publish_allowlist: bool,
}

impl BuildInputs {
    pub fn allowlist_identities(&self) -> Vec<AllowlistIdentity> {
        self.allowlist_sources
            .iter()
            .filter(|source| !source.url.trim().is_empty())
            .map(|source| AllowlistIdentity {
                url: source.url.clone(),
                name: source.name.clone(),
                license: source.license.clone(),
                project_url: source.project_url.clone(),
            })
            .collect()
    }
}

/// User config embedded in user document
#[derive(Debug, Clone, Deserialize)]
pub struct UserConfig {
    pub blocklists: Option<String>,
    pub whitelist: Option<String>,
    #[serde(default)]
    pub allowlist_sources: Option<Vec<AllowlistSourceConfig>>,
    #[serde(default)]
    pub apply_whitelist_to_blocklists: Option<bool>,
    #[serde(default)]
    pub publish_allowlist: Option<bool>,
}

/// User document projection for config retrieval
#[derive(Debug, Deserialize)]
struct UserDoc {
    pub config: Option<UserConfig>,
}

/// System config document for default lists
#[derive(Debug, Deserialize)]
struct SystemConfigDoc {
    pub blocklists: Option<String>,
    pub whitelist: Option<String>,
    #[serde(default)]
    pub allowlist_sources: Option<Vec<AllowlistSourceConfig>>,
    #[serde(default)]
    pub apply_whitelist_to_blocklists: Option<bool>,
    #[serde(default)]
    pub publish_allowlist: Option<bool>,
}

/// Repository for fetching user and system configurations from MongoDB
pub struct UserConfigRepository {
    users_collection: Collection<UserDoc>,
    system_config_collection: Collection<SystemConfigDoc>,
}

impl UserConfigRepository {
    /// Create a new user config repository
    pub fn new(db: &Database) -> Self {
        Self {
            users_collection: db.collection("users"),
            system_config_collection: db.collection("system_config"),
        }
    }

    /// Get config for a user or default lists
    ///
    /// For username "__default__", fetches from system_config collection.
    /// For regular users, fetches from their user document.
    pub async fn get_config(&self, username: &str) -> Result<UserConfig> {
        if username == "__default__" {
            self.get_default_config().await
        } else {
            self.get_user_config(username).await
        }
    }

    /// Get user config from users collection
    async fn get_user_config(&self, username: &str) -> Result<UserConfig> {
        let filter = doc! { "username": username };

        let user = self
            .users_collection
            .find_one(filter)
            .await?
            .ok_or_else(|| anyhow!("User not found: {}", username))?;

        user.config
            .ok_or_else(|| anyhow!("No config found for user: {}", username))
    }

    /// Get default config from system_config collection
    async fn get_default_config(&self) -> Result<UserConfig> {
        let filter = doc! { "_id": "default_config" };

        let config = self
            .system_config_collection
            .find_one(filter)
            .await?
            .ok_or_else(|| anyhow!("Default config not found in system_config collection"))?;

        Ok(UserConfig {
            blocklists: config.blocklists,
            whitelist: config.whitelist,
            allowlist_sources: config.allowlist_sources,
            apply_whitelist_to_blocklists: config.apply_whitelist_to_blocklists,
            publish_allowlist: config.publish_allowlist,
        })
    }

    /// Blocklists, whitelist, allowlist selection, and the filtering toggle.
    pub async fn get_build_inputs(&self, username: &str) -> Result<BuildInputs> {
        let config = self.get_config(username).await?;
        let blocklists = config
            .blocklists
            .ok_or_else(|| anyhow!("No blocklist config found for: {}", username))?;
        Ok(build_inputs_from(
            blocklists,
            config.whitelist.unwrap_or_default(),
            config.allowlist_sources.unwrap_or_default(),
            config.apply_whitelist_to_blocklists,
            config.publish_allowlist,
        ))
    }

    /// Get whitelist content for a user (returns empty string if none)
    pub async fn get_whitelist(&self, username: &str) -> Result<String> {
        let config = self.get_config(username).await?;
        Ok(config.whitelist.unwrap_or_default())
    }
}

fn build_inputs_from(
    blocklists: String,
    whitelist: String,
    allowlist_sources: Vec<AllowlistSourceConfig>,
    apply_whitelist: Option<bool>,
    publish_allowlist: Option<bool>,
) -> BuildInputs {
    BuildInputs {
        blocklists,
        whitelist,
        allowlist_sources,
        apply_whitelist_to_blocklists: should_apply_whitelist(apply_whitelist),
        publish_allowlist: publish_allowlist.unwrap_or(false),
    }
}
