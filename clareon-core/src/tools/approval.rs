// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scoped always-allow rules for tool approval.
//!
//! Built-in file tools are allowed by path (the path and its descendants).
//! Exec is allowed by argv prefix. MCP tools are allowed by name only.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// Built-in tools whose approval is scoped to a `path` argument.
pub const PATH_SCOPED_TOOLS: &[&str] = &["read_file", "write_file", "list_directory"];

/// Built-in tools whose approval is scoped to a command argv prefix.
pub const EXEC_SCOPED_TOOLS: &[&str] = &["exec", "run_command"];

/// A persisted always-allow rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlwaysAllowRule {
    /// Allow `tool` when its `path` argument is this path or a descendant.
    Path { tool: String, path: PathBuf },
    /// Allow this tool for any arguments (MCP and other unscopeable tools).
    Tool { name: String },
    /// Allow an exec-style tool when argv starts with this prefix.
    Exec { argv_prefix: Vec<String> },
}

impl AlwaysAllowRule {
    /// Parse a stored spec string. Unknown or legacy bare names (`read_file`) yield `None`.
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim();
        if spec.is_empty() {
            return None;
        }
        if let Some(rest) = spec.strip_prefix("path:") {
            let (tool, path) = rest.split_once(':')?;
            if tool.is_empty() || path.is_empty() {
                return None;
            }
            return Some(Self::Path {
                tool: tool.to_string(),
                path: PathBuf::from(path),
            });
        }
        if let Some(name) = spec.strip_prefix("tool:") {
            if name.is_empty() {
                return None;
            }
            return Some(Self::Tool {
                name: name.to_string(),
            });
        }
        if let Some(rest) = spec.strip_prefix("exec:") {
            let argv = parse_exec_argv(rest)?;
            if argv.is_empty() {
                return None;
            }
            return Some(Self::Exec { argv_prefix: argv });
        }
        // Legacy / shorthand: MCP names are per-tool. Bare file/exec names are not.
        if is_mcp_tool(spec) {
            return Some(Self::Tool {
                name: spec.to_string(),
            });
        }
        None
    }

    pub fn to_spec(&self) -> String {
        match self {
            Self::Path { tool, path } => format!("path:{tool}:{}", path.display()),
            Self::Tool { name } => format!("tool:{name}"),
            Self::Exec { argv_prefix } => format!("exec:{}", format_exec_argv(argv_prefix)),
        }
    }

    /// Build a rule from a concrete invocation. Returns `None` if it cannot be scoped safely.
    pub fn from_invocation(name: &str, input: &Value) -> Option<Self> {
        if is_mcp_tool(name) {
            return Some(Self::Tool {
                name: name.to_string(),
            });
        }
        if is_path_scoped(name) {
            let path = input.get("path").and_then(Value::as_str)?;
            if path.is_empty() {
                return None;
            }
            return Some(Self::Path {
                tool: name.to_string(),
                path: PathBuf::from(path),
            });
        }
        if is_exec_scoped(name) {
            let argv = extract_exec_argv(input)?;
            if argv.is_empty() {
                return None;
            }
            return Some(Self::Exec { argv_prefix: argv });
        }
        None
    }

    pub fn matches(&self, name: &str, input: &Value) -> bool {
        match self {
            Self::Path { tool, path } => {
                if tool != name {
                    return false;
                }
                input
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|requested| path_covers(Path::new(requested), path))
            }
            Self::Tool { name: allowed } => allowed == name,
            Self::Exec { argv_prefix } => {
                is_exec_scoped(name)
                    && extract_exec_argv(input)
                        .is_some_and(|argv| argv_starts_with(&argv, argv_prefix))
            }
        }
    }

    /// Short description for the approval banner.
    pub fn label(&self) -> String {
        match self {
            Self::Path { tool, path } => {
                format!("{tool} on {} and anything under it", path.display())
            }
            Self::Tool { name } => format!("{name} (any arguments)"),
            Self::Exec { argv_prefix } => format!("exec {}", argv_prefix.join(" ")),
        }
    }
}

pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp_")
}

pub fn is_path_scoped(name: &str) -> bool {
    PATH_SCOPED_TOOLS.contains(&name)
}

pub fn is_exec_scoped(name: &str) -> bool {
    EXEC_SCOPED_TOOLS.contains(&name)
}

pub fn parse_rules(specs: &[String]) -> Vec<AlwaysAllowRule> {
    specs
        .iter()
        .filter_map(|s| AlwaysAllowRule::parse(s))
        .collect()
}

pub fn matches_any(rules: &[AlwaysAllowRule], name: &str, input: &Value) -> bool {
    rules.iter().any(|rule| rule.matches(name, input))
}

pub fn is_denied(always_deny: &[String], name: &str, input: &Value) -> bool {
    matches_any(&parse_rules(always_deny), name, input)
}

/// True when at least one pending call still needs a user prompt.
/// Denied calls never prompt. Auto-execute and always-allow skip the prompt.
/// Deny wins over allow.
pub fn tools_need_approval(
    auto_execute: bool,
    always_allow: &[String],
    always_deny: &[String],
    pending: &[(String, Value)],
) -> bool {
    if pending.is_empty() {
        return false;
    }
    let deny = parse_rules(always_deny);
    let allow = parse_rules(always_allow);
    pending.iter().any(|(name, input)| {
        if matches_any(&deny, name, input) {
            return false;
        }
        if auto_execute || matches_any(&allow, name, input) {
            return false;
        }
        true
    })
}

fn path_covers(requested: &Path, allowed: &Path) -> bool {
    let requested = normalize_path(requested);
    let allowed = normalize_path(allowed);
    let allowed_comps: Vec<_> = allowed.components().collect();
    let requested_comps: Vec<_> = requested.components().collect();
    requested_comps.starts_with(&allowed_comps)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn extract_exec_argv(input: &Value) -> Option<Vec<String>> {
    if let Some(argv) = input.get("argv").and_then(Value::as_array) {
        let parts: Option<Vec<String>> = argv
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect();
        return parts.filter(|p| !p.is_empty());
    }
    let command = input.get("command").and_then(Value::as_str)?;
    let mut argv = if command.contains(char::is_whitespace) && input.get("args").is_none() {
        split_exec_tokens(command)
    } else {
        vec![command.to_string()]
    };
    if let Some(args) = input.get("args").and_then(Value::as_array) {
        for arg in args {
            argv.push(arg.as_str()?.to_string());
        }
    }
    if argv.is_empty() || argv.iter().any(|s| s.is_empty()) {
        None
    } else {
        Some(argv)
    }
}

fn argv_starts_with(argv: &[String], prefix: &[String]) -> bool {
    argv.starts_with(prefix)
}

fn parse_exec_argv(rest: &str) -> Option<Vec<String>> {
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    if rest.starts_with('[') {
        let values: Vec<Value> = serde_json::from_str(rest).ok()?;
        let parts: Option<Vec<String>> = values
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect();
        return parts.filter(|p| !p.is_empty());
    }
    let parts = split_exec_tokens(rest);
    if parts.is_empty() { None } else { Some(parts) }
}

fn format_exec_argv(argv: &[String]) -> String {
    if argv.iter().any(|s| s.chars().any(char::is_whitespace)) {
        serde_json::to_string(argv).unwrap_or_else(|_| argv.join(" "))
    } else {
        argv.join(" ")
    }
}

fn split_exec_tokens(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn path_rule_covers_descendants_not_siblings() {
        let rule = AlwaysAllowRule::parse("path:read_file:/home/proj").unwrap();
        assert!(rule.matches("read_file", &json!({"path": "/home/proj"})));
        assert!(rule.matches("read_file", &json!({"path": "/home/proj/src/main.rs"})));
        assert!(!rule.matches("read_file", &json!({"path": "/home/proj-other"})));
        assert!(!rule.matches("write_file", &json!({"path": "/home/proj"})));
    }

    #[test]
    fn path_rule_normalizes_dotdot() {
        let rule = AlwaysAllowRule::parse("path:read_file:/home/proj").unwrap();
        assert!(!rule.matches("read_file", &json!({"path": "/home/proj/../secret"})));
        assert!(rule.matches("read_file", &json!({"path": "/home/proj/./src"})));
    }

    #[test]
    fn mcp_is_per_tool() {
        let rule =
            AlwaysAllowRule::from_invocation("mcp_github_list_issues", &json!({"owner": "kde"}))
                .unwrap();
        assert!(rule.matches("mcp_github_list_issues", &json!({"owner": "other"})));
        assert!(!rule.matches("mcp_github_create_issue", &json!({})));
        assert_eq!(rule.to_spec(), "tool:mcp_github_list_issues");
    }

    #[test]
    fn bare_read_file_is_not_a_rule() {
        assert!(AlwaysAllowRule::parse("read_file").is_none());
    }

    #[test]
    fn exec_prefix_matches() {
        let rule = AlwaysAllowRule::from_invocation(
            "exec",
            &json!({"command": "git", "args": ["status"]}),
        )
        .unwrap();
        assert!(rule.matches("exec", &json!({"command": "git", "args": ["status"]})));
        assert!(rule.matches(
            "exec",
            &json!({"command": "git", "args": ["status", "--short"]})
        ));
        assert!(!rule.matches("exec", &json!({"command": "git", "args": ["push"]})));
        assert_eq!(rule.to_spec(), "exec:git status");
    }

    #[test]
    fn exec_json_argv_roundtrip() {
        let rule = AlwaysAllowRule::Exec {
            argv_prefix: vec![
                "git".into(),
                "commit".into(),
                "-m".into(),
                "hi there".into(),
            ],
        };
        let parsed = AlwaysAllowRule::parse(&rule.to_spec()).unwrap();
        assert_eq!(parsed, rule);
    }

    #[test]
    fn approval_uses_scoped_rules() {
        let allow = vec!["path:read_file:/tmp/a".into()];
        assert!(!tools_need_approval(
            false,
            &allow,
            &[],
            &[("read_file".into(), json!({"path": "/tmp/a/b.txt"}))]
        ));
        assert!(tools_need_approval(
            false,
            &allow,
            &[],
            &[("read_file".into(), json!({"path": "/tmp/c.txt"}))]
        ));
        assert!(!tools_need_approval(
            true,
            &[],
            &[],
            &[("read_file".into(), json!({}))]
        ));
    }

    #[test]
    fn deny_wins_over_allow() {
        let allow = vec!["path:read_file:/tmp".into()];
        let deny = vec!["path:read_file:/tmp/secret".into()];
        assert!(is_denied(
            &deny,
            "read_file",
            &json!({"path": "/tmp/secret/key"})
        ));
        assert!(!tools_need_approval(
            false,
            &allow,
            &deny,
            &[("read_file".into(), json!({"path": "/tmp/secret/key"}))]
        ));
        assert!(is_denied(
            &deny,
            "read_file",
            &json!({"path": "/tmp/secret"})
        ));
    }

    #[test]
    fn invocation_without_path_cannot_be_scoped() {
        assert!(AlwaysAllowRule::from_invocation("read_file", &json!({})).is_none());
    }
}
