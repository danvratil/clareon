// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Mutex, OnceLock};
use tokio::runtime::Runtime;

use clareon_core::ConfigManager;
use clareon_qt::{QApplicationExt, QIcon};
use cxx_qt_lib::{QQmlApplicationEngine, QString, QUrl};
use cxx_qt_lib_extras::QApplication;

use service::ClareonService;
use unique_app::{try_become_unique, UniqueResult};

use crate::service::Command;

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

// Global service instance
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

fn main() {
    // Try to become the unique instance first, before initializing anything else
    // This ensures we exit quickly if another instance is already running
    let unique_handle = match try_become_unique() {
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

    // Initialize ConfigManager singleton (loads config on first access)
    let config = ConfigManager::get().config();

    // Initialize logging
    let _guard =
        clareon_core::logging::init_logging(&config).expect("Failed to initialize logging");
    logging::init_qt_logging();

    // Create the service (will use ConfigManager internally)
    let service = ClareonService::new().expect("Failed to create service");

    // Get the service handle before storing service
    let handle = service.handle();

    // Store service in global
    SERVICE.set(Mutex::new(service)).ok();

   // Spawn the unique server listener in the background if we have one
    if let Some(unique_server) = unique_handle {
        get_runtime().spawn(async move {
            unique_server.listen(|activation| {
                tracing::info!("Received activation from another instance: {:?}", activation);
                let handle = SERVICE.get().expect("Service not initialized").lock().expect("Service lock poisoned").handle();
                let _ = handle.send(Command::ActivateMainWindow);
            }).await;
        });
    }


    // Initialize Qt - pass handle
    qt::init_service_handle(handle);

    let mut app = QApplication::new();
    app.pin_mut()
        .set_application_name(&QString::from("Clareon"));
    app.pin_mut()
        .set_organization_domain(&QString::from("clareon.cc"));

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
