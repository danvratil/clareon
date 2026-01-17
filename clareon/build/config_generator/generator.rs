use crate::config_generator::ConfigStruct;

fn map_rust_type_to_cpp(rust_type: &str) -> &str {
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

// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

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

