//! Branches published to a hosted GitButler server (experimental): publishing them, finding them
//! and pulling them down.
//!
//! The unit is a branch with, optionally, its uncommitted changes. Locally a branch lives in a
//! worktree or is applied in the workspace; the server shows each published branch as a
//! worktree lane either way. Branches are keyed by name: publishing or pulling a branch that
//! already exists elsewhere replaces it, after asking whenever work would be lost.
//!
//! The server is fixed rather than a git remote: `BUT_HOSTED_URL`, defaulting to a local demo
//! server. Requests to it are made as the signed-in GitButler user, and see only that user's
//! branches. A fetch keeps what it publishes under `refs/gitbutler/hosted/`, apart from the
//! repository's own remotes.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// Where fetched branches and snapshots of published branches are kept.
const HOSTED_REFS: &str = "refs/gitbutler/hosted";

/// The hosted server's URL.
fn hosted_server() -> String {
    let url = std::env::var("BUT_HOSTED_URL").unwrap_or_else(|_| "http://localhost:6980".into());
    url.trim_end_matches('/').to_owned()
}

/// The project as the hosted server knows it: its earliest root commit, the same everywhere.
fn hosted_project(dir: &Path) -> Result<String> {
    let roots = git(dir, &[], &["rev-list", "--max-parents=0", "HEAD"])?;
    Ok(roots
        .lines()
        .last()
        .context("HEAD has no root commit")?
        .to_owned())
}

/// This machine as the hosted server knows it: its host name, as a path segment, or
/// `BUT_MACHINE`, so that one computer can stand in for several.
fn machine_name() -> Option<String> {
    let name = match std::env::var("BUT_MACHINE") {
        Ok(name) => name,
        Err(_) => {
            let out = std::process::Command::new("hostname").output().ok()?;
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
    };
    let name = published_name(name.trim().trim_end_matches(".local"));
    (!name.is_empty()).then_some(name)
}

/// The name a branch is published under: its short name with `/` and other characters a URL
/// path segment can't hold replaced.
fn published_name(branch: &str) -> String {
    branch.replace(
        |c: char| !c.is_ascii_alphanumeric() && !"._-".contains(c),
        "-",
    )
}

/// The snapshot last published or pulled here, so changes from it aren't counted as local work.
fn synced_ref(branch: &str) -> String {
    format!("{HOSTED_REFS}/synced/{}", published_name(branch))
}

/// Git for the repository at `dir`, and only that one: variables inherited from, say, a hook
/// running `but` would point it elsewhere.
fn git_command(dir: &Path) -> std::process::Command {
    let mut git = std::process::Command::new(gix::path::env::exe_invocation());
    git.current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    git
}

/// Run git in `dir` with `env`.
fn git(dir: &Path, env: &[(&str, &std::ffi::OsStr)], args: &[&str]) -> Result<String> {
    let out = git_command(dir)
        .envs(env.iter().copied())
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Run git in `dir` against the hosted server, as the signed-in GitButler user. The token goes in
/// through git's environment, which other processes can't read as they can its arguments.
fn git_as_user(dir: &Path, args: &[&str]) -> Result<String> {
    let user = gitbutler_user::get_user()?.context(
        "Sign in to GitButler first: the hosted server shows each account its own branches",
    )?;
    let token = user
        .access_token()
        .context("Sign in to GitButler again: this account's access token is missing")?;
    let header = std::ffi::OsString::from(format!("X-Auth-Token: {}", *token));
    let env = [
        ("GIT_CONFIG_COUNT", "1".as_ref()),
        ("GIT_CONFIG_KEY_0", "http.extraHeader".as_ref()),
        ("GIT_CONFIG_VALUE_0", header.as_os_str()),
        // A refused token fails rather than asking for a password.
        ("GIT_TERMINAL_PROMPT", "0".as_ref()),
    ];
    git(dir, &env, args)
}

/// Whether `ancestor` is reachable from `descendant`, including being the same commit.
fn is_ancestor(dir: &Path, ancestor: &str, descendant: &str) -> Result<bool> {
    let status = git_command(dir)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .status()?;
    Ok(status.success())
}

/// The object `spec` names, if it exists.
fn rev(dir: &Path, spec: &str) -> Option<String> {
    git(dir, &[], &["rev-parse", "--verify", "--quiet", spec]).ok()
}

/// Whether `snapshot` holds uncommitted changes, i.e. its tree differs from its branch's.
fn has_uncommitted(dir: &Path, snapshot: &str) -> bool {
    rev(dir, &format!("{snapshot}^{{tree}}")) != rev(dir, &format!("{snapshot}^^{{tree}}"))
}

/// What to return instead of going ahead when work would be replaced, or `None` to overwrite.
fn unless_overwrite(
    on_conflict: Option<OnConflict>,
    reason: String,
    kept: String,
) -> Option<SyncOutcome> {
    match on_conflict {
        None => Some(SyncOutcome::NeedsChoice(reason)),
        Some(OnConflict::Keep) => Some(SyncOutcome::Done(kept)),
        Some(OnConflict::Overwrite) => None,
    }
}

/// The tree of `worktree`'s files and index, written through a scratch index so the real one
/// is untouched.
fn worktree_tree(repo: &gix::Repository, worktree: &Path) -> Result<String> {
    // One per call, as the CLI and the app may take snapshots at once.
    let index = repo
        .git_dir()
        .join(format!("but-hosted-index-{}", uuid::Uuid::new_v4()));
    let env = [("GIT_INDEX_FILE", index.as_os_str())];
    let tree = git(worktree, &env, &["read-tree", "HEAD"])
        .and_then(|_| git(worktree, &env, &["add", "-A"]))
        .and_then(|_| git(worktree, &env, &["write-tree"]));
    std::fs::remove_file(&index).ok();
    tree
}

/// The main worktree's directory, where git commands for the project run.
fn workdir(ctx: &but_ctx::Context) -> Result<PathBuf> {
    let repo = ctx.repo.get()?;
    Ok(repo
        .workdir()
        .context("the project needs a worktree")?
        .to_owned())
}

/// Fetch the project's published branches and snapshots from the hosted server.
fn fetch(dir: &Path) -> Result<()> {
    let url = format!("{}/git/{}/fetch", hosted_server(), hosted_project(dir)?);
    let heads = format!("+refs/heads/*:{HOSTED_REFS}/heads/*");
    let snapshots = format!("+refs/gitbutler/snapshots/*:{HOSTED_REFS}/snapshots/*");
    let machines = format!("+refs/gitbutler/machines/*:{HOSTED_REFS}/machines/*");
    git_as_user(
        dir,
        &[
            "fetch",
            "--prune",
            "--no-tags",
            "--quiet",
            &url,
            &heads,
            &snapshots,
            &machines,
        ],
    )?;
    Ok(())
}

/// What a snapshot commit says about its branch: its full ref name.
fn snapshot_head(dir: &Path, snapshot: &str) -> Result<String> {
    let message: serde_json::Value =
        serde_json::from_str(&git(dir, &[], &["log", "-1", "--format=%B", snapshot])?)?;
    message["head"]
        .as_str()
        .filter(|head| head.starts_with("refs/heads/"))
        .map(ToOwned::to_owned)
        .context("the snapshot names no branch")
}

/// Where a branch lives locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum LocalHome {
    /// There's no local branch of that name.
    None,
    /// Checked out in a worktree, at this path.
    Worktree(String),
    /// Applied in the GitButler workspace.
    Workspace,
    /// A local branch that is neither checked out nor applied.
    Branch,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(LocalHome);

/// The local home of `branch` (a short name).
fn local_home(ctx: &but_ctx::Context, dir: &Path, branch: &str) -> Result<LocalHome> {
    let full = format!("refs/heads/{branch}");
    let worktrees = git(dir, &[], &["worktree", "list", "--porcelain"])?;
    let mut path = None;
    for line in worktrees.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(p.to_owned());
        } else if line.strip_prefix("branch ") == Some(full.as_str()) {
            return Ok(LocalHome::Worktree(path.unwrap_or_default()));
        }
    }
    if applied_stack(ctx, &full)?.is_some() {
        return Ok(LocalHome::Workspace);
    }
    let exists = ctx.repo.get()?.try_find_reference(full.as_str())?.is_some();
    Ok(if exists {
        LocalHome::Branch
    } else {
        LocalHome::None
    })
}

/// The workspace stack `full` (a full ref name) is applied in, if any: its id, and how many
/// branches it holds.
fn applied_stack(
    ctx: &but_ctx::Context,
    full: &str,
) -> Result<Option<(Option<but_core::ref_metadata::StackId>, usize)>> {
    let on_workspace = ctx
        .repo
        .get()?
        .head_name()?
        .is_some_and(|head| head.as_bstr() == but_core::WORKSPACE_REF_NAME);
    if !on_workspace {
        return Ok(None);
    }
    let info = crate::legacy::workspace::head_info(ctx)?;
    Ok(info.stacks.iter().find_map(|stack| {
        let branches: Vec<_> = stack
            .segments
            .iter()
            .filter_map(|segment| segment.ref_info.as_ref())
            .collect();
        branches
            .iter()
            .any(|info| info.ref_name.as_bstr() == full)
            .then_some((stack.id, branches.len()))
    }))
}

/// Refuse when the server's `snapshot` was published by a branch other than `full`, as
/// `a/b` and `a-b` share a published name.
fn ensure_published_by(dir: &Path, snapshot: &str, full: &str) -> Result<()> {
    if let Some(head) = rev(dir, snapshot)
        .map(|_| snapshot_head(dir, snapshot))
        .transpose()?
        && head != full
    {
        bail!(
            "{} and {} share a published name; rename one of them",
            full.trim_start_matches("refs/heads/"),
            head.trim_start_matches("refs/heads/")
        );
    }
    Ok(())
}

/// What to do when publishing or pulling would replace work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum OnConflict {
    /// Replace it.
    Overwrite,
    /// Leave it and do nothing.
    Keep,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(OnConflict);

/// The result of publishing or pulling.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum SyncOutcome {
    /// It's done, or nothing needed doing; what happened, in a sentence.
    Done(String),
    /// Nothing happened, because it would replace work; why, in a sentence. Ask, then call again
    /// with an [`OnConflict`].
    NeedsChoice(String),
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(SyncOutcome);

/// A branch published to the hosted server, as of the last fetch.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HostedBranch {
    /// The branch's short name, e.g. `agent/search`.
    pub branch: String,
    /// Whether it was published with uncommitted changes.
    pub uncommitted: bool,
    /// Where a local branch of that name lives.
    pub local: LocalHome,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HostedBranch);

/// Fetch from the hosted server and list the branches published for this project.
#[but_api(napi, provides = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_branches(ctx: &but_ctx::Context) -> Result<Vec<HostedBranch>> {
    let dir = workdir(ctx)?;
    fetch(&dir)?;
    let snapshots = format!("{HOSTED_REFS}/snapshots");
    let refs = git(
        &dir,
        &[],
        &["for-each-ref", "--format=%(refname)", &snapshots],
    )?;
    refs.lines()
        .map(|snapshot| {
            let branch = snapshot_head(&dir, snapshot)?
                .trim_start_matches("refs/heads/")
                .to_owned();
            Ok(HostedBranch {
                uncommitted: has_uncommitted(&dir, snapshot),
                local: local_home(ctx, &dir, &branch)?,
                branch,
            })
        })
        .collect()
}

/// Another machine that published to the hosted server, as of the last fetch.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HostedMachine {
    /// Its host name.
    pub name: String,
    /// When it last published, in milliseconds since the Unix epoch.
    pub published_at: i64,
    /// What it last sent of each branch, most recent first.
    pub branches: Vec<MachineBranch>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HostedMachine);

/// What a machine last sent of a branch.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MachineBranch {
    /// The branch's short name, e.g. `agent/search`.
    pub branch: String,
    /// Whether it was sent with uncommitted changes.
    pub uncommitted: bool,
    /// Whether it's still the published branch: a later publish from elsewhere replaces it,
    /// and pulling brings down the published one.
    pub current: bool,
    /// Where a local branch of that name lives.
    pub local: LocalHome,
    /// Its commits that the target doesn't have, newest first.
    pub commits: Vec<but_workspace::ui::Commit>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(MachineBranch);

/// Fetch from the hosted server and list the other machines that published to this project,
/// most recent first, with what each last sent.
#[but_api(napi, provides = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_machines(ctx: &but_ctx::Context) -> Result<Vec<HostedMachine>> {
    let dir = workdir(ctx)?;
    fetch(&dir)?;
    let this = machine_name();
    let repo = ctx.repo.get()?;
    // Commits are listed up to the target, as a branch's own are in the workspace.
    let target = but_core::ref_metadata::ProjectMeta::resolve(&repo)?
        .target_ref
        .and_then(|name| rev(&dir, name.as_bstr().to_string().as_str()))
        .map(|id| gix::ObjectId::from_hex(id.as_bytes()))
        .transpose()?;
    let prefix = format!("{HOSTED_REFS}/machines/");
    let refs = git(
        &dir,
        &[],
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname) %(committerdate:unix)",
            &prefix,
        ],
    )?;
    let mut machines = BTreeMap::<String, HostedMachine>::new();
    for line in refs.lines() {
        let (sent, time) = line.split_once(' ').context("a ref and its date")?;
        // <user>/<machine>/<published name>
        let mut parts = sent.trim_start_matches(&prefix).splitn(3, '/');
        let (Some(_user), Some(machine), Some(name)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if this.as_deref() == Some(machine) {
            continue;
        }
        let branch = snapshot_head(&dir, sent)?
            .trim_start_matches("refs/heads/")
            .to_owned();
        let entry = machines
            .entry(machine.to_owned())
            .or_insert_with(|| HostedMachine {
                name: machine.to_owned(),
                published_at: time.parse::<i64>().unwrap_or_default() * 1000,
                branches: Vec::new(),
            });
        // The snapshot sits on top of the branch, so the branch's commits start at its parent.
        let tip = gix::ObjectId::from_hex(
            rev(&dir, &format!("{sent}^"))
                .context("a snapshot has its branch as parent")?
                .as_bytes(),
        )?;
        let commits = match target {
            Some(target) => {
                use gix::prelude::ObjectIdExt as _;
                but_workspace::local_commits_for_branch(tip.attach(&repo), target)?
            }
            None => Vec::new(),
        };
        entry.branches.push(MachineBranch {
            commits,
            uncommitted: has_uncommitted(&dir, sent),
            current: rev(&dir, sent) == rev(&dir, &format!("{HOSTED_REFS}/snapshots/{name}")),
            local: local_home(ctx, &dir, &branch)?,
            branch,
        });
    }
    let mut machines: Vec<_> = machines.into_values().collect();
    machines.sort_by_key(|machine| std::cmp::Reverse(machine.published_at));
    Ok(machines)
}

/// Publish `branch` (a short name) to the hosted server, from wherever it lives locally.
///
/// It's one atomic push of the branch, the target branch it's based on, and a snapshot commit
/// on top of the branch whose message describes it. With `include_uncommitted`, the snapshot
/// holds the uncommitted changes of the worktree the branch is checked out in; in the
/// workspace, uncommitted changes don't belong to one branch, so they stay local.
///
/// If the server's branch has commits this one doesn't, publishing would replace them: without
/// `on_conflict` that's a [`SyncOutcome::NeedsChoice`].
#[but_api(napi, invalidates = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_branch_publish(
    ctx: &but_ctx::Context,
    branch: String,
    include_uncommitted: bool,
    on_conflict: Option<OnConflict>,
) -> Result<SyncOutcome> {
    let dir = workdir(ctx)?;
    let full = format!("refs/heads/{branch}");
    if full == but_core::WORKSPACE_REF_NAME {
        bail!("The GitButler workspace itself can't be published, only branches");
    }
    let home = local_home(ctx, &dir, &branch)?;
    let snapshot_dir = match (&home, include_uncommitted) {
        (LocalHome::None, _) => bail!("There's no local branch {branch}"),
        (LocalHome::Worktree(path), true) => Some(PathBuf::from(path)),
        (_, true) => bail!(
            "Uncommitted changes can only be published from a worktree, and {branch} isn't checked out in one"
        ),
        (_, false) => None,
    };

    let main = ctx.repo.get()?.clone();
    fetch(&dir)?;
    let name = published_name(&branch);
    let published_snapshot = format!("{HOSTED_REFS}/snapshots/{name}");
    ensure_published_by(&dir, &published_snapshot, &full)?;
    // Work on the server that publishing would replace: commits this branch lacks, or
    // uncommitted changes published from elsewhere.
    let missing_commits = match rev(&dir, &format!("{HOSTED_REFS}/heads/{branch}")) {
        Some(server_tip) => !is_ancestor(&dir, &server_tip, &full)?,
        None => false,
    };
    let others_uncommitted = has_uncommitted(&dir, &published_snapshot)
        && rev(&dir, &published_snapshot) != rev(&dir, &synced_ref(&branch));
    let what = match (missing_commits, others_uncommitted) {
        (true, true) => "commits this one doesn't and uncommitted changes from elsewhere",
        (true, false) => "commits this one doesn't",
        (false, true) => "uncommitted changes from elsewhere",
        (false, false) => "",
    };
    if !what.is_empty()
        && let Some(outcome) = unless_overwrite(
            on_conflict,
            format!("The published {branch} has {what}. Publishing replaces them."),
            format!("Kept the published {branch}"),
        )
    {
        return Ok(outcome);
    }

    let title = main
        .workdir()
        .and_then(Path::file_name)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let target = but_core::ref_metadata::ProjectMeta::resolve(&main)?
        .target_ref
        .map(|name| name.to_string());
    let message = serde_json::json!({
        "version": 1,
        "title": title,
        "head": full,
        "target": target,
        "machine": machine_name(),
    });

    // The snapshot's tree: the worktree's files, or the branch's own.
    let tree = match snapshot_dir {
        Some(worktree) => worktree_tree(&main, &worktree)?,
        None => rev(&dir, &format!("{full}^{{tree}}")).context("the branch has no tree")?,
    };
    let commit = [
        "commit-tree",
        &tree,
        "-p",
        &full,
        "-m",
        &message.to_string(),
    ];
    let snapshot = git(&dir, &[], &commit)?;

    let mut refspecs = vec![
        format!("+{full}:{full}"),
        format!("+{snapshot}:refs/gitbutler/snapshots/{name}"),
    ];
    refspecs.extend(target.map(|target| format!("+{target}:{target}")));
    let url = format!("{}/git/{}/{name}", hosted_server(), hosted_project(&dir)?);
    // No hooks: they'd run with the token in git's environment.
    let mut push = vec!["push", "--atomic", "--quiet", "--no-verify", &url];
    push.extend(refspecs.iter().map(String::as_str));
    git_as_user(&dir, &push)?;
    git(&dir, &[], &["update-ref", &synced_ref(&branch), &snapshot])?;
    Ok(SyncOutcome::Done(format!("Published {branch}")))
}

/// Pull the published `branch` (a short name) down.
///
/// Where the branch already lives locally, it's updated there. Otherwise it goes into a new
/// worktree next to the main one, with the published uncommitted changes restored, or, with
/// `into_workspace`, it's applied in the workspace without them.
///
/// If the local branch has commits of its own, or its worktree has uncommitted changes,
/// pulling would replace them: without `on_conflict` that's a [`SyncOutcome::NeedsChoice`].
#[but_api(napi, invalidates = [Hosted, Worktrees, Workspace, Branches])]
#[instrument(err(Debug))]
pub fn hosted_branch_pull(
    ctx: &mut but_ctx::Context,
    branch: String,
    into_workspace: bool,
    on_conflict: Option<OnConflict>,
) -> Result<SyncOutcome> {
    let dir = workdir(ctx)?;
    fetch(&dir)?;
    let snapshot = format!("{HOSTED_REFS}/snapshots/{}", published_name(&branch));
    let tip =
        rev(&dir, &format!("{snapshot}^")).with_context(|| format!("{branch} isn't published"))?;
    let full = format!("refs/heads/{branch}");
    ensure_published_by(&dir, &snapshot, &full)?;
    let home = local_home(ctx, &dir, &branch)?;
    if home == LocalHome::Workspace
        && applied_stack(ctx, &full)?.is_some_and(|(_, branches)| branches > 1)
    {
        bail!(
            "{branch} is stacked with other branches in the workspace, which pulling would leave out; tear it off first"
        );
    }

    // Work that pulling would replace.
    let own_commits = match rev(&dir, &full) {
        Some(local) => !is_ancestor(&dir, &local, &tip)?,
        None => false,
    };
    // Uncommitted changes are only local work if they're neither committed nor published. In
    // the workspace they can't be told apart by branch, and unapplying commits them, so any
    // count.
    let dirty = match &home {
        LocalHome::Workspace => !git(&dir, &[], &["status", "--porcelain"])?.is_empty(),
        LocalHome::Worktree(path) => {
            let files = Some(worktree_tree(&*ctx.repo.get()?, Path::new(path))?);
            let known = ["HEAD".to_owned(), snapshot.clone(), synced_ref(&branch)];
            !known
                .iter()
                .any(|known| rev(Path::new(path), &format!("{known}^{{tree}}")) == files)
        }
        _ => false,
    };
    let what = match (own_commits, dirty) {
        (true, true) => "commits and uncommitted changes",
        (true, false) => "commits",
        (false, true) => "uncommitted changes",
        (false, false) => "",
    };
    if !what.is_empty()
        && let Some(outcome) = unless_overwrite(
            on_conflict,
            format!("The local {branch} has {what} that aren't published. Pulling replaces them."),
            format!("Kept the local {branch}"),
        )
    {
        return Ok(outcome);
    }
    let stays_on_server = if has_uncommitted(&dir, &snapshot) {
        "; its uncommitted changes stay on the server, as they only travel between worktrees"
    } else {
        ""
    };

    let in_workspace = home == LocalHome::Workspace;
    let done = match home {
        LocalHome::Worktree(path) => {
            let path = Path::new(&path);
            git(path, &[], &["reset", "--quiet", "--hard", &tip])?;
            git(path, &[], &["clean", "--quiet", "-fd"])?;
            restore_snapshot(path, &snapshot)?;
            format!("Updated {branch} in {}", path.display())
        }
        _ if in_workspace || into_workspace => {
            if in_workspace {
                let stack = applied_stack(ctx, &full)?
                    .and_then(|(id, _)| id)
                    .context("the branch's stack has no id")?;
                crate::legacy::virtual_branches::unapply_stack(ctx, stack)?;
            }
            git(&dir, &[], &["branch", "--force", &branch, &tip])?;
            crate::branch::apply(ctx, full.as_str().try_into()?)?;
            let done = if in_workspace { "Updated" } else { "Applied" };
            format!("{done} {branch} in the workspace{stays_on_server}")
        }
        _ => {
            let name = published_name(&branch);
            let path = dir
                .parent()
                .context("the project has a parent directory")?
                .join(&name);
            if path.exists() {
                bail!("{} already exists", path.display());
            }
            let path_str = path.to_str().context("the path is UTF-8")?;
            git(
                &dir,
                &[],
                &["worktree", "add", "--quiet", "-B", &branch, path_str, &tip],
            )?;
            restore_snapshot(&path, &snapshot)?;
            ctx.set_worktree_archived(name.as_str().into(), false)?;
            format!("Pulled {branch} into {path_str}")
        }
    };
    git(&dir, &[], &["update-ref", &synced_ref(&branch), &snapshot])?;
    Ok(SyncOutcome::Done(done))
}

/// Write the snapshot's tree over the checkout at `path` without committing it, so its
/// uncommitted changes are uncommitted again.
fn restore_snapshot(path: &Path, snapshot: &str) -> Result<()> {
    git(
        path,
        &[],
        &["restore", "--source", snapshot, "--worktree", "--", "."],
    )?;
    Ok(())
}
