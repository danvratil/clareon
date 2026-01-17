// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{fs, path::Path};

use syn::{Fields, File, Item, ItemStruct, Type};

use super::{ConfigStruct, ConfigField};

pub fn parse_config_structs(settings_rs_path: &Path) -> (Vec<ConfigStruct>, Vec<String>) {
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

