use std::{collections::HashMap, env};

use but_settings::AppSettings;
use clap::ValueEnum;
use command_group::AsyncCommandGroup;
use posthog_rs::Client;
use serde::{Deserialize, Serialize};

use crate::args::CommandName;

#[derive(Debug, Clone)]
pub struct Metrics {
    sender: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    Mcp,
    McpInternal,
    #[strum(serialize = "Cli")]
    Cli(Command),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "camelCase")]
pub enum Command {
    Log,
    Status,
    Stf,
    Rub,
    Commit,
    Push,
    New,
    Describe,
    Oplog,
    Restore,
    Undo,
    Snapshot,
    Gui,
    BaseCheck,
    BaseUpdate,
    BranchNew,
    BranchDelete,
    BranchList,
    BranchApply,
    BranchUnapply,
    ClaudePreTool,
    ClaudePostTool,
    ClaudeStop,
    CursorAfterEdit,
    CursorStop,
    Worktree,
    Mark,
    Unmark,
    ForgeAuth,
    ForgeListUsers,
    ForgeForget,
    PublishReview,
    Completions,
    Absorb,
    Unknown,
}

impl From<CommandName> for EventKind {
    fn from(command_name: CommandName) -> Self {
        match command_name {
            CommandName::Log => EventKind::Cli(Command::Log),
            CommandName::Status => EventKind::Cli(Command::Status),
            CommandName::Stf => EventKind::Cli(Command::Stf),
            CommandName::Rub => EventKind::Cli(Command::Rub),
            CommandName::Commit => EventKind::Cli(Command::Commit),
            CommandName::Push => EventKind::Cli(Command::Push),
            CommandName::New => EventKind::Cli(Command::New),
            CommandName::Describe => EventKind::Cli(Command::Describe),
            CommandName::Oplog => EventKind::Cli(Command::Oplog),
            CommandName::Restore => EventKind::Cli(Command::Restore),
            CommandName::Undo => EventKind::Cli(Command::Undo),
            CommandName::Snapshot => EventKind::Cli(Command::Snapshot),
            CommandName::Gui => EventKind::Cli(Command::Gui),
            CommandName::BaseCheck => EventKind::Cli(Command::BaseCheck),
            CommandName::BaseUpdate => EventKind::Cli(Command::BaseUpdate),
            CommandName::BranchNew => EventKind::Cli(Command::BranchNew),
            CommandName::BranchDelete => EventKind::Cli(Command::BranchDelete),
            CommandName::BranchList => EventKind::Cli(Command::BranchList),
            CommandName::BranchApply => EventKind::Cli(Command::BranchApply),
            CommandName::BranchUnapply => EventKind::Cli(Command::BranchUnapply),
            CommandName::ClaudePreTool => EventKind::Cli(Command::ClaudePreTool),
            CommandName::ClaudePostTool => EventKind::Cli(Command::ClaudePostTool),
            CommandName::ClaudeStop => EventKind::Cli(Command::ClaudeStop),
            CommandName::CursorAfterEdit => EventKind::Cli(Command::CursorAfterEdit),
            CommandName::CursorStop => EventKind::Cli(Command::CursorStop),
            CommandName::Worktree => EventKind::Cli(Command::Worktree),
            CommandName::Mark => EventKind::Cli(Command::Mark),
            CommandName::Unmark => EventKind::Cli(Command::Unmark),
            CommandName::ForgeAuth => EventKind::Cli(Command::ForgeAuth),
            CommandName::ForgeListUsers => EventKind::Cli(Command::ForgeListUsers),
            CommandName::ForgeForget => EventKind::Cli(Command::ForgeForget),
            CommandName::PublishReview => EventKind::Cli(Command::PublishReview),
            CommandName::ReviewTemplate => EventKind::Cli(Command::PublishReview),
            CommandName::Completions => EventKind::Cli(Command::Completions),
            CommandName::Absorb => EventKind::Cli(Command::Absorb),
            CommandName::Unknown => EventKind::Cli(Command::Unknown),
        }
    }
}

pub struct Props {
    values: HashMap<String, serde_json::Value>,
}

impl Props {
    pub fn new() -> Self {
        Props {
            values: HashMap::new(),
        }
    }

    pub fn insert<K: Into<String>, V: Serialize>(&mut self, key: K, value: V) {
        if let Ok(value) = serde_json::to_value(value) {
            self.values.insert(key.into(), value);
        }
    }

    pub fn as_json_string(&self) -> String {
        serde_json::to_string(&self.values).unwrap_or_default()
    }

    pub fn from_json_string(json: &str) -> Result<Self, serde_json::Error> {
        let values: HashMap<String, serde_json::Value> = serde_json::from_str(json)?;
        Ok(Props { values })
    }

    pub fn update_event(&self, event: &mut Event) {
        for (key, value) in &self.values {
            event.insert_prop(key, value);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    event_name: EventKind,
    props: HashMap<String, serde_json::Value>,
}

impl Event {
    pub fn new(event_name: EventKind) -> Self {
        let event = &mut Event {
            event_name,
            props: HashMap::new(),
        };
        if let EventKind::Cli(command) = event_name {
            event.insert_prop("command", command);
        }
        event.insert_prop("appVersion", option_env!("VERSION").unwrap_or_default());
        event.insert_prop("releaseChannel", option_env!("CHANNEL").unwrap_or_default());
        event.insert_prop("appName", option_env!("CARGO_BIN_NAME").unwrap_or_default());
        event.insert_prop("OS", Event::normalize_os(env::consts::OS));
        event.insert_prop("Arch", env::consts::ARCH);
        event.clone()
    }

    pub fn insert_prop<K: Into<String>, P: Serialize>(&mut self, key: K, prop: P) {
        if let Ok(value) = serde_json::to_value(prop) {
            let _ = self.props.insert(key.into(), value);
        }
    }

    fn normalize_os(os: &str) -> String {
        match os {
            "macos" => "Mac OS X".to_string(),
            "windows" => "Windows".to_string(),
            "linux" => "Linux".to_string(),
            "android" => "Android".to_string(),
            _ => os.to_string(),
        }
    }
}

impl Metrics {
    pub fn new_with_background_handling(app_settings: &AppSettings) -> Self {
        let metrics_permitted = app_settings.telemetry.app_metrics_enabled;
        // Only create client and sender if metrics are permitted
        let client = posthog_client(app_settings.clone());
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let sender = if metrics_permitted {
            Some(sender)
        } else {
            None
        };
        let metrics = Metrics { sender };

        if let Some(client_future) = client {
            let mut receiver = receiver;
            let app_settings = app_settings.clone();
            tokio::task::spawn(async move {
                let client = client_future.await;
                while let Some(event) = receiver.recv().await {
                    do_capture(&client, event, &app_settings).await.ok();
                }
            });
        }

        metrics
    }

    pub fn capture(&self, event: &Event) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event.clone());
        }
    }

    pub async fn capture_blocking(app_settings: &AppSettings, event: Event) {
        if let Some(client) = posthog_client(app_settings.clone()) {
            do_capture(&client.await, event, app_settings).await.ok();
        }
    }
}

fn do_capture(
    client: &Client,
    event: Event,
    app_settings: &AppSettings,
) -> impl Future<Output = Result<(), posthog_rs::Error>> {
    let mut posthog_event = if let Some(id) = &app_settings.telemetry.app_distinct_id.clone() {
        posthog_rs::Event::new(event.event_name.to_string(), id.clone())
    } else {
        posthog_rs::Event::new_anon(event.event_name.to_string())
    };
    for (key, prop) in event.props {
        let _ = posthog_event.insert_prop(key, prop);
    }
    client.capture(posthog_event)
}

/// Creates a PostHog client if metrics are enabled and the API key is set.
fn posthog_client(app_settings: AppSettings) -> Option<impl Future<Output = posthog_rs::Client>> {
    if app_settings.telemetry.app_metrics_enabled {
        if let Some(api_key) = option_env!("POSTHOG_API_KEY") {
            let options = posthog_rs::ClientOptionsBuilder::default()
                .api_key(api_key.to_string())
                .api_endpoint("https://eu.i.posthog.com/i/v0/e/".to_string())
                .build()
                .ok()?;
            Some(posthog_rs::client(options))
        } else {
            None
        }
    } else {
        None
    }
}

/// If metrics are configured, this function spawns a process to handle metrics logging so that this CLI process can exit as soon as possible.
pub(crate) fn metrics_if_configured(
    app_settings: AppSettings,
    cmd: CommandName,
    props: Props,
) -> anyhow::Result<()> {
    if !app_settings.telemetry.app_metrics_enabled {
        return Ok(());
    }
    if let Some(v) = cmd.to_possible_value() {
        let binary_path = std::env::current_exe().unwrap_or_default();
        tokio::process::Command::new(binary_path)
            .arg("metrics")
            .arg("--command-name")
            .arg(v.get_name())
            .arg("--props")
            .arg(props.as_json_string())
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .group()
            .kill_on_drop(false)
            .spawn()?;
    }
    Ok(())
}
