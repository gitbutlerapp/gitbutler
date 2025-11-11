mod bash;
mod patterns;
mod settings;

use anyhow::{Context, Result};
use bash::split_bash_commands;
pub use patterns::SerializationContext;
use patterns::*;
use serde::{Deserialize, Serialize};
pub use settings::{SettingsKind, add_permission_to_settings};

use crate::ClaudePermissionRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "subject")]
pub enum Permission {
    Bash(Option<BashPattern>),
    Write(Option<PathPattern>),
    Edit(Option<PathPattern>),
    WebFetch(Option<UrlPattern>),
    Mcp(McpPattern),
    Other { tool_name: String },
}

impl Permission {
    pub fn serialize(&self, ctx: &SerializationContext) -> Result<String> {
        match self {
            Self::Bash(a) => Ok(a
                .clone()
                .map(|a| format!("Bash({})", a.serialize()))
                .unwrap_or("Bash".to_owned())),
            Self::Write(a) => Ok(a
                .clone()
                .and_then(|a| Some(format!("Write({})", a.serialize(ctx).ok()?)))
                .unwrap_or("Write".to_owned())),
            Self::Edit(a) => Ok(a
                .clone()
                .and_then(|a| Some(format!("Edit({})", a.serialize(ctx).ok()?)))
                .unwrap_or("Edit".to_owned())),
            Self::WebFetch(a) => Ok(a
                .clone()
                .map(|a| format!("WebFetch({})", a.serialize()))
                .unwrap_or("WebFetch".into())),
            Self::Mcp(a) => Ok(a.serialize()),
            Self::Other { tool_name } => Ok(tool_name.to_owned()),
        }
    }

    /// Create Permissions from a ClaudePermissionRequest
    /// This creates the most specific permissions possible based on the request.
    /// Returns a Vec because bash commands with && or || may contain multiple commands.
    pub fn from_request(request: &ClaudePermissionRequest) -> Result<Vec<Self>> {
        let terms = extract_terms_to_match(request)?;

        if request.tool_name.starts_with("mcp__") {
            // For MCP tools, create a specific permission for the tool
            return Ok(vec![Self::Mcp(McpPattern::new(request.tool_name.clone()))]);
        }

        match request.tool_name.as_str() {
            "Bash" => {
                if let Some(terms) = terms {
                    // Create a permission for each command in the bash request
                    let permissions = terms
                        .into_iter()
                        .map(|cmd| {
                            if request.use_wildcard {
                                let first = cmd.split(' ').next().unwrap_or(&cmd);
                                Self::Bash(Some(BashPattern::new(first.into(), false)))
                            } else {
                                Self::Bash(Some(BashPattern::new(cmd, true)))
                            }
                        })
                        .collect();
                    return Ok(permissions);
                }
                Ok(vec![Self::Bash(None)])
            }
            "Edit" | "Write" => {
                if let Some(terms) = terms
                    && let Some(path_str) = terms.first()
                {
                    let path = std::path::PathBuf::from(path_str);
                    let pattern = if request.use_wildcard {
                        let parent = path.parent().context("Failed to get path parent")?;
                        PathPattern::new(parent.join("**/*"), PathPatternKind::Absolute)
                    } else {
                        PathPattern::new(path, PathPatternKind::Absolute)
                    };
                    return if request.tool_name == "Edit" {
                        Ok(vec![Self::Edit(Some(pattern))])
                    } else {
                        Ok(vec![Self::Write(Some(pattern))])
                    };
                }
                if request.tool_name == "Edit" {
                    Ok(vec![Self::Edit(None)])
                } else {
                    Ok(vec![Self::Write(None)])
                }
            }
            "WebFetch" => {
                if let Some(terms) = terms
                    && let Some(url) = terms.first()
                {
                    return Ok(vec![Self::WebFetch(Some(UrlPattern::full_match(
                        url.clone(),
                    )))]);
                }
                Ok(vec![Self::WebFetch(None)])
            }
            _ => Ok(vec![Self::Other {
                tool_name: request.tool_name.clone(),
            }]),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Permissions {
    approved: Vec<Permission>,
    denied: Vec<Permission>,
}

#[derive(Debug, Clone, Default)]
pub enum PermissionCheck {
    /// The set of permissions didn't approve or deny it - the user should be asked.
    #[default]
    Ask,
    /// The LLM can proceed with the tool call
    Approved,
    /// The LLM must not execute the tool call
    Denied,
}

// Examples of ClaudePermissionRequests
//
// ClaudePermissionRequest { id: "toolu_01JkTMpGfGSg5zJS121qdc5Q", created_at: 2025-11-06T13:16:33.474279, updated_at: 2025-11-06T13:16:33.474289, tool_name: "WebFetch", input: Object {"prompt": String("Summarize the main content and purpose of this page"), "url": String("https://example.com")}, decision: None }
// ClaudePermissionRequest { id: "toolu_01NYmPLovdj7sRdLxHT4AyQs", created_at: 2025-11-06T13:17:58.289023, updated_at: 2025-11-06T13:17:58.289028, tool_name: "Edit", input: Object {"file_path": String("/Users/calebowens/gitbutler-weird-shit/README.md"), "new_string": String("## Why GitButler?\n\n<!-- Another example edit -->\nWe love Git. Our own [@schacon](https://github.com/schacon) has even published the [Pro Git](https://git-scm.com/book/en/v2) book."), "old_string": String("## Why GitButler?\n\nWe love Git. Our own [@schacon](https://github.com/schacon) has even published the [Pro Git](https://git-scm.com/book/en/v2) book.")}, decision: None }
// ClaudePermissionRequest { id: "toolu_01K4YHm2pUr9U3RkLqVzZNmZ", created_at: 2025-11-06T13:18:42.340038, updated_at: 2025-11-06T13:18:42.340054, tool_name: "Bash", input: Object {"command": String("curl -s https://api.github.com/zen"), "description": String("Fetch GitHub Zen quote via curl")}, decision: None }
// ClaudePermissionRequest { id: "toolu_01UNwpMBMCF3PLZQgkWSJdCj", created_at: 2025-11-06T13:20:04.555843, updated_at: 2025-11-06T13:20:04.555848, tool_name: "Write", input: Object {"content": String("This is a test file that requires permission to create.\n"), "file_path": String("/Users/calebowens/gitbutler-weird-shit/test-file.txt")}, decision: None }
// ClaudePermissionRequest { id: "toolu_01HKmYNxg3njtnHrukGu9rCk", created_at: 2025-11-06T13:20:15.395912, updated_at: 2025-11-06T13:20:15.395916, tool_name: "Bash", input: Object {"command": String("git commit -m \"test commit\""), "description": String("Attempt git commit (should be blocked)")}, decision: None }
// ClaudePermissionRequest { id: "toolu_01KEDxwLHZWjnVhVU5nrjQ4b", created_at: 2025-11-06T13:20:23.909079, updated_at: 2025-11-06T13:20:23.909082, tool_name: "Bash", input: Object {"command": String("touch /usr/local/test-permission-file.txt"), "description": String("Attempt to create file in system directory")}, decision: None }
// ClaudePermissionRequest { id: "toolu_01B3WY7EaVhZoNnCZt5zXJ3a", created_at: 2025-11-06T13:21:27.041897, updated_at: 2025-11-06T13:21:27.041910, tool_name: "WebSearch", input: Object {"query": String("GitButler virtual branches git client")}, decision: None }

impl Permissions {
    pub fn new(approved: Vec<Permission>, denied: Vec<Permission>) -> Self {
        Self { approved, denied }
    }

    /// Create permissions from slices, cloning the data
    pub fn from_slices(approved: &[Permission], denied: &[Permission]) -> Self {
        Self {
            approved: approved.to_vec(),
            denied: denied.to_vec(),
        }
    }

    /// Merge multiple permission sources into one
    pub fn merge<'a>(sources: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut approved = Vec::new();
        let mut denied = Vec::new();

        for source in sources {
            approved.extend_from_slice(&source.approved);
            denied.extend_from_slice(&source.denied);
        }

        Self { approved, denied }
    }

    pub fn check(&self, request: &ClaudePermissionRequest) -> Result<PermissionCheck> {
        let terms = extract_terms_to_match(request)?;

        let mut approved = true;
        let mut denied = false;

        if let Some(terms) = terms {
            for term in terms {
                approved &= matches_pattern_rule(&self.approved, &request.tool_name, &term);
                denied |= matches_pattern_rule(&self.denied, &request.tool_name, &term);
            }
        } else {
            approved = matches_blanket_rule(&self.approved, &request.tool_name);
            denied = matches_blanket_rule(&self.denied, &request.tool_name);
        }

        match (approved, denied) {
            (true, false) => Ok(PermissionCheck::Approved),
            (false, true) => Ok(PermissionCheck::Denied),
            _ => Ok(PermissionCheck::Ask),
        }
    }

    /// Add an approved permission to the runtime permissions
    pub fn add_approved(&mut self, permission: Permission) {
        self.approved.push(permission);
    }

    /// Add a denied permission to the runtime permissions
    pub fn add_denied(&mut self, permission: Permission) {
        self.denied.push(permission);
    }

    /// Get the approved permissions
    pub fn approved(&self) -> &[Permission] {
        &self.approved
    }

    /// Get the denied permissions
    pub fn denied(&self) -> &[Permission] {
        &self.denied
    }
}

fn matches_pattern_rule(permissions: &[Permission], tool_name: &str, term: &str) -> bool {
    for perm in permissions {
        let matches = match perm {
            Permission::Bash(Some(pattern)) => tool_name == "Bash" && pattern.matches(term),
            Permission::Edit(Some(pattern)) => {
                tool_name == "Edit" && pattern.matches(std::path::Path::new(term))
            }
            Permission::Write(Some(pattern)) => {
                tool_name == "Write" && pattern.matches(std::path::Path::new(term))
            }
            Permission::WebFetch(Some(pattern)) => {
                tool_name == "WebFetch" && pattern.matches(term).unwrap_or(false)
            }
            _ => false,
        };

        if matches {
            return true;
        }
    }

    false
}

fn matches_blanket_rule(permissions: &[Permission], tool_name: &str) -> bool {
    for perm in permissions {
        let matches = match perm {
            Permission::Bash(None) => tool_name == "Bash",
            Permission::Edit(None) => tool_name == "Edit",
            Permission::Write(None) => tool_name == "Write",
            Permission::WebFetch(None) => tool_name == "WebFetch",
            Permission::Other { tool_name: n } => tool_name == n,
            _ => false,
        };

        if matches {
            return true;
        }
    }

    false
}

/// Requests may either have no term to match against, or they might have have
/// one or (in the case of bash requests) have multiple.
///
/// If there are no terms, we should match against a blanket term. IE just
/// `WebSearch`
///
/// If there multiple, we should AND the terms together.
fn extract_terms_to_match(request: &ClaudePermissionRequest) -> Result<Option<Vec<String>>> {
    if request.tool_name.starts_with("mcp__") {
        return Ok(Some(vec![request.tool_name.clone()]));
    }

    match request.tool_name.as_str() {
        "Bash" => {
            let command = request.input["command"]
                .as_str()
                .context("Expected bash tool to have command string")?;
            let commands = split_bash_commands(command);
            Ok(Some(commands))
        }
        "Edit" | "Write" => {
            let path = request.input["file_path"]
                .as_str()
                .context("Expected edits and writes to have a file path")?;
            Ok(Some(vec![path.into()]))
        }
        "WebFetch" => {
            let url = request.input["url"]
                .as_str()
                .context("Expected webfetch tool to have a url")?;
            Ok(Some(vec![url.into()]))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;

    fn create_test_context(for_global: bool) -> SerializationContext {
        SerializationContext::new(
            PathBuf::from("/home/testuser"),
            PathBuf::from("/home/testuser/projects/myproject"),
            PathBuf::from("/home/testuser/.claude"),
            for_global,
        )
    }

    mod bash_permission_serialization {
        use super::*;

        #[test]
        fn blanket_permission() {
            let perm = Permission::Bash(None);
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Bash");
        }

        #[test]
        fn exact_command() {
            let pattern = BashPattern::new("git status".to_string(), true);
            let perm = Permission::Bash(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Bash(git status)");
        }

        #[test]
        fn command_with_special_chars() {
            let pattern = BashPattern::new("echo 'hello world'".to_string(), true);
            let perm = Permission::Bash(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Bash(echo 'hello world')");
        }
    }

    mod write_permission_serialization {
        use super::*;

        #[test]
        fn blanket_permission() {
            let perm = Permission::Write(None);
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Write");
        }

        #[test]
        fn absolute_path() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/file.txt"),
                PathPatternKind::Absolute,
            );
            let perm = Permission::Write(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "Write(//home/testuser/file.txt)"
            );
        }

        #[test]
        fn home_relative_path() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/documents/file.txt"),
                PathPatternKind::HomeRelative,
            );
            let perm = Permission::Write(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Write(~/documents/file.txt)");
        }

        #[test]
        fn settings_relative_path_project() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/projects/myproject/src/main.rs"),
                PathPatternKind::SettingsRelative,
            );
            let perm = Permission::Write(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Write(/src/main.rs)");
        }

        #[test]
        fn settings_relative_path_global() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/.claude/config.json"),
                PathPatternKind::SettingsRelative,
            );
            let perm = Permission::Write(Some(pattern));
            let ctx = create_test_context(true);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Write(/config.json)");
        }

        #[test]
        fn cwd_relative_path() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/projects/myproject/README.md"),
                PathPatternKind::CwdRelative,
            );
            let perm = Permission::Write(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "Write(projects/myproject/README.md)"
            );
        }

        #[test]
        fn glob_pattern() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/projects/myproject/**/*.rs"),
                PathPatternKind::SettingsRelative,
            );
            let perm = Permission::Write(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Write(/**/*.rs)");
        }
    }

    mod edit_permission_serialization {
        use super::*;

        #[test]
        fn blanket_permission() {
            let perm = Permission::Edit(None);
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Edit");
        }

        #[test]
        fn absolute_path() {
            let pattern =
                PathPattern::new(PathBuf::from("/etc/config.conf"), PathPatternKind::Absolute);
            let perm = Permission::Edit(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "Edit(//etc/config.conf)");
        }

        #[test]
        fn home_relative_with_nested_dirs() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/.config/app/settings.json"),
                PathPatternKind::HomeRelative,
            );
            let perm = Permission::Edit(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "Edit(~/.config/app/settings.json)"
            );
        }

        #[test]
        fn settings_relative_agents_folder() {
            let pattern = PathPattern::new(
                PathBuf::from("/home/testuser/projects/myproject/.agents/memories/index.md"),
                PathPatternKind::SettingsRelative,
            );
            let perm = Permission::Edit(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "Edit(/.agents/memories/index.md)"
            );
        }
    }

    mod webfetch_permission_serialization {
        use super::*;

        #[test]
        fn blanket_permission() {
            let perm = Permission::WebFetch(None);
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "WebFetch");
        }

        #[test]
        fn full_match_url() {
            let pattern = UrlPattern::full_match("https://example.com/api/v1".to_string());
            let perm = Permission::WebFetch(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "WebFetch(https://example.com/api/v1)"
            );
        }

        #[test]
        fn domain_match() {
            let pattern = UrlPattern::Domain("example.com".to_string());
            let perm = Permission::WebFetch(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "WebFetch(domain:example.com)"
            );
        }

        #[test]
        fn url_with_query_params() {
            let pattern = UrlPattern::full_match(
                "https://api.github.com/repos/owner/repo?per_page=100".to_string(),
            );
            let perm = Permission::WebFetch(Some(pattern));
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "WebFetch(https://api.github.com/repos/owner/repo?per_page=100)"
            );
        }
    }

    mod mcp_permission_serialization {
        use super::*;

        #[test]
        fn mcp_server_pattern() {
            let pattern = McpPattern::new("mcp__but-security".to_string());
            let perm = Permission::Mcp(pattern);
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "mcp__but-security");
        }

        #[test]
        fn mcp_tool_pattern() {
            let pattern = McpPattern::new("mcp__but-security__approval_prompt".to_string());
            let perm = Permission::Mcp(pattern);
            let ctx = create_test_context(false);

            assert_eq!(
                perm.serialize(&ctx).unwrap(),
                "mcp__but-security__approval_prompt"
            );
        }
    }

    mod other_permission_serialization {
        use super::*;

        #[test]
        fn websearch_tool() {
            let perm = Permission::Other {
                tool_name: "WebSearch".to_string(),
            };
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "WebSearch");
        }

        #[test]
        fn custom_tool() {
            let perm = Permission::Other {
                tool_name: "CustomTool".to_string(),
            };
            let ctx = create_test_context(false);

            assert_eq!(perm.serialize(&ctx).unwrap(), "CustomTool");
        }
    }
}
