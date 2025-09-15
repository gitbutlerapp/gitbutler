use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use serde::Deserialize;
use tokio::fs;

/// Represents the merged CC settings
///
/// See
/// https://www.notion.so/gitbutler/MCP-Servers-26f5a4bfdeac80bab6efd6a01c05cd0a
/// for more details.
///
/// As a rule, none of the functions related to these types should hard error.
/// We should try to deal with settings as gracefully as possible; making the
/// assumption that the user might have left one of their settings files in an
/// invalid state.
#[derive(Debug, Clone)]
pub struct ClaudeSettings {
    settings: Vec<ClaudeSetting>,
}

#[derive(Deserialize, Debug, Clone)]
struct ClaudeSetting {
    env: Option<HashMap<String, String>>,
    #[serde(rename = "enableAllProjectMcpServers")]
    enable_all_project_mcp_servers: Option<bool>,
    #[serde(rename = "enableMcpjsonServers")]
    enabled_project_mcp_servers: Option<Vec<String>>,
}

#[cfg(target_os = "macos")]
const ENTERPRISE_PATH: &str = "/Library/Application Support/ClaudeCode/managed-settings.json";
#[cfg(target_os = "linux")]
const ENTERPRISE_PATH: &str = "/etc/claude-code/managed-settings.json";
#[cfg(target_os = "windows")]
const ENTERPRISE_PATH: &str = "C:\\ProgramData\\ClaudeCode\\managed-settings.json";

impl ClaudeSettings {
    /// Opens the claude settings
    pub async fn open(project_path: &std::path::Path) -> Self {
        let mut settings = vec![];

        // We try to open paths as a best effort, we don't hard fail if one
        // settings file is formatted invalidly
        let potential_paths = paths(project_path).await;
        for path in potential_paths {
            if let Ok(true) = fs::try_exists(&path).await {
                let string = fs::read_to_string(&path).await;
                if let Ok(string) = string {
                    let setting = serde_json_lenient::from_str(&string);
                    if let Ok(setting) = setting {
                        settings.push(setting);
                    }
                }
            }
        }

        Self { settings }
    }

    #[allow(unused)]
    pub fn enable_all_project_mcp_servers(&self) -> bool {
        let mut out = false;

        for setting in &self.settings {
            if let Some(enabled) = setting.enable_all_project_mcp_servers {
                out = enabled
            }
        }

        out
    }

    #[allow(unused)]
    pub fn enabled_project_mcp_servers(&self) -> HashSet<String> {
        let mut out = HashSet::new();

        for setting in &self.settings {
            if let Some(enabled) = &setting.enabled_project_mcp_servers {
                for name in enabled {
                    out.insert(name.clone());
                }
            }
        }

        out
    }

    pub fn env(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();

        for setting in &self.settings {
            if let Some(env) = &setting.env {
                for (k, v) in env {
                    out.insert(k.clone(), v.clone());
                }
            }
        }

        out
    }
}

/// The potential settings paths ordered from lowest priority to highest priority
async fn paths(project_path: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = vec![];

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".claude/settings.json"));
    }

    paths.push(project_path.join(".claude/settings.json"));
    paths.push(project_path.join(".claude/settings.local.json"));
    paths.push(PathBuf::from(ENTERPRISE_PATH));

    paths
}
