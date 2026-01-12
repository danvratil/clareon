#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("src/cpp/qml.hpp");

        #[cxx_name = "registerClareonQmlTypes"]
        fn register_clareon_qml_types();
    }
}

pub use ffi::register_clareon_qml_types;
