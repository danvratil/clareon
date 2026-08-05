// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! MCP tool name sanitization and prefixing.

/// Characters allowed unescaped in a tool name segment (ASCII alphanumeric + underscore).
fn sanitize_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    // Collapse runs of underscores and trim edges
    let collapsed = out
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if collapsed.is_empty() {
        "x".to_string()
    } else {
        collapsed
    }
}

/// Prefixed tool name exposed to the LLM: `mcp_<server>_<tool>`.
pub fn prefixed_tool_name(server_id: &str, tool_name: &str) -> String {
    format!(
        "mcp_{}_{}",
        sanitize_segment(server_id),
        sanitize_segment(tool_name)
    )
}

/// Ensure uniqueness by appending `_2`, `_3`, … when needed.
pub fn unique_name(base: String, used: &mut std::collections::HashSet<String>) -> String {
    if used.insert(base.clone()) {
        return base;
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{base}_{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn prefixes_and_sanitizes() {
        assert_eq!(
            prefixed_tool_name("file-system", "read/file"),
            "mcp_file_system_read_file"
        );
        assert_eq!(prefixed_tool_name("fs", "list"), "mcp_fs_list");
    }

    #[test]
    fn unique_suffixes() {
        let mut used = HashSet::new();
        assert_eq!(unique_name("mcp_a_t".into(), &mut used), "mcp_a_t");
        assert_eq!(unique_name("mcp_a_t".into(), &mut used), "mcp_a_t_2");
        assert_eq!(unique_name("mcp_a_t".into(), &mut used), "mcp_a_t_3");
    }
}
