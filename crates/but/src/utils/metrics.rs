use std::{collections::HashMap, env};

use but_settings::AppSettings;
use clap::ValueEnum;
use command_group::AsyncCommandGroup;
use posthog_rs::Client;
use serde::{Deserialize, Serialize};

use crate::{
    args::{Subcommands, metrics::CommandName},
    utils::ResultMetricsExt,
};

pub(super) mod types {
    use crate::{args::metrics::CommandName, utils::metrics::Event};

    /// All we need to emit metrics as part of a command invocation, in the background, as spun-off process.
    pub struct OneshotMetricsContext {
        pub(super) start: std::time::Instant,
        pub command: CommandName,
    }

    /// A metrics implementation to run in the background, receiving metrics to send through a channel.
    #[derive(Debug, Clone)]
    pub struct BackgroundMetrics {
        pub(super) sender: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
    }
}
use types::{BackgroundMetrics, OneshotMetricsContext};

impl OneshotMetricsContext {
    pub fn new_if_enabled(settings: &AppSettings, cmd: CommandName) -> Option<Self> {
        settings.telemetry.app_metrics_enabled.then(|| Self {
            start: std::time::Instant::now(),
            command: cmd,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    Mcp,
    McpInternal,
    #[strum(serialize = "Cli")]
    Cli(CommandName),
}

impl Subcommands {
    /// Create all context that is needed to emit metrics for `self` once, if `settings` permit.
    pub fn to_metrics_context(&self, settings: &AppSettings) -> Option<OneshotMetricsContext> {
        let cmd = self.to_metrics_command();
        OneshotMetricsContext::new_if_enabled(settings, cmd)
    }

    fn to_metrics_command(&self) -> CommandName {
        use CommandName::*;

        use crate::args::{base, branch, claude, cursor, forge, worktree};
        match self {
            #[cfg(feature = "legacy")]
            Subcommands::Status { .. } => Status,
            #[cfg(feature = "legacy")]
            Subcommands::Stf { .. } => Stf,
            #[cfg(feature = "legacy")]
            Subcommands::Rub { .. } => Rub,
            #[cfg(feature = "legacy")]
            Subcommands::Diff { .. } => Diff,
            #[cfg(feature = "legacy")]
            Subcommands::Base(base::Platform { cmd }) => match cmd {
                base::Subcommands::Update => BaseUpdate,
                base::Subcommands::Check => BaseCheck,
                base::Subcommands::Fetch => BaseFetch,
            },
            Subcommands::Branch(branch::Platform { cmd }) => match cmd {
                None => BranchList,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::List { .. }) => BranchList,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::New { .. }) => BranchNew,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::Delete { .. }) => BranchDelete,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::Unapply { .. }) => BranchUnapply,
                Some(branch::Subcommands::Apply { .. }) => BranchApply,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::Show { .. }) => BranchShow,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Worktree(worktree::Platform { cmd: _ }) => Worktree,
            #[cfg(feature = "legacy")]
            Subcommands::Mark { .. } => Mark,
            #[cfg(feature = "legacy")]
            Subcommands::Unmark => Unmark,
            Subcommands::Gui => Gui,
            #[cfg(feature = "legacy")]
            Subcommands::Commit { .. } => Commit,
            #[cfg(feature = "legacy")]
            Subcommands::Push(_) => Push,
            #[cfg(feature = "legacy")]
            Subcommands::New { .. } => New,
            #[cfg(feature = "legacy")]
            Subcommands::Describe { .. } => Describe,
            #[cfg(feature = "legacy")]
            Subcommands::Oplog { .. } => Oplog,
            #[cfg(feature = "legacy")]
            Subcommands::Restore { .. } => Restore,
            #[cfg(feature = "legacy")]
            Subcommands::Undo => Undo,
            #[cfg(feature = "legacy")]
            Subcommands::Snapshot { .. } => Snapshot,
            #[cfg(feature = "legacy")]
            Subcommands::Claude(claude::Platform { cmd }) => match cmd {
                claude::Subcommands::PreTool => ClaudePreTool,
                claude::Subcommands::PostTool => ClaudePostTool,
                claude::Subcommands::Stop => ClaudeStop,
                claude::Subcommands::Last { .. }
                | claude::Subcommands::PermissionPromptMcp { .. } => Unknown,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Cursor(cursor::Platform { cmd }) => match cmd {
                cursor::Subcommands::AfterEdit => CursorAfterEdit,
                cursor::Subcommands::Stop { .. } => CursorStop,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Absorb { .. } => Absorb,
            #[cfg(feature = "legacy")]
            Subcommands::Review(forge::review::Platform { cmd }) => match cmd {
                forge::review::Subcommands::Publish { .. } => PublishReview,
                forge::review::Subcommands::Template { .. } => ReviewTemplate,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Actions(_) | Subcommands::Mcp { .. } | Subcommands::Init { .. } => Unknown,
            Subcommands::Forge(forge::integration::Platform { cmd }) => match cmd {
                forge::integration::Subcommands::Auth => ForgeAuth,
                forge::integration::Subcommands::Forget { .. } => ForgeForget,
                forge::integration::Subcommands::ListUsers => ForgeListUsers,
            },
            Subcommands::Completions { .. } => Completions,
            Subcommands::Metrics { .. } => Unknown,
        }
    }
}

impl From<CommandName> for EventKind {
    fn from(command_name: CommandName) -> Self {
        EventKind::Cli(command_name)
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

    pub fn from_result<E, T, R>(start: std::time::Instant, result: R) -> Props
    where
        R: std::ops::Deref<Target = anyhow::Result<T, E>>,
        E: std::fmt::Display,
    {
        let error = result.as_ref().err().map(|e| e.to_string());
        let mut props = Props::new();
        props.insert("durationMs", start.elapsed().as_millis());
        props.insert("error", error);
        props
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

impl From<EventKind> for Event {
    fn from(value: EventKind) -> Self {
        Event::new(value)
    }
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

impl BackgroundMetrics {
    pub fn new_in_background(app_settings: &AppSettings) -> Self {
        let metrics_permitted = app_settings.telemetry.app_metrics_enabled;
        // Only create client and sender if metrics are permitted
        let client = posthog_client(app_settings.clone());
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let sender = if metrics_permitted {
            Some(sender)
        } else {
            None
        };
        let metrics = BackgroundMetrics { sender };

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

    pub fn capture(&self, event: Event) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        }
    }
}

pub async fn capture_event_blocking(app_settings: &AppSettings, event: Event) {
    if let Some(client) = posthog_client(app_settings.clone()) {
        do_capture(&client.await, event, app_settings).await.ok();
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

impl<T> ResultMetricsExt for anyhow::Result<T> {
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> anyhow::Result<()> {
        let Some(OneshotMetricsContext { start, command }) = ctx else {
            return self.map(|_| ());
        };

        let props = Props::from_result(start, &self);
        let Some(v) = command.to_possible_value() else {
            tracing::warn!("BUG: didn't get string value for {command:?}");
            return self.map(|_| ());
        };

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
        self.map(|_| ())
    }
}
