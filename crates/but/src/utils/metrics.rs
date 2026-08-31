use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

use but_error::AnyhowContextExt;
use but_settings::AppSettings;
use clap::ValueEnum;
use command_group::AsyncCommandGroup;
use posthog_rs::Client;
use rand::{Rng, distr::OpenClosed01};
use serde::{Deserialize, Serialize};

use crate::{
    CliError,
    args::{Subcommands, config, metrics::CommandName},
    command::CommandOutcome,
    utils::{ResultMetricsExt, binary_path},
};

const UNRECOGNIZED_SUBCOMMAND_MAX_CHARS: usize = 64;
const INVALID_UNRECOGNIZED_SUBCOMMAND: &str = "<invalid>";

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
    pub fn to_metrics_context(
        &self,
        settings: &AppSettings,
        current_dir: &Path,
    ) -> Option<OneshotMetricsContext> {
        if !settings.telemetry.app_metrics_enabled {
            return None;
        }
        // Comments are still experimental, completions are shell-startup noise, and MCP and the
        // transport child own their own events.
        if matches!(
            self,
            Subcommands::_Comment(_)
                | Subcommands::Completions { .. }
                | Subcommands::Mcp(_)
                | Subcommands::Metrics { .. }
        ) {
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

    /// Return the low-cardinality event identifier.
    pub(crate) fn to_metrics_command(&self) -> CommandName {
        use CommandName::*;

        use crate::args::{agent, alias as alias_args, branch, forge, skill, update, worktree};
        match self {
            Subcommands::_Comment(_) => Comment,
            Subcommands::Completions { .. } => Completions,
            Subcommands::Mcp(_) => Mcp,
            Subcommands::Metrics { .. } => Metrics,
            Subcommands::Help { .. } => Help,
            Subcommands::Onboarding => Onboarding,
            Subcommands::AgentLog { .. } => AgentLog,
            #[cfg(feature = "legacy")]
            Subcommands::Actions(_) => Actions,
            #[cfg(feature = "legacy")]
            Subcommands::Status { .. } => Status,
            #[cfg(feature = "legacy")]
            Subcommands::Tui { .. } => Tui,
            #[cfg(feature = "legacy")]
            Subcommands::Diff(..) => Diff,
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
            Subcommands::Worktree(worktree::Platform { cmd }) => match cmd {
                None | Some(worktree::Subcommands::List { .. }) => WorktreeList,
                Some(worktree::Subcommands::Archive { .. }) => WorktreeArchive,
                Some(worktree::Subcommands::Unarchive { .. }) => WorktreeUnarchive,
                Some(worktree::Subcommands::Remove { .. }) => WorktreeRemove,
            },
            #[cfg(feature = "legacy")]
            Subcommands::Unapply { .. } => BranchUnapply,
            #[cfg(feature = "legacy")]
            Subcommands::Apply { .. } => BranchApply,
            #[cfg(feature = "legacy")]
            Subcommands::Open { .. } => Open,
            #[cfg(feature = "legacy")]
            Subcommands::Switch(..) => Switch,
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
            #[cfg(feature = "legacy")]
            Subcommands::Setup { .. } => Setup,
            #[cfg(feature = "legacy")]
            Subcommands::Teardown { .. } => Teardown,
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
                _ => Config,
            },
            Subcommands::_Expand { .. } => Expand,
            Subcommands::Alias(alias_args::Platform { cmd }) => match cmd {
                None | Some(alias_args::Subcommands::List) => AliasCheck,
                Some(alias_args::Subcommands::Add { .. }) => AliasAdd,
                Some(alias_args::Subcommands::Remove { .. }) => AliasRemove,
            },
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
            Subcommands::Split(..) => Split,
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
            Subcommands::External(_) => External,
        }
    }

    /// Additional low-cardinality dimensions for command modifiers.
    ///
    /// `sourceKind` and `targetKind` describe the kind a command expects, not a
    /// resolved runtime ID.
    pub(crate) fn to_metrics_extra_props(&self) -> Vec<(String, serde_json::Value)> {
        #[cfg(feature = "legacy")]
        use crate::args::commit;
        use crate::args::skill;

        let mut props = Vec::new();
        match self {
            #[cfg(feature = "legacy")]
            Subcommands::Commit(commit::Platform {
                branch,
                empty,
                above,
                below,
                interactive,
                changes,
                ..
            }) => {
                let target_mode = match (branch, above, below) {
                    (Some(Some(_)), None, None) => "namedBranch",
                    (Some(None), None, None) => "generatedBranch",
                    (None, Some(_), None) => "above",
                    (None, None, Some(_)) => "below",
                    _ => "default",
                };
                let selection_mode = if !changes.is_empty() {
                    "explicitChanges"
                } else if *interactive {
                    "interactive"
                } else if *empty {
                    "empty"
                } else {
                    "allChanges"
                };
                push_prop(&mut props, "targetMode", target_mode);
                push_prop(&mut props, "selectionMode", selection_mode);
            }
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
            Subcommands::Skill(skill::Platform {
                cmd: skill::Subcommands::Check { update, .. },
            }) => push_prop(&mut props, "skillCheckUpdate", *update),
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

/// Everything needed to emit one CLI event after a command finishes.
pub struct OneshotMetricsContext {
    start: std::time::Instant,
    pub command: CommandName,
    extra_props: Vec<(String, serde_json::Value)>,
    current_dir: PathBuf,
}

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

    pub(crate) fn record_outcome(&mut self, outcome: &CommandOutcome) {
        self.extra_props.extend(command_outcome_props(outcome));
    }
}

fn command_outcome_props(outcome: &CommandOutcome) -> Vec<(String, serde_json::Value)> {
    let mut props = Vec::new();
    match outcome {
        CommandOutcome::AgentSetupPrintOnly => {
            push_prop(&mut props, "agentSetupOutcome", "printOnly");
        }
        CommandOutcome::AgentSetupCancelled => {
            push_prop(&mut props, "agentSetupOutcome", "cancelled");
        }
        CommandOutcome::AgentSetupCompleted {
            manual_instructions_required,
        } => {
            push_prop(
                &mut props,
                "agentSetupOutcome",
                if *manual_instructions_required {
                    "completedWithManualStep"
                } else {
                    "completed"
                },
            );
            push_prop(
                &mut props,
                "agentSetupManualInstructionsRequired",
                *manual_instructions_required,
            );
        }
        #[cfg(feature = "legacy")]
        CommandOutcome::Commit(outcome) => {
            use crate::command::legacy::commit::BranchNameTarget;

            let target_kind = match &outcome.branch_name {
                Some(BranchNameTarget::New(_)) => "newBranch",
                Some(BranchNameTarget::Existing(_)) => "existingBranch",
                None => "commit",
            };
            push_prop(&mut props, "stateChanged", true);
            push_prop(&mut props, "createdBranch", target_kind == "newBranch");
            push_prop(&mut props, "resolvedTargetKind", target_kind);
            push_prop(
                &mut props,
                "changedPathCountBucket",
                change_count_bucket(outcome.changed_path_count),
            );
        }
    }
    props
}

pub(crate) fn change_count_bucket(count: usize) -> &'static str {
    match count {
        0 => "0",
        1 => "1",
        2..=3 => "2to3",
        4..=10 => "4to10",
        _ => "11plus",
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

    fn from_anyhow_result<T>(start: std::time::Instant, result: &anyhow::Result<T>) -> Props {
        let mut props = Props::new();
        props.insert("durationMs", start.elapsed().as_millis());
        let Some(error) = result.as_ref().err() else {
            props.insert("error", Option::<String>::None);
            return props;
        };

        props.insert_internal_error_details(error);
        props
    }

    fn from_cli_error_result<T>(start: std::time::Instant, result: &Result<T, CliError>) -> Props {
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
            CliError::ExternalCommandFailed(_) => {
                props.insert("error", "External command failed");
                props.insert("errorKind", "externalCommandFailed");
            }
            CliError::Internal(error) => {
                props.insert_internal_error_details(error);
            }
            CliError::Initialization(_) => {
                props.insert("error", "Internal error");
                props.insert("errorKind", "initialization");
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

    fn insert_internal_error_details(&mut self, error: &anyhow::Error) {
        #[cfg(feature = "legacy")]
        let is_explained_rejection = error.is::<crate::utils::rejection::ExplainedRejection>();
        #[cfg(not(feature = "legacy"))]
        let is_explained_rejection = false;
        if is_explained_rejection {
            self.insert("error", "Command rejection");
            self.insert("errorKind", "commandRejection");
            self.insert("errorCode", "changesRejected");
            self.insert("retryable", false);
            self.insert("stateChanged", false);
        } else {
            self.insert("error", "Internal error");
            self.insert("errorKind", "internal");
            let custom_context = error.custom_context();
            if let Some(context) = custom_context
                && context.code != but_error::Code::Unknown
            {
                self.insert("errorCode", context.code.to_string());
            }
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

fn sample_props(mut props: Props, command: CommandName, failed: bool, draw: f32) -> Option<Props> {
    let sampling_rate = if failed { 1.0 } else { command.sample_rate() };
    if sampling_rate < draw {
        return None;
    }
    props.insert("samplingRate", sampling_rate);
    Some(props)
}

pub(crate) fn prepare_transport_props(json: &str) -> anyhow::Result<Props> {
    let mut props = Props::from_json_string(json)?;
    if let Some(rate) = props.values.get("samplingRate") {
        anyhow::ensure!(
            rate.as_f64().is_some_and(|rate| rate > 0.0 && rate <= 1.0),
            "`samplingRate` must be a number in (0, 1]"
        );
    } else {
        props.insert("samplingRate", 1.0);
    }
    Ok(props)
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

/// The workspace as seen from `HEAD`, built without rewriting `virtual_branches.toml`,
/// unlike the `Context::workspace_and_db()` family.
///
/// The caller guarantees the project database already exists, so borrowing it here
/// cannot be what initializes a project.
fn read_only_workspace(ctx: &but_ctx::Context) -> Option<but_graph::Workspace> {
    let repo = ctx.repo.get().ok()?;
    let meta = but_meta::BranchOrderMetadata::from_paths_read_only(
        ctx.project_data_dir().join("virtual_branches.toml"),
        ctx.project_data_dir(),
    )
    .ok()?;
    let mut db = ctx.db.get_cache_mut().ok()?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        ctx.project_meta().ok()?,
        &mut db,
        but_graph::init::Options {
            worktrees: ctx.settings.feature_flags.worktree_manipulation,
            ..but_graph::init::Options::limited()
        },
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
    if let Some(client) = posthog_client(app_settings).await {
        do_capture(&client, event, app_settings).await.ok();
        // Explicit shutdown so dropping the client doesn't block the executor thread.
        client.shutdown().await;
    }
}

/// Note that `client` is *only* available if telemetry is enabled.
async fn do_capture(
    client: &Client,
    event: Event,
    app_settings: &AppSettings,
) -> Result<(), posthog_rs::Error> {
    let id = app_settings
        .telemetry
        .app_distinct_id
        .clone()
        .unwrap_or_else(machine);
    let mut posthog_event = posthog_rs::Event::new(event.event_name.to_string(), id);
    for (key, prop) in event.props {
        let _ = posthog_event.insert_prop(key, prop);
    }
    // The CLI exits right after this, so send inline instead of via the background queue.
    client.capture_immediate(posthog_event).await.map(|_| ())
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
async fn posthog_client(app_settings: &AppSettings) -> Option<Client> {
    if !app_settings.telemetry.app_metrics_enabled {
        return None;
    }
    let api_key = option_env!("POSTHOG_API_KEY")?;
    let options = posthog_rs::ClientOptionsBuilder::default()
        .api_key(api_key.to_string())
        .host("https://eu.i.posthog.com".to_string())
        .is_server(false)
        .build()
        .ok()?;
    Some(posthog_rs::client(options).await)
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

        let mut props = Props::from_anyhow_result(start, &self);
        props.extend(extra_props);
        emit_metrics(command, props, &current_dir, self.is_err());
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

        let mut props = Props::from_cli_error_result(start, &self);
        props.extend(extra_props);
        emit_metrics(command, props, &current_dir, self.is_err());
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
    emit_metrics(command, props, Path::new("."), true);
}

fn emit_metrics(command: CommandName, props: Props, current_dir: &Path, failed: bool) {
    let Some(props) = sample_props(
        props,
        command,
        failed,
        rand::rng().sample::<f32, _>(OpenClosed01),
    ) else {
        return;
    };
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
mod tests;
