//! The only part of the app that talks to GitButler. Everything here is blocking and
//! meant to run on a background thread; results are plain [`model`] data.

pub mod accounts;
pub mod ai;
mod annotate;
pub mod askpass;
pub mod branches;
pub mod cli;
pub mod conflicts;
mod diff;
pub mod edit;
mod highlight;
pub mod history;
pub mod images;
pub mod keymap;
pub mod logs;
mod markers;
pub mod mesh;
pub mod metrics;
pub mod model;
mod ops;
pub mod rebuild;
pub mod reviews;
pub mod saved;
pub mod settings;
pub mod upstream;
pub mod watch;
pub mod worktrees;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_core::{
    ChangeState, IgnoredWorktreeChange, IgnoredWorktreeTreeChangeStatus, TreeChange, TreeStatus,
};
use but_ctx::Context;

pub use diff::{DiffSource, blame_lines, commit_paths, file_lines, load_diff};
pub use model::*;
pub use ops::*;
pub use reviews::{refresh_checks, refresh_reviews};
pub use upstream::{
    Landed, fetch, land, pull_conflicts, pull_remote, push, push_refusal, update_from_target,
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
#[expect(
    clippy::expect_used,
    reason = "nothing GitButler does works without it, and only a system out of threads fails it"
)]
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
            let path = match git_dir.ends_with(".git") {
                true => git_dir.parent()?.to_owned(),
                // A submodule's is in its superproject's, and its config says where its files are.
                false => (gix::open_opts(git_dir, gix::open::Options::isolated()).ok())
                    .and_then(|repo| repo.workdir().map(Path::to_owned))
                    .unwrap_or_else(|| git_dir.to_owned()),
            };
            // Kept when the folder is gone, so the app can say so and offer to find it.
            Some(Project {
                id: project.id.to_string(),
                name: project.title.clone(),
                // When Git last wrote its reflog or index: a few file times, cheap enough for
                // every project, unlike reading its status.
                last_active: last_written(git_dir, &["logs/HEAD", "index"]),
                last_fetch: (project.project_data_last_fetch.as_ref())
                    .and_then(|fetch| fetch.timestamp().duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|since| since.as_secs() as i64),
                path,
            })
        })
        .collect();
    projects.sort_by_key(|project| project.name.to_lowercase());
    Ok(projects)
}

/// When one of `files` was last written in the repository at `git_dir` or in one of its
/// linked worktrees, in seconds since the epoch. An empty one, as a failed fetch leaves
/// FETCH_HEAD, doesn't count.
fn last_written(git_dir: &Path, files: &[&str]) -> Option<i64> {
    let linked = std::fs::read_dir(git_dir.join("worktrees"))
        .into_iter()
        .flatten();
    std::iter::once(git_dir.to_owned())
        .chain(linked.flatten().map(|entry| entry.path()))
        .flat_map(|dir| files.iter().map(move |file| dir.join(file)))
        .filter_map(|file| file.metadata().ok())
        .filter(|metadata| metadata.len() > 0)
        .filter_map(|metadata| metadata.modified().ok())
        .max()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_secs() as i64)
}

/// A folder that isn't in a Git repository and holds none: `git init` makes it one to add.
#[derive(Debug)]
pub struct NotARepository;

impl std::fmt::Display for NotARepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("It isn't a Git repository yet.")
    }
}

impl std::error::Error for NotARepository {}

/// Whether `path` is a folder inside a repository's checkout, not its top, that the
/// repository ignores and has no files of; `None` when it isn't in one below its top, or that
/// can't be read.
fn ignored_by_enclosing_repository(path: &Path) -> Option<bool> {
    use gix::worktree::stack::state::ignore::Source;
    let repo = gix::discover(path).ok()?;
    let (workdir, path) = (
        repo.workdir()?.canonicalize().ok()?,
        path.canonicalize().ok()?,
    );
    let inside = path
        .strip_prefix(&workdir)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())?;
    let index = repo.index_or_empty().ok()?;
    // As a dotfiles repository has ~/.config/nvim, added despite ignoring everything.
    let inside_bstr = gix::path::into_bstr(inside).ok()?;
    let prefix = format!("{}/", gix::path::to_unix_separators_on_windows(inside_bstr));
    if (index.entries().iter()).any(|e| e.path(&index).starts_with(prefix.as_bytes())) {
        return Some(false);
    }
    let mut excludes =
        (repo.excludes(&index, None, Source::WorktreeThenIdMappingIfNotSkipped)).ok()?;
    let at = excludes
        .at_path(inside, Some(gix::index::entry::Mode::DIR))
        .ok()?;
    Some(at.is_excluded())
}

/// Register the repository at `path` with GitButler, so it shows up in [`list_projects()`].
/// Returns its id, and whether GitButler had it already, as from a folder inside it. A folder
/// that could become a repository fails with [`NotARepository`].
pub fn add_project(path: &Path) -> Result<(String, bool)> {
    use AddProjectOutcome as O;
    use gitbutler_project::AddProjectOutcome;
    // As a home folder kept as a dotfiles repository, which ignores all but its dotfiles: the
    // folder isn't that repository's work, and adding it would watch the whole home folder.
    let outcome = match ignored_by_enclosing_repository(path) {
        Some(true) => O::NoDotGitDirectory,
        _ => but_api::legacy::projects::add_project_best_effort(path.to_owned())?,
    };
    let why = match outcome {
        O::Added(project) => return Ok((project.id.to_string(), false)),
        O::AlreadyExists(project) => return Ok((project.id.to_string(), true)),
        O::PathNotFound => "it doesn't exist",
        O::NotADirectory => "it isn't a folder",
        // A folder of projects, picked instead of one of them.
        O::NotAGitRepository(_) | O::NoDotGitDirectory => match repositories_in(path)[..] {
            [] => return Err(NotARepository.into()),
            [ref one] => &format!("it isn't a Git repository, but {one} in it is. Add that"),
            [ref one, ref two, ..] => &format!(
                "it isn't a Git repository, but folders in it are, like {one} and {two}. \
                 Add one of those"
            ),
        },
        // A worktree of a bare clone has files, a layout some keep agents' worktrees in.
        O::BareRepository if gix::discover(path).is_ok_and(|repo| repo.workdir().is_some()) => {
            "it's a worktree of a bare repository, and GitButler needs one with a main worktree"
        }
        O::BareRepository | O::NoWorkdir => "it's a Git repository without files to work on",
        O::NonMainWorktree => "it's a linked worktree. Add the repository it belongs to",
        O::ReftableRefFormatUnsupported => "GitButler can't read its reftable references yet",
    };
    // The folder just picked, by name: a card has room for a few lines.
    let name = path.file_name().map_or(path.as_os_str(), |name| name);
    anyhow::bail!("{} can't be added: {why}.", name.display())
}

/// The names of the folders right inside `path` that are Git repositories, sorted.
fn repositories_in(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = (std::fs::read_dir(path).into_iter().flatten().flatten())
        .filter(|entry| entry.path().join(".git").exists())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Make the folder `path` a Git repository, as `git init` does.
pub fn init_repository(path: &Path) -> Result<()> {
    gix::init(path)
        .with_context(|| format!("Couldn't make {} a Git repository", path.display()))?;
    Ok(())
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
            let target = target.as_local_full_name_ref();
            // A checkout with no commits yet, as a new orphan branch is, can't take one until
            // it has some, and is still watched meanwhile: the next activation tries again.
            let unborn = ctx.repo.get()?.head()?.is_unborn();
            match but_api::workspace::set_target_ref_and_init_project(&mut ctx, target, None) {
                Err(err) if unborn => {
                    tracing::warn!("couldn't make {} the target: {err:#}", target.shorten())
                }
                set => set?,
            }
        }
    }
    Ok(())
}

pub(crate) fn open(path: &Path) -> Result<Context> {
    Context::open(path).with_context(|| format!("Couldn't open {}", path.display()))
}

/// The project's main checkout, or its linked `worktree` with that name.
fn checkout(ctx: &Context, worktree: Option<&bstr::BStr>) -> Result<gix::Repository> {
    let main = ctx.repo.get()?;
    match worktree {
        Some(name) => but_workspace::worktrees::open_worktree_repo(&main, name),
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
    pub linked: HashMap<bstr::BString, KeptWorktree>,
}

/// A linked worktree's uncommitted files as shown, and the tree of its `HEAD` they were read
/// against.
pub type KeptWorktree = (Vec<FileChange>, Option<gix::ObjectId>);

impl Kept {
    /// What `shown` has, but for the main checkout's files with `main` and the `linked`
    /// worktrees' files, which are read again. A checkout's files are read again too when
    /// its `HEAD` has another tree by now: an operation on commits writes a checkout's
    /// files only where it changes that tree, so the rest stay as they were.
    pub fn from(shown: &Workspace, main: bool, linked: &HashSet<bstr::BString>) -> Self {
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
    let remoteless = repo.remote_names().is_empty();
    let managed =
        (head.as_ref()).is_some_and(|name| name.as_bstr() == but_core::WORKSPACE_REF_NAME);
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
                    Some((files, _)) => worktree_files(&repo, &files)?,
                    None => worktree_files(&repo, &[])?,
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
    let mut lanes: Vec<Lane> = info
        .stacks
        .iter()
        .map(|stack| Lane {
            branches: (stack.segments.iter())
                .map(|segment| branch_from(&ctx, &cached, segment))
                .collect(),
            checkout: Checkout::Main,
            rests_on: stack.lane().rests_on.map(CommitId),
        })
        .collect();
    let meta = ctx.project_meta().ok();
    let (target, lost_target) = target(&ctx, meta.as_ref(), &info, &cached)?;
    let (worktrees, archived_worktrees) =
        worktrees::linked(&ctx, &mut linked).unwrap_or_else(|err| {
            tracing::warn!("couldn't list the linked worktrees: {err:#}");
            Default::default()
        });
    let linked_lanes: Vec<Lane> = (worktrees.iter())
        .filter_map(|worktree| {
            let lane = (info.lanes())
                .find(|lane| lane.worktree.is_some_and(|name| *name == *worktree.name))?;
            Some(Lane {
                branches: worktree_branches(&ctx, &cached, &lane, &lanes),
                checkout: Checkout::Linked(worktree.name.clone()),
                rests_on: lane.rests_on.map(CommitId),
            })
        })
        .collect();
    lanes.extend(linked_lanes);
    tracing::debug!(elapsed = ?started.elapsed(), path = %path.display(), "loaded workspace");
    let target_branch = (meta.and_then(|meta| meta.target_ref))
        .map(|name| name.as_local_full_name_ref().to_owned());
    // The oplog records a snapshot against the target, so without one set it records none.
    // One whose upstream went, or isn't there yet, still records, against its stored commit.
    let unrecorded = head.is_none() || tree.is_none() || target_branch.is_none();
    Ok(Workspace {
        target,
        lost_target,
        forge: reviews::forge_name(&ctx).map(reviews::app_forge),
        lanes,
        uncommitted,
        tree,
        managed,
        target_branch,
        git_operation,
        unrecorded,
        remoteless,
        editing: None,
        // The branches the worktrees shown have checked out, which the Branches page leaves off.
        refs: (worktrees.iter()).fold(refs ^ reviews_fingerprint(&cached), |sum, w| {
            use std::hash::{Hash as _, Hasher as _};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            w.branch.hash(&mut hasher);
            sum ^ hasher.finish()
        }),
        worktrees,
        archived_worktrees,
    })
}

/// The target as `info` and the project's `meta` have it, with its new commits named by the
/// pull requests the forge cache has, `cached`; or the name of its upstream when that's
/// configured but gone, as when the remote deleted it.
fn target(
    ctx: &Context,
    meta: Option<&but_core::ref_metadata::ProjectMeta>,
    info: &but_workspace::RefInfo,
    cached: &HashMap<u64, but_forge::ForgeReview>,
) -> Result<(Option<Target>, Option<String>)> {
    // Linked worktrees' lanes count as behind too, but an update only rebases the stacks.
    let target_commit = info.target_commit.as_ref().map(|target| target.commit_id);
    let stacks_behind = info.has_lanes_behind_target
        && (info.stacks.iter())
            .any(|stack| stack.base.is_some_and(|base| Some(base) != target_commit));
    // What's new is what the target branch's upstream has that what's checked out doesn't:
    // what a pull brings in. Without an upstream, it's what the local target branch itself
    // gained, as from a land or a `git merge` into it.
    let repo = ctx.repo.get()?;
    let Some((meta, source)) = meta.and_then(|meta| Some((meta, target_source(&repo, meta)?)))
    else {
        return Ok((None, None));
    };
    let Some(tip) = ref_tip(&repo, source.as_ref()) else {
        // Its upstream went, as when the remote deleted or renamed it, unless the remote has
        // no branches at all, as a repository just made on a forge before its first push.
        let lost = remote_has_branches(&repo, source.as_ref());
        return Ok((None, lost.then(|| source.shorten().to_string())));
    };
    let ahead = target_commit.map(|base| commits_past(&repo, tip, base));
    let new = new_commits(&repo, meta, tip);
    let commits_behind = new.len();
    // The longest remote name that fits, as remote names can hold slashes too.
    let branch =
        but_core::extract_remote_name_and_short_name(source.as_ref(), &repo.remote_names())
            .map_or_else(
                || source.shorten().to_string(),
                |(_, branch)| branch.to_string(),
            );
    let local = source.category() == Some(gix::refs::Category::LocalBranch);
    let untracked = (repo.remote_default_name(gix::remote::Direction::Fetch))
        .filter(|_| local)
        .filter(|remote| {
            let mut name = bstr::BString::from("refs/remotes/");
            name.extend_from_slice(remote);
            name.push(b'/');
            name.extend_from_slice(source.shorten());
            repo.find_reference(name.as_bstr()).is_ok()
        })
        .map(|remote| format!("{remote}/{}", source.shorten()));
    let target = Target {
        name: source.shorten().to_string(),
        branch,
        local,
        untracked,
        behind: first_parent_run(&repo, tip, &new),
        commits_behind,
        // The stored commit can be past what's checked out, as when main is added behind its
        // upstream, which an update fast-forwards.
        needs_update: ahead.is_some_and(|ahead| ahead > 0) || commits_behind > 0 || stacks_behind,
        incoming: match commits_behind {
            0 => Vec::new(),
            _ => upstream::incoming(ctx, cached, model::UPSTREAM_SHOWN).unwrap_or_else(|err| {
                tracing::warn!("couldn't read the target's new commits: {err:#}");
                Vec::new()
            }),
        },
    };
    Ok((Some(target), None))
}

/// The branches of a linked worktree's `lane` the workspace hasn't applied, with their pull
/// requests as the forge cache, `cached`, has them.
fn worktree_branches(
    ctx: &Context,
    cached: &HashMap<u64, but_forge::ForgeReview>,
    lane: &but_workspace::ref_info::Lane<'_>,
    stacks: &[Lane],
) -> Vec<Branch> {
    (lane.segments.iter())
        .filter_map(|segment| {
            // A detached run of commits has no ref to tell it apart by, and a branch the
            // workspace has applied is listed there.
            let ref_name = segment.ref_name()?.to_owned();
            if (stacks.iter().flat_map(|stack| &stack.branches))
                .any(|stacked| stacked.ref_name.as_ref() == Some(&ref_name))
            {
                return None;
            }
            Some(branch_from(ctx, cached, segment))
        })
        .collect()
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
        Some(name) => (name.shorten().to_string(), Some(name.to_owned())),
        None => ("Unnamed branch".into(), None),
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
                let (title, body) = subject_and_body(&commit.message.to_str_lossy());
                RemoteCommit {
                    id: CommitId(commit.id),
                    key: commit.change_id().to_string(),
                    authored: model::authored(&commit.author, commit.message.trim_end()),
                    title,
                    body,
                    author,
                    email,
                    agent,
                    time: commit.author.time.seconds,
                    merge: commit.parent_ids.len() > 1,
                    review: None,
                }
            })
            .collect(),
        remote: (segment.remote_tracking_ref_name.as_ref()).map(|name| name.shorten().to_string()),
        push,
        review,
    }
}

/// Where the target's new commits come from: the local target branch's upstream, as a pull
/// brings in and a pull request compares against, or with none configured, the branch itself.
pub(crate) fn target_source(
    repo: &gix::Repository,
    meta: &but_core::ref_metadata::ProjectMeta,
) -> Option<gix::refs::FullName> {
    match meta.remote_target_ref(repo).ok()? {
        Some(upstream) => Some(upstream),
        None => Some(
            meta.target_ref
                .as_ref()?
                .as_local_full_name_ref()
                .to_owned(),
        ),
    }
}

/// Whether the remote that remote-tracking branch `name` is of has any branches here.
fn remote_has_branches(repo: &gix::Repository, name: &gix::refs::FullNameRef) -> bool {
    let Some((remote, _)) =
        but_core::extract_remote_name_and_short_name(name, &repo.remote_names())
    else {
        return false;
    };
    let Ok(refs) = repo.references() else {
        return false;
    };
    let prefix = format!("refs/remotes/{remote}/");
    (refs
        .prefixed(prefix.as_str())
        .into_iter()
        .flatten()
        .flatten())
    .any(|branch| !branch.name().as_bstr().ends_with(b"/HEAD"))
}

/// The commit `name` points at, if it's there.
pub(crate) fn ref_tip(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
) -> Option<gix::ObjectId> {
    Some(repo.find_reference(name).ok()?.peel_to_id().ok()?.detach())
}

/// Whether `ancestor` is `commit` or in its history.
fn is_ancestor(
    repo: &gix::Repository,
    ancestor: gix::ObjectId,
    commit: gix::ObjectId,
) -> Result<bool> {
    let base = repo.merge_base(ancestor, commit)?;
    Ok(base.is_some_and(|base| base == ancestor))
}

/// How many commits `tip` has that `base` doesn't, merged ones included.
fn commits_past(repo: &gix::Repository, tip: gix::ObjectId, base: gix::ObjectId) -> usize {
    (repo.rev_walk([tip]).with_hidden([base]).all()).map_or(0, |walk| walk.count())
}

/// What target `tip` has that what's checked out doesn't, as `but status` counts from where
/// they part: HEAD can be behind the stored target commit, as main added behind its upstream
/// is, or a branch applied from an older base leaves it, or have more, as a branch the target
/// was merged into does. Nothing when HEAD shares no history with the target, as an orphan
/// branch's, and with no HEAD yet, what's past the stored commit.
fn new_commits(
    repo: &gix::Repository,
    meta: &but_core::ref_metadata::ProjectMeta,
    tip: gix::ObjectId,
) -> HashSet<gix::ObjectId> {
    let hidden = match repo.head_id().ok().map(|id| id.detach()) {
        Some(head) if !matches!(repo.merge_base(head, tip), Ok(Some(_))) => return HashSet::new(),
        Some(head) => head,
        None => match meta.target_commit_id {
            Some(stored) => stored,
            None => return HashSet::new(),
        },
    };
    (repo.rev_walk([tip]).with_hidden([hidden]).all())
        .map(|walk| walk.filter_map(Result::ok).map(|info| info.id).collect())
        .unwrap_or_default()
}

/// How many of `new` are on `tip`'s own line of history, each merge once, as `but status`
/// counts them.
fn first_parent_run(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    new: &HashSet<gix::ObjectId>,
) -> usize {
    let mut run = 0;
    let mut at = Some(tip);
    while let Some(id) = at.filter(|id| new.contains(id)) {
        run += 1;
        at = (repo.find_commit(id).ok())
            .and_then(|commit| commit.parent_ids().next())
            .map(|id| id.detach());
    }
    run
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
    let owned = |who: &gix::actor::Signature| {
        let (name, email) = (who.name.to_str_lossy(), who.email.to_str_lossy());
        (name.into_owned(), email.into_owned())
    };
    byline(owned(author), owned(committer))
}

/// Who `commit` is shown by, as the repository's `mailmap` names them, as `git log` does: see
/// [`byline`].
fn commit_byline(
    mailmap: &gix::mailmap::Snapshot,
    commit: &gix::Commit<'_>,
) -> Result<((String, String), Option<Agent>)> {
    Ok(byline_of(
        &mailmap.resolve(commit.author()?),
        &mailmap.resolve(commit.committer()?),
    ))
}

/// `commit`'s first parent, if this clone has it: a shallow clone's oldest commits name
/// parents it doesn't have.
fn present_first_parent(commit: &gix::Commit<'_>) -> Option<gix::ObjectId> {
    let parent = commit.parent_ids().next()?.detach();
    commit.repo.has_object(parent).then_some(parent)
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

/// A commit message's subject and body as Git and GitHub split them: the subject is its first
/// paragraph, its lines joined, so a title that runs on to a second line without a blank one
/// is all of it; the body is the rest, as written, as a pull request description takes it.
/// Without the codes a terminal colours text with, as test output pasted into one brings.
pub fn subject_and_body(message: &str) -> (String, String) {
    let message = printable(message);
    let blank = |line: &&str| line.trim().is_empty();
    let mut lines = message.lines().skip_while(blank);
    let subject: Vec<&str> = (lines.by_ref().take_while(|line| !blank(line)))
        .map(str::trim)
        .collect();
    let body: Vec<&str> = lines.skip_while(blank).collect();
    (subject.join(" "), body.join("\n").trim_end().to_owned())
}

/// `text` without a terminal's escape sequences, `ESC [`, its parameters and its final letter,
/// nor other control characters but line breaks and tabs.
fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' if chars.as_str().starts_with('[') => {
                chars.next();
                // Parameters sort below `@`, the final letter from it.
                chars.by_ref().find(|c| ('@'..='~').contains(c));
            }
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// A commit message or comment's subject, as [`subject_and_body`] has it.
fn title(message: &[u8]) -> String {
    subject_and_body(&message.to_str_lossy()).0
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
        size: None,
        conflicted: false,
        lines: None,
        converted: false,
        change,
    }
}

#[cfg(test)]
impl FileChange {
    /// A modified file at `path`, with null blobs either side.
    pub fn bare(path: &str) -> Self {
        let state = ChangeState {
            id: gix::ObjectId::null(gix::hash::Kind::Sha1),
            kind: gix::object::tree::EntryKind::Blob,
        };
        file_change(TreeChange {
            path: path.into(),
            status: TreeStatus::Modification {
                previous_state: state,
                state,
                flags: None,
            },
        })
    }
}

/// Past this many uncommitted files, as an agent's generated output can make, their lines
/// aren't counted: reading them all would hold up every reload.
const COUNT_LINES_UP_TO: usize = 300;

/// Uncommitted files, each with when it was last written and, while there are few enough,
/// the lines it adds and removes, and the files the index has conflicted, as a merge leaves
/// them. A file `previous` has with the same change, written at the same time, keeps the lines
/// counted then: while an agent writes, most files stay as they were between reads.
/// What status says changed in `repo`'s checkout, but for the repository's linked worktrees
/// inside it, as agents' under `.claude/worktrees`, which look to status like repositories
/// added: they're checkouts of their own, which committing would record and discarding would
/// reset.
fn own_changes(repo: &gix::Repository) -> Result<but_core::WorktreeChanges> {
    let mut changes = but_core::diff::worktree_changes(repo)?;
    let workdir = repo
        .workdir()
        .and_then(|dir| std::fs::canonicalize(dir).ok());
    let inside = |base: std::path::PathBuf| -> Option<bstr::BString> {
        let base = std::fs::canonicalize(base).ok()?;
        let path = base.strip_prefix(workdir.as_ref()?).ok()?.to_owned();
        Some(
            gix::path::to_unix_separators_on_windows(gix::path::into_bstr(path).ok()?).into_owned(),
        )
    };
    let linked: Vec<bstr::BString> = (repo.worktrees().into_iter().flatten())
        .filter_map(|wt| inside(wt.base().ok()?))
        .collect();
    changes
        .changes
        .retain(|change| !linked.contains(&change.path));
    Ok(changes)
}

fn worktree_files(repo: &gix::Repository, previous: &[FileChange]) -> Result<Vec<FileChange>> {
    let changes = own_changes(repo)?;
    // Status leaves out files with unresolved index conflicts; they're changes too.
    let conflicted = conflicted_changes(repo, &changes.ignored_changes)?;
    let count = conflicted.len() + changes.changes.len() <= COUNT_LINES_UP_TO;
    let counted: HashMap<&bstr::BStr, &FileChange> = (previous.iter())
        .filter(|file| file.lines.is_some())
        .map(|file| (file.change.path.as_bstr(), file))
        .collect();
    let lines = |change: &but_core::TreeChange, modified: Option<i64>, size: Option<u64>| {
        let kept = counted.get(change.path.as_bstr()).filter(|old| {
            (old.modified, old.size) == (modified, size)
                && same_change(&old.change.status, &change.status)
        });
        if let Some(old) = kept {
            return old.lines;
        }
        match change.unified_patch(repo, 0) {
            Ok(Some(but_core::UnifiedPatch::Patch {
                lines_added,
                lines_removed,
                ..
            })) => Some((lines_added, lines_removed)),
            _ => None,
        }
    };
    let changes = changes.changes.into_iter().map(|change| (change, false));
    // Conflicted first, as a conflicted commit lists them: they're what needs doing.
    Ok((conflicted.into_iter().map(|change| (change, true)))
        .chain(changes)
        .map(|(change, conflicted)| {
            let (modified, size) = stamp(repo, change.path.as_bstr());
            FileChange {
                modified,
                size,
                conflicted,
                lines: count.then(|| lines(&change, modified, size)).flatten(),
                ..file_change(change)
            }
        })
        .collect())
}

/// Whether `a` and `b` are the same kind of change from the same base to the same state, as
/// far as status knows it: a worktree file's state isn't hashed, so its write time tells.
fn same_change(a: &but_core::TreeStatus, b: &but_core::TreeStatus) -> bool {
    let state = |s: but_core::ChangeState| (s.id, s.kind);
    let before = |status: &but_core::TreeStatus| {
        (status.previous_state_and_path()).map(|(s, path)| (state(s), path.map(ToOwned::to_owned)))
    };
    a.kind() == b.kind() && before(a) == before(b) && a.state().map(state) == b.state().map(state)
}

/// When the file at `path` in `repo`'s checkout was last written and its size, as
/// [`FileChange`] keeps them to tell whether it changed since; a deleted file has neither.
fn stamp(repo: &gix::Repository, path: &bstr::BStr) -> (Option<i64>, Option<u64>) {
    let metadata = metadata(repo, path);
    let size = metadata.as_ref().map(|metadata| metadata.len());
    (metadata.as_ref().and_then(millis), size)
}

/// The file at `path` in `repo`'s checkout, as `lstat` sees it; a deleted file has none.
fn metadata(repo: &gix::Repository, path: &bstr::BStr) -> Option<std::fs::Metadata> {
    let path = repo.workdir()?.join(gix::path::from_bstr(path).ok()?);
    path.symlink_metadata().ok()
}

/// When `metadata`'s file was last written, in milliseconds since the Unix epoch.
fn millis(metadata: &std::fs::Metadata) -> Option<i64> {
    let since_epoch = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
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

/// Whether the project folder at `path` is gone, as after it was moved or deleted. Off the UI
/// thread, as a disk that's asleep or a network mount that hangs would freeze it.
pub async fn folder_gone(path: std::path::PathBuf) -> bool {
    run(move || Ok(!path.exists())).await.unwrap_or(false)
}

/// What a project that failed to load needs said beyond its error.
#[derive(Default)]
pub struct LoadFailure {
    /// Its folder is gone, which is said instead of the error.
    pub folder_gone: bool,
    /// Its index is sparse, which the engine can't read yet, and which is said instead.
    pub sparse_index: bool,
    /// What to pick a target from, when the one an older GitButler stored names no single branch.
    pub target_choice: Option<TargetChoice>,
}

/// What's behind `err`, if loading the project at `path` failed. Off the UI thread, as finding
/// out reads the disk.
pub async fn load_failure(path: std::path::PathBuf, err: Option<&anyhow::Error>) -> LoadFailure {
    let Some(err) = err else {
        return LoadFailure::default();
    };
    let unresolved = unresolved_target(err).cloned();
    run(move || {
        Ok(LoadFailure {
            folder_gone: !path.exists(),
            sparse_index: (gix::open(&path).ok())
                .and_then(|repo| repo.index_or_empty().ok())
                .is_some_and(|index| index.is_sparse()),
            target_choice: unresolved.and_then(|target| target_choice(&path, &target)),
        })
    })
    .await
    .unwrap_or_default()
}

/// Why a project or its target couldn't be read: the engine's own words, except for a target
/// an older GitButler stored that names no single local branch, which the project's page asks
/// to pick, as [`TargetChoice`].
pub fn read_failure(err: &anyhow::Error) -> String {
    match unresolved_target(err) {
        Some(_) => "Choose which branch is its target".to_owned(),
        None => format!("{err:#}"),
    }
}

/// The target `err` failed on, if it's a remote branch an older GitButler stored that no
/// local branch tracks, or several do.
fn unresolved_target(err: &anyhow::Error) -> Option<&gix::refs::FullName> {
    use but_core::ref_metadata::UnresolvedLegacyTargetRefError;
    (err.downcast_ref::<UnresolvedLegacyTargetRefError>()).map(|err| &err.target_ref)
}

/// A target an older GitButler stored as a remote branch, like origin/main, that names no
/// single local branch: the local branches tracking it, when several do, or with none tracking
/// it, the one named as it, like main, that picking `remote` makes track it.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetChoice {
    pub remote: FullName,
    pub branches: Vec<FullName>,
    /// The branch named as `remote` when none tracks it, and whether picking it creates it.
    /// None when the remote branch is gone too.
    pub to_track: Option<(FullName, bool)>,
}

/// What to choose when the project at `path` has `target`, a remote branch no single local
/// branch tracks: one of those tracking it, or with none, the one named as it.
fn target_choice(path: &Path, target: &gix::refs::FullName) -> Option<TargetChoice> {
    let repo = gix::open(path).ok()?;
    let references = repo.references().ok()?;
    let tracks = |name: &gix::refs::FullNameRef| {
        (repo.branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch))
            .and_then(Result::ok)
            .is_some_and(|upstream| upstream.as_ref() == target.as_ref())
    };
    let branches: Vec<FullName> = (references.local_branches().ok()?)
        .filter_map(Result::ok)
        .filter(|branch| tracks(branch.name()))
        .map(|branch| branch.name().to_owned())
        .collect();
    let to_track = (branches.is_empty())
        .then(|| {
            repo.try_find_reference(target.as_ref()).ok()??;
            let (local, _) =
                (repo.upstream_branch_and_remote_for_tracking_branch(target.as_ref())).ok()??;
            let new = repo.try_find_reference(local.as_ref()).ok()?.is_none();
            Some((local, new))
        })
        .flatten();
    Some(TargetChoice {
        remote: target.clone(),
        branches,
        to_track,
    })
}

/// Whether `err` is Git having no author to record a commit with.
pub fn is_author_missing(err: &anyhow::Error) -> bool {
    err.downcast_ref::<but_error::Code>() == Some(&but_error::Code::AuthorMissing)
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
        // quietly go unsigned. Why first, as the card shows a few lines.
        Some(but_error::Code::CommitSigningFailed) => format!(
            "{}\nThat turns commit signing off for this project, unless gitbutler.signCommits \
             is set in your global Git config. Check Sign commits in Project settings.",
            signing_failure(err)
        ),
        // As on a new machine, where the engine's own words name its code.
        Some(but_error::Code::AuthorMissing) => "Git doesn't know your name and email yet. \
            Set user.name and user.email with git config --global, then try again."
            .to_owned(),
        // A precondition says what stopped it in the user's words; the context wrapped
        // around it names refs and hashes, which a short card fills up with.
        _ => match err.downcast_ref::<but_error::Context>() {
            Some(said) if said.code == but_error::Code::PreconditionFailed => said.to_string(),
            // Without the code's own name, as `DefaultTerminalNotFound`, which is for code.
            _ => {
                let code = err
                    .downcast_ref::<but_error::Code>()
                    .map(ToString::to_string);
                (err.chain().map(ToString::to_string))
                    .filter(|said| Some(said) != code.as_ref())
                    .collect::<Vec<_>>()
                    .join(": ")
            }
        },
    }
}

/// Why signing failed, in the signing program's own last words, as gpg's "signing failed: No
/// secret key", rather than how it was run, which a card cuts off before them; or which
/// program couldn't be run.
fn signing_failure(err: &anyhow::Error) -> String {
    let values = err
        .downcast_ref::<gix::Error>()
        .map(gix::Error::metadata_merged);
    let value = |key: &str| match values.as_ref()?.get(key)? {
        gix::error::MetadataValue::Bytes(bytes) => {
            Some(String::from_utf8_lossy(bytes).into_owned())
        }
        gix::error::MetadataValue::Path(path) => Some(path.display().to_string()),
        other => Some(other.to_string()),
    };
    // gpg reports its status on the same stream, in lines of its own.
    let said = value("stderr").and_then(|said| {
        (said.lines().map(str::trim))
            .rfind(|l| !l.is_empty() && !l.starts_with("[GNUPG:]"))
            .map(str::to_owned)
    });
    // With no exit code, it never ran.
    match (
        said,
        value("program").filter(|_| value("exit_code").is_none()),
    ) {
        (Some(said), _) => said,
        (None, Some(program)) => format!("Couldn't run {program}: {}", err.root_cause()),
        (None, None) => err.root_cause().to_string(),
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
mod message_tests {
    use super::subject_and_body;

    #[test]
    fn the_subject_is_the_first_paragraph_as_git_has_it() {
        assert_eq!(
            subject_and_body("Fix scanner crash\non empty directories\n\nWhy.\n"),
            (
                "Fix scanner crash on empty directories".into(),
                "Why.".into()
            ),
            "with no blank line, the next line is the subject's too, as `git log --oneline` shows"
        );
        assert_eq!(
            subject_and_body("\n\nLeading blank\n"),
            ("Leading blank".into(), String::new()),
            "blank lines before it aren't the subject"
        );
        assert_eq!(
            subject_and_body("Add a flag\n\n    cargo run -- --flag\n- a\n  - nested\n\n").1,
            "    cargo run -- --flag\n- a\n  - nested",
            "the body keeps its indentation, as Markdown's code blocks and nested lists need it"
        );
        assert_eq!(
            subject_and_body("\u{1b}[31;1mFAIL\u{1b}[0m fix flaky test\r\n\r\nSee \u{7}run.\n").0,
            "FAIL fix flaky test",
            "a terminal's colour codes and control characters don't show"
        );
    }
}

#[cfg(test)]
mod behind_tests {
    use but_core::ref_metadata::{LocalRefName, ProjectMeta};

    use crate::backend::ops::tests::{git, head, repo_with};

    #[test]
    fn whats_new_is_what_the_checkout_lacks_past_the_stored_commit() {
        let dir = repo_with("one\n");
        let path = dir.path();
        let commit = |message: &str| {
            git(path, &["commit", "-q", "--allow-empty", "-m", message]);
            head(path).0
        };
        let stored = head(path).0;
        git(path, &["checkout", "-q", "-b", "feature"]);
        commit("feature work");
        git(path, &["checkout", "-q", "main"]);
        commit("two");
        commit("three");
        git(path, &["checkout", "-q", "feature"]);
        git(path, &["merge", "-q", "--no-edit", "main"]);
        git(path, &["checkout", "-q", "main"]);
        let tip = commit("four");
        let mut meta = ProjectMeta {
            target_ref: Some(LocalRefName::new("refs/heads/main".try_into().unwrap()).unwrap()),
            target_commit_id: Some(stored),
            ..ProjectMeta::default()
        };
        let new = |meta: &ProjectMeta| {
            let repo = gix::open(path).unwrap();
            let new = super::new_commits(&repo, meta, tip);
            (super::first_parent_run(&repo, tip, &new), new.len())
        };

        git(path, &["checkout", "-q", "feature"]);
        assert_eq!(
            new(&meta),
            (1, 1),
            "a branch that merged main in has two and three already, as `but status` says"
        );
        git(path, &["checkout", "-q", "--orphan", "pages"]);
        commit("pages");
        assert_eq!(
            new(&meta),
            (0, 0),
            "nothing is new to a checkout that shares no history with the target"
        );
        git(path, &["checkout", "-q", "-b", "old", &stored.to_string()]);
        let past = ProjectMeta {
            target_commit_id: Some(tip),
            ..meta.clone()
        };
        assert_eq!(
            new(&past),
            (3, 3),
            "a checkout behind the stored commit, as a branch applied from an older base \
             leaves it, lacks what came after"
        );
        git(path, &["checkout", "-q", "-f", "main"]);
        git(path, &["reset", "-q", "--hard", "HEAD~2"]);
        meta.target_commit_id = Some(tip);
        assert_eq!(
            new(&meta),
            (2, 2),
            "main checked out behind the stored commit lacks what it hasn't pulled yet"
        );
    }
}

#[cfg(test)]
mod failure_tests {
    use super::{failure_detail, read_failure};

    #[test]
    fn a_legacy_target_naming_no_single_branch_asks_to_pick_one() {
        use but_core::ref_metadata::{
            UnresolvedLegacyTargetReason, UnresolvedLegacyTargetRefError,
        };
        for reason in [
            UnresolvedLegacyTargetReason::MultipleTrackingBranches,
            UnresolvedLegacyTargetReason::NoTrackingBranch,
        ] {
            let err = anyhow::anyhow!("origin/main is no local branch").context(
                UnresolvedLegacyTargetRefError {
                    target_ref: "refs/remotes/origin/main".try_into().unwrap(),
                    reason,
                    target_commit_id: None,
                    push_remote: None,
                },
            );
            assert_eq!(
                read_failure(&err),
                "Choose which branch is its target",
                "the project's page asks for the branch, {reason:?}"
            );
        }
    }

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
        use gix::error::ErrorExt as _;
        let refused = gix::error::message("Signing program failed")
            .with_program("/usr/local/bin/gpg")
            .with(
                "stderr",
                b"gpg: skipped \"ABC\": No secret key\n[GNUPG:] INV_SGNR 9 ABC\n\
                  gpg: signing failed: No secret key\n[GNUPG:] FAILURE sign 17\n"
                    .to_vec(),
            )
            .raise();
        let err = anyhow::Error::from(refused)
            .context("Failed to sign commit")
            .context(but_error::Code::CommitSigningFailed);
        assert!(
            failure_detail(&err).starts_with("gpg: signing failed: No secret key\n"),
            "gpg's own last line says why, first, not how it was run, nor its status lines"
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
        let err = anyhow::anyhow!("No author is configured in Git")
            .context(but_error::Code::AuthorMissing);
        assert!(
            failure_detail(&err).starts_with("Git doesn't know your name and email yet."),
            "what to set, without the engine's code name"
        );
    }
}

#[cfg(test)]
mod lane_tests {
    use std::path::Path;

    use crate::backend::model::build::{describe, describe_chain};
    use crate::backend::ops::tests::{git, repo_with, target_origin_main};
    use crate::backend::ops::{
        Destination, create_branch, create_branch_at_commit, create_dependent_branch,
        insert_empty_commit, rename_branch,
    };
    use crate::backend::{Checkout, CommitId};

    /// A stack `bottom < middle < top` with a commit each beside an empty `beside`, which keeps
    /// `top` out of the main checkout, and linked worktrees `wt` on `middle` with a commit of
    /// its own, `wt-top` with `top` checked out, and `wt-stacked` on `wt`.
    pub(super) fn linked_workspace(test: impl FnOnce(&Path)) {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let mut below = create_branch(path, Some("bottom")).unwrap();
            insert_empty_commit(path, &Destination::Branch(below.clone())).unwrap();
            for name in ["middle", "top"] {
                let new = create_dependent_branch(path, &below).unwrap();
                below = rename_branch(path, &new, name).unwrap();
                insert_empty_commit(path, &Destination::Branch(below.clone())).unwrap();
            }
            create_branch(path, Some("beside")).unwrap();

            let ctx = super::open(path).unwrap();
            (ctx.db.get_cache_mut().unwrap())
                .worktree_meta_mut()
                .mark_adopted()
                .unwrap();
            drop(ctx);
            let outside = tempfile::tempdir().unwrap();
            let at = |name: &str| outside.path().join(name).to_str().unwrap().to_owned();
            git(
                path,
                &["worktree", "add", "-q", "-b", "wt", &at("wt"), "middle"],
            );
            git(
                Path::new(&at("wt")),
                &["commit", "-q", "--allow-empty", "-m", "wt work"],
            );
            git(path, &["worktree", "add", "-q", &at("wt-top"), "top"]);
            git(
                path,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    "wt-stacked",
                    &at("wt-stacked"),
                    "wt",
                ],
            );
            test(path);
        })
    }

    #[test]
    fn a_worktree_lane_loads_its_own_branches() {
        linked_workspace(|path| {
            let loaded = crate::backend::load_workspace(path, Default::default()).unwrap();
            let worktree = |name: &str| (loaded.worktrees.iter()).find(|wt| wt.name == name);
            git(
                &worktree("wt").unwrap().path,
                &["commit", "-q", "--allow-empty", "-m", "more"],
            );
            let wt_work = git(path, &["rev-parse", "wt~"]);
            let wt_work = CommitId(gix::ObjectId::from_hex(wt_work.trim().as_bytes()).unwrap());
            create_branch_at_commit(path, wt_work, false).unwrap();
            let wt_top = worktree("wt-top");
            let out = std::process::Command::new("git")
                .current_dir(&wt_top.unwrap().path)
                .args(["reset", "-q", "--soft", "HEAD"])
                .env("GIT_COMMITTER_DATE", "2090-01-01 00:00:00 +0000")
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            let tree = git(path, &["rev-parse", "HEAD^{tree}"]);
            let mut upstream = git(path, &["rev-parse", "origin/main"]);
            for n in 0..2 {
                let message = format!("upstream {n}");
                let parent = upstream.trim().to_owned();
                upstream = git(
                    path,
                    &["commit-tree", tree.trim(), "-p", &parent, "-m", &message],
                );
            }
            git(
                path,
                &["update-ref", "refs/remotes/origin/main", upstream.trim()],
            );
            let outside = tempfile::tempdir().unwrap();
            for (name, from) in [("agent1", "origin/main"), ("agent2", "origin/main~1")] {
                let at = outside.path().join(name);
                let at = at.to_str().unwrap();
                git(path, &["worktree", "add", "-q", "-b", name, at, from]);
                git(
                    Path::new(at),
                    &["commit", "-q", "--allow-empty", "-m", name],
                );
            }
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            let linked: Vec<_> = (workspace.lanes.iter())
                .filter_map(|lane| match &lane.checkout {
                    Checkout::Main => None,
                    Checkout::Linked(name) => Some(name.to_string()),
                })
                .collect();
            let listed: Vec<_> = (workspace.worktrees.iter())
                .map(|wt| wt.name.to_string())
                .collect();
            assert_eq!(listed[0], "wt-top", "its reflog moved last");
            assert_eq!(
                linked, listed,
                "the lanes are in the order the worktrees list"
            );
            // `top`, checked out in wt-top, names an empty segment in that lane, which leaves
            // its commit in the stack unnamed, and wt-top resting on it.
            // agent2, started from the target's commit agent1 runs down through, rests on agent1.
            snapbox::assert_data_eq!(
                describe(&workspace)
                    + &describe_chain(
                        &workspace,
                        &crate::backend::model::build::full("refs/heads/agent2")
                    ),
                snapbox::str![[r#"
Main: [Unnamed branch, middle, bottom]
Main: [beside]
agent1: [agent1 (checked out)]
agent2: [agent2 (checked out)] rests on agent1~2
wt: [wt (checked out), t-branch-1] rests on middle
wt-stacked: [wt-stacked (checked out)] rests on t-branch-1
wt-top: [top (checked out)] rests on Unnamed branch
agent2: [agent2 (checked out)]
agent1: [agent1 (checked out)] from agent1~2
history: [agent2, agent1~2]

"#]]
            );
        })
    }

    #[test]
    fn a_worktrees_commits_stay_once_the_target_has_them() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            create_branch(path, Some("beside")).unwrap();
            let ctx = super::open(path).unwrap();
            (ctx.db.get_cache_mut().unwrap())
                .worktree_meta_mut()
                .mark_adopted()
                .unwrap();
            drop(ctx);
            let outside = tempfile::tempdir().unwrap();
            let at = outside.path().join("agent");
            git(
                path,
                &["worktree", "add", "-q", "-b", "agent", at.to_str().unwrap()],
            );
            for message in ["one", "two"] {
                git(&at, &["commit", "-q", "--allow-empty", "-m", message]);
            }
            let titles = || {
                let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
                let lane = workspace.worktree_lane("agent".into()).unwrap();
                (lane.branches.iter())
                    .flat_map(|branch| &branch.commits)
                    .map(|commit| commit.title.clone())
                    .collect::<Vec<_>>()
            };
            let tree = git(path, &["rev-parse", "agent^{tree}"]);
            let merge = git(
                path,
                &[
                    "commit-tree",
                    tree.trim(),
                    "-p",
                    "main",
                    "-p",
                    "agent",
                    "-m",
                    "Merge",
                ],
            );
            git(
                path,
                &["update-ref", "refs/remotes/origin/main", merge.trim()],
            );
            assert_eq!(titles(), ["two", "one"], "merged in, as a pull request is");
            git(path, &["update-ref", "refs/remotes/origin/main", "agent"]);
            assert_eq!(titles(), ["two", "one"], "fast-forwarded onto");
        })
    }
}
