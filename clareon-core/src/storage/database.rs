// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! SQLite database connection and initialization

use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use tracing::{debug, info};

use crate::error::Result;

/// Storage layer for persisting conversations and messages
pub struct Storage {
    pub(super) pool: Pool<Sqlite>,
}

impl Storage {
    /// Create a new storage instance and initialize the database
    ///
    /// # Arguments
    /// * `database_url` - SQLite database URL (e.g., "sqlite:///path/to/db.sqlite" or "sqlite::memory:")
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to database: {}", database_url);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let storage = Self { pool };
        storage.run_migrations().await?;

        Ok(storage)
    }

    /// Create an in-memory storage instance (useful for testing)
    pub async fn in_memory() -> Result<Self> {
        Self::new("sqlite::memory:").await
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<()> {
        debug!("Running database migrations");

        sqlx::migrate!("./migrations").run(&self.pool).await?;

        info!("Database migrations completed");
        Ok(())
    }
}
