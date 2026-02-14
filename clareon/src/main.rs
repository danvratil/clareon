// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex, OnceLock};
use tokio::runtime::Runtime;

use clap::Parser;
use clareon_core::{ConfigManager, ProfileId, ProfileManager};
use clareon_qt::{QApplicationExt, QIcon};
use cxx_qt_lib::{QQmlApplicationEngine, QString, QUrl};
use cxx_qt_lib_extras::QApplication;

use service::ClareonService;
use unique_app::{UniqueResult, try_become_unique};

use crate::service::Command;

pub mod artifact_list_model;
pub mod config_manager;
pub mod conversation_list_model;
pub mod logging;
pub mod message_list_model;
pub mod qml;
pub mod qt;
pub mod search_result_model;
pub mod service;
pub mod service_controller;
pub mod unique_app;

// Global service instance (needed for runtime access from Qt callbacks)
static SERVICE: OnceLock<Mutex<ClareonService>> = OnceLock::new();

/// Get the tokio runtime
pub fn get_runtime() -> &'static Runtime {
    // This is a workaround to get a static reference to the runtime
    // We rely on the fact that the SERVICE is never dropped
    unsafe {
        let service_ptr = SERVICE.get().unwrap().lock().unwrap().runtime() as *const Runtime;
        &*service_ptr
    }
}

#[derive(Parser, Debug)]
#[command(version, about = "Clareon - AI-powered chat application")]
pub struct Args {
    #[arg(short, long, action = clap::ArgAction::SetTrue, help = "Show a quick input window to start a new conversation")]
    pub quick_input: bool,

    /// Profile to use (creates a new profile if it doesn't exist)
    #[arg(long, default_value = "default")]
    pub profile: String,
}

fn main() {
    let args = Args::parse();

    // Try to become the unique instance first, before initializing anything else
    // This ensures we exit quickly if another instance is already running
    let unique_handle = match try_become_unique(Some(args.profile.clone())) {
        Ok(UniqueResult::Primary(server)) => Some(server),
        Ok(UniqueResult::Secondary) => {
            // Another instance is running, activation sent, exit
            eprintln!("Another instance of Clareon is already running, activating it");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("FATAL: Failed to establish unique instance: {}", e);
            std::process::exit(1);
        }
    };

    // If we acquired the unique handle, we can proceed with initialization.

    // Resolve or create the profile
    let profile_id = ProfileId::new(&args.profile).unwrap_or_else(|e| {
        eprintln!("FATAL: Invalid profile name '{}': {}", args.profile, e);
        std::process::exit(1);
    });
    let profile = ProfileManager::get_or_create_profile(&profile_id).unwrap_or_else(|e| {
        eprintln!(
            "FATAL: Failed to initialize profile '{}': {}",
            profile_id, e
        );
        std::process::exit(1);
    });

    // Create ConfigManager for this profile (no longer a singleton)
    let config_manager = Arc::new(ConfigManager::new(profile).unwrap_or_else(|e| {
        eprintln!(
            "FATAL: Failed to load configuration for profile '{}': {}",
            profile_id, e
        );
        std::process::exit(1);
    }));

    // Initialize logging from profile's config
    let config = config_manager.config();
    let _guard =
        clareon_core::logging::init_logging(&config).expect("Failed to initialize logging");
    logging::init_qt_logging();

    tracing::info!("Starting Clareon with profile: {}", profile_id);

    // Create the service with profile-aware config
    let service =
        ClareonService::new(Arc::clone(&config_manager)).expect("Failed to create service");

    // Get the service handle before storing service
    let handle = service.handle();

    // Store service in global (needed for runtime access)
    SERVICE.set(Mutex::new(service)).ok();

    // Spawn the unique server listener in the background if we have one
    if let Some(unique_server) = unique_handle {
        get_runtime().spawn(async move {
            unique_server
                .listen(|activation| {
                    tracing::info!(
                        "Received activation from another instance: {:?}",
                        activation
                    );
                    let handle = SERVICE
                        .get()
                        .expect("Service not initialized")
                        .lock()
                        .expect("Service lock poisoned")
                        .handle();

                    let args = Args::parse_from(activation.args);
                    if args.quick_input {
                        let _ = handle.send(Command::ActivateQuickInput);
                    } else {
                        let _ = handle.send(Command::ActivateMainWindow);
                    }
                })
                .await;
        });
    }

    if args.quick_input {
        let _ = handle.send(Command::ActivateQuickInput);
    }

    // Stage the service handle and config manager for QML singletons
    service_controller::stage_service_handle(handle);
    config_manager::stage_config_manager(config_manager);

    let mut app = QApplication::new();
    app.pin_mut()
        .set_application_name(&QString::from("Clareon"));
    app.pin_mut()
        .set_organization_domain(&QString::from("clareon.cc"));
    app.pin_mut()
        .set_desktop_file_name(&QString::from("cc.clareon"));

    // Set window icon (Qt will automatically select appropriate size)
    let mut icon = QIcon::new();
    icon.add_file(&QString::from(":/clareon-16.png"));
    icon.add_file(&QString::from(":/clareon-32.png"));
    icon.add_file(&QString::from(":/clareon-48.png"));
    icon.add_file(&QString::from(":/clareon-256.png"));
    app.pin_mut().set_window_icon(&icon);

    let mut engine = QQmlApplicationEngine::new();

    qml::register_clareon_qml_types();

    let qml_url = QUrl::from("qrc:/qt/qml/cz/dvratil/clareon/qml/main.qml");
    if let Some(engine) = engine.as_mut() {
        engine.load(&qml_url);
    }

    // Check if QML loaded successfully
    // Note: root_objects check may not be available in this version
    // The application will still run and show errors if QML fails to load

    // Run the application event loop
    if let Some(app) = app.as_mut() {
        app.exec();
    }

    logging::clear_qt_logging();
}
