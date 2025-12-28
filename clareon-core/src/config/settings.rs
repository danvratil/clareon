// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration settings

use std::collections::HashMap;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{ConfigError, Result};

/// Main configuration struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default backend to use (bedrock or anthropic)
    #[serde(default = "default_backend")]
    pub default_backend: String,

    /// Default model to use
    #[serde(default = "default_model")]
    pub default_model: String,

    /// Backend-specific configuration
    #[serde(default)]
    pub backends: BackendsConfig,

    /// UI configuration
    #[serde(default)]
    pub ui: UiConfig,

    /// System prompt configuration
    #[serde(default)]
    pub system_prompt: SystemPromptConfig,

    /// Model configuration
    #[serde(default)]
    pub models: ModelsConfig,

    /// Tool execution configuration
    #[serde(default)]
    pub tools: ToolsConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

fn default_backend() -> String {
    "bedrock".to_string()
}

fn default_model() -> String {
    "anthropic.claude-sonnet-4-20250514-v1:0".to_string()
}

/// Backend-specific configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendsConfig {
    /// AWS Bedrock configuration
    #[serde(default)]
    pub bedrock: BedrockConfig,

    /// Anthropic API configuration
    #[serde(default)]
    pub anthropic: AnthropicConfig,
}

/// AWS Bedrock backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConfig {
    /// AWS region to use
    #[serde(default = "default_region")]
    pub region: String,

    /// AWS profile to use (None = default credential chain)
    pub profile: Option<String>,

    /// Enable prompt caching (default: true)
    /// Caches system prompts to reduce costs and latency on subsequent calls
    /// Only works with Claude Sonnet 3.5+, Opus 4, and Nova models
    #[serde(default = "default_true")]
    pub enable_prompt_caching: bool,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

impl Default for BedrockConfig {
    fn default() -> Self {
        Self {
            region: default_region(),
            profile: None,
            enable_prompt_caching: true,
        }
    }
}

/// Anthropic API backend configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnthropicConfig {
    /// Whether the API key is stored in the system keyring
    #[serde(default = "default_true")]
    pub api_key_in_keyring: bool,

    /// Base URL for the API (for custom endpoints)
    pub base_url: Option<String>,
}

fn default_true() -> bool {
    true
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Color theme
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Whether to enable streaming responses
    #[serde(default = "default_true")]
    pub streaming: bool,
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            streaming: true,
        }
    }
}

/// System prompt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptConfig {
    /// Whether to use the default system prompt
    #[serde(default = "default_true")]
    pub use_default: bool,

    /// Custom system prompt (overrides default if use_default is false)
    pub custom_prompt: Option<String>,

    /// Additional instructions appended to the system prompt
    pub custom_instructions: Option<String>,
}

impl Default for SystemPromptConfig {
    fn default() -> Self {
        Self {
            use_default: true,
            custom_prompt: None,
            custom_instructions: None,
        }
    }
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsConfig {
    /// Model to use for title generation
    #[serde(default = "default_title_model")]
    pub title_generation: String,
}

fn default_title_model() -> String {
    "anthropic.claude-3-5-haiku-20241022-v1:0".to_string()
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            title_generation: default_title_model(),
        }
    }
}

/// Tool execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// Whether tools are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Sandbox mode
    #[serde(default)]
    pub sandbox_mode: SandboxModeConfig,

    /// Default timeout for tool execution (seconds)
    #[serde(default = "default_tool_timeout")]
    pub default_timeout: u64,

    /// Whether to automatically execute tools (vs requiring approval)
    #[serde(default = "default_true")]
    pub auto_execute: bool,

    /// Workspace retention policy (days to keep inactive workspaces)
    #[serde(default = "default_workspace_retention_days")]
    pub workspace_retention_days: u64,

    /// Maximum workspace size per conversation (MB)
    #[serde(default = "default_max_workspace_size_mb")]
    pub max_workspace_size_mb: u64,

    /// Maximum file upload size (MB)
    #[serde(default = "default_max_upload_size_mb")]
    pub max_upload_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxModeConfig {
    /// No sandboxing (development only)
    None,
    /// Basic sandboxing
    Basic,
    /// Strict sandboxing (recommended)
    Strict,
}

impl Default for SandboxModeConfig {
    fn default() -> Self {
        Self::Strict
    }
}

fn default_tool_timeout() -> u64 {
    30 // seconds
}

fn default_workspace_retention_days() -> u64 {
    30 // Keep workspaces for 30 days
}

fn default_max_workspace_size_mb() -> u64 {
    500 // 500MB per conversation
}

fn default_max_upload_size_mb() -> u64 {
    100 // 100MB per file upload
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sandbox_mode: SandboxModeConfig::default(),
            default_timeout: default_tool_timeout(),
            auto_execute: true,
            workspace_retention_days: default_workspace_retention_days(),
            max_workspace_size_mb: default_max_workspace_size_mb(),
            max_upload_size_mb: default_max_upload_size_mb(),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Global log level (default for all modules)
    #[serde(default = "default_log_level")]
    pub global: String,

    /// Per-module/crate log level overrides
    #[serde(default = "default_module_levels")]
    pub modules: HashMap<String, String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_module_levels() -> HashMap<String, String> {
    let mut levels = HashMap::new();
    // Default overrides for verbose crates
    levels.insert("clareon".to_string(), "debug".to_string());
    levels.insert("clareon_core".to_string(), "debug".to_string());
    levels.insert("clareon_cli".to_string(), "debug".to_string());
    levels.insert("aws_sdk".to_string(), "warn".to_string());
    levels.insert("aws_smithy".to_string(), "warn".to_string());
    levels.insert("aws_config".to_string(), "warn".to_string());
    levels.insert("sqlx".to_string(), "warn".to_string());
    levels.insert("hyper".to_string(), "warn".to_string());
    levels.insert("h2".to_string(), "warn".to_string());
    levels
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            global: default_log_level(),
            modules: default_module_levels(),
        }
    }
}

impl LoggingConfig {
    /// Build an EnvFilter directive string from this configuration
    ///
    /// Returns a directive string like: "clareon=debug,aws_sdk=warn,info"
    /// suitable for passing to EnvFilter::new()
    pub fn build_filter_directive(&self) -> String {
        let mut parts = Vec::new();

        // Add module-specific directives
        for (module, level) in &self.modules {
            parts.push(format!("{}={}", module, level));
        }

        // Add global level at the end (acts as default)
        parts.push(self.global.clone());

        parts.join(",")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_backend: default_backend(),
            default_model: default_model(),
            backends: BackendsConfig::default(),
            ui: UiConfig::default(),
            system_prompt: SystemPromptConfig::default(),
            models: ModelsConfig::default(),
            tools: ToolsConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            info!("Config file not found, using defaults");
            return Ok(Self::default());
        }

        Self::load_from(&config_path)
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &PathBuf) -> Result<Self> {
        debug!("Loading config from: {:?}", path);

        let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        let config: Config = serde_json::from_str(&content).map_err(ConfigError::Parse)?;

        Ok(config)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<Self> {
        let config_path = Self::config_path()?;
        self.save_to(&config_path)?;
        Ok(self.clone())
    }

    /// Save configuration to a specific path
    pub fn save_to(&self, path: &PathBuf) -> Result<()> {
        debug!("Saving config to: {:?}", path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }

        let content = serde_json::to_string_pretty(self).map_err(ConfigError::Parse)?;
        std::fs::write(path, content).map_err(ConfigError::Io)?;

        info!("Config saved to: {:?}", path);
        Ok(())
    }

    /// Get the default config file path
    pub fn config_path() -> Result<PathBuf> {
        let dirs = Self::project_dirs()?;
        Ok(dirs.config_dir().join("config.json"))
    }

    /// Get the default database file path
    pub fn database_path() -> Result<PathBuf> {
        let dirs = Self::project_dirs()?;
        let data_dir = dirs.data_dir();
        std::fs::create_dir_all(data_dir).map_err(ConfigError::Io)?;
        Ok(data_dir.join("clareon.db"))
    }

    /// Get the database URL for SQLite
    pub fn database_url() -> Result<String> {
        let path = Self::database_path()?;
        Ok(format!("sqlite://{}?mode=rwc", path.display()))
    }

    /// Get the cache root directory
    pub fn cache_root() -> Result<PathBuf> {
        let dirs = Self::project_dirs()?;
        let cache_dir = dirs.cache_dir();
        std::fs::create_dir_all(cache_dir).map_err(ConfigError::Io)?;
        Ok(cache_dir.to_path_buf())
    }

    /// Get the workspace cache directory
    pub fn workspace_cache_dir() -> Result<PathBuf> {
        let cache_root = Self::cache_root()?;
        let workspace_dir = cache_root.join("conversations");
        std::fs::create_dir_all(&workspace_dir).map_err(ConfigError::Io)?;
        Ok(workspace_dir)
    }

    /// Get the shared cache directory
    pub fn shared_cache_dir() -> Result<PathBuf> {
        let cache_root = Self::cache_root()?;
        let shared_dir = cache_root.join("shared");
        std::fs::create_dir_all(&shared_dir).map_err(ConfigError::Io)?;
        Ok(shared_dir)
    }

    /// Get the project directories
    fn project_dirs() -> Result<ProjectDirs> {
        ProjectDirs::from("org", "clareon", "clareon").ok_or_else(|| {
            ConfigError::Invalid("Could not determine home directory".to_string()).into()
        })
    }

    /// Get the default system prompt
    pub fn default_system_prompt() -> &'static str {
        include_str!("../../resources/system_prompt.txt")
    }

    /// Get the effective system prompt based on configuration
    pub fn get_system_prompt(&self) -> String {
        let base_prompt = if self.system_prompt.use_default {
            if let Some(custom) = &self.system_prompt.custom_prompt {
                custom.clone()
            } else {
                Self::default_system_prompt().to_string()
            }
        } else {
            self.system_prompt.custom_prompt.clone().unwrap_or_default()
        };

        // Append custom instructions if present
        if let Some(instructions) = &self.system_prompt.custom_instructions {
            format!("{}\n\n{}", base_prompt, instructions)
        } else {
            base_prompt
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_backend, "bedrock");
        assert!(config.ui.streaming);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.default_backend, loaded.default_backend);
    }

    #[test]
    fn test_config_deserialization_with_defaults() {
        // Test that missing fields use defaults
        let json = r#"{"default_backend": "anthropic"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.default_backend, "anthropic");
        assert!(config.ui.streaming); // Should use default
    }

    #[test]
    fn test_logging_config_default() {
        let logging = LoggingConfig::default();
        assert_eq!(logging.global, "info");
        assert_eq!(logging.modules.get("clareon"), Some(&"debug".to_string()));
        assert_eq!(logging.modules.get("aws_sdk"), Some(&"warn".to_string()));
    }

    #[test]
    fn test_logging_filter_directive() {
        let logging = LoggingConfig::default();
        let directive = logging.build_filter_directive();

        // Should contain module directives
        assert!(directive.contains("clareon=debug"));
        assert!(directive.contains("aws_sdk=warn"));
        assert!(directive.contains("sqlx=warn"));

        // Should end with global level
        assert!(directive.ends_with("info"));
    }

    #[test]
    fn test_logging_config_custom() {
        let mut modules = HashMap::new();
        modules.insert("my_crate".to_string(), "trace".to_string());
        modules.insert("other_crate".to_string(), "error".to_string());

        let logging = LoggingConfig {
            global: "warn".to_string(),
            modules,
        };

        let directive = logging.build_filter_directive();
        assert!(directive.contains("my_crate=trace"));
        assert!(directive.contains("other_crate=error"));
        assert!(directive.ends_with("warn"));
    }

    #[test]
    fn test_logging_config_serialization() {
        let json = r#"{
            "global": "debug",
            "modules": {
                "my_app": "trace",
                "aws_sdk": "warn"
            }
        }"#;

        let logging: LoggingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(logging.global, "debug");
        assert_eq!(logging.modules.get("my_app"), Some(&"trace".to_string()));
        assert_eq!(logging.modules.get("aws_sdk"), Some(&"warn".to_string()));
    }
}
