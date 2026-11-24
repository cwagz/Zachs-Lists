use anyhow::{anyhow, Result};
use bson::{doc, Document};
use mongodb::{Collection, Database};
use serde::Deserialize;

use crate::allowlist_compose::AllowlistIdentity;
use crate::config::DEFAULT_TARGET;

/// User config embedded in user document
#[derive(Debug, Clone, Deserialize)]
pub struct UserConfig {
    pub blocklists: Option<String>,
    pub whitelist: Option<String>,
    #[serde(default)]
    pub allowlist_sources: Vec<AllowlistIdentity>,
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
    pub allowlist_sources: Vec<AllowlistIdentity>,
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
        if username == DEFAULT_TARGET {
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

    pub async fn record_allowlist_reports(
        &self,
        username: &str,
        reports: Vec<Document>,
    ) -> Result<()> {
        let update = if username == DEFAULT_TARGET {
            self.system_config_collection
                .update_one(
                    doc! { "_id": "default_config" },
                    doc! { "$set": { "allowlist_reports": reports } },
                )
                .await?
        } else {
            self.users_collection
                .update_one(
                    doc! { "username": username },
                    doc! { "$set": { "config.allowlist_reports": reports } },
                )
                .await?
        };
        if update.matched_count == 0 {
            anyhow::bail!("No config document for {username}");
        }
        Ok(())
    }
}
