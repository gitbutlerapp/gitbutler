//! The project's local and remote branches, including its target and branches in use.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use bstr::{BStr, BString, ByteSlice};
use but_branches::ListedStackStatus;

use super::intent::{Intent, Run};
use super::reviews::from_cache;
use super::upstream::{TargetCommit, target_commit};
use super::{Agent, CommitId, Review, open};

#[derive(Debug, Clone)]
pub struct OtherBranch {
    /// The exact local or remote-tracking ref this row displays.
    pub ref_name: super::FullName,
    pub name: String,
    pub local: bool,
    /// Its local counterpart, if any, also for a remote row whose tip differs from it.
    pub local_ref: Option<super::FullName>,
    /// This ref is the local target or the target's configured upstream.
    pub target: bool,
    /// Its local counterpart is applied or checked out in a worktree.
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
    /// A local stack's display group, or a remote ref's own group in the flat inventory.
    pub stack: usize,
    pub tip: CommitId,
    /// Its pull request, without CI checks: the app asks the forge about those only for the
    /// workspace's branches, so the cache's would be stale.
    pub review: Option<Review>,
    /// A local branch's description, as `git branch --edit-description` writes it.
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OwnCommit {
    /// The day it was written, as [`super::upstream::day_of`] numbers days: a rebase leaves
    /// that be.
    pub day: i64,
    /// Who it's shown by, by where they are in [`OtherBranch::people`].
    pub person: usize,
}

/// Every local branch and every remote-tracking branch as separate rows. Local stacks keep
/// their order; remote refs are listed independently, newest first, including the target.
pub fn all_branches(path: &Path) -> Result<Vec<OtherBranch>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let descriptions = descriptions(&repo);
    let mailmap = repo.open_mailmap();
    let target = super::target_source(&repo, &ctx.project_meta()?);
    let stacks = but_api::branch::branch_list(&ctx)?;
    let mut remote_group = stacks.len();
    let (mut local, mut remotes) = (Vec::new(), Vec::new());
    for (stack, listed_stack) in stacks.into_iter().enumerate() {
        let applied = listed_stack.status == ListedStackStatus::Applied;
        let is_target = listed_stack.status == ListedStackStatus::Target;
        for listed in listed_stack.branches {
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
            let row = OtherBranch {
                ref_name: branch.ref_name.clone(),
                name: branch.display_name.to_str_lossy().into_owned(),
                local: branch.has_local,
                local_ref: branch.has_local.then(|| branch.ref_name.clone()),
                target: is_target,
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
            };
            if branch.has_local {
                local.push(row.clone());
            }
            // The shared listing already enumerates every remote ref. Expand those here
            // instead of another ref walk, and never substitute a local namesake's tip.
            for ref_name in branch.remote_refs {
                let Some(tip) = super::ref_tip(&repo, ref_name.as_ref()).map(CommitId) else {
                    continue;
                };
                let mut remote = row.clone();
                remote.ref_name = ref_name;
                remote.local = false;
                remote.target = target.as_ref() == Some(&remote.ref_name);
                remote.description = None;
                remote.stack = remote_group;
                remote_group += 1;
                if tip != row.tip {
                    let commit = repo.find_commit(tip.0)?;
                    let byline = super::commit_byline(&mailmap, &commit).ok();
                    let (person, agent) = byline.unzip();
                    let (author, email) = person.unzip();
                    remote.tip = tip;
                    remote.author = author;
                    remote.email = email;
                    remote.agent = agent.flatten();
                    remote.time = commit.time().ok().map(|time| time.seconds);
                    // The graph counts belong to the primary ref, not this different tip.
                    // Its full history is read on demand when its row is opened.
                    remote.ahead = None;
                    remote.own_commits = None;
                    remote.people = remote.author.iter().cloned().collect();
                    remote.commits.clear();
                }
                if remote.target {
                    remote.ahead = Some(0);
                    remote.own_commits = Some(0);
                    remote.people.clear();
                    remote.commits.clear();
                }
                remotes.push(remote);
            }
        }
    }
    remotes.sort_by(|a, b| {
        b.time
            .cmp(&a.time)
            .then_with(|| a.ref_name.cmp(&b.ref_name))
    });
    local.extend(remotes);
    Ok(local)
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

/// Set local branch `branch`'s description where Git keeps it, as `git branch
/// --edit-description` does; a blank one removes it. Git config isn't in the oplog, so it
/// isn't a step to undo.
pub struct DescribeBranch {
    pub branch: super::FullName,
    pub description: String,
}

impl Intent for DescribeBranch {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        let Self {
            branch,
            description,
        } = self;
        anyhow::ensure!(
            branch.category() == Some(gix::refs::Category::LocalBranch),
            "Only a local branch has a description"
        );
        run.locked(|ctx, _| {
            let repo = ctx.repo.get()?;
            // Not under a name that's gone, as after an agent renamed it while it was being
            // written, where a branch made later by that name would take it up. Copying the
            // error keeps it.
            if repo.try_find_reference(branch.as_ref())?.is_none() {
                but_error::bail_precondition!(
                    "{} was renamed or deleted meanwhile, so this wasn't saved:\n\n{}",
                    branch.shorten(),
                    description
                );
            }
            let description = Some(description.trim()).filter(|d| !d.is_empty());
            write_description(&repo, branch.shorten(), description.map(BStr::new))
        })
    }
}

/// Move branch `from`'s description to `to`, as `git branch -m` does.
pub(super) fn move_description(repo: &gix::Repository, from: &BStr, to: &BStr) -> Result<()> {
    let description = (but_core::git_config::open_repo_local_config_for_reading(repo)?)
        .section("branch", Some(from))
        .ok()
        .and_then(|section| section.value("description"));
    if description.is_none() {
        return Ok(());
    }
    write_description(repo, from, None)?;
    write_description(repo, to, description.as_ref().map(|d| d.as_bstr()))
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

/// Apply `branch` to the workspace, once the archived worktree `freeing`, if any, let go of
/// it. A remote branch gets a local branch to apply.
pub struct ApplyBranch {
    pub branch: super::FullName,
    pub freeing: Option<BString>,
}

impl Intent for ApplyBranch {
    type Done = ApplyOutcome;

    fn apply(self, run: &mut Run) -> Result<ApplyOutcome> {
        use but_workspace::branch::apply::OutcomeStatus;
        let Self { branch, freeing } = self;
        let point_head = |repo: &gix::Repository, at, why: String| {
            but_core::update_head_reference(repo, at, false, "checkout", why.as_str().into(), 0)
        };
        let outcome = run.locked(|ctx, perm| {
            // The archived worktree `freeing` lets go of the branch first, staying on its
            // commit with its files as they are, or the workspace's commits would move the
            // branch under it.
            let freed = match &freeing {
                Some(worktree) => {
                    let repo = super::checkout(ctx, Some(worktree.as_ref()))?;
                    (repo.head_name()?.as_ref() == Some(&branch)).then_some(repo)
                }
                None => None,
            };
            if let Some(repo) = &freed {
                let at = gix::refs::Target::Object(repo.head_id()?.detach());
                let why = format!("leaving {} to the workspace", branch.shorten());
                point_head(repo, at, why)?;
            }
            let outcome = but_api::branch::apply_with_perm(ctx, branch.as_ref(), perm);
            // Not applied after all, it goes back to the worktree, so its next commits move it
            // again.
            let kept =
                matches!(&outcome, Ok(o) if !matches!(o.status, OutcomeStatus::ConflictAborted));
            if let Some(repo) = freed.filter(|_| !kept) {
                let back = gix::refs::Target::Symbolic(branch.clone());
                let why = format!("taking {} back, as it wasn't applied", branch.shorten());
                point_head(&repo, back, why)?;
            }
            outcome
        })?;
        let remotes = run.ctx().repo.get()?.remote_names();
        let local = match but_core::extract_remote_name_and_short_name(branch.as_ref(), &remotes) {
            Some((_, short)) => {
                let mut local = BString::from("refs/heads/");
                local.extend_from_slice(&short);
                super::FullName::try_from(local)?
            }
            None => branch,
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
}

#[cfg(test)]
mod tests {
    use super::all_branches;
    use crate::backend::ops::tests::{git, repo_with, target_origin_main};

    #[test]
    fn remote_inventory_includes_target_current_and_same_tip_branches() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("base\n");
            let path = dir.path();
            target_origin_main(path);
            git(
                path,
                &[
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    "refs/remotes/origin/main",
                ],
            );
            git(path, &["update-ref", "refs/remotes/origin/master", "main"]);
            git(path, &["checkout", "-qb", "xsh/dev"]);
            git(
                path,
                &["commit", "-q", "--allow-empty", "-m", "active work"],
            );
            git(path, &["branch", "dev"]);
            git(path, &["update-ref", "refs/remotes/origin/xsh/dev", "HEAD"]);
            let advanced = git(
                path,
                &[
                    "commit-tree",
                    "HEAD^{tree}",
                    "-p",
                    "HEAD",
                    "-m",
                    "remote work",
                ],
            );
            git(
                path,
                &["update-ref", "refs/remotes/origin/dev", advanced.trim()],
            );
            let feature = git(
                path,
                &[
                    "commit-tree",
                    "main^{tree}",
                    "-p",
                    "main",
                    "-m",
                    "feature work",
                ],
            );
            git(
                path,
                &["update-ref", "refs/remotes/origin/feature", feature.trim()],
            );

            let branches = all_branches(path).expect("the branch inventory can be read");
            let mut remotes: Vec<_> = branches
                .iter()
                .filter(|b| !b.local)
                .map(|b| b.ref_name.to_string())
                .collect();
            remotes.sort();
            assert_eq!(
                remotes,
                [
                    "refs/remotes/origin/dev",
                    "refs/remotes/origin/feature",
                    "refs/remotes/origin/main",
                    "refs/remotes/origin/master",
                    "refs/remotes/origin/xsh/dev",
                ],
                "every remote branch gets a row, including the target and the current branch, without symbolic HEAD"
            );
            let dev = branches
                .iter()
                .find(|b| b.ref_name.to_string() == "refs/remotes/origin/dev")
                .expect("the remote dev branch is listed");
            assert_eq!(
                dev.tip.0.to_string(),
                advanced.trim(),
                "a local namesake cannot replace the remote branch's tip"
            );
            assert!(
                dev.own_commits.is_none(),
                "the local namesake's graph count is not reused for a different remote tip"
            );
            assert_eq!(
                dev.local_ref.as_ref().map(ToString::to_string).as_deref(),
                Some("refs/heads/dev"),
                "the local counterpart remains known for in-use checks"
            );
            assert!(
                branches
                    .iter()
                    .any(|b| !b.local && b.target && b.name == "main"),
                "the configured target upstream is marked"
            );
            assert!(
                branches
                    .iter()
                    .any(|b| !b.local && b.applied && b.name == "xsh/dev"),
                "the current branch's remote counterpart retains its in-use status"
            );
        });
    }

    #[test]
    fn remote_inventory_keeps_the_same_name_on_multiple_remotes() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("base\n");
            let path = dir.path();
            target_origin_main(path);
            git(
                path,
                &["remote", "add", "backup", "https://example.com/backup.git"],
            );
            let other = git(
                path,
                &[
                    "commit-tree",
                    "HEAD^{tree}",
                    "-p",
                    "HEAD",
                    "-m",
                    "other remote",
                ],
            );
            git(
                path,
                &["update-ref", "refs/remotes/backup/main", other.trim()],
            );
            let branches = all_branches(path).expect("the inventory can be read");
            let remotes: Vec<_> = branches.iter().filter(|b| !b.local).collect();
            assert_eq!(
                remotes.len(),
                2,
                "both remotes have their own row despite sharing the branch name"
            );
            let backup = remotes
                .iter()
                .find(|b| b.ref_name.to_string() == "refs/remotes/backup/main")
                .expect("backup/main is listed");
            assert_eq!(
                backup.tip.0.to_string(),
                other.trim(),
                "each remote keeps its own tip"
            );
            assert!(
                !backup.target,
                "a different remote's main is not the configured target upstream"
            );
        });
    }
}
