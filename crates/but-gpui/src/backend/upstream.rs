//! The remotes and the target branch: pushing and pulling branches, updating the workspace
//! from the target, fetching and landing; and the target's history, with what came in that
//! the workspace doesn't have yet, the pull request each commit landed, and when the
//! project last fetched.

use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;
use but_core::DryRun;
use but_core::sync::RepoExclusive;

use super::ops::{full_name, holds};
use super::reviews::{cached_reviews, from_cache};
use super::{CommitId, Review, ReviewState, open, runtime, short_ref, title};

#[derive(Debug, Clone)]
pub struct TargetCommit {
    pub id: CommitId,
    pub title: String,
    /// Seconds since the epoch.
    pub time: i64,
    /// Who wrote it, by name.
    pub author: String,
    /// The author's, for their picture.
    pub email: String,
    /// The pull request it landed, without CI checks.
    pub review: Option<Review>,
    /// The workspace doesn't have it yet.
    pub incoming: bool,
}

/// Up to `limit` of the target's commits, newest first, after `after` or from its tip;
/// and whether there are older ones.
pub fn target_commits(
    path: &Path,
    after: Option<CommitId>,
    limit: u32,
) -> Result<(Vec<TargetCommit>, bool)> {
    let ctx = open(path)?;
    let page = but_api::target_commits::workspace_target_commits(
        &ctx,
        after.map(|id| but_api::json::HexHash(id.0)),
        Some(limit),
    )?;
    let cached = cached_reviews(&ctx);
    let commits = page
        .commits
        .into_iter()
        .map(|target| {
            let commit = target.commit;
            TargetCommit {
                id: CommitId(commit.id),
                title: title(&commit.message),
                time: (commit.committed_at / 1000) as i64,
                author: commit.author.name,
                email: commit.author.email,
                review: target.review.map(|review| {
                    let mut found = match cached.get(&(review.number as u64)) {
                        Some(found) => from_cache(found),
                        // Named by the merge commit's message, which the cache has no record of.
                        None => Review {
                            number: review.number as u64,
                            title: review.title,
                            url: Some(review.html_url),
                            ..Review::default()
                        },
                    };
                    // It landed, whatever the cache saw last.
                    found.state = ReviewState::Merged;
                    found
                }),
                incoming: !target.in_workspace,
            }
        })
        .collect::<Vec<TargetCommit>>();
    // The first page stops where the workspace forked off, with older commits below that
    // unless it's where the history starts.
    let repo = ctx.repo.get()?;
    let forked_above_root = after.is_none()
        && (commits.last()).is_some_and(|last| {
            // A shallow clone's oldest commits name parents it doesn't have.
            (repo.find_commit(last.id.0))
                .is_ok_and(|c| c.parent_ids().next().is_some_and(|p| repo.has_object(p)))
        });
    Ok((commits, page.has_more || forked_above_root))
}

/// When the project last fetched, in seconds since the epoch, and why the last attempt
/// failed if it did. `None` when it has no remote to fetch from.
pub fn fetch_status(path: &Path) -> Result<Option<(Option<i64>, Option<String>)>> {
    let ctx = open(path)?;
    if ctx.repo.get()?.remote_names().is_empty() {
        return Ok(None);
    }
    let status = but_api::workspace::workspace_fetch_status(&ctx)?;
    // A fetch in a terminal counts too: Git notes every fetch in FETCH_HEAD, one for each
    // worktree, as an agent fetching in its own leaves one there.
    let common = ctx.repo.get()?.common_dir().to_owned();
    let linked = std::fs::read_dir(common.join("worktrees"))
        .into_iter()
        .flatten();
    let fetch_head = std::iter::once(common.clone())
        .chain(linked.flatten().map(|entry| entry.path()))
        .filter_map(|dir| {
            dir.join("FETCH_HEAD")
                .metadata()
                .and_then(|m| m.modified())
                .ok()
        })
        .max()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_secs() as i64);
    let fetched = status.last_successful_ms.map(|ms| (ms / 1000) as i64);
    // An error from before a later fetch elsewhere is stale.
    let attempted = status.last_attempted_ms.map(|ms| (ms / 1000) as i64);
    let error = status.last_error.filter(|_| fetch_head <= attempted);
    Ok(Some((fetched.max(fetch_head), error)))
}

/// Push `ref_name` and the branches below it, forced when one of them was rewritten since it
/// was pushed, and with `run_hooks` after the repository's pre-push hook passes. Returns the
/// name it has on its remote, which needn't be its local one, and why updating the stack's
/// pull requests afterwards failed, if it did.
pub fn push(
    path: &Path,
    ref_name: &str,
    protect: bool,
    drops: Option<&[CommitId]>,
    run_hooks: bool,
) -> Result<(Option<String>, Option<String>)> {
    // Forced, as decided when it runs, the push goes past the lease, which would refuse to
    // drop even the commits only the remote has that were shown. Unless pushing anyway, it
    // fetches again first and stops if the remote moved, as a teammate's amend still matches
    // the commit rewritten here, or has commits that weren't shown.
    if let Some(shown) = drops {
        let (info, tips) = with_remote_tips(path, ref_name)?;
        if needs_force(&info, ref_name) {
            fetch(path)?;
            let (info, now) = with_remote_tips(path, ref_name)?;
            // A stack pushes the branches below it too: the refusal names the one it's about.
            let name = |segment: Option<&but_workspace::ref_info::Segment>| {
                (segment.and_then(|segment| segment.ref_name())).map_or_else(
                    || short_ref(ref_name).to_owned(),
                    |n| n.shorten().to_string(),
                )
            };
            if now != tips {
                let moved = name(
                    (pushed_with(&info, ref_name).zip(tips.iter().zip(&now)))
                        .find(|(_, (before, after))| before != after)
                        .map(|(segment, _)| segment),
                );
                anyhow::bail!(
                    "Someone pushed to {moved} since the last fetch, and forcing would replace \
                     it. Check what's there before pushing again."
                );
            }
            if let Some(unseen) = pushed_with(&info, ref_name).find(|segment| {
                (segment.commits_on_remote.iter()).any(|c| !shown.contains(&CommitId(c.id)))
            }) {
                anyhow::bail!(
                    "The remote has commits on {} that weren't shown, and forcing would drop \
                     them. Push again to see them.",
                    name(Some(unseen))
                );
            }
        }
    }
    let ctx = open(path)?;
    // Decided when it runs, as an amend queued before it rewrites the branch.
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    // As in the desktop app, a conflicted commit stays here until it's resolved.
    if pushed_with(&info, ref_name).any(|segment| segment.commits.iter().any(|c| c.has_conflicts)) {
        anyhow::bail!("Resolve the conflicted commits first, so they aren't pushed");
    }
    let force = needs_force(&info, ref_name);
    let outcome = runtime()
        .handle()
        .block_on(
            but_api::legacy::workspace::workspace_branch_and_ancestors_push(
                ctx.into_sync(),
                force,
                !protect,
                ref_name.to_owned(),
                run_hooks,
                Vec::new(),
            ),
        )
        .map_err(|err| {
            if err.downcast_ref::<but_error::Code>() != Some(&but_error::Code::GitNonFastForward) {
                return err;
            }
            tracing::warn!(?err, "push rejected");
            anyhow::anyhow!("The remote has commits this branch doesn't. Fetch, then pull them in.")
        })?;
    let short = short_ref(ref_name);
    let remote = (outcome.push.branch_to_remote.into_iter())
        .find(|(branch, _, _)| branch == short || branch == ref_name)
        .map(|(_, _, remote)| remote);
    Ok((remote, sync_failure(outcome.review_sync)))
}

/// Why updating the stack's pull requests failed, if it did.
pub(super) fn sync_failure(outcome: but_forge::ReviewSyncOutcome) -> Option<String> {
    match outcome {
        but_forge::ReviewSyncOutcome::Failed { message } => {
            tracing::warn!(%message, "the stack's pull requests weren't updated");
            Some(message)
        }
        _ => None,
    }
}

/// Whether pushing `ref_name` has to force: it or a branch below it in its stack was rewritten
/// since it was pushed.
fn needs_force(info: &but_workspace::RefInfo, ref_name: &str) -> bool {
    use but_core::ui::PushStatus;
    pushed_with(info, ref_name)
        .any(|segment| segment.push_status == PushStatus::UnpushedCommitsRequiringForce)
}

/// The workspace now, and where the remote-tracking branches of `ref_name` and the ones below
/// it point.
fn with_remote_tips(
    path: &Path,
    ref_name: &str,
) -> Result<(but_workspace::RefInfo, Vec<Option<gix::ObjectId>>)> {
    let ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let repo = ctx.repo.get()?;
    let tips = pushed_with(&info, ref_name)
        .map(|segment| {
            (segment.remote_tracking_ref_name.as_ref())
                .and_then(|name| repo.find_reference(name.as_ref()).ok())
                .and_then(|mut tip| tip.peel_to_id().ok())
                .map(|id| id.detach())
        })
        .collect();
    Ok((info, tips))
}

/// `ref_name`'s branch and the ones below it in its stack, which pushing it pushes too.
fn pushed_with<'a>(
    info: &'a but_workspace::RefInfo,
    ref_name: &'a str,
) -> impl Iterator<Item = &'a but_workspace::ref_info::Segment> {
    (info.stacks.iter()).flat_map(move |stack| {
        (stack.segments.iter()).skip_while(move |segment| !holds(segment, ref_name))
    })
}

/// When `err` is force push protection stopping a push, what the remote itself said, if
/// anything: Git counts any refused force push as that, a protected branch's too.
pub fn push_refusal(err: &anyhow::Error) -> Option<String> {
    if err.downcast_ref::<but_error::Code>() != Some(&but_error::Code::GitForcePushProtection) {
        return None;
    }
    tracing::warn!(?err, "push refused");
    let text = format!("{err:#}");
    let said: Vec<&str> = (text.lines().map(str::trim))
        .filter_map(|line| line.strip_prefix("remote:").map(str::trim))
        .filter(|line| !line.is_empty())
        .collect();
    Some(said.join("\n"))
}

/// Bring in the commits only `branch`'s remote has: the local commits rebased on top of
/// them, or with `theirs` the remote's version of the branch instead of the local one.
pub fn pull_remote(path: &Path, branch: &str, theirs: bool) -> Result<()> {
    integrate_remote(&mut open(path)?, branch, theirs, DryRun::No)?;
    Ok(())
}

/// A dry run of pulling into `branch`: the branches (full refs) it would leave with
/// conflicted commits they don't have now, each with how many.
pub fn pull_conflicts(path: &Path, branch: &str) -> Result<Vec<(String, usize)>> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let preview = integrate_remote(&mut ctx, branch, false, DryRun::Yes)?;
    Ok(newly_conflicted(
        &info,
        &preview.workspace.conflicted_commits,
    ))
}

fn integrate_remote(
    ctx: &mut but_ctx::Context,
    branch: &str,
    theirs: bool,
    dry_run: DryRun,
) -> Result<but_api::branch::IntegrateBranchResult> {
    use but_api::branch::json::BranchIntegrationStrategy as Strategy;
    let name = full_name(branch)?;
    let strategy = if theirs {
        Strategy::PickRemote
    } else {
        Strategy::PullRebase
    };
    let plan = but_api::branch::get_initial_branch_integration(ctx, name.as_ref(), Some(strategy))?;
    // Rebasing would bring back the remote's versions of commits rewritten here, beside
    // their new ones: the same change ID on both sides, or with neither having one, as
    // after plain git, the same subject. A change ID on one side only tells them apart.
    let divergence = &plan.divergence;
    let rewritten_here = divergence.upstream_only.iter().any(|remote| {
        (divergence.local_only.iter()).any(|local| match (&remote.change_id, &local.change_id) {
            (Some(a), Some(b)) => a == b,
            (None, None) => remote.subject == local.subject,
            _ => false,
        })
    });
    anyhow::ensure!(
        theirs || !rewritten_here,
        "The remote has older versions of commits rewritten here, which pulling would bring \
         back. Replace the branch with the remote's version, or push this one over it."
    );
    let mut guard = ctx.exclusive_worktree_access();
    but_api::branch::apply_branch_integration_with_perm(
        ctx,
        name.as_ref(),
        plan.integration,
        dry_run,
        guard.write_permission(),
    )
}

/// What [`update_from_target()`] did.
#[derive(Debug)]
pub enum UpdateOutcome {
    /// It would have left these conflicted that aren't now, so nothing changed.
    WouldConflict(UpdateConflicts),
    Updated {
        /// Branches (full refs) left with conflicted commits they didn't have, each with
        /// how many.
        conflicted: Vec<(String, usize)>,
        /// Branches (full refs) that landed on the target, which the update removed.
        landed: Vec<String>,
    },
}

/// Rebase every stack onto the target as it is now. Doesn't fetch first; see [`fetch()`].
/// A dry run goes first, and when the rebase would leave anything conflicted that isn't now,
/// it stops there, unless that's what was `accepted`: a fetch since can bring more.
pub fn update_from_target(
    path: &Path,
    accepted: Option<&UpdateConflicts>,
) -> Result<UpdateOutcome> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    // Held from the dry run through the update, so nothing lands in between.
    let mut guard = ctx.exclusive_worktree_access();
    let preview = integrate_upstream(&mut ctx, &info, DryRun::Yes, guard.write_permission())?;
    let found = UpdateConflicts {
        branches: newly_conflicted(&info, &preview.workspace_state.conflicted_commits),
        files: (preview.worktree_conflicts.iter())
            .map(|path| path.to_str_lossy().into_owned())
            .collect(),
    };
    if (!found.branches.is_empty() || !found.files.is_empty()) && accepted != Some(&found) {
        return Ok(UpdateOutcome::WouldConflict(found));
    }
    let updated = integrate_upstream(&mut ctx, &info, DryRun::No, guard.write_permission())?;
    let after = but_api::legacy::workspace::head_info(&ctx)?;
    let names = |info: &but_workspace::RefInfo| -> Vec<String> {
        (info.stacks.iter())
            .flat_map(|stack| &stack.segments)
            .filter_map(|segment| Some(segment.ref_name()?.to_string()))
            .collect()
    };
    let remaining = names(&after);
    Ok(UpdateOutcome::Updated {
        conflicted: newly_conflicted(&info, &updated.workspace_state.conflicted_commits),
        landed: (names(&info).into_iter())
            .filter(|name| !remaining.contains(name))
            .collect(),
    })
}

/// What [`update_from_target()`] would leave conflicted that isn't now.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateConflicts {
    /// Branches (full refs), each with how many of its commits.
    pub branches: Vec<(String, usize)>,
    /// Uncommitted files the target changed too, which get conflict markers.
    pub files: Vec<String>,
}

/// The branches (full refs, empty for an unnamed one) in `info` with commits among
/// `conflicted` that aren't conflicted yet, each with how many.
fn newly_conflicted(
    info: &but_workspace::RefInfo,
    conflicted: &[(gix::ObjectId, but_core::ChangeId)],
) -> Vec<(String, usize)> {
    // A commit made without a change ID gets one from its commit ID, as the rebase does.
    let newly = |commit: &&but_workspace::ref_info::LocalCommit| {
        let id = commit.inner.change_id();
        !commit.inner.has_conflicts && conflicted.iter().any(|(_, change)| *change == *id)
    };
    (info.stacks.iter())
        .flat_map(|stack| &stack.segments)
        .filter_map(|segment| {
            let count = segment.commits.iter().filter(newly).count();
            let name = segment.ref_name().map(ToString::to_string);
            (count > 0).then(|| (name.unwrap_or_default(), count))
        })
        .collect()
}

fn integrate_upstream(
    ctx: &mut but_ctx::Context,
    info: &but_workspace::RefInfo,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> Result<but_api::workspace::WorkspaceIntegrateUpstreamOutcome> {
    let updates = but_api::workspace::rebase_lane_bottoms(info.stacks.iter().map(|s| s.lane()));
    but_api::workspace::workspace_integrate_upstream_with_perm(ctx, updates, dry_run, perm)
}

/// Fetch from the project's remotes. `false` when it has none to fetch from.
pub fn fetch(path: &Path) -> Result<bool> {
    let mut ctx = open(path)?;
    if ctx.repo.get()?.remote_names().is_empty() {
        return Ok(false);
    }
    but_api::workspace::workspace_fetch_from_remotes(&mut ctx, Some("fetch".into()))?;
    Ok(true)
}

/// Put `branch` (a short name), or with `whole_stack` it and every branch below it,
/// straight onto the target and push the target. Returns whether the other branches still
/// need an update to sit on the moved target.
pub fn land(path: &Path, branch: &str, whole_stack: bool) -> Result<bool> {
    let mut ctx = open(path)?;
    let landed = but_api::land::branch_land(&mut ctx, branch.to_owned(), false, whole_stack)?;
    Ok(landed.reconcile_skipped)
}

#[cfg(test)]
mod tests {
    use super::push_refusal;

    #[test]
    fn a_refused_push_keeps_only_what_the_remote_said() {
        use but_error::Code;
        let refused = |stderr: &str| {
            anyhow::anyhow!("git command exited with non-zero exit code 1:\n\nSTDERR:\n{stderr}")
                .context(Code::GitForcePushProtection)
        };
        let protected = refused(
            "remote: error: GH006: Protected branch update failed for refs/heads/main.\n\
             remote: \n ! [remote rejected] main -> main (protected branch hook declined)",
        );
        assert_eq!(
            push_refusal(&protected).as_deref(),
            Some("error: GH006: Protected branch update failed for refs/heads/main."),
            "the remote's own words, blank lines dropped"
        );
        let stale = refused(" ! [rejected] main -> main (stale info)");
        assert_eq!(
            push_refusal(&stale).as_deref(),
            Some(""),
            "nothing from the remote: someone pushed since the last fetch"
        );
        let other = anyhow::anyhow!("failed to connect");
        assert_eq!(push_refusal(&other), None, "not protection at all");
    }
}
