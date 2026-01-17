use std::{fs, path::PathBuf};

mod generator;
mod moc;
mod parser;

#[derive(Debug, Clone)]
pub struct ConfigStruct {
    pub name: String,
    pub fields: Vec<ConfigField>,
}

#[derive(Debug, Clone)]
pub struct ConfigField {
    pub name: String,
    pub rust_type: String,
    #[allow(dead_code)] // Will be used in Phase 4 for proper handling of Option types
    pub is_optional: bool,
}

pub struct GeneratedConfig {
    pub header_path: PathBuf,
    pub impl_path: PathBuf,
    pub moc_path: PathBuf,
}

pub fn generate_config_cpp() -> GeneratedConfig {
    let root_path = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is not set")
            .as_str(),
    );
    let settings_rs_path = root_path.join("../clareon-core/src/config/settings.rs");

    // Parse config structs and enums from settings.rs
    let (config_structs, config_enums) = parser::parse_config_structs(&settings_rs_path);

    println!("Found {} config structs", config_structs.len());
    for s in &config_structs {
        println!("  - {}: {} fields", s.name, s.fields.len());
    }
    println!(
        "Found {} config enums: {:?}",
        config_enums.len(),
        config_enums
    );

    // Generate C++ code
    let header_code = generator::generate_cpp_header(&config_structs, &config_enums);
    let impl_code = generator::generate_cpp_implementation(&config_structs, &config_enums);

    // Use OUT_DIR for generated files
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));

    let header_path = out_dir.join("config_generated.h");
    let impl_path = out_dir.join("config_generated.cpp");
    let moc_path = out_dir.join("moc_config_generated.cpp");

    // Write generated files
    fs::write(&header_path, header_code).expect("Failed to write config_generated.h");
    fs::write(&impl_path, impl_code).expect("Failed to write config_generated.cpp");

    println!("Generated config_generated.h and config_generated.cpp in OUT_DIR");
    println!("  Header: {}", header_path.display());
    println!("  Implementation: {}", impl_path.display());

    // Run MOC on the generated header
    moc::run_moc(&header_path, &moc_path).expect("Failed to run MOC on config_generated.h");

    // Tell Cargo to rerun if settings.rs changes
    println!("cargo:rerun-if-changed={}", settings_rs_path.display());

    GeneratedConfig {
        header_path,
        impl_path,
        moc_path,
    }
}
