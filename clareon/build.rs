// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::PathBuf;

use cxx_qt_build::{CxxQtBuilder, QmlModule};
use syn::{Fields, File, Item, ItemStruct, Type};

mod config_codegen {
    use super::*;

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

    pub fn parse_config_structs(settings_rs_path: &str) -> (Vec<ConfigStruct>, Vec<String>) {
        let content = fs::read_to_string(settings_rs_path).expect("Failed to read settings.rs");

        let ast: File = syn::parse_file(&content).expect("Failed to parse settings.rs");

        let mut config_structs = Vec::new();
        let mut config_enums = Vec::new();

        for item in &ast.items {
            match item {
                Item::Struct(item_struct) => {
                    let struct_name = item_struct.ident.to_string();
                    // Include all config-related structs
                    if struct_name.ends_with("Config") {
                        config_structs.push(extract_struct_info(item_struct));
                    }
                }
                Item::Enum(item_enum) => {
                    let enum_name = item_enum.ident.to_string();
                    // Track enum names so we can map them to QString
                    if enum_name.ends_with("Config") || enum_name == "Backend" {
                        config_enums.push(enum_name);
                    }
                }
                _ => {}
            }
        }

        (config_structs, config_enums)
    }

    fn extract_struct_info(item_struct: &ItemStruct) -> ConfigStruct {
        let name = item_struct.ident.to_string();
        let mut fields = Vec::new();

        if let Fields::Named(named_fields) = &item_struct.fields {
            for field in &named_fields.named {
                let field_name = field.ident.as_ref().unwrap().to_string();
                let (rust_type, is_optional) = extract_type_info(&field.ty);

                fields.push(ConfigField {
                    name: field_name,
                    rust_type,
                    is_optional,
                });
            }
        }

        ConfigStruct { name, fields }
    }

    fn extract_type_info(ty: &Type) -> (String, bool) {
        match ty {
            Type::Path(type_path) => {
                let segments = &type_path.path.segments;
                if segments.is_empty() {
                    return ("unknown".to_string(), false);
                }

                let first_segment = &segments[0];
                let type_name = first_segment.ident.to_string();

                // Check if it's Option<T>
                if type_name == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &first_segment.arguments
                        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
                    {
                        let (inner_type, _) = extract_type_info(inner_ty);
                        return (inner_type, true);
                    }
                    return ("String".to_string(), true);
                }

                (type_name, false)
            }
            _ => ("unknown".to_string(), false),
        }
    }

    pub fn map_rust_type_to_cpp(rust_type: &str) -> &str {
        match rust_type {
            "String" => "QString",
            "bool" => "bool",
            "u64" => "quint64",
            "i32" => "qint32",
            "HashMap" => "QVariantMap",
            // Nested config structs - convert XyzConfig to XyzConfigCpp
            s if s.ends_with("Config") => {
                // Return a static string - we'll need to handle this differently
                "ConfigCpp" // Placeholder, we'll construct the actual name at call site
            }
            _ => "QString", // Default fallback
        }
    }

    pub fn map_rust_type_to_cpp_owned(rust_type: &str, enums: &[String]) -> String {
        // Check if this is an enum - enums are mapped to QString
        if enums.iter().any(|e| e == rust_type) {
            "QString".to_string()
        } else if rust_type.ends_with("Config") || rust_type == "Backend" {
            // Only create Cpp suffix for actual config structs, not enums
            format!("{}Cpp", rust_type)
        } else {
            map_rust_type_to_cpp(rust_type).to_string()
        }
    }

    fn order_structs_by_dependency(structs: &[ConfigStruct]) -> Vec<&ConfigStruct> {
        let mut ordered = Vec::new();
        let mut remaining: Vec<_> = structs.iter().collect();

        // Keep trying to add structs until all are ordered
        while !remaining.is_empty() {
            let mut added_any = false;

            // Find structs with no unresolved dependencies
            remaining.retain(|s| {
                // Check if all field types (that are config structs) are already in ordered
                let deps_satisfied = s.fields.iter().all(|field| {
                    if field.rust_type.ends_with("Config") {
                        // This field depends on another config struct
                        // Check if that struct is already in ordered
                        ordered
                            .iter()
                            .any(|o: &&ConfigStruct| o.name == field.rust_type)
                    } else {
                        // Not a config struct dependency
                        true
                    }
                });

                if deps_satisfied {
                    ordered.push(*s);
                    added_any = true;
                    false // Remove from remaining
                } else {
                    true // Keep in remaining
                }
            });

            if !added_any && !remaining.is_empty() {
                // Circular dependency or other issue - just add the rest
                ordered.append(&mut remaining);
                break;
            }
        }

        // Ensure Config is last (root)
        if let Some(config_idx) = ordered.iter().position(|s| s.name == "Config") {
            let config = ordered.remove(config_idx);
            ordered.push(config);
        }

        ordered
    }

    pub fn generate_cpp_header(structs: &[ConfigStruct], enums: &[String]) -> String {
        let mut header = String::new();

        // Header guard and includes
        header.push_str(
            r#"// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This file is AUTO-GENERATED by build.rs - DO NOT EDIT

#pragma once

#include <QObject>
#include <QString>
#include <QVariantMap>

"#,
        );

        // Generate classes in dependency order (leaf classes first, then root)
        let ordered_structs = order_structs_by_dependency(structs);

        for config_struct in ordered_structs {
            let is_root = config_struct.name == "Config";
            generate_cpp_class_definition(&mut header, config_struct, is_root, enums);
            header.push('\n');
        }

        header
    }

    fn generate_cpp_class_definition(
        header: &mut String,
        config_struct: &ConfigStruct,
        is_root: bool,
        enums: &[String],
    ) {
        let class_name = format!("{}Cpp", config_struct.name);

        // Root Config is QObject, nested configs are Q_GADGET
        if is_root {
            header.push_str(&format!("class {} : public QObject {{\n", class_name));
            header.push_str("    Q_OBJECT\n\n");
        } else {
            header.push_str(&format!("class {} {{\n", class_name));
            header.push_str("    Q_GADGET\n\n");
        }

        // Q_PROPERTY declarations
        header.push_str("public:\n");
        for field in &config_struct.fields {
            let cpp_type = map_rust_type_to_cpp_owned(&field.rust_type, enums);
            let camel_name = to_camel_case(&field.name);

            header.push_str(&format!(
                "    Q_PROPERTY({} {} READ {} WRITE set{})\n",
                cpp_type, camel_name, camel_name, camel_name
            ));
        }
        header.push('\n');

        // Constructors
        if is_root {
            header.push_str(&format!(
                "    explicit {}(QObject* parent = nullptr);\n",
                class_name
            ));
        } else {
            header.push_str(&format!("    {}() = default;\n", class_name));
        }
        header.push_str(&format!(
            "    explicit {}(const QVariantMap& data);\n\n",
            class_name
        ));

        // Getter/setter declarations
        for field in &config_struct.fields {
            let cpp_type = map_rust_type_to_cpp_owned(&field.rust_type, enums);
            let camel_name = to_camel_case(&field.name);

            header.push_str(&format!("    {} {}() const;\n", cpp_type, camel_name));
            header.push_str(&format!(
                "    void set{}(const {}& value);\n",
                camel_name, cpp_type
            ));
        }
        header.push('\n');

        header.push_str("    QVariantMap toVariantMap() const;\n\n");

        // Private members
        header.push_str("private:\n");
        for field in &config_struct.fields {
            let cpp_type = map_rust_type_to_cpp_owned(&field.rust_type, enums);
            let member_name = format!("m_{}", to_camel_case(&field.name));
            header.push_str(&format!("    {} {};\n", cpp_type, member_name));
        }

        header.push_str("};\n");
    }

    pub fn generate_cpp_implementation(structs: &[ConfigStruct], enums: &[String]) -> String {
        let mut impl_code = String::new();

        impl_code.push_str(
            r#"// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This file is AUTO-GENERATED by build.rs - DO NOT EDIT

#include "config_generated.h"

"#,
        );

        // Use same ordering as header
        let ordered_structs = order_structs_by_dependency(structs);

        for config_struct in ordered_structs {
            let is_root = config_struct.name == "Config";
            generate_cpp_class_implementation(&mut impl_code, config_struct, is_root, enums);
            impl_code.push('\n');
        }

        impl_code
    }

    fn generate_cpp_class_implementation(
        impl_code: &mut String,
        config_struct: &ConfigStruct,
        is_root: bool,
        enums: &[String],
    ) {
        let class_name = format!("{}Cpp", config_struct.name);

        // QObject constructor for root config
        if is_root {
            impl_code.push_str(&format!(
                "{}::{}(QObject* parent)\n    : QObject(parent)\n{{}}\n\n",
                class_name, class_name
            ));
        }

        // Constructor from QVariantMap
        if is_root {
            impl_code.push_str(&format!(
                "{}::{}(const QVariantMap& data)\n    : QObject(nullptr)\n{{\n",
                class_name, class_name
            ));
        } else {
            impl_code.push_str(&format!(
                "{}::{}(const QVariantMap& data) {{\n",
                class_name, class_name
            ));
        }

        for field in &config_struct.fields {
            let camel_name = to_camel_case(&field.name);
            let member_name = format!("m_{}", camel_name);
            let key = &field.name;
            let cpp_type = map_rust_type_to_cpp_owned(&field.rust_type, enums);

            // Use appropriate conversion based on type
            let conversion = match cpp_type.as_str() {
                "bool" => "toBool()",
                "qint32" => "toInt()",
                "quint64" => "toULongLong()",
                "QString" => "toString()",
                "QVariantMap" => "toMap()",
                // Nested gadgets need construction from variant map
                _ if cpp_type.ends_with("Cpp") => {
                    impl_code.push_str(&format!(
                        "    {} = {}(data.value(\"{}\").toMap());\n",
                        member_name, cpp_type, key
                    ));
                    continue;
                }
                _ => "toString()", // Default
            };

            impl_code.push_str(&format!(
                "    {} = data.value(\"{}\").{};\n",
                member_name, key, conversion
            ));
        }
        impl_code.push_str("}\n\n");

        // Getters
        for field in &config_struct.fields {
            let cpp_type = map_rust_type_to_cpp_owned(&field.rust_type, enums);
            let camel_name = to_camel_case(&field.name);
            let member_name = format!("m_{}", camel_name);

            impl_code.push_str(&format!(
                "{} {}::{}() const {{\n    return {};\n}}\n\n",
                cpp_type, class_name, camel_name, member_name
            ));
        }

        // Setters
        for field in &config_struct.fields {
            let cpp_type = map_rust_type_to_cpp_owned(&field.rust_type, enums);
            let camel_name = to_camel_case(&field.name);
            let member_name = format!("m_{}", camel_name);

            impl_code.push_str(&format!(
                "void {}::set{}(const {}& value) {{\n",
                class_name, camel_name, cpp_type
            ));
            impl_code.push_str(&format!("    {} = value;\n", member_name));
            impl_code.push_str("}\n\n");
        }

        // toVariantMap
        impl_code.push_str(&format!(
            "QVariantMap {}::toVariantMap() const {{\n",
            class_name
        ));
        impl_code.push_str("    QVariantMap map;\n");
        for field in &config_struct.fields {
            let camel_name = to_camel_case(&field.name);
            let member_name = format!("m_{}", camel_name);
            let key = &field.name;
            let cpp_type = map_rust_type_to_cpp_owned(&field.rust_type, enums);

            // Nested gadgets need their own toVariantMap() called
            // Enums are QString and don't need toVariantMap()
            if cpp_type.ends_with("Cpp") {
                impl_code.push_str(&format!(
                    "    map.insert(\"{}\", {}.toVariantMap());\n",
                    key, member_name
                ));
            } else {
                impl_code.push_str(&format!("    map.insert(\"{}\", {});\n", key, member_name));
            }
        }
        impl_code.push_str("    return map;\n}\n\n");
    }

    fn to_camel_case(snake_case: &str) -> String {
        let mut camel = String::new();
        let mut capitalize_next = false;

        for ch in snake_case.chars() {
            if ch == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                camel.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                camel.push(ch);
            }
        }

        camel
    }
}

fn run_moc(header_path: &PathBuf, output_path: &PathBuf) -> Result<(), String> {
    // Find Qt installation
    let qt_path = std::env::var("QT_PATH")
        .or_else(|_| std::env::var("QTDIR"))
        .or_else(|_| std::env::var("Qt6_DIR"))
        .unwrap_or_else(|_| "/usr".to_string());

    // Try common MOC locations (prioritize Qt6 versions)
    let moc_candidates = vec![
        PathBuf::from("/usr/lib/qt6/libexec/moc"), // Most common Qt6 location on Linux
        PathBuf::from("/usr/lib/qt6/bin/moc"),
        PathBuf::from("/usr/bin/moc6"), // Some distros use moc6
        PathBuf::from("/usr/bin/moc-qt6"),
        PathBuf::from(&qt_path).join("libexec/moc"),
        PathBuf::from(&qt_path).join("bin/moc"),
        PathBuf::from("/usr/bin/moc"), // Fallback to generic moc (might be Qt5)
    ];

    let moc_path = moc_candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| "Could not find MOC executable".to_string())?;

    println!("Found MOC at: {}", moc_path.display());

    // Run MOC
    let status = std::process::Command::new(&moc_path)
        .arg(header_path)
        .arg("-o")
        .arg(output_path)
        .status()
        .map_err(|e| format!("Failed to run MOC: {}", e))?;

    if !status.success() {
        return Err(format!("MOC failed with status: {}", status));
    }

    println!("Generated MOC file: {}", output_path.display());
    Ok(())
}

fn generate_config_cpp() -> (PathBuf, PathBuf, PathBuf) {
    let settings_rs_path = "../clareon-core/src/config/settings.rs";

    // Parse config structs and enums from settings.rs
    let (config_structs, config_enums) = config_codegen::parse_config_structs(settings_rs_path);

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
    let header_code = config_codegen::generate_cpp_header(&config_structs, &config_enums);
    let impl_code = config_codegen::generate_cpp_implementation(&config_structs, &config_enums);

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
    run_moc(&header_path, &moc_path).expect("Failed to run MOC on config_generated.h");

    // Tell Cargo to rerun if settings.rs changes
    println!("cargo:rerun-if-changed={}", settings_rs_path);

    (header_path, impl_path, moc_path)
}

fn find_qml_files() -> Vec<String> {
    let root_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap().as_str());
    let qml_dir = root_dir.join("qml");
    let files = glob::glob(qml_dir.join("**/*.qml").to_str().unwrap()).unwrap();
    files
        .map(|entry| {
            entry
                .unwrap()
                .strip_prefix(&root_dir)
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

fn main() {
    // Generate C++ config code from Rust settings
    let (_header_path, impl_path, moc_path) = generate_config_cpp();

    // Get OUT_DIR for include path
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    CxxQtBuilder::new_qml_module(
        QmlModule::new("cz.dvratil.clareon")
            .qml_files(find_qml_files())
            .depend("QtQuick"),
    )
    .files([
        "src/qml.rs",
        "src/logging.rs",
        "src/service_controller.rs",
        "src/conversation_list_model.rs",
        "src/message_list_model.rs",
        "src/search_result_model.rs",
        "src/config_manager_qt.rs",
    ])
    .cpp_file("src/cpp/logging.cpp")
    .cpp_file("src/cpp/qml.cpp")
    .cpp_file(impl_path) // Add generated config implementation
    .cpp_file(moc_path) // Add MOC-generated file
    .include_dir(&out_dir) // Add OUT_DIR to include path for config_generated.h
    .include_dir("src/cpp") // Add src/cpp to include path for config_bridge.hpp
    .qt_module("Quick")
    .qt_module("Qml")
    .build();
}
