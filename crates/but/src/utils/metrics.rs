use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

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
const UNRECOGNIZED_SUBCOMMAND_MAX_CHARS: usize = 64;
const INVALID_UNRECOGNIZED_SUBCOMMAND: &str = "<invalid>";

pub(super) mod types {
    use crate::args::metrics::CommandName;

    /// All we need to emit metrics as part of a command invocation, in the background, as spun-off process.
    pub struct OneshotMetricsContext {
        pub(super) start: std::time::Instant,
        pub command: CommandName,
        pub(super) extra_props: Vec<(String, serde_json::Value)>,
        pub(super) current_dir: std::path::PathBuf,
    }
}
use types::OneshotMetricsContext;

impl OneshotMetricsContext {
    pub fn new(
        cmd: CommandName,
        extra_props: Vec<(String, serde_json::Value)>,
        current_dir: PathBuf,
    ) -> Self {
        Self {
            start: std::time::Instant::now(),
            command: cmd,
            extra_props,
            current_dir,
        }
    }

    pub(crate) fn push_extra_prop<T: Serialize>(&mut self, key: &str, value: T) {
        push_prop(&mut self.extra_props, key, value);
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
    pub fn to_metrics_context(
        &self,
        settings: &AppSettings,
        current_dir: &Path,
    ) -> Option<OneshotMetricsContext> {
        if !settings.telemetry.app_metrics_enabled {
            return None;
        }
        // The comments experiment emits no metrics while the idea is being validated.
        if matches!(self, Subcommands::_Comment(_)) {
            return None;
        }
        let cmd = self.to_metrics_command();
        let extra_props = self.to_metrics_extra_props();
        Some(OneshotMetricsContext::new(
            cmd,
            extra_props,
            current_dir.to_owned(),
        ))
    }

    /// Turn `self` into a `CommandName` that serves as metric identifier.
    pub(crate) fn to_metrics_command(&self) -> CommandName {
        use CommandName::*;

        use crate::args::{agent, alias as alias_args, branch, forge, skill, update, worktree};
        match self {
            // Unreachable: the comments experiment opts out of metrics in `to_metrics_context`.
            Subcommands::_Comment(_) => Unknown,
            #[cfg(feature = "legacy")]
            Subcommands::Status { .. } => Status,
            #[cfg(feature = "legacy")]
            Subcommands::Tui { .. } => Tui,
            #[cfg(feature = "legacy")]
            Subcommands::Diff { .. } => Diff,
            #[cfg(feature = "legacy")]
            Subcommands::_Diff2(..) => Diff2,
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
            },
            #[cfg(feature = "legacy")]
            Subcommands::Unapply { .. } => BranchUnapply,
            #[cfg(feature = "legacy")]
            Subcommands::Apply { .. } => BranchApply,
            Subcommands::Switch { .. } => Switch,
            #[cfg(feature = "legacy")]
            Subcommands::Worktree(worktree::Platform { cmd: _ }) => Worktree,
            Subcommands::Gui { .. } => Gui,
            Subcommands::_Open { .. } => Open,
            #[cfg(feature = "legacy")]
            Subcommands::Commit(..) => Commit,
            #[cfg(feature = "legacy")]
            Subcommands::Push(_) => Push,
            #[cfg(feature = "legacy")]
            Subcommands::Reword { .. } | Subcommands::_Reword2(..) => Reword,
            #[cfg(feature = "legacy")]
            Subcommands::Oplog(crate::args::oplog::Platform { cmd }) => match cmd {
                None => OplogList,
                Some(crate::args::oplog::Subcommands::List { .. }) => OplogList,
                Some(crate::args::oplog::Subcommands::Snapshot { .. }) => OplogSnapshot,
                Some(crate::args::oplog::Subcommands::Restore { .. }) => Restore,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Undo(..) => Undo,
            #[cfg(feature = "legacy")]
            Subcommands::Redo(..) => Redo,
            #[cfg(feature = "legacy")]
            Subcommands::Absorb { .. } => Absorb,
            #[cfg(feature = "legacy")]
            Subcommands::Discard(..) => Discard,
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
            Subcommands::Mcp(_) => Unknown,
            #[cfg(feature = "legacy")]
            Subcommands::Actions(_) | Subcommands::Setup { .. } | Subcommands::Teardown { .. } => {
                Unknown
            }
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
            Subcommands::Help { .. } => Unknown,
            Subcommands::_Expand { .. } => Unknown,
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
            Subcommands::Amend(..) => Amend,
            #[cfg(feature = "legacy")]
            Subcommands::Squash(..) => Squash,
            #[cfg(feature = "legacy")]
            Subcommands::Move(..) => Move,
            #[cfg(feature = "legacy")]
            Subcommands::Land { .. } => Land,
            #[cfg(feature = "legacy")]
            Subcommands::Pick(..) => Pick,
            Subcommands::Skill(skill::Platform { cmd }) => match cmd {
                skill::Subcommands::Install { .. } => SkillInstall,
                skill::Subcommands::Check { .. } => SkillCheck,
            },
            // Bare `but agent` (None) runs the setup wizard, same as `agent setup`.
            Subcommands::Agent(agent::Platform { cmd }) => match cmd {
                None | Some(agent::Subcommands::Setup { .. }) => AgentSetup,
            },
            Subcommands::Edit { .. } => Edit,
            #[cfg(feature = "legacy")]
            Subcommands::Clean { .. } => Clean,
            Subcommands::Onboarding => Unknown,
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
            Subcommands::Uncommit(..) => {
                push_prop(&mut props, "sourceKind", "commitOrCommittedFile");
            }
            #[cfg(feature = "legacy")]
            Subcommands::Amend(..) => {
                push_prop(&mut props, "sourceKind", "fileOrHunk");
                push_prop(&mut props, "targetKind", "commitOrBranch");
            }
            #[cfg(feature = "legacy")]
            Subcommands::Squash(..) => {
                push_prop(&mut props, "sourceKind", "commitOrBranch");
                push_prop(&mut props, "targetKind", "commit");
            }
            #[cfg(feature = "legacy")]
            Subcommands::Move(..) => {
                push_prop(&mut props, "sourceKind", "commitOrBranch");
                push_prop(&mut props, "targetKind", "commitOrBranchOrUnassigned");
            }
            Subcommands::Skill(skill::Platform { cmd }) => match cmd {
                skill::Subcommands::Install { .. } => {}
                skill::Subcommands::Check { update, .. } => {
                    push_prop(&mut props, "skillCheckUpdate", *update);
                }
            },
            Subcommands::External(extra) => {
                if let Some(command_name) = extra.first() {
                    push_prop(
                        &mut props,
                        "externalSubcommand",
                        external_subcommand_metric_value(command_name),
                    );
                }
            }
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
            CliError::CommandRejection => {
                props.insert("error", "Command rejection");
                props.insert("errorKind", "commandRejection");
            }
            CliError::ExternalCommandNotFound(command_name) => {
                props.insert("error", "Unrecognized subcommand");
                props.insert("errorKind", "externalCommandNotFound");
                props.insert(
                    "unrecognizedSubcommand",
                    unrecognized_subcommand_metric_value(command_name),
                );
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

fn unrecognized_subcommand_metric_value(command_name: &std::ffi::OsStr) -> String {
    let command_name = command_name.to_string_lossy();
    let command_name = command_name.trim();

    if command_name.is_empty()
        || !command_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return INVALID_UNRECOGNIZED_SUBCOMMAND.to_string();
    }

    command_name
        .chars()
        .take(UNRECOGNIZED_SUBCOMMAND_MAX_CHARS)
        .collect()
}

fn external_subcommand_metric_value(command_name: &std::ffi::OsStr) -> String {
    let command_name = command_name.to_string_lossy();
    let command_name = command_name.trim();

    if command_name.is_empty()
        || !command_name
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '-' || c == '_')
    {
        return INVALID_UNRECOGNIZED_SUBCOMMAND.to_string();
    }

    command_name
        .chars()
        .take(UNRECOGNIZED_SUBCOMMAND_MAX_CHARS)
        .collect()
}

fn captures_detailed_error_message(command: CommandName) -> bool {
    matches!(
        command,
        CommandName::Uncommit | CommandName::Amend | CommandName::Squash
    )
}

fn truncate_error_message(message: String) -> String {
    message.chars().take(ERROR_MESSAGE_MAX_CHARS).collect()
}

/// Add lane and branch counts to `event`, read from the managed workspace at `current_dir`;
/// on failure the props are simply absent.
///
/// This runs in the spun-off `but metrics` process, shortly after the user-facing command
/// finished. Property names are shared with the desktop commit analytics where the meaning
/// matches; do not reuse desktop names like `branchCount` whose meaning differs.
pub fn add_workspace_shape(event: &mut Event, current_dir: &Path) {
    let Ok(ctx) = but_ctx::Context::discover(current_dir) else {
        return;
    };
    // Only inspect repositories that already carry GitButler project state; capturing an
    // event must never be what initializes a project.
    if !but_db::DbHandle::db_file_path(ctx.project_data_dir()).exists() {
        return;
    }
    let _guard = ctx.shared_worktree_access();
    let Some(ws) = read_only_workspace(&ctx) else {
        return;
    };
    // Without an intact workspace commit stack segments aren't reliable, and without a
    // lower bound they extend into unrelated history and would count historical branches.
    if !ws.kind.has_managed_commit() || ws.lower_bound.is_none() {
        return;
    }
    let branches_per_lane: Vec<usize> = ws
        .stacks
        .iter()
        .map(|stack| {
            stack
                .segments
                .iter()
                .filter(|segment| segment.ref_name().is_some())
                .count()
        })
        .collect();
    event.insert_prop("totalLanesInWorkspace", branches_per_lane.len());
    event.insert_prop(
        "totalBranchesInWorkspace",
        branches_per_lane.iter().sum::<usize>(),
    );
    event.insert_prop(
        "maxBranchesPerLane",
        branches_per_lane.iter().max().copied().unwrap_or_default(),
    );
}

/// The workspace as seen from `HEAD`, built strictly read-only: unlike the
/// `Context::workspace_and_db()` family this never creates or migrates the project database
/// or rewrites `virtual_branches.toml`.
fn read_only_workspace(ctx: &but_ctx::Context) -> Option<but_graph::Workspace> {
    let repo = ctx.repo.get().ok()?;
    let meta = but_meta::BranchOrderMetadata::from_paths_read_only(
        ctx.project_data_dir().join("virtual_branches.toml"),
        ctx.project_data_dir(),
    )
    .ok()?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        ctx.project_meta().ok()?,
        but_graph::init::Options::limited(),
    )
    .ok()?;
    graph.into_workspace().ok()
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
            event.insert_prop("agent", agent.as_str());
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
            current_dir,
        }) = ctx
        else {
            return self;
        };

        let mut props = Props::from_anyhow_result(start, &self, command);
        props.extend(extra_props);
        emit_metrics(command, &props, &current_dir);
        self
    }
}

impl<T> ResultMetricsExt<T, CliError> for Result<T, CliError> {
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> Result<T, CliError> {
        let Some(OneshotMetricsContext {
            start,
            command,
            extra_props,
            current_dir,
        }) = ctx
        else {
            return self;
        };

        let mut props = Props::from_cli_error_result(start, &self, command);
        props.extend(extra_props);
        emit_metrics(command, &props, &current_dir);
        self
    }
}

/// Emit an event for a command line that never ran because the parser
/// rejected it and a retired-syntax teaching hint was shown.
///
/// The event carries a `retiredSyntaxHint` prop, so remaining pre-revamp
/// usage can be tracked to decide when the hints in `retired_syntax` can be
/// removed. Root options like `-C` are not parsed on the failure paths that
/// call this, so the event reports the process working directory.
pub(crate) fn emit_retired_syntax_hint(command: CommandName) {
    let Ok(settings) = crate::app_settings() else {
        return;
    };
    if !settings.telemetry.app_metrics_enabled {
        return;
    }
    let mut props = Props::new();
    props.insert("retiredSyntaxHint", true);
    emit_metrics(command, &props, Path::new("."));
}

fn emit_metrics(command: CommandName, props: &Props, current_dir: &Path) {
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

    // Pass the invocation directory as an argument rather than as the child's working
    // directory: an invalid directory (say a bad `-C` value, which itself is worth
    // capturing) must neither fail the spawn nor let the child report another repository.
    // The `=`-joined form parses even for values starting with a hyphen.
    let mut current_dir_arg = std::ffi::OsString::from("--current-dir=");
    current_dir_arg.push(current_dir);
    let _ = tokio::process::Command::new(but_path)
        .arg(current_dir_arg)
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
        args::{Subcommands, agent, update},
        bad_input,
    };

    #[cfg(feature = "legacy")]
    use crate::args::atoms::CliIdArg;

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
    fn workspace_shape_never_initializes_a_project() {
        but_testsupport::isolated_app_data_dir(|| {
            let tmp = tempfile::tempdir().expect("tempdir");
            let repo_dir = tmp.path().join("repo");
            gix::init(&repo_dir).expect("init plain repo");
            let mut event = Event::new(EventKind::Cli(CommandName::Commit));

            // A plain repository without GitButler state must stay untouched.
            add_workspace_shape(&mut event, &repo_dir);
            let data_dir = repo_dir.join(".git/gitbutler");
            assert!(!data_dir.exists());
            assert!(!event.props.contains_key("totalLanesInWorkspace"));

            // Even with a project data dir present, the project database must not be created.
            std::fs::create_dir(&data_dir).expect("create project data dir");
            add_workspace_shape(&mut event, &repo_dir);
            assert_eq!(
                std::fs::read_dir(&data_dir).expect("read data dir").count(),
                0
            );
            assert!(!event.props.contains_key("totalLanesInWorkspace"));
        });
    }

    #[test]
    fn workspace_shape_counts_lanes_and_stacked_branches() {
        but_testsupport::isolated_app_data_dir(|| {
            let sandbox =
                but_testsupport::Sandbox::open_or_init_scenario_with_target_and_default_settings(
                    "one-stack-three-dependent-branches",
                );
            let repo = sandbox.open_repo();
            let workdir = repo.workdir().expect("scenario is not bare").to_owned();
            // The shape is only read from repositories that already carry a project database.
            but_db::DbHandle::new_in_directory(repo.git_dir().join("gitbutler"))
                .expect("create project db");

            let mut event = Event::new(EventKind::Cli(CommandName::Commit));
            add_workspace_shape(&mut event, &workdir);

            assert_eq!(event.props["totalLanesInWorkspace"], serde_json::json!(1));
            assert_eq!(
                event.props["totalBranchesInWorkspace"],
                serde_json::json!(3)
            );
            assert_eq!(event.props["maxBranchesPerLane"], serde_json::json!(3));
        });
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
            Subcommands::Agent(agent::Platform {
                cmd: Some(agent::Subcommands::Setup { print: false }),
            }),
            "agentSetup",
        );
        // Bare `but agent` (no subcommand) maps to the same metric.
        assert_command(
            Subcommands::Agent(agent::Platform { cmd: None }),
            "agentSetup",
        );
        #[cfg(feature = "legacy")]
        assert_command(
            Subcommands::Move(crate::args::r#move::Platform {
                branch: Some(Some(CliIdArg("main".to_owned()))),
                above: None,
                below: None,
                unstack: false,
                sources: Vec::from([CliIdArg("ci".to_owned())]),
                allow_merged: Default::default(),
            }),
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
                Subcommands::Amend(crate::args::amend::Platform {
                    target: CliIdArg("c1".into()),
                    sources: vec![CliIdArg("a1".into())],
                    allow_merged: Default::default(),
                }),
                "amend",
            );
        }
    }

    #[test]
    fn extra_props_keep_useful_source_and_target_kinds() {
        #[cfg(feature = "legacy")]
        {
            let moved = Subcommands::Move(crate::args::r#move::Platform {
                branch: Some(Some(CliIdArg("main".to_owned()))),
                above: None,
                below: None,
                unstack: false,
                sources: Vec::from([CliIdArg("ci".to_owned())]),
                allow_merged: Default::default(),
            });
            let props = moved.to_metrics_extra_props();
            assert_eq!(
                prop(&props, "sourceKind"),
                Some(&serde_json::json!("commitOrBranch"))
            );
            assert_eq!(
                prop(&props, "targetKind"),
                Some(&serde_json::json!("commitOrBranchOrUnassigned"))
            );
        }
    }

    #[test]
    fn external_extra_props_include_sanitized_subcommand() {
        let props = Subcommands::External(vec![" typo-OK ".into()]).to_metrics_extra_props();
        assert_eq!(
            prop(&props, "externalSubcommand"),
            Some(&serde_json::json!("typo-OK"))
        );

        let props = Subcommands::External(vec!["/tmp/private".into()]).to_metrics_extra_props();
        assert_eq!(
            prop(&props, "externalSubcommand"),
            Some(&serde_json::json!(INVALID_UNRECOGNIZED_SUBCOMMAND))
        );

        let props = Subcommands::External(vec!["customer123".into()]).to_metrics_extra_props();
        assert_eq!(
            prop(&props, "externalSubcommand"),
            Some(&serde_json::json!(INVALID_UNRECOGNIZED_SUBCOMMAND))
        );
    }

    #[test]
    fn internal_error_details_are_allowlisted() {
        let anyhow_result = Err::<(), _>(
            anyhow::anyhow!("stale id. If you just performed a Git operation, refresh")
                .context("Failed to uncommit."),
        );

        let props = Props::from_anyhow_result(
            std::time::Instant::now(),
            &anyhow_result,
            CommandName::Uncommit,
        );

        assert_eq!(props.values["error"], "Internal error");
        assert_eq!(props.values["errorKind"], "internal");
        assert_eq!(
            props.values["errorMessage"],
            "Failed to uncommit.: stale id."
        );
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
            CommandName::Commit,
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
        assert_eq!(props.values["unrecognizedSubcommand"], "typo");
        assert!(!props.values.contains_key("errorMessage"));

        let external_result =
            Err::<(), _>(CliError::ExternalCommandNotFound(" typo-123_OK ".into()));
        let props = Props::from_cli_error_result(
            std::time::Instant::now(),
            &external_result,
            CommandName::External,
        );
        assert_eq!(props.values["unrecognizedSubcommand"], "typo-123_OK");

        let external_result =
            Err::<(), _>(CliError::ExternalCommandNotFound("/tmp/private".into()));
        let props = Props::from_cli_error_result(
            std::time::Instant::now(),
            &external_result,
            CommandName::External,
        );
        assert_eq!(
            props.values["unrecognizedSubcommand"],
            INVALID_UNRECOGNIZED_SUBCOMMAND
        );
        assert!(!props.as_json_string().contains("/tmp/private"));

        let long_command = "a".repeat(UNRECOGNIZED_SUBCOMMAND_MAX_CHARS + 1);
        let external_result = Err::<(), _>(CliError::ExternalCommandNotFound(long_command.into()));
        let props = Props::from_cli_error_result(
            std::time::Instant::now(),
            &external_result,
            CommandName::External,
        );
        assert_eq!(
            props.values["unrecognizedSubcommand"]
                .as_str()
                .expect("metric value is a string")
                .len(),
            UNRECOGNIZED_SUBCOMMAND_MAX_CHARS
        );
    }

    #[test]
    fn detailed_error_messages_are_normalized_and_capped() {
        let multiline_result =
            Err::<(), _>(anyhow::anyhow!("first line\nsecond line").context("Failed to uncommit."));

        let props = Props::from_anyhow_result(
            std::time::Instant::now(),
            &multiline_result,
            CommandName::Uncommit,
        );

        assert_eq!(
            props.values["errorMessage"],
            "Failed to uncommit.: first line second line"
        );
        assert_eq!(props.values["errorRoot"], "first line second line");

        let long_result = Err::<(), _>(anyhow::anyhow!("{}", "a".repeat(1100)));

        let props = Props::from_anyhow_result(
            std::time::Instant::now(),
            &long_result,
            CommandName::Uncommit,
        );

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
