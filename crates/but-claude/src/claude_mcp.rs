use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use claude_agent_sdk_rs::{McpServerConfig, McpServers, types::mcp::McpStdioServerConfig};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::claude_settings::ClaudeSettings;

/// Represents the MCP-relevant parts of ~/.claude.json
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ClaudeJson {
    projects: Option<HashMap<String, Project>>,
    mcp_servers: Option<McpServerMap>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct Project {
    mcp_servers: Option<McpServerMap>,
}

/// The on-disk format of a `.mcp.json` file.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct McpJsonFile {
    mcp_servers: McpServerMap,
}

/// Claude integration config returned to the frontend via `claude_get_config`.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeConfig {
    pub mcp_servers: McpServerMap,
    /// Whether this project has been registered in `~/.claude.json`. When
    /// `false` the user should run `claude` in the project directory first so
    /// Claude Code picks it up and adds it to the projects map.
    pub project_registered: bool,
}

/// Map of server name to server configuration.
pub type McpServerMap = HashMap<String, McpServer>;

/// MCP server configuration as stored in JSON config files.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct ClaudeProjectConfig {
    settings: ClaudeSettings,
    project_path: PathBuf,
    claude_json: Option<ClaudeJson>,
    mcp_json: Option<McpJsonFile>,
}

impl ClaudeProjectConfig {
    pub async fn open(settings: &ClaudeSettings, project_path: &Path) -> Self {
        Self {
            claude_json: read_claude_json().await,
            mcp_json: read_mcp_json(project_path).await,
            project_path: project_path.to_owned(),
            settings: settings.clone(),
        }
    }

    /// Returns the Claude integration config for the API/UI.
    pub fn config(&self) -> ClaudeConfig {
        ClaudeConfig {
            mcp_servers: self.collect_servers(),
            project_registered: self.is_project_registered(),
        }
    }

    /// Returns MCP servers in SDK format, ready to pass to the Claude agent.
    ///
    /// Only stdio-based MCP servers are supported by the SDK.
    pub fn mcp_servers_for_sdk(&self, disabled_servers: &[&str]) -> McpServers {
        let mut servers = self.collect_servers();

        // Filter out disabled servers
        for server in disabled_servers {
            servers.remove(*server);
        }

        convert_to_sdk_format(servers)
    }

    /// Returns `true` if this project path appears in the `projects` map of
    /// `~/.claude.json`, meaning the user has already run `claude` in this
    /// directory and Claude Code has registered it.
    fn is_project_registered(&self) -> bool {
        let Some(claude_json) = &self.claude_json else {
            return false;
        };
        let Some(projects) = &claude_json.projects else {
            return false;
        };
        find_project(projects, &self.project_path).is_some()
    }

    /// Collects servers from all sources, applying settings filters.
    fn collect_servers(&self) -> McpServerMap {
        let mut servers: McpServerMap = HashMap::new();

        // Collect from ~/.claude.json
        if let Some(claude_json) = &self.claude_json {
            // Project-specific servers
            let project_servers = claude_json
                .projects
                .as_ref()
                .and_then(|projects| find_project(projects, &self.project_path).cloned())
                .and_then(|project| project.mcp_servers);

            if let Some(project_servers) = project_servers {
                for (k, v) in project_servers {
                    servers.insert(k, v);
                }
            }

            // Global servers
            if let Some(global_servers) = &claude_json.mcp_servers {
                for (k, v) in global_servers {
                    servers.insert(k.clone(), v.clone());
                }
            }
        }

        // Collect from .mcp.json (filtered by settings)
        let all_enabled = self.settings.enable_all_project_mcp_servers();
        let enabled = self.settings.enabled_project_mcp_servers();

        if let Some(mcp_json) = &self.mcp_json {
            for (k, v) in &mcp_json.mcp_servers {
                if all_enabled || enabled.contains(k) {
                    servers.insert(k.clone(), v.clone());
                }
            }
        }

        servers
    }
}

/// Converts MCP server configs to SDK format.
/// Only stdio-based servers are supported; others are logged and skipped.
fn convert_to_sdk_format(servers: McpServerMap) -> McpServers {
    let mut sdk_servers = HashMap::new();

    for (name, server) in servers {
        // Check if this is a stdio server (has command, and type is either "stdio" or unset)
        let is_stdio = server.command.is_some()
            && server
                .r#type
                .as_ref()
                .is_none_or(|t| t == "stdio" || t.is_empty());

        if is_stdio {
            if let Some(command) = server.command {
                sdk_servers.insert(
                    name,
                    McpServerConfig::Stdio(McpStdioServerConfig {
                        command,
                        args: server.args,
                        env: server.env,
                    }),
                );
            }
        } else {
            let server_type = server.r#type.as_deref().unwrap_or("unknown");
            tracing::warn!(
                "MCP server '{}' has unsupported type '{}' (only stdio supported)",
                name,
                server_type
            );
        }
    }

    McpServers::Dict(sdk_servers)
}

/// Lightweight check that only reads `~/.claude.json` to see if the
/// project path is listed in its `projects` map.
pub async fn is_project_registered(project_path: &Path) -> bool {
    let Some(claude_json) = read_claude_json().await else {
        return false;
    };
    let Some(projects) = &claude_json.projects else {
        return false;
    };
    find_project(projects, project_path).is_some()
}

/// Find a `project_path` in `projects`, dealing with special cases on Windows
/// where Claude stores them with slashes in paths, while we see them with backslashes
/// in native OS paths. This handling is only active on Windows to avoid the possibility
/// of false-positives on Linux.
/// As `projects` is based on an unmodified `~/.claude.json` file, this function
/// will have to adapt to changes without anticipating every possible future case.
fn find_project<'a>(
    projects: &'a HashMap<String, Project>,
    project_path: &Path,
) -> Option<&'a Project> {
    let path = project_path.to_string_lossy();
    #[cfg(not(windows))]
    {
        projects.get(path.as_ref())
    }
    #[cfg(windows)]
    {
        fn backslashes_to_slashes(path: &str) -> String {
            path.replace('\\', "/")
        }

        if let Some(project) = projects.get(path.as_ref()) {
            return Some(project);
        }

        // We might not have found it with backslashes, try again with slashes.
        if path.contains('\\') {
            let path_with_slashes = backslashes_to_slashes(path.as_ref());
            if let Some(project) = projects.get(&path_with_slashes) {
                return Some(project);
            }
        }

        None
    }
}

async fn read_claude_json() -> Option<ClaudeJson> {
    let home = dirs::home_dir()?;
    let path = home.join(".claude.json");
    let string = fs::read_to_string(&path).await.ok()?;
    let out = serde_json_lenient::from_str(&string).ok()?;
    Some(out)
}

async fn read_mcp_json(project_path: &Path) -> Option<McpJsonFile> {
    let path = project_path.join(".mcp.json");
    let string = fs::read_to_string(&path).await.ok()?;
    let out = serde_json_lenient::from_str(&string).ok()?;
    Some(out)
}

#[cfg(test)]
mod find_project_tests {
    use std::{collections::HashMap, path::PathBuf};

    use super::{Project, find_project};

    #[test]
    fn matches_windows_and_posix_separators() {
        let mut projects = HashMap::new();
        projects.insert(
            "C:/Users/test/workspace/repo".to_string(),
            Project { mcp_servers: None },
        );

        let project = find_project(&projects, &PathBuf::from(r"C:\Users\test\workspace\repo"));
        assert_eq!(
            project.is_some(),
            cfg!(windows),
            "On Windows, we match this path, on Unix we never do a conversion and paths have to match verbatim"
        );
    }

    #[test]
    fn we_must_use_filesystem_paths() {
        let mut projects = HashMap::new();
        projects.insert(
            "C:\\Users\\test\\workspace\\repo".to_string(),
            Project { mcp_servers: None },
        );

        let project = find_project(&projects, &PathBuf::from("C:/Users/test/workspace/repo"));
        assert!(
            project.is_none(),
            "if we were to use slashes in paths and claude.json doesn't, paths won't match"
        );
    }

    #[test]
    fn matches_direct_path_without_normalization() {
        let mut projects = HashMap::new();
        projects.insert(
            "/home/test/workspace/repo".to_string(),
            Project { mcp_servers: None },
        );

        let project = find_project(&projects, &PathBuf::from("/home/test/workspace/repo"));
        assert!(project.is_some());
    }
}
