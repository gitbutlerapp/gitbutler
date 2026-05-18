use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use agent_client_protocol::{
    AcpAgent, Agent, Client, ConnectionTo,
    schema::{
        InitializeRequest, NewSessionRequest, PermissionOptionKind, ProtocolVersion,
        RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
        SelectedPermissionOutcome, SessionNotification,
    },
};
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

const REGISTRY_URL: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcpCommandConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}
but_schemars::register_sdk_type!(AcpCommandConfig);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AcpAgentSource {
    BuiltIn,
    User,
    Registry,
}
but_schemars::register_sdk_type!(AcpAgentSource);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AcpAvailability {
    Available,
    MissingCommand,
    Suggestion,
}
but_schemars::register_sdk_type!(AcpAvailability);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcpAgentDescriptor {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: AcpAgentSource,
    pub availability: AcpAvailability,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub command_preview: String,
}
but_schemars::register_sdk_type!(AcpAgentDescriptor);

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcpDiscovery {
    pub agents: Vec<AcpAgentDescriptor>,
}
but_schemars::register_sdk_type!(AcpDiscovery);

pub fn built_in_agents() -> Vec<AcpCommandConfig> {
    vec![
        AcpCommandConfig {
            id: "codex".to_string(),
            name: "Codex CLI".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@zed-industries/codex-acp@latest".to_string(),
            ],
            env: BTreeMap::new(),
        },
        AcpCommandConfig {
            id: "claude-agent".to_string(),
            name: "Claude Agent".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@agentclientprotocol/claude-agent-acp@latest".to_string(),
            ],
            env: BTreeMap::new(),
        },
        AcpCommandConfig {
            id: "gemini".to_string(),
            name: "Gemini CLI".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "--".to_string(),
                "@google/gemini-cli@latest".to_string(),
                "--acp".to_string(),
            ],
            env: BTreeMap::new(),
        },
        AcpCommandConfig {
            id: "copilot".to_string(),
            name: "GitHub Copilot".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@github/copilot-language-server@latest".to_string(),
                "--acp".to_string(),
            ],
            env: BTreeMap::new(),
        },
        AcpCommandConfig {
            id: "opencode".to_string(),
            name: "OpenCode".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "opencode-ai@latest".to_string(),
                "acp".to_string(),
            ],
            env: BTreeMap::new(),
        },
    ]
}

pub async fn discover_agents(user_agents: Vec<AcpCommandConfig>) -> Result<AcpDiscovery> {
    let mut agents = Vec::new();
    for agent in built_in_agents() {
        agents.push(descriptor(agent, AcpAgentSource::BuiltIn, None));
    }
    for agent in user_agents {
        agents.push(descriptor(agent, AcpAgentSource::User, None));
    }

    match fetch_registry_suggestions().await {
        Ok(mut suggestions) => agents.append(&mut suggestions),
        Err(error) => tracing::debug!(%error, "failed to fetch ACP registry suggestions"),
    }

    Ok(AcpDiscovery { agents })
}

pub async fn prompt_once(config: AcpCommandConfig, cwd: PathBuf, prompt: String) -> Result<String> {
    let agent = AcpAgent::from_str(&agent_json(&config)?)
        .with_context(|| format!("failed to create ACP agent '{}'", config.id))?;

    Client
        .builder()
        .on_receive_notification(
            async move |_notification: SessionNotification, _cx| Ok(()),
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, _connection| {
                tracing::debug!(
                    ?request.tool_call,
                    ?request.options,
                    "denying ACP permission request"
                );
                let response = RequestPermissionResponse::new(deny_permission_outcome(&request));
                responder.respond(response)
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, move |connection: ConnectionTo<Agent>| {
            let cwd = cwd.clone();
            let prompt = prompt.clone();
            async move {
                connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1))
                    .block_task()
                    .await?;

                let response = connection
                    .send_request(NewSessionRequest::new(cwd))
                    .block_task()
                    .await?;
                let mut session = connection.attach_session(response, Vec::new())?;
                session.send_prompt(prompt)?;
                let output = session.read_to_string().await?;
                tracing::debug!(output_len = output.len(), "ACP agent response received");
                if output.trim().is_empty() {
                    return Err(agent_client_protocol::Error::internal_error()
                        .data("ACP agent returned an empty response"));
                }
                Ok(output)
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn deny_permission_outcome(request: &RequestPermissionRequest) -> RequestPermissionOutcome {
    request
        .options
        .iter()
        .find(|option| {
            matches!(
                option.kind,
                PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
            )
        })
        .map(|option| {
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                option.option_id.clone(),
            ))
        })
        .unwrap_or(RequestPermissionOutcome::Cancelled)
}

fn descriptor(
    config: AcpCommandConfig,
    source: AcpAgentSource,
    description: Option<String>,
) -> AcpAgentDescriptor {
    let availability = if matches!(source, AcpAgentSource::Registry) {
        AcpAvailability::Suggestion
    } else if command_available(&config.command) {
        AcpAvailability::Available
    } else {
        AcpAvailability::MissingCommand
    };
    let command_preview = command_preview(&config);
    AcpAgentDescriptor {
        id: config.id,
        name: config.name,
        description,
        source,
        availability,
        command: config.command,
        args: config.args,
        env: config.env,
        command_preview,
    }
}

async fn fetch_registry_suggestions() -> Result<Vec<AcpAgentDescriptor>> {
    let response = reqwest::Client::new()
        .get(REGISTRY_URL)
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;
    let registry: Registry = response.json().await?;
    Ok(registry
        .agents
        .into_iter()
        .filter_map(registry_agent_to_descriptor)
        .collect())
}

fn registry_agent_to_descriptor(agent: RegistryAgent) -> Option<AcpAgentDescriptor> {
    let npx = agent.distribution?.npx?;
    let mut args = vec!["-y".to_string(), npx.package];
    args.extend(npx.args);
    Some(descriptor(
        AcpCommandConfig {
            id: agent.id,
            name: agent.name,
            command: "npx".to_string(),
            args,
            env: BTreeMap::new(),
        },
        AcpAgentSource::Registry,
        agent.description,
    ))
}

fn agent_json(config: &AcpCommandConfig) -> Result<String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct EnvVar<'a> {
        name: &'a str,
        value: &'a str,
    }

    #[derive(Serialize)]
    struct JsonConfig<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        name: &'a str,
        command: &'a str,
        args: &'a [String],
        env: Vec<EnvVar<'a>>,
    }

    let command = resolve_command_for_spawn(&config.command)
        .with_context(|| format!("failed to resolve ACP command '{}'", config.command))?
        .to_string_lossy()
        .to_string();
    let env = config
        .env
        .iter()
        .map(|(name, value)| EnvVar { name, value })
        .collect();
    Ok(serde_json::to_string(&JsonConfig {
        kind: "stdio",
        name: &config.id,
        command: &command,
        args: &config.args,
        env,
    })?)
}

fn command_preview(config: &AcpCommandConfig) -> String {
    std::iter::once(config.command.as_str())
        .chain(config.args.iter().map(String::as_str))
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '@' | ':'))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn command_available(command: &str) -> bool {
    resolve_command_for_spawn(command).is_some()
}

fn resolve_command_for_spawn(command: &str) -> Option<OsString> {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 {
        return executable_candidates(command)
            .into_iter()
            .find(|candidate| {
                let candidate_path = Path::new(candidate);
                candidate_path.is_file()
            });
    }

    let paths = std::env::var_os("PATH")?;
    let candidates = executable_candidates(command);
    std::env::split_paths(&paths).find_map(|path| {
        candidates.iter().find_map(|candidate| {
            let candidate_path = path.join(candidate);
            candidate_path
                .is_file()
                .then(|| candidate_path.into_os_string())
        })
    })
}

#[cfg(windows)]
fn executable_candidates(command: &str) -> Vec<OsString> {
    let path = Path::new(command);
    if path.extension().is_some() {
        return vec![OsString::from(command)];
    }
    let pathext = std::env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
    let mut values: Vec<_> = pathext
        .to_string_lossy()
        .split(';')
        .filter(|ext| !ext.is_empty())
        .map(|ext| OsString::from(format!("{command}{ext}")))
        .collect();
    values.extend(
        pathext
            .to_string_lossy()
            .split(';')
            .filter(|ext| !ext.is_empty())
            .map(|ext| OsString::from(format!("{command}{}", ext.to_ascii_lowercase()))),
    );
    values.push(OsString::from(command));
    values
}

#[cfg(not(windows))]
fn executable_candidates(command: &str) -> Vec<OsString> {
    vec![OsString::from(command)]
}

#[derive(Debug, Deserialize)]
struct Registry {
    #[allow(dead_code)]
    version: Option<String>,
    #[serde(default)]
    agents: Vec<RegistryAgent>,
}

#[derive(Debug, Deserialize)]
struct RegistryAgent {
    id: String,
    name: String,
    description: Option<String>,
    distribution: Option<RegistryDistribution>,
}

#[derive(Debug, Deserialize)]
struct RegistryDistribution {
    npx: Option<RegistryNpx>,
}

#[derive(Debug, Deserialize)]
struct RegistryNpx {
    package: String,
    #[serde(default)]
    args: Vec<String>,
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::{PermissionOption, ToolCallUpdate, ToolCallUpdateFields};

    use super::*;

    #[cfg(windows)]
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(windows)]
    struct EnvGuard {
        path: Option<OsString>,
        pathext: Option<OsString>,
    }

    #[cfg(windows)]
    impl EnvGuard {
        fn set_path(path: &Path, pathext: &str) -> Self {
            let guard = Self {
                path: std::env::var_os("PATH"),
                pathext: std::env::var_os("PATHEXT"),
            };
            // Environment mutation is process-global; tests serialize this with ENV_LOCK.
            unsafe {
                std::env::set_var("PATH", path);
                std::env::set_var("PATHEXT", pathext);
            }
            guard
        }
    }

    #[cfg(windows)]
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Environment mutation is process-global; tests serialize this with ENV_LOCK.
            unsafe {
                if let Some(path) = &self.path {
                    std::env::set_var("PATH", path);
                } else {
                    std::env::remove_var("PATH");
                }
                if let Some(pathext) = &self.pathext {
                    std::env::set_var("PATHEXT", pathext);
                } else {
                    std::env::remove_var("PATHEXT");
                }
            }
        }
    }

    #[test]
    fn preview_quotes_only_when_needed() {
        let config = AcpCommandConfig {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            command: "node".to_string(),
            args: vec!["agent path.js".to_string(), "--acp".to_string()],
            env: BTreeMap::new(),
        };
        assert_eq!(command_preview(&config), "node \"agent path.js\" --acp");
    }

    #[test]
    fn registry_npx_agent_becomes_suggestion() {
        let json = r#"{
            "version": "1.0.0",
            "agents": [{
                "id": "codex",
                "name": "Codex",
                "description": "ACP adapter",
                "distribution": {
                    "npx": {
                        "package": "@zed-industries/codex-acp",
                        "args": ["--flag"]
                    }
                }
            }]
        }"#;
        let registry: Registry = serde_json::from_str(json).unwrap();
        let descriptor = registry_agent_to_descriptor(registry.agents.into_iter().next().unwrap())
            .expect("npx distribution is usable");
        assert_eq!(descriptor.source, AcpAgentSource::Registry);
        assert_eq!(descriptor.command, "npx");
        assert_eq!(
            descriptor.args,
            ["-y", "@zed-industries/codex-acp", "--flag"]
        );
        assert_eq!(descriptor.availability, AcpAvailability::Suggestion);
    }

    #[test]
    fn permission_request_denial_selects_reject_instead_of_cancelling_turn() {
        let request = RequestPermissionRequest::new(
            "session",
            ToolCallUpdate::new("tool", ToolCallUpdateFields::new()),
            vec![
                PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
                PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
            ],
        );

        let outcome = deny_permission_outcome(&request);

        assert!(
            matches!(
                outcome,
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome {
                    option_id,
                    ..
                }) if option_id.0.as_ref() == "reject"
            ),
            "denying a tool permission should not cancel the whole ACP prompt turn"
        );
    }

    #[cfg(windows)]
    #[test]
    fn agent_json_resolves_pathext_command_for_spawn() {
        let _guard = ENV_LOCK.lock().expect("environment mutation test lock");
        let temp_dir =
            std::env::temp_dir().join(format!("but-acp-pathext-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir is created for command resolution");
        std::fs::write(temp_dir.join("npx"), "").expect("extensionless npx fixture is written");
        std::fs::write(temp_dir.join("npx.cmd"), "@echo off\r\n")
            .expect("cmd npx fixture is written");
        let _env_guard = EnvGuard::set_path(&temp_dir, ".CMD");

        let config = AcpCommandConfig {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            command: "npx".to_string(),
            args: vec!["--version".to_string()],
            env: BTreeMap::new(),
        };
        let json: serde_json::Value =
            serde_json::from_str(&agent_json(&config).expect("agent json is generated"))
                .expect("agent json is valid");
        let command = json["command"]
            .as_str()
            .expect("serialized agent command is a string");

        assert!(
            command.to_ascii_lowercase().ends_with("npx.cmd"),
            "ACP spawn command should use PATHEXT executable candidate, got {command}"
        );
    }
}
