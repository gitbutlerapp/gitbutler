//! What the app tells GitButler, through PostHog, about how it's used and what goes wrong in
//! it: launches, the operations the user starts and their failures, errors shown, panics and
//! crashes. It must never break or slow the app. The UI thread only hands an event to a queue,
//! which a thread of the PostHog client's own sends, and anything failing here means only
//! fewer events.
//!
//! Events follow the CLI's and the desktop app's: snake_case names, camelCase properties and
//! the workspace's shape in counts, never a branch name, path, message or anything else the
//! user wrote. They go out under the ID the apps on the machine share through the settings,
//! only from nightly and release builds, and only while the settings say so: on unless turned
//! off, as the CLI's are, in Settings › General › Metrics or the desktop app's Telemetry page.
//! Development builds log them at debug instead.

mod crashes;

use std::collections::{HashMap, HashSet};
use std::panic::Location;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

use anyhow::Result;
use bstr::ByteSlice as _;
use but_error::AnyhowContextExt as _;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};

use super::{Checkout, Workspace};

const HOST: &str = "https://eu.i.posthog.com";
/// Tells this app's events from the desktop app's, the CLI's and Lite's.
const APP_NAME: &str = "gitbutler-gpui";
/// The longest quitting waits for what's queued to go out.
const SHUTDOWN: Duration = Duration::from_secs(2);
/// A failure or error of one kind goes out once in this long; the next counts those between.
const ONCE_IN: Duration = Duration::from_secs(60);
/// How often, at most, `app_active` says the app is in use.
const ACTIVE_EVERY: Duration = Duration::from_secs(60 * 60);

type Props = Map<String, Value>;

/// Which setting lets an event out: `appMetricsEnabled` or `appErrorReportingEnabled`.
#[derive(Clone, Copy)]
enum Kind {
    Usage,
    Errors,
}

struct Metrics {
    /// `None` in development builds and builds without the project key, which only log.
    client: Option<posthog_rs::Client>,
    /// The settings' switches, which Settings turns off at once and back on.
    usage: AtomicBool,
    errors: AtomicBool,
    /// The settings' ID, or `user_<id>` once signed in.
    id: Mutex<String>,
    /// Sent with every event.
    context: Props,
}

/// Set once [`start`] has read the settings; `None` when nothing is to be sent.
static METRICS: OnceLock<Option<Metrics>> = OnceLock::new();
/// What was captured before then, sent once it's known whether to.
static EARLY: Mutex<Vec<Early>> = Mutex::new(Vec::new());

/// An event captured before [`start`] read the settings, with when.
type Early = (Kind, String, Props, DateTime<Utc>);
/// Says [`start`] has read the settings and queued what was captured before, for quitting to
/// wait on.
static STARTED: Mutex<Option<mpsc::Receiver<()>>> = Mutex::new(None);
/// When each kind of failure last went out, and how many were held back since.
static LIMITS: Mutex<Option<HashMap<String, (Instant, u64)>>> = Mutex::new(None);
/// When `app_active` last went out, and the views opened since, by kind.
static ACTIVE: Mutex<Active> = Mutex::new(Active {
    last: None,
    views: Vec::new(),
});

struct Active {
    last: Option<Instant>,
    views: Vec<(&'static str, u64)>,
}

/// What `app_active` says about the app as it is: the settings the desktop app reports too, by
/// its names and values, how many projects there are, and the workspace being looked at.
pub struct State<'a> {
    /// `system`, `light` or `dark`.
    pub theme: &'static str,
    /// The interface's scale, 1 at 100%.
    pub zoom: f64,
    /// The editor files open in, by the id the desktop app uses too, if one was picked.
    pub editor: Option<&'a str>,
    pub ligatures: bool,
    pub projects: usize,
    pub workspace: &'a Workspace,
}

/// Start in the background, before the window opens: read the settings, report how the last
/// run ended and say the app opened. `version` is the desktop app's, when it started the
/// client.
pub fn start(version: Option<String>) {
    let (done, started) = mpsc::channel();
    *STARTED.lock() = Some(started);
    let spawned = std::thread::Builder::new()
        .name("metrics".into())
        .spawn(move || {
            let launched_by = match version {
                Some(_) => "desktopApp",
                None => "itself",
            };
            let metrics = open(version).unwrap_or_else(|err| {
                tracing::debug!("not sending metrics: {err:#}");
                None
            });
            let early = {
                let mut early = EARLY.lock();
                METRICS.set(metrics).ok();
                std::mem::take(&mut *early)
            };
            let metrics = METRICS.get().and_then(Option::as_ref);
            for (kind, name, props, at) in early {
                if let Some(metrics) = metrics {
                    metrics.send(kind, &name, props, Some(at));
                }
            }
            // Quitting waits only for this: what the launch reports next can take seconds.
            done.send(()).ok();
            if let Some(metrics) = metrics {
                metrics.opened(launched_by);
            }
        });
    if let Err(err) = spawned {
        tracing::debug!("not sending metrics: {err}");
        METRICS.set(None).ok();
    }
}

/// Before the app ends: say the run ended cleanly, and send what's queued, waiting at most
/// [`SHUTDOWN`]. It runs on the UI thread, which then has nothing left to draw.
pub fn quit() {
    let deadline = Instant::now() + SHUTDOWN;
    if let Some(started) = STARTED.lock().take() {
        started.recv_timeout(SHUTDOWN).ok();
    }
    let Some(Some(metrics)) = METRICS.get() else {
        return;
    };
    crashes::ended_cleanly();
    if let Some(client) = &metrics.client {
        let left = deadline.saturating_duration_since(Instant::now());
        super::runtime()
            .block_on(async { tokio::time::timeout(left, client.shutdown()).await.ok() });
    }
}

/// An operation the user started, named `name`, finished after `took`, failing with
/// `failure` if it did, on `workspace` as it was shown. An operation that handles its own
/// failure, as a fetch does, finishes without one, and its error card reports it instead.
pub fn operation(
    name: &'static str,
    took: Duration,
    failure: Option<&anyhow::Error>,
    workspace: Option<&Workspace>,
) {
    if matches!(METRICS.get(), Some(None)) {
        return;
    }
    let command = camel_case(name);
    let mut props = Props::new();
    props.insert("command".into(), command.clone().into());
    props.insert("durationMs".into(), (took.as_millis() as u64).into());
    props.insert("failure".into(), failure.is_some().into());
    if let Some(workspace) = workspace {
        shape(workspace, &mut props);
    }
    if let Some(err) = failure {
        let Some(occurrences) = limit(format!("api_command {command}")) else {
            return;
        };
        props.insert("occurrenceCount".into(), occurrences.into());
        let names = names(workspace);
        error_props(err, &names, &mut props);
        if unexpected(err) {
            report_error(format!("{name} failed"), err, &command, &names);
        }
    }
    queue(Kind::Usage, "api_command", props);
}

/// An error card showed `err`, asked for at `site` in the app's code, which names it without
/// its title, as that may name a branch or a project. What's named in `workspaces` is left out.
pub fn failure_shown<'a>(
    site: &Location<'_>,
    err: &anyhow::Error,
    workspaces: impl IntoIterator<Item = &'a Workspace>,
) {
    if matches!(METRICS.get(), Some(None)) {
        return;
    }
    let file = crashes::short_path(site.file());
    let site = format!("{file}:{}", site.line());
    let Some(occurrences) = limit(format!("error_shown {site}")) else {
        return;
    };
    let mut props = Props::new();
    props.insert("site".into(), site.into());
    props.insert("occurrenceCount".into(), occurrences.into());
    let names = names(workspaces);
    error_props(err, &names, &mut props);
    queue(Kind::Usage, "error_shown", props);
    // By file, so the issue stays one as lines above it change from release to release.
    if unexpected(err) {
        report_error(format!("Error in {file}"), err, &file, &names);
    }
}

/// The user opened `view` in the window, as `commit` or `history`: count it, and once an hour
/// say the app is in use, with the app as `state` gives it and the views counted since. So
/// it's seen in use on days nothing is changed with it, as when reading what agents did.
pub fn viewed<'a>(view: &'static str, state: impl FnOnce() -> State<'a>) {
    if matches!(METRICS.get(), Some(None)) {
        return;
    }
    let now = Instant::now();
    let views = {
        let mut active = ACTIVE.lock();
        match active.views.iter_mut().find(|(kind, _)| *kind == view) {
            Some((_, count)) => *count += 1,
            None => active.views.push((view, 1)),
        }
        if (active.last).is_some_and(|last| now.duration_since(last) < ACTIVE_EVERY) {
            return;
        }
        active.last = Some(now);
        std::mem::take(&mut active.views)
    };
    let state = state();
    let mut props = Props::new();
    props.insert("theme".into(), state.theme.into());
    props.insert("zoom".into(), state.zoom.into());
    if let Some(editor) = state.editor {
        props.insert("defaultCodeEditor".into(), editor.into());
    }
    props.insert("diffLigatures".into(), state.ligatures.into());
    props.insert("projectCount".into(), state.projects.into());
    shape(state.workspace, &mut props);
    for (kind, count) in views {
        props.insert(format!("{kind}Views"), count.into());
    }
    queue(Kind::Usage, "app_active", props);
}

/// A project that read can't be read any more, as after an agent's `git merge` onto the
/// workspace branch, by the desktop app's name for it, leaving out what `last` read named.
/// Only what isn't a known condition, as its folder gone, goes to error tracking too.
pub fn load_failed(failure: &super::LoadFailure, err: &anyhow::Error, last: Option<&Workspace>) {
    if matches!(METRICS.get(), Some(None)) {
        return;
    }
    let names = names(last);
    let message = redact(&err.to_string(), &names);
    let Some(occurrences) = limit(format!("repo:load_failed {message}")) else {
        return;
    };
    let mut props = Props::new();
    props.insert("occurrenceCount".into(), occurrences.into());
    props.insert("folderGone".into(), failure.folder_gone.into());
    props.insert("sparseIndex".into(), failure.sparse_index.into());
    props.insert(
        "targetChoice".into(),
        failure.target_choice.is_some().into(),
    );
    // Of those, one no local branch tracks rather than several.
    let untracked =
        (failure.target_choice.as_ref()).is_some_and(|choice| choice.branches.is_empty());
    props.insert("targetUntracked".into(), untracked.into());
    error_props(err, &names, &mut props);
    queue(Kind::Usage, "repo:load_failed", props);
    let known = failure.folder_gone || failure.sparse_index || failure.target_choice.is_some();
    if !known && unexpected(err) {
        report_error("Couldn't read the project".into(), err, "load", &names);
    }
}

/// The window couldn't open, as when the GPU's drivers can't draw it, and the desktop app's
/// web app opens instead.
pub fn fell_back_to_web(err: &anyhow::Error) {
    let mut props = Props::new();
    error_props(err, &HashSet::new(), &mut props);
    queue(Kind::Usage, "app_fell_back_to_web", props);
}

/// Signed in to GitButler as `user`: events go out as their account's from now on, joined to
/// what this machine sent before, as the desktop app's sign-in does.
pub fn signed_in(user: &but_api::legacy::users::UserProfile) {
    if let Some(Some(metrics)) = METRICS.get() {
        metrics.identify(user);
    }
}

/// Signed out: the account's ID goes, as the desktop app's sign-out takes it away, and events
/// go out under the machine's again.
pub fn signed_out() {
    let metrics = METRICS.get().and_then(Option::as_ref);
    if let Some(metrics) = metrics.filter(|metrics| metrics.usage.load(Ordering::Relaxed)) {
        metrics.send(Kind::Usage, "logout", Props::new(), None);
    }
    // Whatever the switches say, so nothing sent later, by this app or another, goes out as the
    // account's.
    let machine = super::settings::app_settings().and_then(|settings| {
        settings.update_telemetry_distinct_id(None)?;
        Ok(settings.get()?.telemetry.distinct_id())
    });
    match (machine, metrics) {
        (Ok(id), Some(metrics)) => *metrics.id.lock() = id,
        (Ok(_), None) => {}
        (Err(err), _) => tracing::debug!("couldn't save the metrics ID: {err:#}"),
    }
}

/// Whether the settings send usage metrics, then error reports, as Settings shows them.
pub fn enabled() -> (bool, bool) {
    super::settings::app_settings()
        .and_then(|settings| {
            let telemetry = &settings.get()?.telemetry;
            Ok((
                telemetry.app_metrics_enabled,
                telemetry.app_error_reporting_enabled,
            ))
        })
        .unwrap_or_default()
}

/// Turn usage metrics on or off for all of GitButler's apps, as Settings does.
pub fn set_usage(on: bool) -> Result<()> {
    set(Kind::Usage, on)
}

/// Turn error reports on or off for all of GitButler's apps, as Settings does.
pub fn set_errors(on: bool) -> Result<()> {
    set(Kind::Errors, on)
}

/// Save `kind`'s switch, and follow it from now on: off stops it at once. On starts it at
/// once when the other was on at launch, and otherwise at the next launch, which reads the
/// settings again; panics are caught only from a launch with error reports on.
fn set(kind: Kind, on: bool) -> Result<()> {
    let update = match kind {
        Kind::Usage => but_settings::api::TelemetryUpdate {
            app_metrics_enabled: Some(on),
            ..Default::default()
        },
        Kind::Errors => but_settings::api::TelemetryUpdate {
            app_error_reporting_enabled: Some(on),
            ..Default::default()
        },
    };
    super::settings::app_settings()?.update_telemetry(update)?;
    if let Some(Some(metrics)) = METRICS.get() {
        match kind {
            Kind::Usage => &metrics.usage,
            Kind::Errors => &metrics.errors,
        }
        .store(on, Ordering::Relaxed);
    }
    Ok(())
}

/// The settings say whether and under which ID; the build says where to.
fn open(version: Option<String>) -> Result<Option<Metrics>> {
    let telemetry = super::settings::app_settings()?.get()?.telemetry.clone();
    let usage = telemetry.app_metrics_enabled;
    let errors = telemetry.app_error_reporting_enabled;
    if !usage && !errors {
        return Ok(None);
    }
    let shipped = matches!(
        but_path::AppChannel::new(),
        but_path::AppChannel::Nightly | but_path::AppChannel::Release
    ) && std::env::var_os("CI").is_none()
        && std::env::var_os("E2E_TEST_APP_DATA_DIR").is_none();
    // The project the desktop app and the CLI send to. Release builds get its key from CI, as
    // the CLI's do, so a build without one only logs.
    let client = match option_env!("POSTHOG_API_KEY").filter(|_| shipped) {
        Some(key) => {
            let options = posthog_rs::ClientOptionsBuilder::default()
                .api_key(key.to_owned())
                .host(HOST.to_owned())
                // A desktop app, whose OS is the user's.
                .is_server(false)
                .request_timeout_seconds(10)
                .max_queue_size(1000)
                .shutdown_timeout_ms(SHUTDOWN.as_millis() as u64)
                .build()?;
            Some(super::runtime().block_on(posthog_rs::client(options)))
        }
        None => None,
    };
    let mut context = Props::new();
    context.insert("appName".into(), APP_NAME.into());
    context.insert(
        "appVersion".into(),
        version
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned())
            .into(),
    );
    context.insert("releaseChannel".into(), super::settings::channel().into());
    // As the desktop app's PostHog names them.
    let os = match std::env::consts::OS {
        "macos" => "Mac OS X",
        "windows" => "Windows",
        "linux" => "Linux",
        os => os,
    };
    context.insert("$os".into(), os.into());
    context.insert("Arch".into(), std::env::consts::ARCH.into());
    // Nothing is sampled, but the CLI and Lite stamp every event, and totals divide by it.
    context.insert("samplingRate".into(), 1.into());
    context.insert(
        "$session_id".into(),
        uuid::Uuid::now_v7().to_string().into(),
    );
    Ok(Some(Metrics {
        client,
        usage: AtomicBool::new(usage),
        errors: AtomicBool::new(errors),
        id: Mutex::new(telemetry.distinct_id()),
        context,
    }))
}

impl Metrics {
    /// Report how the last run ended, catch this one's panics, and say the app opened.
    fn opened(&self, launched_by: &str) {
        if self.errors.load(Ordering::Relaxed) {
            crashes::catch_panics();
        }
        let previous_exit = crashes::previous_run(self);
        // Signed in, perhaps in another of GitButler's apps, as the desktop app checks at start.
        if self.usage.load(Ordering::Relaxed)
            && let Ok(Some(user)) = but_api::legacy::users::get_user_profile_local()
        {
            self.identify(&user);
        }
        let projects = but_api::legacy::projects::list_projects_stateless().map_or(0, |p| p.len());
        let props = json!({
            "previousExit": previous_exit,
            "launchedBy": launched_by,
            "projectCount": projects,
        });
        if let Value::Object(props) = props {
            self.send(Kind::Usage, "app_opened", props, None);
        }
    }

    fn identify(&self, user: &but_api::legacy::users::UserProfile) {
        if !self.usage.load(Ordering::Relaxed) {
            return;
        }
        let account = format!("user_{}", user.id);
        let before = std::mem::replace(&mut *self.id.lock(), account.clone());
        if before == account {
            return;
        }
        let mut props = Props::new();
        // What this machine sent before becomes the account's, unless it was another account.
        if !before.starts_with("user_") {
            props.insert("$anon_distinct_id".into(), before.into());
        }
        props.insert(
            "$set".into(),
            json!({ "email": user.email, "name": user.name }),
        );
        self.send(Kind::Usage, "$identify", props, None);
        // The CLI reads it there, and the desktop app's sign-in writes it there too.
        let saved = super::settings::app_settings()
            .and_then(|settings| settings.update_telemetry_distinct_id(Some(account)));
        if let Err(err) = saved {
            tracing::debug!("couldn't save the metrics ID: {err:#}");
        }
    }

    fn send(&self, kind: Kind, name: &str, props: Props, at: Option<DateTime<Utc>>) {
        let wanted = match kind {
            Kind::Usage => &self.usage,
            Kind::Errors => &self.errors,
        }
        .load(Ordering::Relaxed);
        if !wanted {
            return;
        }
        tracing::debug!(
            "metrics: {name} {}",
            serde_json::to_string(&props).unwrap_or_default()
        );
        let Some(client) = &self.client else {
            return;
        };
        let id = self.id.lock().clone();
        // Only a signed-in account's events make a person in PostHog, as the desktop app's do.
        let person = id.starts_with("user_");
        let mut event = posthog_rs::Event::new(name.to_owned(), id);
        for (key, value) in self.context.iter().chain(&props) {
            event.insert_prop(key.as_str(), value).ok();
        }
        event.insert_prop("$process_person_profile", person).ok();
        if let Some(at) = at {
            event.set_timestamp(at).ok();
        }
        client.capture(event);
    }
}

/// Send `props` as `name`, or keep it until [`start`] knows whether to.
fn queue(kind: Kind, name: &str, props: Props) {
    if let Some(metrics) = METRICS.get() {
        if let Some(metrics) = metrics {
            metrics.send(kind, name, props, None);
        }
        return;
    }
    let mut early = EARLY.lock();
    // Set meanwhile, and the events kept until then already sent.
    match METRICS.get() {
        Some(Some(metrics)) => metrics.send(kind, name, props, None),
        Some(None) => {}
        None if early.len() < 64 => early.push((kind, name.to_owned(), props, Utc::now())),
        None => {}
    }
}

/// How many of `key`'s events the one going out stands for, or `None` to hold this one back:
/// one goes out every [`ONCE_IN`], so a loop failing thousands of times sends one a minute.
fn limit(key: String) -> Option<u64> {
    let now = Instant::now();
    let mut limits = LIMITS.lock();
    let limits = limits.get_or_insert_with(HashMap::new);
    match limits.get_mut(&key) {
        Some((last, held)) if now.duration_since(*last) < ONCE_IN => {
            *held += 1;
            None
        }
        Some((last, held)) => {
            let stands_for = *held + 1;
            (*last, *held) = (now, 0);
            Some(stands_for)
        }
        None => {
            limits.insert(key, (now, 0));
            Some(1)
        }
    }
}

/// The engine's code for `err`, and its outermost message without the user's names, by the
/// desktop app's names for them.
fn error_props(err: &anyhow::Error, names: &HashSet<String>, props: &mut Props) {
    if let Some(context) = err.custom_context()
        && context.code != but_error::Code::Unknown
    {
        props.insert("error_code".into(), context.code.to_string().into());
    }
    props.insert(
        "error_message".into(),
        redact(&err.to_string(), names).into(),
    );
}

/// Whether `err` is one the engine didn't foresee, which carries no code of its own.
fn unexpected(err: &anyhow::Error) -> bool {
    err.custom_context()
        .is_none_or(|context| context.code == but_error::Code::Unknown)
}

/// `err` for error tracking, grouped by `key` and what it says.
fn report_error(kind: String, err: &anyhow::Error, key: &str, names: &HashSet<String>) {
    let value = redact(&format!("{err:#}"), names);
    let fingerprint = format!("{:x}", md5::compute(format!("{key}\n{value}")));
    let props = json!({
        "$exception_list": [{
            "type": kind,
            "value": value,
            "mechanism": { "type": "generic", "handled": true },
        }],
        "$exception_level": "error",
        "$exception_fingerprint": fingerprint,
    });
    if let Value::Object(props) = props {
        queue(Kind::Errors, "$exception", props);
    }
}

/// What the user named in `workspaces`, which an error may show as a plain word, as a file
/// called `README` or a branch called `main`: branches, remotes, worktrees and changed files,
/// and each part of their paths, in lower case.
fn names<'a>(workspaces: impl IntoIterator<Item = &'a Workspace>) -> HashSet<String> {
    let mut named: Vec<&str> = Vec::new();
    let mut lossy = Vec::new();
    for workspace in workspaces {
        named.extend(workspace.target.iter().map(|target| target.name.as_str()));
        lossy.extend(
            workspace
                .target_branch
                .iter()
                .map(|name| name.to_string().into()),
        );
        named.extend(workspace.lost_target.as_deref());
        let worktrees = workspace.worktrees.iter();
        lossy.extend(
            worktrees
                .clone()
                .map(|worktree| worktree.name.to_str_lossy()),
        );
        lossy.extend(
            worktrees
                .clone()
                .filter_map(|worktree| worktree.branch.as_ref().map(super::short_ref)),
        );
        lossy.extend((workspace.archived_worktrees.iter()).map(|(name, _)| name.to_str_lossy()));
        for branch in workspace.all_branches() {
            named.push(&branch.name);
            named.extend(branch.remote.as_deref());
        }
        let files = (workspace.uncommitted.iter())
            .chain(worktrees.flat_map(|worktree| &worktree.uncommitted));
        for file in files {
            named.push(&file.path);
            named.extend(file.previous_path.as_deref());
        }
    }
    (named.into_iter().chain(lossy.iter().map(|name| &**name)))
        .flat_map(|name| name.split('/'))
        .map(str::to_lowercase)
        .collect()
}

/// The workspace's shape, by the names the CLI's and the desktop app's events give it.
fn shape(workspace: &Workspace, props: &mut Props) {
    let branches = workspace.lanes_in(&Checkout::Main).map(|stack| {
        (stack.branches.iter())
            .filter(|branch| branch.ref_name.is_some())
            .count()
    });
    props.insert(
        "totalLanesInWorkspace".into(),
        workspace.lanes_in(&Checkout::Main).count().into(),
    );
    props.insert(
        "totalBranchesInWorkspace".into(),
        branches.clone().sum::<usize>().into(),
    );
    props.insert(
        "maxBranchesPerLane".into(),
        branches.max().unwrap_or(0).into(),
    );
    // The desktop app's name: this app assigns no changes to lanes, so all are unassigned.
    props.insert(
        "totalUnassignedFiles".into(),
        workspace.uncommitted.len().into(),
    );
    props.insert("linkedWorktrees".into(), workspace.worktrees.len().into());
    props.insert("singleBranch".into(), (!workspace.managed).into());
    if let Some(forge) = workspace.forge {
        props.insert("forge".into(), forge.name().into());
    }
}

/// `Move to a new branch` as `moveToANewBranch`, as the CLI names its commands.
fn camel_case(name: &str) -> String {
    let mut words = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty());
    let first = words.next().unwrap_or_default().to_lowercase();
    words.fold(first, |mut camel, word| {
        let mut chars = word.chars();
        if let Some(c) = chars.next() {
            camel.extend(c.to_uppercase());
            camel.push_str(&chars.as_str().to_lowercase());
        }
        camel
    })
}

/// `text` without what may be the user's: what's quoted, the `names` they gave things, and
/// every word that's more than letters or a short number, as paths, refs, URLs, host and file
/// names, addresses, hashes and tokens all are. Other plain words and codes, as `404` or `128`,
/// stay, so the same failures group together. Code in backticks, as a panic's
/// `Result::unwrap()`, stays too.
fn redact(text: &str, names: &HashSet<String>) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut word = String::new();
    let mut chars = text.chars();
    let mut before = ' ';
    while let Some(c) = chars.next() {
        // An apostrophe inside a word, as in "couldn't", opens nothing.
        if matches!(c, '"' | '`') || (c == '\'' && !before.is_alphanumeric()) {
            push_word(&mut redacted, &mut word, names);
            let quoted: String = chars.by_ref().take_while(|&q| q != c).collect();
            redacted.push(c);
            match c == '`' && quoted.contains("::") {
                true => redacted.push_str(&quoted),
                false => redacted.push('…'),
            }
            redacted.push(c);
        } else if c.is_whitespace() {
            push_word(&mut redacted, &mut word, names);
            redacted.push(c);
        } else {
            word.push(c);
        }
        before = c;
    }
    push_word(&mut redacted, &mut word, names);
    if let Some((cut, _)) = redacted.char_indices().nth(500) {
        redacted.truncate(cut);
    }
    redacted
}

/// `word`, taken out of `word`, onto `redacted`, or `…` unless it's plain letters or a short
/// number once the punctuation around it is set aside, and not one of `names`.
fn push_word(redacted: &mut String, word: &mut String, names: &HashSet<String>) {
    let core = word.trim_matches(|c: char| !c.is_alphanumeric());
    let plain = (core.chars().all(|c| c.is_alphabetic() || c == '\'')
        || (core.len() <= 5 && core.chars().all(|c| c.is_ascii_digit())))
        && !names.contains(&core.to_lowercase());
    redacted.push_str(if plain { word } else { "…" });
    word.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_keeps_what_groups_failures_and_drops_the_users_names() {
        let none = HashSet::new();
        assert_eq!(
            redact(
                "Couldn't find 'feature/login' in \"my repo\" at /Users/me/src/app",
                &none
            ),
            "Couldn't find '…' in \"…\" at …",
            "quotes and paths go, an apostrophe inside a word opens no quote"
        );
        assert_eq!(
            redact(
                "called `Result::unwrap()` on `x` for refs/heads/main at 3f2a9c1d",
                &none
            ),
            "called `Result::unwrap()` on `…` for … at …",
            "code in backticks stays, refs and hashes go"
        );
        assert_eq!(
            redact(
                "Could not resolve host: my-laptop\nssh: connect to host git.example-corp.com \
                 port 22 as me@example.com for README.md with ghp_1a2B3c4D\n  \
                 git reset --soft 3f2a9c1d2e4b: exit code 128",
                &none
            ),
            "Could not resolve host: …\nssh: connect to host … port 22 as … for … with …\n  \
             git reset --soft … exit code 128",
            "hosts, addresses, file names and tokens go, after a line break too; codes stay"
        );
    }

    #[test]
    fn redact_drops_the_names_the_user_gave_things_even_as_plain_words() {
        let workspace = Workspace {
            target_branch: Some(crate::backend::model::build::full("refs/heads/Develop")),
            lost_target: Some("acme/main".into()),
            ..Default::default()
        };
        assert_eq!(
            redact(
                "Couldn't push develop to acme as main moved",
                &names([&workspace])
            ),
            "Couldn't push … to … as … moved",
            "branch and remote names go whatever their case, the words around them stay"
        );
    }

    #[test]
    fn camel_case_names_commands_as_the_cli_does() {
        assert_eq!(camel_case("Move to a new branch"), "moveToANewBranch");
        assert_eq!(camel_case("Cherry-pick"), "cherryPick");
        assert_eq!(camel_case("Commit"), "commit");
    }

    #[test]
    fn limit_lets_one_failure_out_a_minute_and_counts_the_rest() {
        let key = "test limit".to_owned();
        assert_eq!(limit(key.clone()), Some(1), "the first goes out");
        assert_eq!(limit(key.clone()), None, "a repeat within the minute waits");
        assert_eq!(limit(key.clone()), None, "and so does the next");
        if let Some(limits) = LIMITS.lock().as_mut()
            && let Some((last, _)) = limits.get_mut(&key)
        {
            *last -= ONCE_IN;
        }
        assert_eq!(
            limit(key),
            Some(3),
            "the next to go out stands for all three"
        );
    }
}
