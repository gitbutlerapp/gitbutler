//! The project's branches: local ones, applied or not, and ones only a remote has, like a
//! teammate's.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use bstr::{BStr, BString, ByteSlice};
use but_branches::ListedStackStatus;

use super::reviews::from_cache;
use super::upstream::{TargetCommit, target_commit};
use super::{Agent, CommitId, Review, open};

#[derive(Debug, Clone)]
pub struct OtherBranch {
    /// The local branch if there is one, else the remote-tracking one.
    pub ref_name: super::FullName,
    pub name: String,
    pub local: bool,
    /// The workspace has it applied: the Branches page lists only the others, and its
    /// overview counts both.
    pub applied: bool,
    /// Who its newest commit is shown by, as [`super::byline`] picks.
    pub author: Option<String>,
    /// Everyone its own commits are shown by, newest first, as `author` is.
    pub people: Vec<String>,
    /// Its own commits, newest first, as its overview sums them up.
    pub commits: Vec<OwnCommit>,
    /// Theirs, for their picture.
    pub email: Option<String>,
    /// The agent that wrote its newest commit, when `author` is the person who committed it.
    pub agent: Option<Agent>,
    /// When it last changed, in seconds since the epoch.
    pub time: Option<i64>,
    /// Commits it has that the target doesn't, when known.
    pub ahead: Option<usize>,
    /// Commits of its own, down to the branch it's stacked on or where it forked, when known.
    pub own_commits: Option<usize>,
    /// Which of the listed stacks it's in, so a stack's branches stay together.
    pub stack: usize,
    pub tip: CommitId,
    /// Its pull request, without CI checks: the app asks the forge about those only for the
    /// workspace's branches, so the cache's would be stale.
    pub review: Option<Review>,
    /// A local branch's description, as `git branch --edit-description` writes it.
    pub description: Option<String>,
}

/// One of a branch's own commits.
#[derive(Debug, Clone)]
pub struct OwnCommit {
    /// The day it was written, as [`super::upstream::day_of`] numbers days: a rebase leaves
    /// that be.
    pub day: i64,
    /// Who it's shown by, by where they are in [`OtherBranch::people`].
    pub person: usize,
}

/// Every branch but the target, newest first, a stack's branches together from the top, those
/// the workspace has applied too.
pub fn all_branches(path: &Path) -> Result<Vec<OtherBranch>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let descriptions = descriptions(&repo);
    let mailmap = repo.open_mailmap();
    Ok(but_api::branch::branch_list(&ctx)?
        .into_iter()
        .filter(|stack| stack.status != ListedStackStatus::Target)
        .enumerate()
        .flat_map(|(stack, listed)| {
            let applied = listed.status == ListedStackStatus::Applied;
            (listed.branches.into_iter()).map(move |b| (stack, applied, b))
        })
        .map(|(stack, applied, listed)| {
            let branch = listed.branch;
            let byline = (repo.find_commit(branch.tip).ok())
                .and_then(|tip| super::commit_byline(&mailmap, &tip).ok());
            let (person, agent) = byline.unzip();
            let (author, email) = person.unzip();
            let (people, commits) = own_commits(
                &repo,
                &mailmap,
                branch.tip,
                branch.commit_count.unwrap_or(1),
            );
            OtherBranch {
                ref_name: branch.ref_name.clone(),
                name: branch.display_name.to_str_lossy().into_owned(),
                local: branch.has_local,
                applied,
                people,
                commits,
                author,
                email,
                agent: agent.flatten(),
                time: branch.updated_at_ms.map(|ms| ms / 1000),
                ahead: branch.commits_ahead_of_target,
                own_commits: branch.commit_count,
                stack,
                tip: CommitId(branch.tip),
                review: listed.review.as_ref().map(from_cache),
                description: (branch.ref_name.category())
                    .filter(|c| *c == gix::refs::Category::LocalBranch)
                    .and_then(|_| descriptions.get(branch.ref_name.shorten()))
                    .map(|d| d.to_str_lossy().trim().to_owned())
                    .filter(|d| !d.is_empty()),
            }
        })
        .collect())
}

/// The `count` commits from `tip` down its first parents, newest first, and who they're
/// shown by, each once.
fn own_commits(
    repo: &gix::Repository,
    mailmap: &gix::mailmap::Snapshot,
    tip: gix::ObjectId,
    count: usize,
) -> (Vec<String>, Vec<OwnCommit>) {
    let (mut people, mut commits) = (Vec::new(), Vec::new());
    let mut next = Some(tip);
    for _ in 0..count {
        let Some(commit) = next.and_then(|id| repo.find_commit(id).ok()) else {
            break;
        };
        next = commit.parent_ids().next().map(|id| id.detach());
        let Ok(((name, _), _)) = super::commit_byline(mailmap, &commit) else {
            continue;
        };
        let person = match people.iter().position(|known| *known == name) {
            Some(at) => at,
            None => {
                people.push(name);
                people.len() - 1
            }
        };
        // Its day is worked out here, off the UI thread, as the overview sums them often.
        if let Ok(time) = commit.author().and_then(|author| author.time()) {
            let day = super::upstream::day_of(time.seconds);
            commits.push(OwnCommit { day, person });
        }
    }
    (people, commits)
}

/// Every local branch's description, by its short name, as `git branch --edit-description`
/// keeps them in the repository's own config: read afresh, so one just set is seen.
fn descriptions(repo: &gix::Repository) -> HashMap<BString, BString> {
    let Ok(config) = but_core::git_config::open_repo_local_config_for_reading(repo) else {
        return HashMap::new();
    };
    (config.sections_by_name("branch").into_iter().flatten())
        .filter_map(|section| {
            let name = section.header().subsection_name()?.to_owned();
            Some((name, section.value("description")?))
        })
        .collect()
}

/// Set local branch `ref_name`'s description where Git keeps it, as `git branch
/// --edit-description` does; a blank one removes it. Git config isn't in the oplog, so it
/// isn't a step to undo.
pub fn set_branch_description(
    path: &Path,
    ref_name: &super::FullName,
    description: &str,
) -> Result<()> {
    let mut ctx = open(path)?;
    let name = ref_name;
    anyhow::ensure!(
        name.category() == Some(gix::refs::Category::LocalBranch),
        "Only a local branch has a description"
    );
    let _guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;
    // Not under a name that's gone, as after an agent renamed it while it was being written,
    // where a branch made later by that name would take it up. Copying the error keeps it.
    if repo.try_find_reference(name.as_ref())?.is_none() {
        anyhow::bail!(
            "{} was renamed or deleted meanwhile, so this wasn't saved:\n\n{description}",
            name.shorten()
        );
    }
    let description = Some(description.trim()).filter(|d| !d.is_empty());
    write_description(&repo, name.shorten(), description.map(BStr::new))
}

/// Move branch `from`'s description to `to`, as `git branch -m` does, or drop it with no `to`,
/// as `git branch -d` does.
pub(super) fn move_description(
    repo: &gix::Repository,
    from: &BStr,
    to: Option<&BStr>,
) -> Result<()> {
    let description = (but_core::git_config::open_repo_local_config_for_reading(repo)?)
        .section("branch", Some(from))
        .ok()
        .and_then(|section| section.value("description"));
    if description.is_none() {
        return Ok(());
    }
    write_description(repo, from, None)?;
    match to {
        Some(to) => write_description(repo, to, description.as_ref().map(|d| d.as_bstr())),
        None => Ok(()),
    }
}

fn write_description(
    repo: &gix::Repository,
    name: &BStr,
    description: Option<&BStr>,
) -> Result<()> {
    but_core::git_config::edit_repo_config(repo, gix::config::Source::Local, |config| {
        if let Ok(mut section) = config.section_mut("branch", Some(name)) {
            section.remove("description");
        }
        if let Some(description) = description {
            config
                .section_mut_or_create_new("branch", Some(name))?
                .set("description", description)?;
        }
        Ok(())
    })?;
    Ok(())
}

/// The commits `ref_name` has that the target doesn't, newest first, as the project's page
/// has a target's, so a picked one shows its message as a target commit does.
pub fn branch_commits(path: &Path, ref_name: &super::FullName) -> Result<Vec<TargetCommit>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let name = ref_name;
    let remote = but_core::extract_remote_name_and_short_name(
        name.as_ref(),
        &ctx.repo.get()?.remote_names(),
    );
    let (short, remote) = match remote {
        Some((remote, short)) => (short.to_string(), Some(remote)),
        None => (name.shorten().to_string(), None),
    };
    let details = but_api::legacy::workspace::branch_details(&ctx, short, remote)?;
    let mailmap = repo.open_mailmap();
    (details.commits.iter())
        .map(|commit| target_commit(&mailmap, &repo.find_commit(commit.id)?))
        .collect()
}

/// How applying a branch went, and the local branch it's applied as.
pub enum ApplyOutcome {
    Applied(super::FullName),
    AlreadyApplied(super::FullName),
    /// It conflicts with these applied stacks, so nothing changed.
    Conflicts(Vec<String>),
}

/// Apply `ref_name` to the workspace, once the archived worktree `freeing`, if any, let go of
/// it. A remote branch gets a local branch to apply.
pub fn apply(
    path: &Path,
    ref_name: &super::FullName,
    freeing: Option<&BStr>,
) -> Result<ApplyOutcome> {
    use but_workspace::branch::apply::OutcomeStatus;
    let mut ctx = open(path)?;
    let name = ref_name;
    // One lock throughout, so nothing else moves the worktree's HEAD or the branch meanwhile.
    let mut guard = ctx.exclusive_worktree_access();
    // The archived worktree `freeing` lets go of the branch first, staying on its commit with
    // its files as they are, or the workspace's commits would move the branch under it.
    let freed = match freeing {
        Some(worktree) => {
            let repo = super::checkout(&ctx, Some(worktree))?;
            (repo.head_name()?.as_ref() == Some(name)).then_some(repo)
        }
        None => None,
    };
    if let Some(repo) = &freed {
        let at = gix::refs::Target::Object(repo.head_id()?.detach());
        let why = format!("leaving {} to the workspace", name.shorten());
        but_core::update_head_reference(repo, at, false, "checkout", why.as_str().into(), 0)?;
    }
    let outcome =
        but_api::branch::apply_with_perm(&mut ctx, name.as_ref(), guard.write_permission());
    // Not applied after all, it goes back to the worktree, so its next commits move it again.
    let kept = matches!(&outcome, Ok(o) if !matches!(o.status, OutcomeStatus::ConflictAborted));
    if let Some(repo) = freed.filter(|_| !kept) {
        let back = gix::refs::Target::Symbolic(name.clone());
        let why = format!("taking {} back, as it wasn't applied", name.shorten());
        but_core::update_head_reference(&repo, back, false, "checkout", why.as_str().into(), 0)?;
    }
    let outcome = outcome?;
    let remotes = ctx.repo.get()?.remote_names();
    let local = match but_core::extract_remote_name_and_short_name(name.as_ref(), &remotes) {
        Some((_, short)) => {
            let mut local = BString::from("refs/heads/");
            local.extend_from_slice(&short);
            super::FullName::try_from(local)?
        }
        None => name.clone(),
    };
    Ok(match outcome.status {
        OutcomeStatus::Applied => ApplyOutcome::Applied(local),
        OutcomeStatus::AlreadyApplied => ApplyOutcome::AlreadyApplied(local),
        OutcomeStatus::ConflictAborted => ApplyOutcome::Conflicts(
            outcome
                .conflicting_stacks
                .iter()
                .map(|stack| stack.ref_name.shorten().to_string())
                .collect(),
        ),
    })
}
