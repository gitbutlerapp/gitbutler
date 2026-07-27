//! A shared guard against mutating history that has already landed in the
//! target branch.
//!
//! Commands that rewrite commits or add work to branches build a
//! [`MergedUpstream`] once and validate their targets against it. Amending,
//! squashing into, or uncommitting a landed commit produces content that
//! upstream-integration detection no longer recognizes, so the duplicated
//! work resurfaces as conflicts on the next `but pull`.

use but_workspace::{
    RefInfo,
    ref_info::{LocalCommitRelation, Segment},
    ui::PushStatus,
};
use gix::refs::{FullName, FullNameRef};

use crate::{CliResult, args::atoms::AllowMergedArg, bad_input};

/// The shared hint for refused operations: pulling is almost always the right
/// fix, the escape hatch is for the rare deliberate case.
const PULL_FIRST_HINT: &str = "Most likely you want `but pull`, which updates the workspace and removes landed work. \
     In rare cases `--allow-merged` can bypass this check";

/// The branches and commits of the workspace that have already landed in the
/// target branch. Mirrors the strongest signals behind the `(merged upstream)`
/// marker in `but status`, using only data present on [`RefInfo`] segments.
pub struct MergedUpstream {
    integrated_commits: gix::hashtable::HashSet<gix::ObjectId>,
    merged_branches: std::collections::BTreeSet<FullName>,
}

impl MergedUpstream {
    /// Build the guard from the canonical workspace view, the same
    /// [`but_api::legacy::workspace::head_info`] that `but commit` and the GUI
    /// use. When `--allow-merged` was passed, the guard is permissive and
    /// every check passes; the workspace query is skipped entirely.
    ///
    /// Acquires no worktree guard, so this is safe to call while one is held.
    pub fn from_ctx(ctx: &but_ctx::Context, allow_merged: AllowMergedArg) -> anyhow::Result<Self> {
        if allow_merged.allow_merged {
            return Ok(Self::permissive());
        }
        let head_info = but_api::legacy::workspace::head_info(ctx)?;
        Ok(Self::new(&*ctx.repo.get()?, &head_info, allow_merged))
    }

    /// Collect merged branches and integrated commits from `head_info`, for
    /// callers that already computed it. When `--allow-merged` was passed, the
    /// guard is permissive and every check passes.
    ///
    /// `repo` is only read to classify empty branches, whose merged state
    /// cannot be derived from `head_info` alone.
    pub fn new(repo: &gix::Repository, head_info: &RefInfo, allow_merged: AllowMergedArg) -> Self {
        let mut this = Self::permissive();
        if allow_merged.allow_merged {
            return this;
        }
        for segment in head_info.stacks.iter().flat_map(|stack| &stack.segments) {
            if segment_is_merged_upstream(segment)
                && let Some(ref_info) = &segment.ref_info
            {
                this.merged_branches.insert(ref_info.ref_name.clone());
            }
            for commit in &segment.commits {
                if matches!(commit.relation, LocalCommitRelation::Integrated(_)) {
                    this.integrated_commits.insert(commit.id);
                }
            }
        }
        this.collect_merged_empty_branches(repo, head_info);
        this
    }

    /// Empty segments have no commits to classify, so detect landed ones the
    /// way `but status` does: their remote tip is reachable from the target
    /// tip. Only the bottom-most segment of each stack is considered — it
    /// rests on the workspace base, where ancestry is meaningful. Best-effort:
    /// lookup failures leave the branch unguarded rather than erroring.
    fn collect_merged_empty_branches(&mut self, repo: &gix::Repository, head_info: &RefInfo) {
        fn peel(repo: &gix::Repository, name: &FullNameRef) -> Option<gix::ObjectId> {
            repo.try_find_reference(name)
                .ok()
                .flatten()
                .and_then(|mut reference| reference.peel_to_id().ok())
                .map(|id| id.detach())
        }

        let target_ref_name = head_info
            .target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref());
        let Some(target_tip) = target_ref_name.and_then(|name| peel(repo, name)) else {
            return;
        };
        let stored_target_base = head_info
            .target_commit
            .as_ref()
            .map(|target| target.commit_id);

        for stack in &head_info.stacks {
            let Some(segment) = stack
                .segments
                .last()
                .filter(|segment| segment.commits.is_empty())
            else {
                continue;
            };
            let (Some(ref_info), Some(remote_ref_name)) =
                (&segment.ref_info, segment.remote_tracking_ref_name.as_ref())
            else {
                continue;
            };
            if Some(remote_ref_name.as_ref()) == target_ref_name {
                continue;
            }
            let Some(remote_tip) = peel(repo, remote_ref_name.as_ref()) else {
                continue;
            };
            // A fresh empty branch pushed while sitting at the (possibly stale)
            // base is trivially an ancestor of the target without having landed.
            if stored_target_base == Some(remote_tip) && remote_tip != target_tip {
                continue;
            }
            if repo
                .merge_base(remote_tip, target_tip)
                .is_ok_and(|merge_base| merge_base.detach() == remote_tip)
            {
                self.merged_branches.insert(ref_info.ref_name.clone());
            }
        }
    }

    fn permissive() -> Self {
        Self {
            integrated_commits: Default::default(),
            merged_branches: Default::default(),
        }
    }

    /// Whether `commit_id` has landed in the target branch.
    pub fn contains_commit(&self, commit_id: gix::ObjectId) -> bool {
        self.integrated_commits.contains(&commit_id)
    }

    /// Whether the branch at the head of this segment has landed in the target branch.
    pub fn contains_segment(&self, segment: &Segment) -> bool {
        segment
            .ref_info
            .as_ref()
            .is_some_and(|ref_info| self.merged_branches.contains(&ref_info.ref_name))
    }

    /// Reject `commit_id` as a mutation source or target if it has landed.
    pub fn ensure_commit_not_merged(&self, commit_id: gix::ObjectId) -> CliResult<()> {
        if self.contains_commit(commit_id) {
            return Err(bad_input(format!(
                "Commit {} is merged upstream",
                commit_id.to_hex_with_len(7)
            ))
            .hint(PULL_FIRST_HINT)
            .into());
        }
        Ok(())
    }

    /// Reject the branch `name` as a mutation or push target if it has landed.
    pub fn ensure_branch_not_merged(&self, name: &FullNameRef) -> CliResult<()> {
        if self.merged_branches.contains(name) {
            return Err(
                bad_input(format!("Branch '{}' is merged upstream", name.shorten()))
                    .hint(PULL_FIRST_HINT)
                    .into(),
            );
        }
        Ok(())
    }
}

/// Whether the branch at the tip of this segment has already landed in the
/// target branch, using the same signals as the `(merged upstream)` marker in
/// `but status`. The push-status check looks redundant with the commit
/// relation today, but keeps the guard aligned should its derivation grow
/// more inputs.
fn segment_is_merged_upstream(segment: &Segment) -> bool {
    matches!(segment.push_status, PushStatus::Integrated)
        || segment
            .commits
            .first()
            .is_some_and(|commit| matches!(commit.relation, LocalCommitRelation::Integrated(_)))
}
