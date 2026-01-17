// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Mutex, OnceLock};
use tokio::runtime::Runtime;

use clareon_core::ConfigManager;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

use service::ClareonService;

pub mod config_manager;
pub mod conversation_list_model;
pub mod logging;
pub mod message_list_model;
pub mod qml;
pub mod qt;
pub mod search_result_model;
pub mod service;
pub mod service_controller;

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

    // Initialize Qt - pass handle
    qt::init_service_handle(handle);

    let mut app = QGuiApplication::new();
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
}
