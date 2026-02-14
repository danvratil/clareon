// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration manager for a specific profile

use std::sync::{Arc, Mutex};

use super::profile::Profile;
use super::settings::Config;
use crate::error::Result;

/// Configuration manager bound to a specific profile
///
/// Provides thread-safe access to configuration for a single profile.
/// Each profile gets its own ConfigManager instance.
///
/// # Example
///
/// ```no_run
/// use clareon_core::config::{ConfigManager, ProfileId, ProfileManager};
///
/// let profile_id = ProfileId::new("default").unwrap();
/// let profile = ProfileManager::get_or_create_profile(&profile_id).unwrap();
/// let manager = ConfigManager::new(profile).unwrap();
/// let config = manager.config();
/// println!("Default backend: {:?}", config.default_backend);
/// ```
pub struct ConfigManager {
    profile: Profile,
    config: Arc<Mutex<Config>>,
}

impl ConfigManager {
    /// Create a new ConfigManager for a specific profile
    ///
    /// Loads configuration from the profile's config path. If the config file
    /// doesn't exist, uses default configuration.
    pub fn new(profile: Profile) -> Result<Self> {
        let config = Config::load_from(&profile.config_path)?;
        Ok(Self {
            profile,
            config: Arc::new(Mutex::new(config)),
        })
    }

    /// Get a reference to the profile
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Get a clone of the current configuration
    ///
    /// Returns a snapshot of the current configuration. Changes to the returned
    /// Config will not affect the managed configuration.
    pub fn config(&self) -> Config {
        self.config.lock().expect("Config mutex poisoned").clone()
    }

    /// Update configuration by applying a function
    ///
    /// The provided function receives a mutable reference to the config.
    /// Changes are kept in memory only - call `save()` to persist.
    pub fn update_config<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Config),
    {
        let mut config = self.config.lock().expect("Config mutex poisoned");
        f(&mut config);
        Ok(())
    }

    /// Save the current configuration to disk
    ///
    /// Writes the current in-memory configuration to the profile's config file.
    pub fn save(&self) -> Result<()> {
        let config = self.config.lock().expect("Config mutex poisoned");
        config.save_to(&self.profile.config_path)
    }

    /// Reload configuration from disk
    ///
    /// Discards any in-memory changes and reloads from the profile's config file.
    pub fn reload(&self) -> Result<()> {
        let config = Config::load_from(&self.profile.config_path)?;
        *self.config.lock().expect("Config mutex poisoned") = config;
        Ok(())
    }

    /// Replace the entire configuration
    ///
    /// Replaces the in-memory configuration with the provided config.
    /// Does not persist to disk - call `save()` to persist.
    pub fn replace_config(&self, new_config: Config) {
        *self.config.lock().expect("Config mutex poisoned") = new_config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Backend;

    /// Helper to create a test ConfigManager with a temporary profile
    fn create_test_manager() -> ConfigManager {
        let profile = Profile::new_for_test("test-config-manager");
        ConfigManager {
            profile,
            config: Arc::new(Mutex::new(Config::default())),
        }
    }

    #[test]
    fn test_config_clone() {
        let manager = create_test_manager();
        let config1 = manager.config();
        let config2 = manager.config();

        // Configs should be equal but not the same instance
        assert_eq!(config1.default_backend, config2.default_backend);
    }

    #[test]
    fn test_update_config() {
        let manager = create_test_manager();
        let original = manager.config();

        manager
            .update_config(|config| {
                config.default_backend = Backend::Anthropic;
            })
            .unwrap();

        let updated = manager.config();
        assert_eq!(updated.default_backend, Backend::Anthropic);

        // Verify original was Bedrock (default)
        assert_eq!(original.default_backend, Backend::Bedrock);
    }

    #[test]
    fn test_replace_config() {
        let manager = create_test_manager();

        // Create a custom config
        let custom_config = Config {
            default_backend: Backend::Anthropic,
            default_model: "custom-model".to_string(),
            ..Default::default()
        };

        // Replace config
        manager.replace_config(custom_config);

        // Verify it was replaced
        let current = manager.config();
        assert_eq!(current.default_backend, Backend::Anthropic);
        assert_eq!(current.default_model, "custom-model");
    }

    #[test]
    fn test_profile_access() {
        let manager = create_test_manager();
        assert_eq!(manager.profile().id.as_str(), "test-config-manager");
    }
}
