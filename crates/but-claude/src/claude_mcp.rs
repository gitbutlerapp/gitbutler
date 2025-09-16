use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::claude_settings::ClaudeSettings;

/// Represents the MCP-relevant parts of Claude Json
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeJson {
    projects: Option<HashMap<String, Project>>,
    mcp_servers: Option<McpServers>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct Project {
    mcp_servers: Option<McpServers>,
}

/// Represents an Mcp Config.
///
/// This is the expected file format of a `.mcp.json`. It is also the format that
/// CC expects to be given when using the `--mcp-config` command.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    mcp_servers: McpServers,
}

type McpServers = HashMap<String, McpServer>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct McpServer {
    r#type: Option<String>,
    command: Option<String>,
    url: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct ClaudeMcpConfig {
    settings: ClaudeSettings,
    project_path: PathBuf,
    claude_json: Option<ClaudeJson>,
    mcp_json: Option<McpConfig>,
}

impl ClaudeMcpConfig {
    pub async fn open(settings: &ClaudeSettings, project_path: &Path) -> Self {
        Self {
            claude_json: read_claude_json().await,
            mcp_json: read_mcp_json(project_path).await,
            project_path: project_path.to_owned(),
            settings: settings.clone(),
        }
    }

    pub fn mcp_servers(&self) -> McpConfig {
        let mut out = McpConfig {
            mcp_servers: HashMap::new(),
        };

        if let Some(claude_json) = &self.claude_json {
            let servers = claude_json
                .projects
                .as_ref()
                .and_then(|projects| {
                    let path_str = self.project_path.to_string_lossy().to_string();
                    projects.get(&path_str).cloned()
                })
                .and_then(|project| project.mcp_servers);

            if let Some(servers) = servers {
                for (k, v) in servers {
                    out.mcp_servers.insert(k, v);
                }
            }

            if let Some(servers) = &claude_json.mcp_servers {
                for (k, v) in servers {
                    out.mcp_servers.insert(k.clone(), v.clone());
                }
            }
        }

        let all_enabled = self.settings.enable_all_project_mcp_servers();
        let enabled = self.settings.enabled_project_mcp_servers();

        if let Some(mcp_json) = &self.mcp_json {
            for (k, v) in &mcp_json.mcp_servers {
                if all_enabled || enabled.contains(k) {
                    out.mcp_servers.insert(k.clone(), v.clone());
                }
            }
        }

        out
    }

    pub fn mcp_servers_with_security(&self) -> McpConfig {
        let mut out = self.mcp_servers();
        out.mcp_servers.insert(
            "but-security".to_owned(),
            McpServer {
                r#type: Some("stdio".to_owned()),
                command: Some("but".to_owned()),
                url: None,
                args: Some(vec!["claude".to_owned(), "pp".to_owned()]),
                env: Some(HashMap::new()),
            },
        );
        out
    }
}

async fn read_claude_json() -> Option<ClaudeJson> {
    let home = dirs::home_dir()?;
    let path = home.join(".claude.json");
    let string = fs::read_to_string(&path).await.ok()?;
    let out = serde_json_lenient::from_str(&string).ok()?;
    Some(out)
}

async fn read_mcp_json(project_path: &Path) -> Option<McpConfig> {
    let path = project_path.join(".mcp.json");
    let string = fs::read_to_string(&path).await.ok()?;
    let out = serde_json_lenient::from_str(&string).ok()?;
    Some(out)
}

impl McpConfig {
    #[allow(unused)]
    pub(crate) fn exclude(&self, to_exclude: &[&str]) -> Self {
        let mut out = self.clone();
        for server in to_exclude {
            out.mcp_servers.remove(*server);
        }
        out
    }
}
