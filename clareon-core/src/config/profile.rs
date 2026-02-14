// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Profile management for multi-account support
//!
//! Each profile is a fully isolated environment with its own config, database,
//! cache, and keyring secrets. Profiles are discovered by scanning a profiles
//! directory — each subdirectory containing a config.json is a profile.

use std::fmt;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{ConfigError, Result};

/// Identifies a profile by name (directory name)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(String);

impl ProfileId {
    /// Create a new ProfileId, validating the name
    ///
    /// Profile names must be non-empty, contain only alphanumeric characters,
    /// hyphens, and underscores, and be at most 64 characters long.
    pub fn new(name: &str) -> Result<Self> {
        if name.is_empty() {
            return Err(ConfigError::Invalid("Profile name cannot be empty".to_string()).into());
        }
        if name.len() > 64 {
            return Err(ConfigError::Invalid(
                "Profile name cannot exceed 64 characters".to_string(),
            )
            .into());
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ConfigError::Invalid(
                "Profile name can only contain alphanumeric characters, hyphens, and underscores"
                    .to_string(),
            )
            .into());
        }
        Ok(Self(name.to_string()))
    }

    /// Get the profile name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Runtime-resolved profile with all paths computed
#[derive(Debug, Clone)]
pub struct Profile {
    /// The profile identifier
    pub id: ProfileId,
    /// Path to the profile's config.json
    pub config_path: PathBuf,
    /// Path to the profile's database file
    pub database_path: PathBuf,
    /// SQLite connection URL for the database
    pub database_url: String,
    /// Root cache directory for this profile
    pub cache_root: PathBuf,
    /// Workspace cache directory for conversations
    pub workspace_cache_dir: PathBuf,
    /// Shared cache directory (e.g., pip cache)
    pub shared_cache_dir: PathBuf,
}

impl Profile {
    /// Create a new Profile with all paths resolved from the profile ID
    fn new(id: ProfileId, dirs: &ProjectDirs) -> Self {
        let config_path = dirs
            .config_dir()
            .join("profiles")
            .join(id.as_str())
            .join("config.json");

        let data_dir = dirs.data_dir().join("profiles").join(id.as_str());
        let database_path = data_dir.join("clareon.db");
        let database_url = format!("sqlite://{}?mode=rwc", database_path.display());

        let cache_root = dirs.cache_dir().join("profiles").join(id.as_str());
        let workspace_cache_dir = cache_root.join("conversations");
        let shared_cache_dir = cache_root.join("shared");

        Self {
            id,
            config_path,
            database_path,
            database_url,
            cache_root,
            workspace_cache_dir,
            shared_cache_dir,
        }
    }

    /// Create a Profile for testing with temporary paths
    #[cfg(test)]
    pub(crate) fn new_for_test(name: &str) -> Self {
        let dirs = ProjectDirs::from("org", "clareon", "clareon")
            .expect("Could not determine project dirs");
        let id = ProfileId::new(name).expect("Invalid profile name for test");
        Self::new(id, &dirs)
    }

    /// Ensure all profile directories exist
    fn ensure_dirs(&self) -> Result<()> {
        if let Some(config_parent) = self.config_path.parent() {
            std::fs::create_dir_all(config_parent).map_err(ConfigError::Io)?;
        }
        if let Some(db_parent) = self.database_path.parent() {
            std::fs::create_dir_all(db_parent).map_err(ConfigError::Io)?;
        }
        std::fs::create_dir_all(&self.cache_root).map_err(ConfigError::Io)?;
        std::fs::create_dir_all(&self.workspace_cache_dir).map_err(ConfigError::Io)?;
        std::fs::create_dir_all(&self.shared_cache_dir).map_err(ConfigError::Io)?;
        Ok(())
    }
}

/// Stateless utility for discovering and managing profiles
pub struct ProfileManager;

impl ProfileManager {
    /// List all available profiles by scanning the profiles directory
    pub fn list_profiles() -> Result<Vec<ProfileId>> {
        let profiles_dir = Self::profiles_dir()?;

        if !profiles_dir.exists() {
            return Ok(Vec::new());
        }

        let mut profiles = Vec::new();
        let entries = std::fs::read_dir(&profiles_dir).map_err(ConfigError::Io)?;

        for entry in entries {
            let entry = entry.map_err(ConfigError::Io)?;
            let path = entry.path();

            if path.is_dir()
                && path.join("config.json").exists()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && let Ok(id) = ProfileId::new(name)
            {
                profiles.push(id);
            }
        }

        profiles.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        Ok(profiles)
    }

    /// Get a resolved profile (computes paths, ensures directories exist)
    pub fn get_profile(id: &ProfileId) -> Result<Profile> {
        let dirs = Self::project_dirs()?;
        let profile = Profile::new(id.clone(), &dirs);

        if !profile.config_path.exists() {
            return Err(ConfigError::NotFound(profile.config_path.clone()).into());
        }

        profile.ensure_dirs()?;
        Ok(profile)
    }

    /// Create a new profile with default configuration
    pub fn create_profile(id: &ProfileId) -> Result<Profile> {
        let dirs = Self::project_dirs()?;
        let profile = Profile::new(id.clone(), &dirs);

        if profile.config_path.exists() {
            return Err(ConfigError::Invalid(format!("Profile '{}' already exists", id)).into());
        }

        info!("Creating new profile: {}", id);
        profile.ensure_dirs()?;

        // Write default config
        let default_config = super::settings::Config::default();
        default_config.save_to(&profile.config_path)?;

        debug!("Profile '{}' created at {:?}", id, profile.config_path);
        Ok(profile)
    }

    /// Check if a profile exists
    pub fn profile_exists(id: &ProfileId) -> Result<bool> {
        let dirs = Self::project_dirs()?;
        let profile = Profile::new(id.clone(), &dirs);
        Ok(profile.config_path.exists())
    }

    /// Get an existing profile or create a new one with default config
    pub fn get_or_create_profile(id: &ProfileId) -> Result<Profile> {
        if Self::profile_exists(id)? {
            Self::get_profile(id)
        } else {
            Self::create_profile(id)
        }
    }

    /// Get the root profiles directory
    fn profiles_dir() -> Result<PathBuf> {
        let dirs = Self::project_dirs()?;
        Ok(dirs.config_dir().join("profiles"))
    }

    /// Get the project directories
    fn project_dirs() -> Result<ProjectDirs> {
        ProjectDirs::from("org", "clareon", "clareon").ok_or_else(|| {
            ConfigError::Invalid("Could not determine home directory".to_string()).into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_id_valid() {
        assert!(ProfileId::new("default").is_ok());
        assert!(ProfileId::new("my-profile").is_ok());
        assert!(ProfileId::new("work_account").is_ok());
        assert!(ProfileId::new("test123").is_ok());
    }

    #[test]
    fn test_profile_id_invalid() {
        assert!(ProfileId::new("").is_err());
        assert!(ProfileId::new("has spaces").is_err());
        assert!(ProfileId::new("has/slash").is_err());
        assert!(ProfileId::new("has.dot").is_err());
        assert!(ProfileId::new(&"a".repeat(65)).is_err());
    }

    #[test]
    fn test_profile_id_display() {
        let id = ProfileId::new("my-profile").unwrap();
        assert_eq!(id.to_string(), "my-profile");
        assert_eq!(id.as_str(), "my-profile");
    }

    #[test]
    fn test_profile_paths() {
        let id = ProfileId::new("test-profile").unwrap();
        let dirs = ProjectDirs::from("org", "clareon", "clareon").unwrap();
        let profile = Profile::new(id, &dirs);

        assert!(
            profile
                .config_path
                .to_str()
                .unwrap()
                .contains("profiles/test-profile/config.json")
        );
        assert!(
            profile
                .database_path
                .to_str()
                .unwrap()
                .contains("profiles/test-profile/clareon.db")
        );
        assert!(
            profile
                .database_url
                .contains("profiles/test-profile/clareon.db")
        );
        assert!(
            profile
                .workspace_cache_dir
                .to_str()
                .unwrap()
                .contains("profiles/test-profile/conversations")
        );
        assert!(
            profile
                .shared_cache_dir
                .to_str()
                .unwrap()
                .contains("profiles/test-profile/shared")
        );
    }
}
