// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use clareon_core::backend::{AnthropicBackend, BedrockBackend, LlmBackend};
use clareon_core::{Config, ConversationManager, Storage};
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};
use std::sync::Arc;

pub mod app_controller;
pub mod mock;
pub mod models;

fn main() {
    // Initialize tokio runtime
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime"),
    );

    // Initialize core components (blocking on the runtime)
    let manager = runtime.block_on(async {
        // Load configuration
        let config = Config::load().expect("Failed to load config");

        // Get database URL and create storage
        let db_url = Config::database_url().expect("Failed to get database URL");
        let storage = Storage::new(&db_url)
            .await
            .expect("Failed to initialize storage");

        // Create backend based on configuration
        let backend = create_backend_from_config(&config)
            .await
            .expect("Failed to create backend");

        // Create title generation backend (same as main backend for now)
        let title_backend = create_backend_from_config(&config)
            .await
            .expect("Failed to create title backend");

        Arc::new(ConversationManager::new(
            storage,
            backend,
            title_backend,
            config,
        ))
    });

    // Initialize global singletons for app_controller and models
    app_controller::init_runtime(runtime.clone(), manager.clone());
    models::init_runtime(runtime.clone(), manager.clone());

    // Initialize the Qt application
    let mut app = QGuiApplication::new();

    // Create the QML engine
    let mut engine = QQmlApplicationEngine::new();

    // Load the main QML file from the QML module
    let qml_url = QUrl::from("qrc:/qt/qml/cz/dvratil/clareon/qml/main.qml");

    if let Some(engine_pin) = engine.as_mut() {
        engine_pin.load(&qml_url);
    }

    // Check if QML loaded successfully
    // Note: root_objects check may not be available in this version
    // The application will still run and show errors if QML fails to load

    // Run the application event loop
    if let Some(app_pin) = app.as_mut() {
        app_pin.exec();
    }
}

/// Create an LLM backend based on the configuration
async fn create_backend_from_config(config: &Config) -> Result<Arc<dyn LlmBackend>, String> {
    match config.default_backend.as_str() {
        "anthropic" => {
            // For now, only support API key from environment variable
            // Keyring support can be added later
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

            Ok(Arc::new(AnthropicBackend::new(api_key)))
        }
        "bedrock" => {
            let region = &config.backends.bedrock.region;
            let profile = config.backends.bedrock.profile.as_deref();

            let backend = if let Some(profile) = profile {
                BedrockBackend::with_profile(region.clone(), profile.to_string())
                    .await
                    .map_err(|e| format!("Failed to create Bedrock backend: {}", e))?
            } else {
                BedrockBackend::new(region.clone())
                    .await
                    .map_err(|e| format!("Failed to create Bedrock backend: {}", e))?
            };

            Ok(Arc::new(backend))
        }
        other => Err(format!("Unknown backend: {}", other)),
    }
}
