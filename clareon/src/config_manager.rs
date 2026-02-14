// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, OnceLock};

/// Staged ConfigManager instance, set once before QML initialization.
static STAGED_CONFIG_MANAGER: OnceLock<Arc<clareon_core::ConfigManager>> = OnceLock::new();

/// Stage a ConfigManager for the QML ConfigManager bridge to use
pub fn stage_config_manager(manager: Arc<clareon_core::ConfigManager>) {
    STAGED_CONFIG_MANAGER.set(manager).ok();
}

/// Get the staged ConfigManager
fn get_config_manager() -> &'static Arc<clareon_core::ConfigManager> {
    STAGED_CONFIG_MANAGER
        .get()
        .expect("ConfigManager not staged before QML initialization")
}

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "C++" {
        include!("config_generated.h");

        // Q_GADGET config types from generated code
        type ConfigCpp;
        type BackendsConfigCpp;
        type BedrockConfigCpp;
        type AnthropicConfigCpp;
        type SystemPromptConfigCpp;
        type ModelsConfigCpp;
        type ToolsConfigCpp;
        type LoggingConfigCpp;
    }

    unsafe extern "C++" {
        include!("config_bridge.hpp");

        #[namespace = "config_bridge"]
        #[rust_name = "create_config_from_json"]
        fn createConfigFromJson(json_string: &str) -> UniquePtr<ConfigCpp>;

        #[namespace = "config_bridge"]
        #[rust_name = "config_to_json_string"]
        fn configToJson(config: &ConfigCpp) -> String;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type ConfigManager = super::ConfigManagerRust;

        /// Get the current configuration as ConfigCpp object
        #[qinvokable]
        #[rust_name = "get_config"]
        fn getConfig(self: &ConfigManager) -> *mut ConfigCpp;

        /// Save configuration from ConfigCpp object
        /// Returns true on success, false on error
        #[qinvokable]
        #[rust_name = "save_config"]
        fn saveConfig(self: &ConfigManager, config: *mut ConfigCpp) -> bool;

        /// Reload configuration from disk, discarding any unsaved changes
        #[qinvokable]
        fn reload(self: &ConfigManager) -> bool;
    }
}

/// Rust implementation of ConfigManagerQt
#[derive(Default)]
pub struct ConfigManagerRust {}

impl ffi::ConfigManager {
    /// Get the current configuration as ConfigCpp
    fn get_config(&self) -> *mut ffi::ConfigCpp {
        let config = get_config_manager().config();

        // Serialize to JSON
        let json = match serde_json::to_string(&config) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to serialize config to JSON: {}", e);
                "{}".to_string() // Return empty JSON object on error
            }
        };

        // Convert JSON to ConfigCpp using C++ bridge
        ffi::create_config_from_json(&json).into_raw()
    }

    /// Save configuration from ConfigCpp
    fn save_config(&self, config: *mut ffi::ConfigCpp) -> bool {
        if config.is_null() {
            tracing::error!("Received null ConfigCpp pointer");
            return false;
        }

        // Convert ConfigCpp to JSON using C++ bridge
        let json = ffi::config_to_json_string(unsafe { &*config });

        // Deserialize to Rust Config
        match serde_json::from_str::<clareon_core::config::Config>(&json) {
            Ok(new_config) => {
                get_config_manager().replace_config(new_config);
                match get_config_manager().save() {
                    Ok(()) => {
                        tracing::info!("Configuration saved successfully");
                        true
                    }
                    Err(e) => {
                        tracing::error!("Failed to save configuration: {}", e);
                        false
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to deserialize config from JSON: {}", e);
                false
            }
        }
    }

    /// Reload configuration from disk
    fn reload(&self) -> bool {
        match get_config_manager().reload() {
            Ok(()) => {
                tracing::info!("Configuration reloaded successfully");
                true
            }
            Err(e) => {
                tracing::error!("Failed to reload configuration: {}", e);
                false
            }
        }
    }
}
