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

const ERROR_MESSAGE_MAX_CHARS: usize = 1024;

pub(super) mod types {
    use crate::{args::metrics::CommandName, utils::metrics::Event};

    /// All we need to emit metrics as part of a command invocation, in the background, as spun-off process.
    pub struct OneshotMetricsContext {
        pub(super) start: std::time::Instant,
        pub command: CommandName,
        pub(super) extra_props: Vec<(String, serde_json::Value)>,
    }

    /// A metrics implementation to run in the background, receiving metrics to send through a channel.
    #[derive(Debug, Clone)]
    pub struct BackgroundMetrics {
        pub(super) sender: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
    }
}
use types::{BackgroundMetrics, OneshotMetricsContext};

impl OneshotMetricsContext {
    pub fn new(cmd: CommandName, extra_props: Vec<(String, serde_json::Value)>) -> Self {
        Self {
            start: std::time::Instant::now(),
            command: cmd,
            extra_props,
        }
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
        if !settings.telemetry.app_metrics_enabled {
            return None;
        }
        let cmd = self.to_metrics_command();
        let extra_props = self.to_metrics_extra_props();
        Some(OneshotMetricsContext::new(cmd, extra_props))
    }

    /// Turn `self` into a `CommandName` that serves as metric identifier.
    pub(crate) fn to_metrics_command(&self) -> CommandName {
        use CommandName::*;

        use crate::args::{alias as alias_args, branch, forge, skill, update, worktree};
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
                Some(branch::Subcommands::Update { .. }) => BranchUpdate,
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
            #[cfg(all(feature = "legacy", feature = "but-2"))]
            Subcommands::Commit2(..) => Commit2,
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
            Subcommands::Update(update::Platform { cmd }) => match cmd {
                update::Subcommands::Check => UpdateCheck,
                update::Subcommands::Suppress { .. } => UpdateSuppress,
                #[cfg(all(unix, not(feature = "packaged-but-distribution")))]
                update::Subcommands::Install { .. } => UpdateInstall,
            },
            #[cfg(feature = "legacy")]
            Subcommands::RefreshRemoteData { .. } => RefreshRemoteData,
            #[cfg(feature = "legacy")]
            Subcommands::Resolve { .. } => Resolve,
            #[cfg(feature = "legacy")]
            Subcommands::Uncommit { .. } => Uncommit,
            #[cfg(feature = "legacy")]
            Subcommands::Amend { .. } => Amend,
            #[cfg(feature = "legacy")]
            Subcommands::Stage { .. } => Stage,
            #[cfg(feature = "legacy")]
            Subcommands::Unstage { .. } => Unstage,
            #[cfg(feature = "legacy")]
            Subcommands::Squash { .. } => Squash,
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
            Subcommands::External(_) => External,
        }
    }

    /// Additional low-cardinality dimensions for command modifiers.
    ///
    /// `sourceKind` and `targetKind` describe the kind a command expects, not a
    /// resolved runtime ID.
    pub(crate) fn to_metrics_extra_props(&self) -> Vec<(String, serde_json::Value)> {
        use crate::args::skill;

        let mut props = Vec::new();
        match self {
            #[cfg(feature = "legacy")]
            Subcommands::Uncommit { discard, diff, .. } => {
                push_prop(&mut props, "uncommitDiscard", *discard);
                push_prop(&mut props, "uncommitDiff", *diff);
                push_prop(&mut props, "sourceKind", "commitOrCommittedFile");
                if !*discard {
                    push_prop(&mut props, "targetKind", "unassigned");
                }
            }
            #[cfg(feature = "legacy")]
            Subcommands::Amend { .. } => {
                push_prop(&mut props, "sourceKind", "fileOrHunk");
                push_prop(&mut props, "targetKind", "commit");
            }
            #[cfg(feature = "legacy")]
            Subcommands::Stage { file_or_hunk, .. } => {
                push_prop(
                    &mut props,
                    "stageMode",
                    if file_or_hunk.is_some() {
                        "direct"
                    } else {
                        "interactive"
                    },
                );
                if file_or_hunk.is_some() {
                    push_prop(&mut props, "sourceKind", "fileOrHunk");
                }
                push_prop(&mut props, "targetKind", "branch");
            }
            #[cfg(feature = "legacy")]
            Subcommands::Unstage { .. } => {
                push_prop(&mut props, "sourceKind", "fileOrHunk");
                push_prop(&mut props, "targetKind", "branch");
            }
            #[cfg(feature = "legacy")]
            Subcommands::Squash { .. } => {
                push_prop(&mut props, "sourceKind", "commitOrBranch");
                push_prop(&mut props, "targetKind", "commit");
            }
            Subcommands::Move { .. } => {
                push_prop(&mut props, "sourceKind", "commitOrBranch");
                push_prop(&mut props, "targetKind", "commitOrBranchOrUnassigned");
            }
            Subcommands::Skill(skill::Platform { cmd }) => match cmd {
                skill::Subcommands::Install { .. } => {}
                skill::Subcommands::Check { update, .. } => {
                    push_prop(&mut props, "skillCheckUpdate", *update);
                }
            },
            _ => {}
        }
        props
    }
}

fn push_prop<T: Serialize>(props: &mut Vec<(String, serde_json::Value)>, key: &str, value: T) {
    if let Ok(value) = serde_json::to_value(value) {
        props.push((key.to_string(), value));
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

    fn from_anyhow_result<T>(
        start: std::time::Instant,
        result: &anyhow::Result<T>,
        command: CommandName,
    ) -> Props {
        let mut props = Props::new();
        props.insert("durationMs", start.elapsed().as_millis());
        let Some(error) = result.as_ref().err() else {
            props.insert("error", Option::<String>::None);
            return props;
        };

        props.insert_internal_error_details(error, command);
        props
    }

    fn from_cli_error_result<T>(
        start: std::time::Instant,
        result: &Result<T, CliError>,
        command: CommandName,
    ) -> Props {
        let mut props = Props::new();
        props.insert("durationMs", start.elapsed().as_millis());
        let Some(error) = result.as_ref().err() else {
            props.insert("error", Option::<String>::None);
            return props;
        };

        match error {
            CliError::BadInput(bad_input) => {
                props.insert("error", "Bad input");
                props.insert("errorKind", "badInput");
                if let Some(arg_name) = bad_input.argument_name() {
                    props.insert("badInputArgName", arg_name);
                }
                props.insert("badInputHasHint", bad_input.has_hint());
            }
            CliError::ExternalCommandNotFound(_) => {
                props.insert("error", "Unrecognized subcommand");
                props.insert("errorKind", "externalCommandNotFound");
            }
            CliError::Internal(error) => {
                props.insert_internal_error_details(error, command);
            }
        }
        props
    }

    pub fn insert<K: Into<String>, V: Serialize>(&mut self, key: K, value: V) {
        if let Ok(value) = serde_json::to_value(value) {
            self.values.insert(key.into(), value);
        }
    }

    fn extend(&mut self, props: Vec<(String, serde_json::Value)>) {
        for (key, value) in props {
            self.values.insert(key, value);
        }
    }

    fn insert_internal_error_details(&mut self, error: &anyhow::Error, command: CommandName) {
        self.insert("error", "Internal error");
        self.insert("errorKind", "internal");
        if captures_detailed_error_message(command) {
            self.insert("errorMessage", error_message(error));
            self.insert(
                "errorRoot",
                error_message(error.root_cause()).trim().to_string(),
            );
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

fn error_message(error: &(impl std::fmt::Display + ?Sized)) -> String {
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
    truncate_error_message(message)
}

fn captures_detailed_error_message(command: CommandName) -> bool {
    matches!(
        command,
        CommandName::Rub
            | CommandName::Uncommit
            | CommandName::Amend
            | CommandName::Stage
            | CommandName::Unstage
            | CommandName::Squash
    )
}

fn truncate_error_message(message: String) -> String {
    message.chars().take(ERROR_MESSAGE_MAX_CHARS).collect()
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
        event.insert_prop("$os", Event::normalize_os(env::consts::OS));
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

    let id = app_settings
        .telemetry
        .app_distinct_id
        .clone()
        .unwrap_or_else(machine);
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
        let Some(OneshotMetricsContext {
            start,
            command,
            extra_props,
        }) = ctx
        else {
            return self;
        };

        let mut props = Props::from_anyhow_result(start, &self, command);
        props.extend(extra_props);
        emit_metrics(command, &props);
        self
    }
}

impl<T> ResultMetricsExt<T, CliError> for Result<T, CliError> {
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> Result<T, CliError> {
        let Some(OneshotMetricsContext {
            start,
            command,
            extra_props,
        }) = ctx
        else {
            return self;
        };

        let mut props = Props::from_cli_error_result(start, &self, command);
        props.extend(extra_props);
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
    use crate::{
        args::{Subcommands, update},
        bad_input,
    };

    fn prop<'a>(
        props: &'a [(String, serde_json::Value)],
        key: &str,
    ) -> Option<&'a serde_json::Value> {
        props
            .iter()
            .find_map(|(prop_key, value)| (prop_key.as_str() == key).then_some(value))
    }

    fn assert_command(subcommand: Subcommands, expected: &str) {
        assert_eq!(
            Event::new(EventKind::Cli(subcommand.to_metrics_command())).props["command"],
            serde_json::json!(expected)
        );
    }

    #[test]
    fn metrics_use_invoked_command_names() {
        assert_command(
            Subcommands::Update(update::Platform {
                cmd: update::Subcommands::Check,
            }),
            "updateCheck",
        );
        assert_command(
            Subcommands::Update(update::Platform {
                cmd: update::Subcommands::Suppress { days: 7 },
            }),
            "updateSuppress",
        );
        assert_command(
            Subcommands::Move {
                source: "c1".into(),
                target: "main".into(),
                after: false,
            },
            "move",
        );

        #[cfg(all(unix, not(feature = "packaged-but-distribution")))]
        assert_command(
            Subcommands::Update(update::Platform {
                cmd: update::Subcommands::Install {
                    target: Some("0.20.0".into()),
                },
            }),
            "updateInstall",
        );

        #[cfg(feature = "legacy")]
        {
            assert_command(
                Subcommands::Amend {
                    target_or_source: "c1".into(),
                    legacy_commit: None,
                    changes: vec!["a1".into()],
                },
                "amend",
            );
            assert_command(
                Subcommands::Stage {
                    file_or_hunk: Some("a1".into()),
                    branch_pos: Some("main".into()),
                    branch: None,
                },
                "stage",
            );
        }
    }

    #[test]
    fn extra_props_keep_useful_source_and_target_kinds() {
        let moved = Subcommands::Move {
            source: "c1".into(),
            target: "main".into(),
            after: false,
        };
        let props = moved.to_metrics_extra_props();
        assert_eq!(
            prop(&props, "sourceKind"),
            Some(&serde_json::json!("commitOrBranch"))
        );
        assert_eq!(
            prop(&props, "targetKind"),
            Some(&serde_json::json!("commitOrBranchOrUnassigned"))
        );

        #[cfg(feature = "legacy")]
        {
            let direct_stage = Subcommands::Stage {
                file_or_hunk: Some("a1".into()),
                branch_pos: Some("main".into()),
                branch: None,
            };
            let props = direct_stage.to_metrics_extra_props();
            assert_eq!(
                prop(&props, "stageMode"),
                Some(&serde_json::json!("direct"))
            );
            assert_eq!(
                prop(&props, "sourceKind"),
                Some(&serde_json::json!("fileOrHunk"))
            );
            assert_eq!(
                prop(&props, "targetKind"),
                Some(&serde_json::json!("branch"))
            );

            let discard = Subcommands::Uncommit {
                source: "c1".into(),
                discard: true,
                diff: false,
            };
            let props = discard.to_metrics_extra_props();
            assert_eq!(
                prop(&props, "sourceKind"),
                Some(&serde_json::json!("commitOrCommittedFile"))
            );
            assert_eq!(
                prop(&props, "uncommitDiff"),
                Some(&serde_json::json!(false))
            );
            assert_eq!(prop(&props, "targetKind"), None);

            let with_diff = Subcommands::Uncommit {
                source: "c1".into(),
                discard: false,
                diff: true,
            };
            let props = with_diff.to_metrics_extra_props();
            assert_eq!(prop(&props, "uncommitDiff"), Some(&serde_json::json!(true)));
            assert_eq!(
                prop(&props, "targetKind"),
                Some(&serde_json::json!("unassigned"))
            );
        }
    }

    #[test]
    fn internal_error_details_are_allowlisted() {
        let anyhow_result = Err::<(), _>(
            anyhow::anyhow!("stale id. If you just performed a Git operation, refresh")
                .context("Failed to stage."),
        );

        let props = Props::from_anyhow_result(
            std::time::Instant::now(),
            &anyhow_result,
            CommandName::Stage,
        );

        assert_eq!(props.values["error"], "Internal error");
        assert_eq!(props.values["errorKind"], "internal");
        assert_eq!(props.values["errorMessage"], "Failed to stage.: stale id.");
        assert_eq!(props.values["errorRoot"], "stale id.");

        let result = Err::<(), _>(
            anyhow::anyhow!("private-branch-name failed").context("private-path failed"),
        );

        let props =
            Props::from_anyhow_result(std::time::Instant::now(), &result, CommandName::Commit);

        assert_eq!(props.values["error"], "Internal error");
        assert_eq!(props.values["errorKind"], "internal");
        assert!(!props.values.contains_key("errorMessage"));
        assert!(!props.values.contains_key("errorRoot"));
        assert!(!props.as_json_string().contains("private-branch-name"));
        assert!(!props.as_json_string().contains("private-path"));
    }

    #[test]
    fn cli_error_metrics_use_low_cardinality_failure_details() {
        let bad_input_result = Err::<(), _>(
            bad_input("Branch 'branch-with-private-name' not found")
                .arg_name("<BRANCH>")
                .arg_value("another-private-branch-name")
                .hint("Use a branch name")
                .into(),
        );

        let props = Props::from_cli_error_result(
            std::time::Instant::now(),
            &bad_input_result,
            CommandName::Stage,
        );

        assert_eq!(props.values["errorKind"], "badInput");
        assert_eq!(props.values["error"], "Bad input");
        assert!(!props.values.contains_key("errorMessage"));
        assert_eq!(props.values["badInputArgName"], "<BRANCH>");
        assert_eq!(props.values["badInputHasHint"], true);
        assert!(!props.as_json_string().contains("branch-with-private-name"));
        assert!(
            !props
                .as_json_string()
                .contains("another-private-branch-name")
        );

        let external_result = Err::<(), _>(CliError::ExternalCommandNotFound("typo".into()));
        let props = Props::from_cli_error_result(
            std::time::Instant::now(),
            &external_result,
            CommandName::External,
        );

        assert_eq!(props.values["error"], "Unrecognized subcommand");
        assert_eq!(props.values["errorKind"], "externalCommandNotFound");
        assert!(!props.values.contains_key("errorMessage"));
        assert!(!props.as_json_string().contains("typo"));
    }

    #[test]
    fn detailed_error_messages_are_normalized_and_capped() {
        let multiline_result =
            Err::<(), _>(anyhow::anyhow!("first line\nsecond line").context("Failed to stage."));

        let props = Props::from_anyhow_result(
            std::time::Instant::now(),
            &multiline_result,
            CommandName::Stage,
        );

        assert_eq!(
            props.values["errorMessage"],
            "Failed to stage.: first line second line"
        );
        assert_eq!(props.values["errorRoot"], "first line second line");

        let long_result = Err::<(), _>(anyhow::anyhow!("{}", "a".repeat(1100)));

        let props =
            Props::from_anyhow_result(std::time::Instant::now(), &long_result, CommandName::Stage);

        assert_eq!(
            props.values["errorMessage"].as_str().unwrap().len(),
            ERROR_MESSAGE_MAX_CHARS
        );
        assert_eq!(
            props.values["errorRoot"].as_str().unwrap().len(),
            ERROR_MESSAGE_MAX_CHARS
        );
    }
}
