// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::pin::Pin;

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
        type ProvidersConfigCpp;
        type OpenAiBackendConfigCpp;
        type BedrockConfigCpp;
        type AnthropicConfigCpp;
        type SystemPromptConfigCpp;
        type ModelsConfigCpp;
        type ToolsConfigCpp;
        type McpConfigCpp;
        type McpServerConfigCpp;
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
        fn saveConfig(self: Pin<&mut ConfigManager>, config: *mut ConfigCpp) -> bool;

        /// Reload configuration from disk, discarding any unsaved changes
        #[qinvokable]
        fn reload(self: Pin<&mut ConfigManager>) -> bool;

        /// Emitted after configuration is successfully saved or reloaded from disk.
        /// QML should re-fetch via getConfig() to pick up the new values.
        #[qsignal]
        #[rust_name = "config_changed"]
        fn configChanged(self: Pin<&mut ConfigManager>);
    }
}

/// Rust implementation of ConfigManagerQt
#[derive(Default)]
pub struct ConfigManagerRust {}

impl ffi::ConfigManager {
    /// Get the current configuration as ConfigCpp
    fn get_config(&self) -> *mut ffi::ConfigCpp {
        // Get config from Rust ConfigManager
        let config = clareon_core::ConfigManager::get().config();

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
    fn save_config(self: Pin<&mut Self>, config: *mut ffi::ConfigCpp) -> bool {
        if config.is_null() {
            tracing::error!("Received null ConfigCpp pointer");
            return false;
        }

        // Convert ConfigCpp to JSON using C++ bridge
        let json = ffi::config_to_json_string(unsafe { &*config });

        // Deserialize to Rust Config
        match serde_json::from_str::<clareon_core::config::Config>(&json) {
            Ok(new_config) => {
                clareon_core::ConfigManager::get().replace_config(new_config);
                match clareon_core::ConfigManager::get().save() {
                    Ok(()) => {
                        tracing::info!("Configuration saved successfully");
                        // Notify the service worker to reload with the new config
                        if let Some(handle) = crate::service_controller::try_get_service_handle() {
                            let _ = handle.send(crate::service::Command::ReloadConfig);
                        }
                        // Notify QML bindings (TitlePage model label, etc.)
                        self.config_changed();
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
    fn reload(self: Pin<&mut Self>) -> bool {
        match clareon_core::ConfigManager::get().reload() {
            Ok(()) => {
                tracing::info!("Configuration reloaded successfully");
                self.config_changed();
                true
            }
            Err(e) => {
                tracing::error!("Failed to reload configuration: {}", e);
                false
            }
        }
    }
}
