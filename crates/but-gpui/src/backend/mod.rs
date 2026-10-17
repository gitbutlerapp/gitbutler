//! The only part of the app that talks to GitButler. Everything here is blocking and
//! meant to run on a background thread; results are plain [`model`] data.

pub mod accounts;
pub mod ai;
mod annotate;
pub mod askpass;
pub mod branches;
pub mod conflicts;
mod diff;
pub mod edit;
mod highlight;
pub mod history;
pub mod model;
mod ops;
pub mod reviews;
pub mod settings;
pub mod upstream;
pub mod watch;
pub mod worktrees;

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_ctx::Context;

pub use diff::{DiffSource, file_lines, load_diff};
pub use model::*;
pub use ops::*;
pub use reviews::{refresh_checks, refresh_reviews};
pub use upstream::{
    fetch, land, pull_conflicts, pull_remote, push, push_refusal, update_conflicts,
    update_from_target,
};
pub use worktrees::reload_uncommitted;

/// Process-wide setup, before any other call.
pub fn init() {
    // Forge tokens live in the keychain under the app's identifier.
    but_api::platform::init_application_namespace(None).ok();
}

/// The runtime every GitButler call runs on. Its threads get big stacks: GitButler's
/// graph walks and async push futures are deep, especially in debug builds, and would
/// overflow the small stacks of the UI framework's own worker threads.
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_stack_size(16 * 1024 * 1024)
            .enable_all()
            .thread_name("gitbutler")
            .build()
            .expect("tokio runtime starts")
    })
}

/// Run blocking GitButler work off the UI thread; await the result from anywhere.
pub fn run<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> impl std::future::Future<Output = Result<T>> {
    let handle = runtime().spawn_blocking(work);
    async move { handle.await.context("GitButler task failed")? }
}

pub fn list_projects() -> Result<Vec<Project>> {
    let mut projects: Vec<Project> = but_api::legacy::projects::list_projects_stateless()?
        .into_iter()
        .filter_map(|entry| {
            let project: gitbutler_project::Project = entry.inner.into();
            let git_dir = project.git_dir();
            let path = if git_dir.ends_with(".git") {
                git_dir.parent()?.to_owned()
            } else {
                git_dir.to_owned()
            };
            // Kept when the folder is gone, so the app can say so and offer to find it.
            Some(Project {
                id: project.id.to_string(),
                name: project.title.clone(),
                path,
            })
        })
        .collect();
    projects.sort_by_key(|project| project.name.to_lowercase());
    Ok(projects)
}

/// Register the repository at `path` with GitButler, so it shows up in [`list_projects()`].
pub fn add_project(path: &Path) -> Result<()> {
    use gitbutler_project::AddProjectOutcome;
    match but_api::legacy::projects::add_project_best_effort(path.to_owned())? {
        AddProjectOutcome::Added(_) | AddProjectOutcome::AlreadyExists(_) => Ok(()),
        other => anyhow::bail!("Can't add {}: {other}", path.display()),
    }
}

/// Take the project out of GitButler. The repository stays as it is on disk.
pub fn remove_project(id: &str) -> Result<()> {
    but_api::legacy::projects::delete_project(id.parse().context("Invalid project id")?)
}

/// Tell GitButler the project `id` now lives at `path`.
pub fn relocate_project(id: &str, path: &Path) -> Result<()> {
    gitbutler_project::update(gitbutler_project::UpdateRequest {
        path: Some(path.to_owned()),
        git_dir: Some(path.join(".git")),
        ..gitbutler_project::UpdateRequest::default_with_id(
            id.parse().context("Invalid project id")?,
        )
    })?;
    Ok(())
}

/// Clone `url` into the folder `into`, which shouldn't exist yet.
pub fn clone(url: &str, into: &Path) -> Result<()> {
    runtime()
        .handle()
        .block_on(but_api::legacy::repo::git_clone_repository(
            url.to_owned(),
            into.to_owned(),
        ))
}

/// Get a project ready to show: the first time, GitButler sets up its workspace
/// metadata, like which branch is the target.
pub fn activate(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::legacy::projects::prepare_project_for_activation(&mut ctx)
}

pub(crate) fn open(path: &Path) -> Result<Context> {
    Context::open(path).with_context(|| format!("Couldn't open {}", path.display()))
}

pub fn load_workspace(path: &Path) -> Result<Workspace> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    let uncommitted = worktree_files(&*ctx.repo.get()?)?;
    if let Some(editing) = edit::editing(&ctx)? {
        return Ok(Workspace {
            uncommitted,
            editing: Some(editing),
            ..Default::default()
        });
    }
    let info = but_api::legacy::workspace::head_info(&ctx)?;

    let cached = reviews::cached_reviews(&ctx);
    let stacks: Vec<Stack> = info
        .stacks
        .iter()
        .map(|stack| Stack {
            branches: (stack.segments.iter())
                .map(|segment| branch_from(&ctx, &cached, segment))
                .collect(),
        })
        .collect();
    let target = info.target_ref.as_ref().map(|target| Target {
        name: target.ref_name.shorten().to_string(),
        behind: target.commits_ahead,
        needs_update: !info.is_target_current || info.has_lanes_behind_target,
    });
    let listed = worktrees::linked(&ctx).unwrap_or_else(|err| {
        tracing::warn!("couldn't list the linked worktrees: {err:#}");
        None
    });
    let worktrees_listed = listed.is_some();
    let (worktrees, archived_worktrees) = listed.unwrap_or_default();
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "loaded workspace");
    Ok(Workspace {
        target,
        forge: reviews::forge_of(&ctx),
        stacks,
        uncommitted,
        editing: None,
        worktrees,
        archived_worktrees,
        worktrees_listed,
    })
}

/// `message` with its body wrapped as `git log` and `but reword` expect.
pub fn wrapped_message(message: &str) -> String {
    but_action::commit_format::format_commit_message(message)
}

/// The branch of `segment`, with its pull request as the forge cache, `cached`, has it.
fn branch_from(
    ctx: &Context,
    cached: &HashMap<u64, but_forge::ForgeReview>,
    segment: &but_workspace::ref_info::Segment,
) -> Branch {
    let (name, ref_name) = match segment.ref_name() {
        Some(name) => (name.shorten().to_string(), name.to_string()),
        None => ("Unnamed branch".into(), String::new()),
    };
    use but_core::ui::PushStatus as P;
    let push = match segment.push_status {
        P::NothingToPush => PushState::NothingToPush,
        P::UnpushedCommits => PushState::Unpushed,
        P::UnpushedCommitsRequiringForce => PushState::NeedsForce,
        P::CompletelyUnpushed => PushState::NeverPushed,
        P::Integrated => PushState::Integrated,
    };
    let review = (segment.metadata.as_ref())
        .and_then(|meta| meta.review.pull_request)
        .map(|number| reviews::review(ctx, cached, number as u64, &name, push));
    Branch {
        name,
        ref_name,
        commits: segment.commits.iter().map(commit_from).collect(),
        incoming: (segment.commits_on_remote.iter())
            .map(|commit| title(&commit.message))
            .collect(),
        push,
        review,
    }
}

fn commit_from(local: &but_workspace::ref_info::LocalCommit) -> Commit {
    use but_workspace::ref_info::LocalCommitRelation as R;
    let inner = &local.inner;
    Commit {
        id: CommitId(inner.id),
        key: inner
            .change_id
            .as_ref()
            .map_or_else(|| inner.id.to_string(), ToString::to_string),
        title: title(&inner.message),
        message: inner.message.to_str_lossy().trim_end().to_string(),
        author: inner.author.name.to_str_lossy().into_owned(),
        time: inner.author.time.seconds,
        state: match local.relation {
            R::LocalOnly => CommitState::Local,
            R::LocalAndRemote(_) => CommitState::Pushed,
            R::Integrated(_) => CommitState::Integrated,
        },
        conflicted: inner.has_conflicts,
    }
}

/// The first line of a commit message or comment, without the whitespace around it.
fn title(message: &[u8]) -> String {
    let line = message.lines().next().unwrap_or_default();
    line.to_str_lossy().trim().to_owned()
}

fn file_change(change: but_core::TreeChange) -> FileChange {
    use but_core::TreeStatus as S;
    let (kind, previous_path) = match &change.status {
        S::Addition { .. } => (ChangeKind::Added, None),
        S::Deletion { .. } => (ChangeKind::Deleted, None),
        S::Modification { .. } => (ChangeKind::Modified, None),
        S::Rename { previous_path, .. } => (
            ChangeKind::Renamed,
            Some(previous_path.to_str_lossy().into_owned()),
        ),
    };
    FileChange {
        path: change.path.to_str_lossy().into_owned(),
        previous_path,
        kind,
        modified: None,
        conflicted: false,
        change,
    }
}

/// Uncommitted files, each with when it was last written, and the files the index has
/// conflicted, as a merge leaves them.
fn worktree_files(repo: &gix::Repository) -> Result<Vec<FileChange>> {
    let changes = but_core::diff::worktree_changes(repo)?;
    // Status leaves out files with unresolved index conflicts; they're changes too.
    let conflicted = edit::conflicted_changes(repo, &changes.ignored_changes)?;
    // In milliseconds; a deleted file has none.
    let modified = |path: &bstr::BStr| -> Option<i64> {
        let path = repo.workdir()?.join(gix::path::from_bstr(path).ok()?);
        let written = path.symlink_metadata().ok()?.modified().ok()?;
        let since_epoch = written.duration_since(std::time::UNIX_EPOCH).ok()?;
        i64::try_from(since_epoch.as_millis()).ok()
    };
    let changes = changes.changes.into_iter().map(|change| (change, false));
    Ok(changes
        .chain(conflicted.into_iter().map(|change| (change, true)))
        .map(|(change, conflicted)| FileChange {
            modified: modified(change.path.as_bstr()),
            conflicted,
            ..file_change(change)
        })
        .collect())
}
