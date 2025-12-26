//! Configuration settings

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
}

fn default_region() -> String {
    "us-east-1".to_string()
}

impl Default for BedrockConfig {
    fn default() -> Self {
        Self {
            region: default_region(),
            profile: None,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemPromptConfig {
    /// Whether to use the default system prompt
    #[serde(default = "default_true")]
    pub use_default: bool,

    /// Custom system prompt (overrides default if use_default is false)
    pub custom_prompt: Option<String>,

    /// Additional instructions appended to the system prompt
    pub custom_instructions: Option<String>,
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsConfig {
    /// Model to use for title generation
    #[serde(default = "default_title_model")]
    pub title_generation: String,
}

fn default_title_model() -> String {
    "anthropic.claude-3-haiku-20240307-v1:0".to_string()
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            title_generation: default_title_model(),
        }
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
            self.system_prompt
                .custom_prompt
                .clone()
                .unwrap_or_default()
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
}
