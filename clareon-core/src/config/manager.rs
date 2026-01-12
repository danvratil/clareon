// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration manager singleton

use std::sync::{Arc, Mutex, OnceLock};

use super::settings::Config;
use crate::error::Result;

/// Global configuration manager singleton
///
/// Provides thread-safe access to application configuration.
/// All application code should use `ConfigManager::get()` to access configuration
/// instead of passing `Config` instances around.
///
/// # Example
///
/// ```no_run
/// use clareon_core::ConfigManager;
///
/// let config = ConfigManager::get().config();
/// println!("Default backend: {:?}", config.default_backend);
/// ```
pub struct ConfigManager {
    config: Arc<Mutex<Config>>,
}

impl ConfigManager {
    /// Get the global ConfigManager singleton instance
    ///
    /// On first access, loads configuration from the default location.
    /// Panics if config cannot be loaded (this is intentional - app cannot run without config).
    pub fn get() -> &'static ConfigManager {
        static INSTANCE: OnceLock<ConfigManager> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let config = Config::load().expect("Failed to load configuration");
            ConfigManager {
                config: Arc::new(Mutex::new(config)),
            }
        })
    }

    /// Get a clone of the current configuration
    ///
    /// Returns a snapshot of the current configuration. Changes to the returned
    /// Config will not affect the global configuration.
    pub fn config(&self) -> Config {
        self.config.lock().expect("Config mutex poisoned").clone()
    }

    /// Update configuration by applying a function
    ///
    /// The provided function receives a mutable reference to the config.
    /// Changes are kept in memory only - call `save()` to persist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use clareon_core::ConfigManager;
    ///
    /// ConfigManager::get().update_config(|config| {
    ///     config.default_backend = clareon_core::config::Backend::Anthropic;
    /// }).unwrap();
    /// ```
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
    /// Writes the current in-memory configuration to the default config file.
    pub fn save(&self) -> Result<()> {
        let config = self.config.lock().expect("Config mutex poisoned");
        config.save().map(|_| ())
    }

    /// Reload configuration from disk
    ///
    /// Discards any in-memory changes and reloads from the config file.
    pub fn reload(&self) -> Result<()> {
        let config = Config::load()?;
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

    #[test]
    fn test_config_manager_singleton() {
        // Get the singleton twice and verify it's the same instance
        let manager1 = ConfigManager::get();
        let manager2 = ConfigManager::get();

        // They should point to the same memory
        assert!(std::ptr::eq(manager1, manager2));
    }

    #[test]
    fn test_config_clone() {
        let config1 = ConfigManager::get().config();
        let config2 = ConfigManager::get().config();

        // Configs should be equal but not the same instance
        assert_eq!(config1.default_backend, config2.default_backend);
    }

    #[test]
    fn test_update_config() {
        use crate::config::Backend;

        let original = ConfigManager::get().config();

        ConfigManager::get()
            .update_config(|config| {
                config.default_backend = Backend::Anthropic;
            })
            .unwrap();

        let updated = ConfigManager::get().config();
        assert_eq!(updated.default_backend, Backend::Anthropic);

        // Restore original
        ConfigManager::get().replace_config(original);
    }
}
