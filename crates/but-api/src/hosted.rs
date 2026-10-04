//! Branches published to a hosted GitButler server (experimental): publishing them, finding them
//! and pulling them down.
//!
//! The unit is a branch with, optionally, its uncommitted changes. Locally a branch lives in a
//! worktree or is applied in the workspace. Each machine publishes into its own namespace,
//! named by its host name, so machines never replace each other's branches: pulling names the
//! machine to pull from, and asks before replacing local work.
//!
//! The server is fixed rather than a git remote: `BUT_HOSTED_URL`, defaulting to
//! https://mesh.but.dev. Requests to it are made as the signed-in GitButler user, and see only that user's
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

mod listen;
pub use listen::{HostedEvent, HostedListener, listen};

/// Where fetched branches and snapshots of published branches are kept.
const HOSTED_REFS: &str = "refs/gitbutler/hosted";

/// The hosted server's URL.
fn hosted_server() -> String {
    let url = std::env::var("BUT_HOSTED_URL").unwrap_or_else(|_| "https://mesh.but.dev".into());
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
    let header = std::ffi::OsString::from(format!("X-Auth-Token: {}", access_token()?));
    let env = [
        ("GIT_CONFIG_COUNT", "1".as_ref()),
        ("GIT_CONFIG_KEY_0", "http.extraHeader".as_ref()),
        ("GIT_CONFIG_VALUE_0", header.as_os_str()),
        // A refused token fails rather than asking for a password.
        ("GIT_TERMINAL_PROMPT", "0".as_ref()),
    ];
    git(dir, &env, args)
}

/// The signed-in GitButler account's access token, which the hosted server knows it by.
fn access_token() -> Result<String> {
    let user = gitbutler_user::get_user()?.context(
        "Sign in to GitButler first: the hosted server shows each account its own branches",
    )?;
    let token = user
        .access_token()
        .context("Sign in to GitButler again: this account's access token is missing")?;
    Ok(token.0)
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

/// Fetch the project's published branches and snapshots from the hosted server, and what was
/// sent to this machine.
fn fetch(dir: &Path) -> Result<()> {
    let url = format!("{}/git/{}", hosted_server(), hosted_project(dir)?);
    let heads = format!("+refs/heads/*:{HOSTED_REFS}/heads/*");
    let snapshots = format!("+refs/gitbutler/snapshots/*:{HOSTED_REFS}/snapshots/*");
    let inbox =
        machine_name().map(|this| format!("+refs/gitbutler/inbox/{this}/*:{HOSTED_REFS}/inbox/*"));
    let mut fetch = vec![
        "fetch",
        "--prune",
        "--no-tags",
        "--quiet",
        &url,
        &heads,
        &snapshots,
    ];
    fetch.extend(inbox.as_deref());
    git_as_user(dir, &fetch)?;
    Ok(())
}

/// Where the hosted server keeps what `from` sent `to` of a branch, by its published name.
fn inbox_ref(to: &str, from: &str, name: &str) -> String {
    format!("refs/gitbutler/inbox/{to}/{from}/{name}")
}

/// Take what `from` sent of `branch` out of this machine's inbox, here and on the server.
fn clear_inbox(dir: &Path, from: &str, branch: &str) -> Result<()> {
    let name = published_name(branch);
    let mirror = format!("{HOSTED_REFS}/inbox/{from}/{name}");
    if rev(dir, &mirror).is_none() {
        return Ok(());
    }
    let this = machine_name().context("This machine has no name; set BUT_MACHINE")?;
    let url = format!("{}/git/{}", hosted_server(), hosted_project(dir)?);
    let delete = format!(":{}", inbox_ref(&this, from, &name));
    git_as_user(dir, &["push", "--quiet", "--no-verify", &url, &delete])?;
    git(dir, &[], &["update-ref", "-d", &mirror])?;
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

/// One of this machine's branches as it last published it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublishedBranch {
    /// The branch's short name, e.g. `agent/search`.
    pub branch: String,
    /// How the local branch compares with it.
    pub state: PublishState,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PublishedBranch);

/// How a local branch compares with what this machine last published of it, by commits only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum PublishState {
    /// The published tip is the local tip.
    Published,
    /// The local branch has this many commits the published one doesn't.
    Ahead(u32),
    /// The published branch has this many commits the local one doesn't, as after undoing.
    Behind(u32),
    /// Each has commits the other doesn't, as after an amend or a rebase.
    Diverged,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PublishState);

/// How `local` compares with `published`, both commit ids in `dir`.
fn publish_state(dir: &Path, published: &str, local: &str) -> Result<PublishState> {
    let count = |range: String| -> Result<u32> {
        Ok(git(dir, &[], &["rev-list", "--count", &range])?.parse()?)
    };
    Ok(if published == local {
        PublishState::Published
    } else if is_ancestor(dir, published, local)? {
        PublishState::Ahead(count(format!("{published}..{local}"))?)
    } else if is_ancestor(dir, local, published)? {
        PublishState::Behind(count(format!("{local}..{published}"))?)
    } else {
        PublishState::Diverged
    })
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

/// The machines that published a project, and which project that is, as the hosted server
/// names it in what it announces.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HostedProject {
    /// The project's root commit.
    pub root: String,
    /// Most recent first.
    pub machines: Vec<HostedMachine>,
    /// This machine's own published branches that are still local, and how each compares.
    pub published_here: Vec<PublishedBranch>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HostedProject);

/// What the account's machines published to one project, as the hosted server lists it for the
/// whole account at once, with the local projects that are checkouts of it.
#[derive(Clone, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HostedAccountProject {
    /// The project's root commit, which the server knows it by.
    pub root: String,
    /// The project's name, as its first publish gave it.
    pub title: String,
    /// The local projects checked out from it; none if this machine has no checkout.
    #[serde(default)]
    pub project_ids: Vec<String>,
    /// The other machines that published to it, most recent first.
    pub machines: Vec<HostedMachineSummary>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HostedAccountProject);

/// A machine's publishes to one project, without their branches.
#[derive(Clone, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HostedMachineSummary {
    /// Its host name.
    pub name: String,
    /// When it last published, in milliseconds since the Unix epoch.
    pub published_at: i64,
    /// How many branches it published.
    pub branches: u32,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HostedMachineSummary);

/// Everything the account's other machines published, across all projects, in one request to
/// the hosted server; no project is fetched. Local projects are matched to what was published by
/// root commit, and one whose directory is gone is left out rather than failing the call.
#[but_api(napi, provides = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_account() -> Result<Vec<HostedAccountProject>> {
    let token = access_token()?;
    let url = format!("{}/machines", hosted_server());
    let mut projects: Vec<HostedAccountProject> = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async move {
                let response = reqwest::Client::new()
                    .get(&url)
                    .header("x-auth-token", token)
                    .send()
                    .await
                    .context("the hosted server can't be reached")?
                    .error_for_status()?;
                Ok::<_, anyhow::Error>(response.json().await?)
            })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("the request to the hosted server panicked"))??;

    let this = machine_name();
    let mut by_root = std::collections::HashMap::<String, Vec<String>>::new();
    // As the frontend lists them, which is how it knows them: by `id`, at `path`.
    for project in serde_json::to_value(crate::legacy::projects::list_projects_stateless()?)?
        .as_array()
        .into_iter()
        .flatten()
    {
        let (Some(id), Some(path)) = (project["id"].as_str(), project["path"].as_str()) else {
            continue;
        };
        if let Ok(root) = hosted_project(Path::new(path)) {
            by_root.entry(root).or_default().push(id.to_owned());
        }
    }
    for project in &mut projects {
        project
            .machines
            .retain(|machine| Some(&machine.name) != this.as_ref());
        project.project_ids = by_root.remove(&project.root).unwrap_or_default();
    }
    projects.retain(|project| !project.machines.is_empty());
    Ok(projects)
}

/// A machine that published to the hosted server, as of the last fetch.
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
    /// The full name of the ref its commits are read from here: the fetched copy locally, the
    /// published branch itself on the hosted server.
    pub ref_name: String,
    /// The snapshot holding its uncommitted changes, if it was sent with any: a commit on top
    /// of the branch, whose diff is those changes.
    pub uncommitted: Option<but_workspace::ui::Commit>,
    /// Where a local branch of that name lives.
    pub local: LocalHome,
    /// Its commits that the target doesn't have, newest first.
    pub commits: Vec<but_workspace::ui::Commit>,
    /// Whether that machine sent it to this one, and it wasn't pulled or dismissed since.
    pub sent: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(MachineBranch);

/// The machines that published to this project, most recent first, with what each last sent.
///
/// Locally that's after a fetch from the hosted server, and leaves this machine out, whose
/// branches are here already. On the hosted server, which has no files, it's the server's own
/// record of every machine.
#[but_api(napi, provides = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_machines(ctx: &but_ctx::Context) -> Result<HostedProject> {
    use gix::prelude::ObjectIdExt as _;

    let repo = ctx.repo.get()?;
    let (dir, prefix, this) = match repo.workdir() {
        Some(workdir) => {
            fetch(workdir)?;
            let prefix = format!("{HOSTED_REFS}/snapshots/");
            (workdir.to_owned(), prefix, machine_name())
        }
        None => (
            repo.git_dir().to_owned(),
            "refs/gitbutler/snapshots/".to_owned(),
            None,
        ),
    };
    let on_hub = repo.workdir().is_none();
    // The hub keeps each project as `<root>.git`.
    let root = match repo.workdir() {
        Some(workdir) => hosted_project(workdir)?,
        None => dir
            .file_stem()
            .context("a hosted project's directory")?
            .to_string_lossy()
            .into_owned(),
    };
    // Commits are listed up to the target, as a branch's own are in the workspace.
    let target = but_core::ref_metadata::ProjectMeta::resolve(&repo)?
        .target_ref
        .and_then(|name| rev(&dir, name.as_bstr().to_string().as_str()))
        .map(|id| gix::ObjectId::from_hex(id.as_bytes()))
        .transpose()?;
    let oid = |spec: &str| -> Result<gix::ObjectId> {
        Ok(gix::ObjectId::from_hex(
            rev(&dir, spec)
                .with_context(|| format!("no {spec}"))?
                .as_bytes(),
        )?)
    };
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
    let mut published_here = Vec::new();
    for line in refs.lines() {
        let (sent, time) = line.split_once(' ').context("a ref and its date")?;
        // <machine>/<published name>
        let Some((machine, name)) = sent.trim_start_matches(&prefix).split_once('/') else {
            continue;
        };
        if this.as_deref() == Some(machine) {
            let branch = snapshot_head(&dir, sent)?
                .trim_start_matches("refs/heads/")
                .to_owned();
            if let Some(local) = rev(&dir, &format!("refs/heads/{branch}")) {
                let published = rev(&dir, &format!("{sent}^")).context("a snapshot's branch")?;
                let state = publish_state(&dir, &published, &local)?;
                published_here.push(PublishedBranch { branch, state });
            }
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
        let tip = oid(&format!("{sent}^"))?;
        let commits = match target {
            Some(target) => but_workspace::local_commits_for_branch(tip.attach(&repo), target)?,
            None => Vec::new(),
        };
        let uncommitted = if has_uncommitted(&dir, sent) {
            but_workspace::local_commits_for_branch(oid(sent)?.attach(&repo), tip)?
                .into_iter()
                .next()
        } else {
            None
        };
        let heads = if on_hub {
            "refs/heads".to_owned()
        } else {
            format!("{HOSTED_REFS}/heads")
        };
        entry.branches.push(MachineBranch {
            ref_name: format!("{heads}/{machine}/{branch}"),
            commits,
            uncommitted,
            sent: !on_hub && rev(&dir, &format!("{HOSTED_REFS}/inbox/{machine}/{name}")).is_some(),
            local: if on_hub {
                LocalHome::None
            } else {
                local_home(ctx, &dir, &branch)?
            },
            branch,
        });
    }
    let mut machines: Vec<_> = machines.into_values().collect();
    machines.sort_by_key(|machine| std::cmp::Reverse(machine.published_at));
    Ok(HostedProject {
        root,
        machines,
        published_here,
    })
}

/// Publish `branch` (a short name) to the hosted server, from wherever it lives locally.
///
/// It's one atomic push, into this machine's own namespace, of the branch, the target branch
/// it's based on, and a snapshot commit on top of the branch whose message describes it. With
/// `include_uncommitted`, the snapshot holds the uncommitted changes of the worktree the branch
/// is checked out in; in the workspace, uncommitted changes don't belong to one branch, so
/// they stay local. Nothing another machine published is replaced, so it never asks.
#[but_api(napi, invalidates = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_branch_publish(
    ctx: &but_ctx::Context,
    branch: String,
    include_uncommitted: bool,
) -> Result<String> {
    publish(ctx, &branch, include_uncommitted, None)
}

/// Publish `branch` as [`hosted_branch_publish()`] does, and send it to the machine named `to`:
/// it shows up there as sent, to pull or dismiss, and if `to` is online, it's told right away.
#[but_api(napi, invalidates = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_branch_send(
    ctx: &but_ctx::Context,
    branch: String,
    to: String,
    include_uncommitted: bool,
) -> Result<String> {
    if published_name(&to) != to {
        bail!("{to} isn't a machine name");
    }
    publish(ctx, &branch, include_uncommitted, Some(&to))
}

/// Take what `machine` sent of `branch` out of this machine's inbox, without pulling it.
#[but_api(napi, invalidates = [Hosted])]
#[instrument(err(Debug))]
pub fn hosted_branch_dismiss(
    ctx: &but_ctx::Context,
    machine: String,
    branch: String,
) -> Result<()> {
    clear_inbox(&workdir(ctx)?, &machine, &branch)
}

fn publish(
    ctx: &but_ctx::Context,
    branch: &str,
    include_uncommitted: bool,
    to: Option<&str>,
) -> Result<String> {
    let branch = branch.to_owned();
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

    let machine = machine_name()
        .context("This machine has no name to publish under; set BUT_MACHINE to give it one")?;
    if to == Some(machine.as_str()) {
        bail!("{branch} is already on {machine}, this machine");
    }
    let main = ctx.repo.get()?.clone();
    fetch(&dir)?;
    let name = published_name(&branch);
    ensure_published_by(
        &dir,
        &format!("{HOSTED_REFS}/snapshots/{machine}/{name}"),
        &full,
    )?;

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
        "machine": machine,
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
        format!("+{full}:refs/heads/{machine}/{branch}"),
        format!("+{snapshot}:refs/gitbutler/snapshots/{machine}/{name}"),
    ];
    refspecs.extend(target.map(|target| format!("+{target}:{target}")));
    refspecs.extend(to.map(|to| format!("+{snapshot}:{}", inbox_ref(to, &machine, &name))));
    let url = format!("{}/git/{}", hosted_server(), hosted_project(&dir)?);
    // No hooks: they'd run with the token in git's environment.
    let mut push = vec!["push", "--atomic", "--quiet", "--no-verify", &url];
    push.extend(refspecs.iter().map(String::as_str));
    git_as_user(&dir, &push)?;
    git(&dir, &[], &["update-ref", &synced_ref(&branch), &snapshot])?;
    Ok(match to {
        Some(to) => format!("Sent {branch} to {to}"),
        None => format!("Published {branch} as {machine}"),
    })
}

/// Pull `branch` (a short name) down as `machine` last published it.
///
/// Where the branch already lives locally, it's updated there. Otherwise it goes into a new
/// worktree next to the main one, with the published uncommitted changes restored, or, with
/// `into_workspace`, it's applied in the workspace without them.
///
/// If the local branch has commits of its own, or its worktree has uncommitted changes,
/// pulling would replace them: without `on_conflict` that's a [`SyncOutcome::NeedsChoice`].
///
/// A branch this machine had published is published again as pulled, so it doesn't disagree
/// with itself; one it never published stays unpublished. Pulling also clears anything
/// `machine` sent of it to this machine.
#[but_api(napi, invalidates = [Hosted, Worktrees, Workspace, Branches])]
#[instrument(err(Debug))]
pub fn hosted_branch_pull(
    ctx: &mut but_ctx::Context,
    machine: String,
    branch: String,
    into_workspace: bool,
    on_conflict: Option<OnConflict>,
) -> Result<SyncOutcome> {
    let dir = workdir(ctx)?;
    fetch(&dir)?;
    let snapshot = format!(
        "{HOSTED_REFS}/snapshots/{machine}/{}",
        published_name(&branch)
    );
    let tip = rev(&dir, &format!("{snapshot}^"))
        .with_context(|| format!("{machine} hasn't published {branch}"))?;
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
    // Pulled is handled; the branch itself is here either way.
    if let Err(err) = clear_inbox(&dir, &machine, &branch) {
        tracing::warn!("{branch} was pulled, but stays in the inbox: {err:#}");
    }
    // What this machine published of the branch would now disagree with it, so it's published
    // again as pulled. After the pull, so a pull that stops never leaves it claiming more.
    let own = machine_name()
        .map(|this| format!("{HOSTED_REFS}/snapshots/{this}/{}", published_name(&branch)));
    let done = match own.filter(|own| rev(&dir, own).is_some()) {
        None => done,
        Some(_) => {
            let with_uncommitted =
                matches!(local_home(ctx, &dir, &branch)?, LocalHome::Worktree(_))
                    && has_uncommitted(&dir, &snapshot);
            match publish(ctx, &branch, with_uncommitted, None) {
                Ok(_) => format!("{done}, and published it again from here"),
                Err(err) => format!("{done}, but couldn't publish it again from here: {err:#}"),
            }
        }
    };
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

#[cfg(test)]
mod tests {
    use super::{PublishState, git, publish_state};
    use but_testsupport::{CommandExt, git_at_dir};

    #[test]
    fn publish_state_compares_commits_both_ways() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let dir = tmp.path();
        git_at_dir(dir).args(["init", "-q", "-b", "main"]).run();
        let commit = |message: &str| -> anyhow::Result<String> {
            git_at_dir(dir)
                .args(["commit", "-q", "--allow-empty", "-m", message])
                .run();
            git(dir, &[], &["rev-parse", "HEAD"])
        };
        let published = commit("published")?;
        let ahead = commit("one more")?;
        git_at_dir(dir).args(["reset", "-q", "--hard", &published]).run();
        let diverged = commit("instead")?;

        assert_eq!(
            publish_state(dir, &published, &published)?,
            PublishState::Published,
            "the same tip is published"
        );
        assert_eq!(
            publish_state(dir, &published, &ahead)?,
            PublishState::Ahead(1),
            "a commit on top is one not yet published"
        );
        assert_eq!(
            publish_state(dir, &ahead, &published)?,
            PublishState::Behind(1),
            "a published commit the local branch lost is behind"
        );
        assert_eq!(
            publish_state(dir, &ahead, &diverged)?,
            PublishState::Diverged,
            "each having a commit the other lacks is diverged"
        );
        Ok(())
    }
}
