use std::{collections::HashMap, env};

use but_settings::AppSettings;
use clap::ValueEnum;
use command_group::AsyncCommandGroup;
use posthog_rs::Client;
use rand::{Rng, distr::OpenClosed01};
use serde::{Deserialize, Serialize};

use crate::{
    CliError,
    args::{Subcommands, config, metrics::CommandName},
    utils::{ResultMetricsExt, binary_path},
};

const RUB_ERROR_MESSAGE_MAX_CHARS: usize = 1024;

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

impl EventKind {
    /// Percentage sample rate, between 0 and 1.
    ///
    /// 1 indicates that the command should always be submitted to posthog, and
    /// 0 should never be submitted to posthog.
    pub fn sample_rate(&self) -> f32 {
        match self {
            Self::Mcp | Self::McpInternal => 1.0,
            Self::Cli(c) => c.sample_rate(),
        }
    }
}

impl Subcommands {
    /// Create all context that is needed to emit metrics for `self` once, if `settings` permit.
    pub fn to_metrics_context(&self, settings: &AppSettings) -> Option<OneshotMetricsContext> {
        let cmd = self.to_metrics_command();
        OneshotMetricsContext::new_if_enabled(settings, cmd)
    }

    /// Turn `self` into a `CommandName` that serves as metric identifier.
    pub(crate) fn to_metrics_command(&self) -> CommandName {
        use CommandName::*;

        use crate::args::{alias as alias_args, branch, forge, skill, worktree};
        match self {
            #[cfg(feature = "legacy")]
            Subcommands::Status { .. } => Status,
            #[cfg(feature = "legacy")]
            Subcommands::Tui { .. } => Tui,
            #[cfg(feature = "legacy")]
            Subcommands::Rub { .. } => Rub,
            #[cfg(feature = "legacy")]
            Subcommands::Diff { .. } => Diff,
            #[cfg(feature = "legacy")]
            Subcommands::Show { .. } => Show,
            #[cfg(feature = "legacy")]
            Subcommands::Pull { .. } => Pull,
            #[cfg(feature = "legacy")]
            Subcommands::Fetch => Pull,
            Subcommands::Branch(branch::Platform { cmd }) => match cmd {
                None => BranchList,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::List { .. }) => BranchList,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::New { .. }) => BranchNew,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::Delete { .. }) => BranchDelete,
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::Show { .. }) => BranchShow,
                Some(branch::Subcommands::Move { .. }) => BranchMove,
                #[cfg(not(feature = "legacy"))]
                Some(branch::Subcommands::Apply { .. }) => BranchApply,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Unapply { .. } => BranchUnapply,
            #[cfg(feature = "legacy")]
            Subcommands::Apply { .. } => BranchApply,
            #[cfg(feature = "legacy")]
            Subcommands::Worktree(worktree::Platform { cmd: _ }) => Worktree,
            #[cfg(feature = "legacy")]
            Subcommands::Mark { .. } => Mark,
            #[cfg(feature = "legacy")]
            Subcommands::Unmark => Unmark,
            Subcommands::Gui { .. } => Gui,
            #[cfg(feature = "legacy")]
            Subcommands::Commit(crate::args::commit::Platform { cmd, .. }) => match cmd {
                None => Commit,
                Some(crate::args::commit::Subcommands::Empty { .. }) => CommitEmpty,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Push(_) => Push,
            #[cfg(feature = "legacy")]
            Subcommands::Reword { .. } => Reword,
            #[cfg(feature = "legacy")]
            Subcommands::Oplog(crate::args::oplog::Platform { cmd }) => match cmd {
                None => OplogList,
                Some(crate::args::oplog::Subcommands::List { .. }) => OplogList,
                Some(crate::args::oplog::Subcommands::Snapshot { .. }) => OplogSnapshot,
                Some(crate::args::oplog::Subcommands::Restore { .. }) => Restore,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Undo => Undo,
            #[cfg(feature = "legacy")]
            Subcommands::Redo => Redo,
            #[cfg(feature = "legacy")]
            Subcommands::Absorb { .. } => Absorb,
            #[cfg(feature = "legacy")]
            Subcommands::Discard { .. } => Discard,
            #[cfg(feature = "legacy")]
            Subcommands::Pr(forge::pr::Platform { cmd, .. }) => match cmd {
                None | Some(forge::pr::Subcommands::New { .. }) => PrNew,
                Some(forge::pr::Subcommands::Template { .. }) => PrTemplate,
                Some(forge::pr::Subcommands::AutoMerge { off, .. }) => {
                    if *off {
                        DisableAutoMerge
                    } else {
                        EnableAutoMerge
                    }
                }
                Some(forge::pr::Subcommands::SetDraft { .. }) => SetReviewDraft,
                Some(forge::pr::Subcommands::SetReady { .. }) => SetReviewReady,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Actions(_)
            | Subcommands::Mcp
            | Subcommands::Setup { .. }
            | Subcommands::Teardown { .. } => Unknown,
            Subcommands::Config(config::Platform { cmd }) => match cmd {
                Some(config::Subcommands::Forge {
                    cmd: Some(config::ForgeSubcommand::Auth),
                }) => ForgeAuth,
                Some(config::Subcommands::Forge {
                    cmd: Some(config::ForgeSubcommand::Forget { .. }),
                }) => ForgeForget,
                Some(config::Subcommands::Forge {
                    cmd: Some(config::ForgeSubcommand::ListUsers),
                }) => ForgeListUsers,
                _ => Unknown,
            },
            Subcommands::Completions { .. } => Completions,
            Subcommands::Help => Unknown,
            Subcommands::Alias(alias_args::Platform { cmd }) => match cmd {
                None | Some(alias_args::Subcommands::List) => AliasCheck,
                Some(alias_args::Subcommands::Add { .. }) => AliasAdd,
                Some(alias_args::Subcommands::Remove { .. }) => AliasRemove,
            },
            Subcommands::Metrics { .. } => Unknown,
            Subcommands::Update { .. } => Update,
            #[cfg(feature = "legacy")]
            Subcommands::RefreshRemoteData { .. } => RefreshRemoteData,
            #[cfg(feature = "legacy")]
            Subcommands::Resolve { .. } => Resolve,
            #[cfg(feature = "legacy")]
            Subcommands::Uncommit { .. } => Rub,
            #[cfg(feature = "legacy")]
            Subcommands::Amend { .. } => Rub,
            #[cfg(feature = "legacy")]
            Subcommands::Stage { .. } => Rub,
            #[cfg(feature = "legacy")]
            Subcommands::Unstage { .. } => Rub,
            #[cfg(feature = "legacy")]
            Subcommands::Squash { .. } => Rub,
            #[cfg(feature = "legacy")]
            Subcommands::Merge { .. } => Merge,
            Subcommands::Move { .. } => Move,
            #[cfg(feature = "legacy")]
            Subcommands::Pick { .. } => Pick,
            Subcommands::Skill(skill::Platform { cmd }) => match cmd {
                skill::Subcommands::Install { .. } => SkillInstall,
                skill::Subcommands::Check { .. } => SkillCheck,
            },
            Subcommands::Edit { .. } => Edit,
            #[cfg(feature = "legacy")]
            Subcommands::Clean { .. } => Clean,
            Subcommands::Onboarding | Subcommands::EvalHook => Unknown,
            Subcommands::AgentLog { .. } => Unknown,
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

    fn from_result<E, T>(start: std::time::Instant, result: &Result<T, E>) -> Props
    where
        E: std::fmt::Display,
    {
        let error = result.as_ref().err();
        let mut props = Props::new();
        props.insert("durationMs", start.elapsed().as_millis());
        props.insert("error", error.map(|e| e.to_string()));
        props
    }

    fn from_anyhow_result<T>(
        start: std::time::Instant,
        result: &anyhow::Result<T>,
        command: CommandName,
    ) -> Props {
        let mut props = Self::from_result(start, result);
        let Some(error) = result.as_ref().err() else {
            return props;
        };
        if !matches!(command, CommandName::Rub) {
            return props;
        }

        let error_message = rub_error_message(error);
        props.insert("errorMessage", &error_message);
        props.insert(
            "errorRoot",
            rub_error_message(error.root_cause()).trim().to_string(),
        );
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

fn rub_error_message(error: &(impl std::fmt::Display + ?Sized)) -> String {
    let error_message = format!("{error:#}");
    let mut message = error_message.as_str();

    if let Some((value, _)) = message.split_once("\nHint: ") {
        message = value;
    }
    let message =
        if let Some((value, _)) = message.split_once(". If you just performed a Git operation") {
            format!("{value}.")
        } else {
            message.to_string()
        };

    let message = message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    truncate_rub_error_message(message)
}

fn truncate_rub_error_message(message: String) -> String {
    message.chars().take(RUB_ERROR_MESSAGE_MAX_CHARS).collect()
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
        if let Some(agent) = super::detect_agent::detect() {
            event.insert_prop("agent", agent.name());
        }
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

/// Capture an event *only* if `app_settings.telemetry.app_metrics_enabled` is `true`.
pub async fn capture_event_blocking(app_settings: &AppSettings, event: Event) {
    if let Some(client) = posthog_client(app_settings.clone()) {
        do_capture(&client.await, event, app_settings).await.ok();
    }
}

/// Note that `client` is *only* available if telemetry is enabled.
async fn do_capture(
    client: &Client,
    event: Event,
    app_settings: &AppSettings,
) -> Result<(), posthog_rs::Error> {
    if event.event_name.sample_rate() < rand::rng().sample::<f32, _>(OpenClosed01) {
        return Ok(());
    }

    let id = if let Some(id) = app_settings.telemetry.app_distinct_id.clone() {
        id
    } else if app_settings.telemetry.app_non_anon_metrics_enabled {
        machine()
    } else {
        "anonymous".to_string()
    };
    let mut posthog_event = posthog_rs::Event::new(event.event_name.to_string(), id);
    for (key, prop) in event.props {
        let _ = posthog_event.insert_prop(key, prop);
    }
    client.capture(posthog_event).await
}

fn machine() -> String {
    if let Ok(id) = machine_uid::get() {
        format!(
            "machine_{:x}",
            <sha2::Sha256 as sha2::Digest>::digest(format!("{}{}", id, "gitbutler").as_bytes())
        )
    } else {
        "anonymous".to_string()
    }
}

/// Creates a PostHog client if metrics are enabled and the API key is set.
fn posthog_client(app_settings: AppSettings) -> Option<impl Future<Output = posthog_rs::Client>> {
    if app_settings.telemetry.app_metrics_enabled
        && let Some(api_key) = option_env!("POSTHOG_API_KEY")
    {
        let options = posthog_rs::ClientOptionsBuilder::default()
            .api_key(api_key.to_string())
            .host("https://eu.i.posthog.com".to_string())
            .build()
            .ok()?;
        Some(posthog_rs::client(options))
    } else {
        None
    }
}

impl<T> ResultMetricsExt<T, anyhow::Error> for anyhow::Result<T> {
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> anyhow::Result<T> {
        let Some(OneshotMetricsContext { start, command }) = ctx else {
            return self;
        };

        let props = Props::from_anyhow_result(start, &self, command);
        emit_metrics(command, &props);
        self
    }
}

impl<T> ResultMetricsExt<T, CliError> for Result<T, CliError> {
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> Result<T, CliError> {
        let Some(OneshotMetricsContext { start, command }) = ctx else {
            return self;
        };

        let props = Props::from_result(start, &self);
        emit_metrics(command, &props);
        self
    }
}

fn emit_metrics(command: CommandName, props: &Props) {
    let Some(v) = command.to_possible_value() else {
        tracing::warn!("BUG: didn't get string value for {command:?}");
        return;
    };

    // We can fail both in resolving the path to the but binary, and in invoking it. As metrics
    // emissions shouldn't impact user experience, we swallow these errors.
    let but_path = match binary_path::current_exe_for_but_exec() {
        Err(err) => {
            tracing::warn!(?err, "Failed to resolve binary path to `but`");
            return;
        }
        Ok(path) => path,
    };

    let _ = tokio::process::Command::new(but_path)
        .arg("metrics")
        .arg("--command-name")
        .arg(v.get_name())
        .arg("--props")
        .arg(props.as_json_string())
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .group()
        .kill_on_drop(false)
        .spawn()
        .map_err(|err| tracing::warn!(?err, "Failed to emit metrics"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rub_metrics_include_detailed_error_properties() {
        let result = Err::<(), _>(
            anyhow::anyhow!(
                "Source 'old-id' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state."
            )
            .context("Failed to stage."),
        );

        let props = Props::from_anyhow_result(std::time::Instant::now(), &result, CommandName::Rub);

        assert_eq!(props.values["error"], "Failed to stage.");
        assert_eq!(
            props.values["errorMessage"],
            "Failed to stage.: Source 'old-id' not found."
        );
        assert_eq!(props.values["errorRoot"], "Source 'old-id' not found.");
    }

    #[test]
    fn non_rub_metrics_do_not_include_detailed_error_properties() {
        let result =
            Err::<(), _>(anyhow::anyhow!("Source 'old-id' not found.").context("Failed to stage."));

        let props =
            Props::from_anyhow_result(std::time::Instant::now(), &result, CommandName::Commit);

        assert_eq!(props.values["error"], "Failed to stage.");
        assert!(!props.values.contains_key("errorMessage"));
        assert!(!props.values.contains_key("errorRoot"));
    }

    #[test]
    fn rub_metrics_normalize_multiline_error_properties() {
        let result =
            Err::<(), _>(anyhow::anyhow!("first line\nsecond line").context("Failed to stage."));

        let props = Props::from_anyhow_result(std::time::Instant::now(), &result, CommandName::Rub);

        assert_eq!(
            props.values["errorMessage"],
            "Failed to stage.: first line second line"
        );
        assert_eq!(props.values["errorRoot"], "first line second line");
    }

    #[test]
    fn rub_metrics_cap_detailed_error_properties() {
        let result = Err::<(), _>(anyhow::anyhow!("{}", "a".repeat(1100)));

        let props = Props::from_anyhow_result(std::time::Instant::now(), &result, CommandName::Rub);

        assert_eq!(
            props.values["errorMessage"].as_str().unwrap().len(),
            RUB_ERROR_MESSAGE_MAX_CHARS
        );
        assert_eq!(
            props.values["errorRoot"].as_str().unwrap().len(),
            RUB_ERROR_MESSAGE_MAX_CHARS
        );
    }
}
