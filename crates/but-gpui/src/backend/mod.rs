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
mod markers;
pub mod mesh;
pub mod model;
mod ops;
pub mod reviews;
pub mod settings;
pub mod upstream;
pub mod watch;
pub mod worktrees;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_core::{
    ChangeState, IgnoredWorktreeChange, IgnoredWorktreeTreeChangeStatus, TreeChange, TreeStatus,
};
use but_ctx::Context;

pub use diff::{DiffSource, file_lines, load_diff};
pub use model::*;
pub use ops::*;
pub use reviews::{refresh_checks, refresh_reviews};
pub use upstream::{
    fetch, land, pull_conflicts, pull_remote, push, push_refusal, update_from_target,
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
                last_active: last_active(git_dir),
                path,
            })
        })
        .collect();
    projects.sort_by_key(|project| project.name.to_lowercase());
    Ok(projects)
}

/// When Git last wrote the reflog or index of the repository at `git_dir` or one of its linked
/// worktrees: a few file times, cheap enough for every project, unlike reading its status.
fn last_active(git_dir: &Path) -> Option<i64> {
    last_written(git_dir, &["logs/HEAD", "index"])
}

/// When one of `files` was last written in the repository at `git_dir` or in one of its
/// linked worktrees, in seconds since the epoch.
fn last_written(git_dir: &Path, files: &[&str]) -> Option<i64> {
    let linked = std::fs::read_dir(git_dir.join("worktrees"))
        .into_iter()
        .flatten();
    std::iter::once(git_dir.to_owned())
        .chain(linked.flatten().map(|entry| entry.path()))
        .flat_map(|dir| files.iter().map(move |file| dir.join(file)))
        .filter_map(|file| file.metadata().and_then(|m| m.modified()).ok())
        .max()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_secs() as i64)
}

/// Register the repository at `path` with GitButler, so it shows up in [`list_projects()`].
/// Returns its id, and whether GitButler had it already, as from a folder inside it.
pub fn add_project(path: &Path) -> Result<(String, bool)> {
    use AddProjectOutcome as O;
    use gitbutler_project::AddProjectOutcome;
    let why = match but_api::legacy::projects::add_project_best_effort(path.to_owned())? {
        O::Added(project) => return Ok((project.id.to_string(), false)),
        O::AlreadyExists(project) => return Ok((project.id.to_string(), true)),
        O::PathNotFound => "it doesn't exist",
        O::NotADirectory => "it isn't a folder",
        O::NotAGitRepository(_) | O::NoDotGitDirectory => "it isn't a Git repository",
        O::BareRepository | O::NoWorkdir => "it's a Git repository without files to work on",
        O::NonMainWorktree => "it's a linked worktree. Add the repository it belongs to",
        O::ReftableRefFormatUnsupported => "GitButler can't read its reftable references yet",
    };
    anyhow::bail!("{} can't be added: {why}.", path.display())
}

/// Take the project out of GitButler. The repository stays as it is on disk.
pub fn remove_project(id: &str) -> Result<()> {
    but_api::legacy::projects::delete_project(id.parse().context("Invalid project id")?)
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
    but_api::legacy::projects::prepare_project_for_activation(&mut ctx)?;
    // As the CLI does: without a target, pushing and recording undo fail, so a new project
    // takes the one its remote suggests.
    if ctx.project_meta()?.target_ref.is_none() {
        let inferred = but_workspace::init::infer_default_target_ref(&*ctx.repo.get()?)?;
        if let Some(target) = inferred {
            but_api::workspace::set_target_ref_and_init_project(&mut ctx, target.as_ref(), None)?;
        }
    }
    Ok(())
}

/// Check GitButler's workspace out again, after a plain `git checkout` of one branch left it.
pub fn switch_back_to_workspace(path: &Path) -> Result<()> {
    but_api::legacy::virtual_branches::switch_back_to_workspace(&mut open(path)?)?;
    Ok(())
}

pub(crate) fn open(path: &Path) -> Result<Context> {
    Context::open(path).with_context(|| format!("Couldn't open {}", path.display()))
}

/// The project's main checkout, or its linked `worktree` with that name.
fn checkout(ctx: &Context, worktree: Option<&str>) -> Result<gix::Repository> {
    let main = ctx.repo.get()?;
    match worktree {
        Some(name) => {
            but_workspace::worktrees::open_worktree_repo(&main, name.as_bytes().as_bstr())
        }
        None => Ok(main.clone()),
    }
}

/// Uncommitted files read before, shown again rather than read: reading them is most of a
/// load, and an operation that changes only commits and branches leaves them as they are.
#[derive(Default)]
pub struct Kept {
    /// The main checkout's, with the tree of its `HEAD` they were read against.
    pub main: Option<(Vec<FileChange>, Option<gix::ObjectId>)>,
    /// Linked worktrees' by name.
    pub linked: HashMap<Arc<str>, KeptWorktree>,
}

/// A linked worktree's uncommitted files as shown, and the tree of its `HEAD` they were read
/// against.
pub type KeptWorktree = (Vec<FileChange>, Option<gix::ObjectId>);

impl Kept {
    /// What `shown` has, but for the main checkout's files with `main` and the `linked`
    /// worktrees' files, which are read again. A checkout's files are read again too when
    /// its `HEAD` has another tree by now: an operation on commits writes a checkout's
    /// files only where it changes that tree, so the rest stay as they were.
    pub fn from(shown: &Workspace, main: bool, linked: &HashSet<Arc<str>>) -> Self {
        Self {
            main: (!main).then(|| (shown.uncommitted.clone(), shown.tree)),
            linked: (shown.worktrees.iter())
                .filter(|worktree| !linked.contains(&worktree.name))
                .map(|w| (w.name.clone(), (w.uncommitted.clone(), w.tree)))
                .collect(),
        }
    }
}

/// The tree of `repo`'s `HEAD`, which its uncommitted files are read against.
fn head_tree(repo: &gix::Repository) -> Option<gix::ObjectId> {
    repo.head_tree_id().ok().map(|id| id.detach())
}

/// The project's workspace, with the uncommitted files `kept` doesn't have read from disk.
pub fn load_workspace(path: &Path, kept: Kept) -> Result<Workspace> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    let Kept { main, mut linked } = kept;
    let repo = ctx.repo.get()?.clone();
    let head = repo.head_name()?;
    let managed =
        (head.as_ref()).is_some_and(|name| name.as_bstr() == but_core::WORKSPACE_REF_NAME);
    let checked_out = head
        .filter(|_| !managed)
        .map(|name| name.shorten().to_string());
    let tree = head_tree(&repo);
    let git_operation = git_operation(&repo);
    // The uncommitted files and the refs are read beside the workspace's graph, which takes
    // longest, rather than before it: on a large repository, cold from disk, they take
    // hundreds of milliseconds.
    let (side, view) = std::thread::scope(|scope| -> Result<_> {
        let side = std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn_scoped(scope, move || -> Result<_> {
                let files = match main {
                    Some((files, read_against)) if read_against == tree => files,
                    _ => worktree_files(&repo)?,
                };
                Ok((files, refs_fingerprint(&repo)?))
            })?;
        // While a commit is being edited, its files are all there is to show.
        let view = match edit::editing(&ctx)? {
            Some(editing) => Err(editing),
            None => Ok(but_api::legacy::workspace::head_info(&ctx)?),
        };
        let side = side
            .join()
            .map_err(|_| anyhow::anyhow!("reading the uncommitted files panicked"))?;
        Ok((side?, view))
    })?;
    let (uncommitted, refs) = side;
    let info = match view {
        Ok(info) => info,
        Err(editing) => {
            return Ok(Workspace {
                uncommitted,
                tree,
                git_operation,
                refs,
                editing: Some(editing),
                ..Default::default()
            });
        }
    };

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
    // Linked worktrees' lanes count as behind too, but an update only rebases the stacks.
    let target_commit = info.target_commit.as_ref().map(|target| target.commit_id);
    let stacks_behind = info.has_lanes_behind_target
        && (info.stacks.iter())
            .any(|stack| stack.base.is_some_and(|base| Some(base) != target_commit));
    // Not `is_target_current`: a project added since its target last moved has no stored
    // target commit, and nothing to bring in.
    let target = info.target_ref.as_ref().map(|target| Target {
        name: target.ref_name.shorten().to_string(),
        behind: (target_commit.zip(ctx.repo.get().ok()))
            .and_then(|(base, repo)| {
                landed_since(&repo, target.ref_name.as_ref(), base, target.commits_ahead)
            })
            .unwrap_or(target.commits_ahead),
        commits_behind: target.commits_ahead,
        needs_update: target.commits_ahead > 0 || stacks_behind,
        incoming: match target.commits_ahead {
            0 => Vec::new(),
            _ => upstream::incoming(&ctx, model::UPSTREAM_SHOWN).unwrap_or_else(|err| {
                tracing::warn!("couldn't read the target's new commits: {err:#}");
                Vec::new()
            }),
        },
    });
    let lost_target = (target.is_none())
        .then(|| ctx.project_meta().ok()?.target_ref)
        .flatten()
        .map(|name| name.shorten().to_string());
    let listed = worktrees::linked(&ctx, &mut linked).unwrap_or_else(|err| {
        tracing::warn!("couldn't list the linked worktrees: {err:#}");
        None
    });
    let (mut worktrees, archived_worktrees) = listed.unwrap_or_default();
    for worktree in &mut worktrees {
        let lane = (info.worktrees.iter()).find(|lane| lane.name == worktree.name.as_ref());
        let segments = lane.iter().flat_map(|lane| &lane.segments);
        let mut past_theirs = false;
        worktree.branches = segments
            .filter_map(|segment| {
                use but_workspace::ref_info::LocalCommitRelation as R;
                // A lane started from a newer target than the workspace's runs on down through
                // the target's own commits, which aren't the worktree's work. The branch the
                // worktree's work starts from shows, even with no commits of its own, but not
                // the target's branches further down.
                let theirs = (segment.commits.iter()).position(
                    |commit| matches!(commit.relation, R::Integrated(by) if by == commit.inner.id),
                );
                if past_theirs {
                    return None;
                }
                past_theirs = theirs.is_some();
                let mut branch = branch_from(&ctx, &cached, segment);
                // A detached run of commits has no ref to tell it apart by, and a branch the
                // workspace has applied is listed there.
                let applied = (stacks.iter().flat_map(|stack| &stack.branches))
                    .any(|stacked| stacked.ref_name == branch.ref_name);
                if branch.ref_name.is_empty() || applied {
                    return None;
                }
                if let Some(theirs) = theirs {
                    branch.commits.truncate(theirs);
                }
                Some(branch)
            })
            .collect();
    }
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "loaded workspace");
    Ok(Workspace {
        target,
        lost_target,
        forge: reviews::forge_of(&ctx),
        stacks,
        uncommitted,
        tree,
        managed,
        checked_out,
        git_operation,
        editing: None,
        worktrees,
        archived_worktrees,
        refs: refs ^ reviews_fingerprint(&cached),
    })
}

/// A hash of the pull requests the forge cache knows, so a refresh that learns of one, or
/// of a new title or state, has the Branches page list them again.
fn reviews_fingerprint(cached: &HashMap<u64, but_forge::ForgeReview>) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    // Order-free, as the map's order isn't fixed.
    cached.values().fold(0, |sum, review| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let settled = (&review.merged_at, &review.closed_at);
        (review.number, &review.title, review.draft, settled).hash(&mut hasher);
        sum ^ hasher.finish()
    })
}

/// The Git command `repo`'s checkout is stopped partway through, waiting for `--continue`.
fn git_operation(repo: &gix::Repository) -> Option<&'static str> {
    use gix::state::InProgress as P;
    repo.state().and_then(|state| match state {
        P::Rebase | P::RebaseInteractive | P::ApplyMailboxRebase => Some("git rebase"),
        P::Merge => Some("git merge"),
        P::CherryPick | P::CherryPickSequence => Some("git cherry-pick"),
        P::Revert | P::RevertSequence => Some("git revert"),
        P::ApplyMailbox => Some("git am"),
        P::Bisect => None,
    })
}

/// A hash of every local and remote branch's name and tip.
fn refs_fingerprint(repo: &gix::Repository) -> Result<u64> {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let refs = repo.references()?;
    // A ref gix can't read, like an empty file left by a crashed fetch, is skipped as the
    // graph skips it.
    for reference in (refs.local_branches()?.chain(refs.remote_branches()?)).filter_map(Result::ok)
    {
        reference.name().as_bstr().hash(&mut hasher);
        reference
            .target()
            .try_id()
            .map(|id| id.to_owned())
            .hash(&mut hasher);
    }
    Ok(hasher.finish())
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
            .map(|commit| {
                let ((author, email), agent) = byline_of(&commit.author, &commit.committer);
                RemoteCommit {
                    id: CommitId(commit.id),
                    title: title(&commit.message),
                    author,
                    email,
                    agent,
                    time: commit.author.time.seconds,
                }
            })
            .collect(),
        remote: (segment.remote_tracking_ref_name.as_ref()).map(|name| name.shorten().to_string()),
        push,
        review,
    }
}

/// How many commits along `target`'s own line of history its tip is past `base`, what the
/// workspace was last updated to: each pull request merged one, as the project's page lists
/// them, rather than one for each commit it merged too, of which there are `all`. `None` when
/// `base` isn't on that line, as after the target was rewritten.
fn landed_since(
    repo: &gix::Repository,
    target: &gix::refs::FullNameRef,
    base: gix::ObjectId,
    all: usize,
) -> Option<usize> {
    let tip = repo
        .find_reference(target)
        .ok()?
        .peel_to_id()
        .ok()?
        .detach();
    first_parent_steps(repo, tip, base, all)
}

/// How many first parents lead back from `tip` to `base`, at most `max`; `None` when `base`
/// isn't among them.
fn first_parent_steps(
    repo: &gix::Repository,
    mut tip: gix::ObjectId,
    base: gix::ObjectId,
    max: usize,
) -> Option<usize> {
    let mut steps = 0;
    while tip != base {
        steps += 1;
        if steps > max {
            return None;
        }
        tip = repo.find_commit(tip).ok()?.parent_ids().next()?.detach();
    }
    Some(steps)
}

/// Who a commit is shown by, from its author and committer: see [`byline`].
fn byline_of(
    author: &gix::actor::Signature,
    committer: &gix::actor::Signature,
) -> ((String, String), Option<Agent>) {
    let signature = |s: &gix::actor::Signature| signature(s.name.as_ref(), s.email.as_ref());
    byline(signature(author), signature(committer))
}

/// A signature's `name` and `email`, as [`byline`] takes them.
fn signature(name: &gix::bstr::BStr, email: &gix::bstr::BStr) -> (String, String) {
    (
        name.to_str_lossy().into_owned(),
        email.to_str_lossy().into_owned(),
    )
}

/// The committer of commit `id`, as [`byline`] takes it, when the repository has it.
fn committer(repo: &gix::Repository, id: gix::ObjectId) -> Option<(String, String)> {
    let commit = repo.find_commit(id).ok()?;
    let committer = commit.committer().ok()?;
    Some(signature(committer.name, committer.email))
}

fn commit_from(local: &but_workspace::ref_info::LocalCommit) -> Commit {
    use but_workspace::ref_info::LocalCommitRelation as R;
    let inner = &local.inner;
    let ((author, email), agent) = byline_of(&inner.author, &inner.committer);
    Commit {
        id: CommitId(inner.id),
        key: inner.change_id().to_string(),
        authored: model::authored(&inner.author, inner.message.trim_end()),
        title: title(&inner.message),
        message: inner.message.to_str_lossy().trim_end().to_string(),
        author,
        email,
        agent,
        time: inner.author.time.seconds,
        state: match local.relation {
            R::LocalOnly => CommitState::Local,
            // The remote has an older version of it: amended or reworded since the push.
            R::LocalAndRemote(remote) if remote != inner.id => CommitState::Local,
            R::LocalAndRemote(_) => CommitState::Pushed,
            R::Integrated(_) => CommitState::Integrated,
        },
        conflicted: inner.has_conflicts,
        parent: inner.parent_ids.first().copied().map(CommitId),
        tree: inner.tree_id,
    }
}

/// What signed commit `id`, like "an SSH key", or `None` when it isn't signed. The signature
/// isn't checked: that takes the signer's public keys, which only the user's tools know.
pub fn commit_signer(path: &Path, id: CommitId) -> Result<Option<&'static str>> {
    use gix::objs::signature::Format;
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let commit = repo.find_commit(id.0)?;
    let signature = commit.signature()?;
    Ok(
        signature.map(|(signature, _)| match Format::from_signature(&signature) {
            Some(Format::OpenPgp) => "a GPG key",
            Some(Format::Ssh) => "an SSH key",
            Some(Format::X509) => "an X.509 certificate",
            None => "a key",
        }),
    )
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
        lines: None,
        change,
    }
}

/// Past this many uncommitted files, as an agent's generated output can make, their lines
/// aren't counted: reading them all would hold up every reload.
const COUNT_LINES_UP_TO: usize = 300;

/// Uncommitted files, each with when it was last written and, while there are few enough,
/// the lines it adds and removes, and the files the index has conflicted, as a merge leaves
/// them.
fn worktree_files(repo: &gix::Repository) -> Result<Vec<FileChange>> {
    let changes = but_core::diff::worktree_changes(repo)?;
    // Status leaves out files with unresolved index conflicts; they're changes too.
    let conflicted = conflicted_changes(repo, &changes.ignored_changes)?;
    let count = conflicted.len() + changes.changes.len() <= COUNT_LINES_UP_TO;
    let lines = |change: &but_core::TreeChange| match change.unified_patch(repo, 0) {
        Ok(Some(but_core::UnifiedPatch::Patch {
            lines_added,
            lines_removed,
            ..
        })) => Some((lines_added, lines_removed)),
        _ => None,
    };
    let changes = changes.changes.into_iter().map(|change| (change, false));
    // Conflicted first, as a conflicted commit lists them: they're what needs doing.
    Ok((conflicted.into_iter().map(|change| (change, true)))
        .chain(changes)
        .map(|(change, conflicted)| FileChange {
            modified: written(repo, change.path.as_bstr()),
            conflicted,
            lines: count.then(|| lines(&change)).flatten(),
            ..file_change(change)
        })
        .collect())
}

/// When the file at `path` in `repo`'s checkout was last written, in milliseconds since the
/// Unix epoch; a deleted file has none.
fn written(repo: &gix::Repository, path: &bstr::BStr) -> Option<i64> {
    let path = repo.workdir()?.join(gix::path::from_bstr(path).ok()?);
    let written = path.symlink_metadata().ok()?.modified().ok()?;
    let since_epoch = written.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(since_epoch.as_millis()).ok()
}

/// The files Git's index still has conflicted, as changes from `HEAD` to what's on disk:
/// a merge leaves its conflicts there, and status passes over them.
fn conflicted_changes(
    repo: &gix::Repository,
    ignored: &[IgnoredWorktreeChange],
) -> Result<Vec<TreeChange>> {
    let conflicted = |change: &&IgnoredWorktreeChange| {
        change.status == IgnoredWorktreeTreeChangeStatus::Conflict
    };
    // Status runs on every change to the worktree; most of the time nothing's conflicted.
    if !ignored.iter().any(|change| conflicted(&change)) {
        return Ok(Vec::new());
    }
    let head = repo.head_tree()?;
    let on_disk = |kind| ChangeState {
        id: gix::ObjectId::null(repo.object_hash()),
        kind,
    };
    let mut changes = Vec::new();
    for change in ignored.iter().filter(conflicted) {
        let path = gix::path::from_bstr(change.path.as_bstr())?;
        let status = match head.lookup_entry_by_path(path)? {
            Some(entry) => {
                let kind = entry.mode().kind();
                TreeStatus::Modification {
                    previous_state: ChangeState {
                        id: entry.object_id(),
                        kind,
                    },
                    state: on_disk(kind),
                    flags: None,
                }
            }
            None => TreeStatus::Addition {
                state: on_disk(gix::object::tree::EntryKind::Blob),
                is_untracked: false,
            },
        };
        changes.push(TreeChange {
            path: change.path.clone(),
            status,
        });
    }
    Ok(changes)
}

/// What a failed operation's card says: the engine's own words, except where those name a
/// commit the user never saw, or leave out what the engine did about it.
pub fn failure_detail(err: &anyhow::Error) -> String {
    match err.downcast_ref::<but_error::Code>() {
        Some(but_error::Code::ConflictedCommitCheckout) => {
            "That would leave the checked-out commit conflicted, which Git can't check out."
                .to_owned()
        }
        // The engine turns signing off for the project then, and the next commit would
        // quietly go unsigned.
        Some(but_error::Code::CommitSigningFailed) => format!(
            "Signing it failed, which turns commit signing off for this project unless it's \
             on in your global Git config. Check Sign commits in Project settings.\n{}",
            err.root_cause()
        ),
        // A precondition says what stopped it in the user's words; the context wrapped
        // around it names refs and hashes, which a short card fills up with.
        _ => match err.downcast_ref::<but_error::Context>() {
            Some(said) if said.code == but_error::Code::PreconditionFailed => said.to_string(),
            _ => format!("{err:#}"),
        },
    }
}

/// `name` as a branch's short name, as the engine normalizes it, or `None` when Git doesn't
/// allow it.
pub fn branch_name(name: &str) -> Option<String> {
    but_core::branch::normalize_short_name(name)
        .ok()
        .map(|short| short.to_string())
}

#[cfg(test)]
mod failure_tests {
    use super::failure_detail;

    #[test]
    fn a_refused_conflicted_checkout_says_why_without_a_hash() {
        let err = anyhow::anyhow!(but_error::Code::ConflictedCommitCheckout)
            .context("Refusing to check out conflicted commit 7c20f085")
            .context("Moving the commit");
        assert!(
            failure_detail(&err).starts_with("That would leave the checked-out commit"),
            "the engine's code is found under the context it adds"
        );
        let err = anyhow::anyhow!(but_error::Code::CommitSigningFailed).context("Signing failed");
        assert!(
            failure_detail(&err).contains("turns commit signing off"),
            "the engine turned signing off, which the next commit would otherwise not show"
        );
        let err = anyhow::anyhow!(
            but_error::Context::new("Uncommitted files would be overwritten by checkout: \"a.rs\"")
                .with_code(but_error::Code::PreconditionFailed)
        )
        .context("Could not safely check out 'refs/heads/b' from 1234 to 5678");
        assert_eq!(
            failure_detail(&err),
            "Uncommitted files would be overwritten by checkout: \"a.rs\"",
            "what the engine says stopped it, not the hashes of what it was doing"
        );
    }
}
