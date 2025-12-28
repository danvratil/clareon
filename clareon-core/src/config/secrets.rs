// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Secure secret storage using the system keyring

use secret_service::{EncryptionType, SecretService};
use tracing::{debug, info};

use crate::error::{ConfigError, Result};

/// Secret store for secure credential storage
pub struct SecretStore {
    service: SecretService<'static>,
}

/// Anthropic API key
pub const ANTHROPIC_API_KEY: &str = "anthropic-api-key";

impl SecretStore {
    /// Create a new secret store
    pub async fn new() -> Result<Self> {
        debug!("Connecting to secret service");

        let service = SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        Ok(Self { service })
    }

    /// Store a secret in the keyring
    pub async fn store_secret(&self, key: &str, value: &str) -> Result<()> {
        info!("Storing secret: {}", key);

        let collection = self
            .service
            .get_default_collection()
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        // Unlock collection if needed
        if collection.is_locked().await.unwrap_or(true) {
            collection
                .unlock()
                .await
                .map_err(|e| ConfigError::SecretService(e.to_string()))?;
        }

        // Create attributes for the secret
        let attributes = vec![("application", "clareon"), ("key", key)];

        // Create or update the secret
        collection
            .create_item(
                &format!("Clareon: {}", key),
                attributes.into_iter().collect(),
                value.as_bytes(),
                true, // Replace existing
                "text/plain",
            )
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        Ok(())
    }

    /// Retrieve a secret from the keyring
    pub async fn get_secret(&self, key: &str) -> Result<String> {
        debug!("Retrieving secret: {}", key);

        let collection = self
            .service
            .get_default_collection()
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        // Unlock collection if needed
        if collection.is_locked().await.unwrap_or(true) {
            collection
                .unlock()
                .await
                .map_err(|e| ConfigError::SecretService(e.to_string()))?;
        }

        // Search for the secret
        let attributes = vec![("application", "clareon"), ("key", key)];

        let items = collection
            .search_items(attributes.into_iter().collect())
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        let item = items
            .first()
            .ok_or_else(|| ConfigError::SecretNotFound(key.to_string()))?;

        let secret = item
            .get_secret()
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        String::from_utf8(secret).map_err(|e| ConfigError::SecretService(e.to_string()).into())
    }

    /// Delete a secret from the keyring
    pub async fn delete_secret(&self, key: &str) -> Result<()> {
        info!("Deleting secret: {}", key);

        let collection = self
            .service
            .get_default_collection()
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        // Unlock collection if needed
        if collection.is_locked().await.unwrap_or(true) {
            collection
                .unlock()
                .await
                .map_err(|e| ConfigError::SecretService(e.to_string()))?;
        }

        // Search for the secret
        let attributes = vec![("application", "clareon"), ("key", key)];

        let items = collection
            .search_items(attributes.into_iter().collect())
            .await
            .map_err(|e| ConfigError::SecretService(e.to_string()))?;

        for item in items {
            item.delete()
                .await
                .map_err(|e| ConfigError::SecretService(e.to_string()))?;
        }

        Ok(())
    }

    /// Check if a secret exists
    pub async fn has_secret(&self, key: &str) -> bool {
        self.get_secret(key).await.is_ok()
    }
}

// Note: We can't easily unit test secret-service as it requires
// a running D-Bus session and secret service daemon.
// Integration tests would be more appropriate here.
