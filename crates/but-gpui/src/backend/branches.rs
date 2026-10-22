//! The branches outside the workspace: local ones it doesn't have applied, and ones only
//! a remote has, like a teammate's.

use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;
use but_branches::ListedStackStatus;

use super::ops::full_name;
use super::reviews::from_cache;
use super::upstream::{TargetCommit, target_commit};
use super::{Agent, CommitId, Review, open};

#[derive(Debug, Clone)]
pub struct OtherBranch {
    /// The local branch if there is one, else the remote-tracking one.
    pub ref_name: String,
    pub name: String,
    pub local: bool,
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
    /// When it was written, in seconds since the epoch: a rebase leaves that be.
    pub time: i64,
    /// Who it's shown by, by where they are in [`OtherBranch::people`].
    pub person: usize,
}

/// Every branch the workspace doesn't have applied, newest first, a stack's branches together
/// from the top; not the target.
pub fn other_branches(path: &Path) -> Result<Vec<OtherBranch>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    Ok(but_api::branch::branch_list(&ctx)?
        .into_iter()
        .filter(|stack| {
            matches!(
                stack.status,
                ListedStackStatus::Unapplied | ListedStackStatus::Standalone
            )
        })
        .enumerate()
        .flat_map(|(stack, listed)| listed.branches.into_iter().map(move |b| (stack, b)))
        .map(|(stack, listed)| {
            let branch = listed.branch;
            let byline =
                (repo.find_commit(branch.tip).ok()).and_then(|tip| super::commit_byline(&tip).ok());
            let (person, agent) = byline.unzip();
            let (author, email) = person.unzip();
            let (people, commits) =
                own_commits(&repo, branch.tip, branch.commit_count.unwrap_or(1));
            OtherBranch {
                ref_name: branch.ref_name.to_string(),
                name: branch.display_name.to_str_lossy().into_owned(),
                local: branch.has_local,
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
                description: (branch.description.as_ref())
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
        let Ok(((name, _), _)) = super::commit_byline(&commit) else {
            continue;
        };
        let Ok(time) = commit.author().and_then(|author| author.time()) else {
            continue;
        };
        let person = match people.iter().position(|known| *known == name) {
            Some(at) => at,
            None => {
                people.push(name);
                people.len() - 1
            }
        };
        commits.push(OwnCommit {
            time: time.seconds,
            person,
        });
    }
    (people, commits)
}

/// Set local branch `ref_name`'s description where Git keeps it, as `git branch
/// --edit-description` does; a blank one removes it.
pub fn set_branch_description(path: &Path, ref_name: &str, description: &str) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::branch::branch_set_description(
        &mut ctx,
        full_name(ref_name)?,
        Some(description.to_owned()),
    )
}

/// The commits `ref_name` has that the target doesn't, newest first, as the project's page
/// has a target's, so a picked one shows its message as a target commit does.
pub fn branch_commits(path: &Path, ref_name: &str) -> Result<Vec<TargetCommit>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let name = full_name(ref_name)?;
    let remote = but_core::extract_remote_name_and_short_name(
        name.as_ref(),
        &ctx.repo.get()?.remote_names(),
    );
    let (short, remote) = match remote {
        Some((remote, short)) => (short.to_string(), Some(remote)),
        None => (name.shorten().to_string(), None),
    };
    let details = but_api::legacy::workspace::branch_details(&ctx, short, remote)?;
    (details.commits.iter())
        .map(|commit| target_commit(&repo.find_commit(commit.id)?))
        .collect()
}

/// How applying a branch went.
pub enum ApplyOutcome {
    Applied,
    AlreadyApplied,
    /// It conflicts with these applied stacks, so nothing changed.
    Conflicts(Vec<String>),
}

/// Apply `ref_name` to the workspace. A remote branch gets a local branch to apply.
pub fn apply(path: &Path, ref_name: &str) -> Result<ApplyOutcome> {
    use but_workspace::branch::apply::OutcomeStatus;
    let mut ctx = open(path)?;
    let name = full_name(ref_name)?;
    let outcome = but_api::branch::apply(&mut ctx, name.as_ref())?;
    Ok(match outcome.status {
        OutcomeStatus::Applied => ApplyOutcome::Applied,
        OutcomeStatus::AlreadyApplied => ApplyOutcome::AlreadyApplied,
        OutcomeStatus::ConflictAborted => ApplyOutcome::Conflicts(
            outcome
                .conflicting_stacks
                .iter()
                .map(|stack| stack.ref_name.shorten().to_string())
                .collect(),
        ),
    })
}
