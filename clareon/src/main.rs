use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

pub mod app_controller;
pub mod models;
pub mod mock;

fn main() {
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
