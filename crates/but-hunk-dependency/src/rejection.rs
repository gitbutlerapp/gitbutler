//! Explain why changes were rejected from a commit or amend.
//!
//! GitButler allows several independent branches to be applied at once, and a
//! change that depends on a commit in another branch cannot be committed
//! elsewhere on its own. The commit engine only reports a coarse
//! [`RejectionReason`] per change; this module joins those rejections with the
//! workspace hunk dependencies to say which commit and branch each change
//! depends on, and which branch they could all be committed to instead.

use but_core::{DiffSpec, HunkHeader, ref_metadata::StackId, tree::create_tree::RejectionReason};
use but_graph::Workspace;
use gix::bstr::{BStr, BString, ByteSlice as _};

use crate::ui::{
    HunkDependencies, HunkLockTarget, hunk_dependencies_for_workspace_changes_by_worktree_dir,
};

/// A single rejected change, with the workspace dependencies that explain why
/// it could not be applied.
#[derive(Debug, Clone)]
pub struct RejectedChange {
    /// The worktree-relative path of the rejected change.
    pub path: BString,
    /// The coarse reason the change was rejected.
    pub reason: RejectionReason,
    /// The hunks of this change that depend on a commit in the workspace,
    /// together with the commits they depend on.
    ///
    /// Empty when the rejection was not caused by a dependency, or when the
    /// dependency could not be pinpointed to a specific hunk.
    pub dependencies: Vec<HunkDependency>,
    /// When a dependency rejection cannot be pinpointed to a hunk (adjacent
    /// insertions, for example, do not intersect as ranges), the branches
    /// whose workspace commits also touch this path — the likely dependency.
    pub suspected_branches: Vec<gix::refs::FullName>,
}

impl RejectedChange {
    /// Every branch this change depends on, or is suspected to depend on,
    /// with `None` for a dependency commit whose branch is unknown.
    pub fn branches(&self) -> impl Iterator<Item = Option<&gix::refs::FullNameRef>> {
        (self.dependencies.iter())
            .flat_map(|dependency| &dependency.commits)
            .map(|commit| commit.branch.as_ref().map(|name| name.as_ref()))
            .chain(
                self.suspected_branches
                    .iter()
                    .map(|name| Some(name.as_ref())),
            )
    }
}

/// A hunk that depends on one or more commits in the workspace.
#[derive(Debug, Clone)]
pub struct HunkDependency {
    /// The dependent hunk, in worktree coordinates.
    pub hunk: HunkHeader,
    /// The dependent hunk as a unified diff without context lines, its `@@` header first.
    pub diff: BString,
    /// The commits (and their branches, when known) this hunk depends on.
    pub commits: Vec<DependencyCommit>,
}

/// A commit a hunk depends on.
#[derive(Debug, Clone)]
pub struct DependencyCommit {
    /// The commit in the workspace the hunk depends on.
    pub commit_id: gix::ObjectId,
    /// The branch owning [`Self::commit_id`], if it could be resolved from the
    /// workspace.
    pub branch: Option<gix::refs::FullName>,
}

/// Whether `reason` indicates a change depends on another commit in the
/// workspace, and is therefore worth resolving to a commit and branch.
pub fn is_dependency_reason(reason: RejectionReason) -> bool {
    matches!(
        reason,
        RejectionReason::CherryPickMergeConflict | RejectionReason::WorkspaceMergeConflict
    )
}

/// Explain `rejected_specs` by attaching the workspace dependency information
/// (which commit and branch each rejected hunk depends on) to the
/// dependency-related rejections. `target_branch`, the branch the changes were
/// committed to, is never suspected of its own rejection.
///
/// Must be called with the workspace as it was before the failed operation,
/// which is what the dependencies are computed against.
///
/// The hunk dependencies are computed only when at least one rejection looks
/// like a dependency conflict. Failing to compute them isn't fatal — the
/// change is still reported, just without the dependency detail — because
/// explaining a failure must never mask it.
pub fn explain_rejections(
    repo: &gix::Repository,
    ws: &Workspace,
    rejected_specs: &[(RejectionReason, DiffSpec)],
    target_branch: Option<&gix::refs::FullNameRef>,
) -> Vec<RejectedChange> {
    let needs_dependencies = rejected_specs
        .iter()
        .any(|(reason, _)| is_dependency_reason(*reason));

    let dependencies = if needs_dependencies {
        match hunk_dependencies_for_workspace_changes_by_worktree_dir(repo, ws, None) {
            Ok(dependencies) => {
                if !dependencies.errors.is_empty() {
                    tracing::warn!(
                        errors = ?dependencies.errors,
                        "Partial failure computing hunk dependencies for rejected changes"
                    );
                }
                Some(dependencies)
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "Failed to compute hunk dependencies for rejected changes"
                );
                None
            }
        }
    } else {
        None
    };

    rejected_specs
        .iter()
        .map(|(reason, spec)| {
            let dependencies = match &dependencies {
                Some(deps) if is_dependency_reason(*reason) => {
                    dependencies_for_spec(ws, deps, spec)
                }
                _ => Vec::new(),
            };
            // The branch being committed to never explains its own rejection,
            // so it is excluded from the suspects.
            let suspected_branches = if dependencies.is_empty() && is_dependency_reason(*reason) {
                branches_touching_path(repo, ws, spec.path.as_bstr())
                    .into_iter()
                    .filter(|branch| Some(branch.as_ref()) != target_branch)
                    .collect()
            } else {
                Vec::new()
            };
            RejectedChange {
                path: spec.path.clone(),
                reason: *reason,
                dependencies,
                suspected_branches,
            }
        })
        .collect()
}

/// The one branch all `changes` could be committed to, or stacked on: every
/// branch they depend on is in the same stack, and this is the topmost of
/// them, which has all the others below it.
///
/// `None` if that wouldn't be a complete fix: some rejected change isn't
/// dependency-related, a dependency couldn't be resolved to a branch in the
/// workspace, or the dependencies span more than one stack.
pub fn dependency_branch(
    ws: &Workspace,
    changes: &[RejectedChange],
) -> Option<gix::refs::FullName> {
    let stacks = stacks_depended_on(ws, changes)?;
    let [(stack, segment)] = stacks.as_slice() else {
        return None;
    };
    let segment = ws.stacks.get(*stack)?.segments.get(*segment)?;
    segment.ref_name().map(ToOwned::to_owned)
}

/// Each stack `changes` depend on, in the order they're met, with the topmost segment they
/// depend on in it, counted from its tip.
///
/// `None` if some rejected change isn't dependency-related or a dependency couldn't be
/// resolved to a branch in the workspace, as then no branch above them would take every change.
fn stacks_depended_on(ws: &Workspace, changes: &[RejectedChange]) -> Option<Vec<(usize, usize)>> {
    let mut stacks: Vec<(usize, usize)> = Vec::new();
    for change in changes {
        if change.dependencies.is_empty() && change.suspected_branches.is_empty() {
            return None;
        }
        for name in change.branches() {
            let (stack, segment) = position_of(ws, name?)?;
            match stacks.iter_mut().find(|(at, _)| *at == stack) {
                Some((_, top)) => *top = (*top).min(segment),
                None => stacks.push((stack, segment)),
            }
        }
    }
    Some(stacks)
}

/// A way to stack the stacks that changes depend on into one, so that one branch has every
/// commit they depend on below it: the branches of the others moved, whole stacks one after
/// another, onto a branch of the one that stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stacking {
    /// The branch the first of `branches` goes directly above: the topmost the changes depend
    /// on in the stack that stays, or its top.
    pub onto: gix::refs::FullName,
    /// The branches moved, each directly above the one before it, the first above `onto`: the
    /// stacks moved, each bottom branch first. Never empty.
    pub branches: Vec<gix::refs::FullName>,
    /// The branch on top of those moved, the last of `branches`, which takes the changes. Any
    /// above `onto` in the stack that stays end up above it.
    pub top: gix::refs::FullName,
    /// The branches it rewrites that have commits on their remote, which then need a force push:
    /// those moved, and those above `onto`, which are rebased onto them.
    pub pushed: Vec<gix::refs::FullName>,
}

/// The ways to stack the stacks `changes` depend on, when they depend on more than one, so
/// that a branch above them all could take every change: each of them staying in turn, the
/// others moved onto it whole, one after another in the workspace's order. They go above the
/// topmost branch the changes depend on in the one that stays. Those rewriting the fewest
/// pushed branches, then moving the fewest, come first.
///
/// A branch in `merged` never moves, as it has landed: a stack with one only ever stays, and
/// what's above where the others go is rebased, so with a merged branch there they go on top
/// of its stack instead. Merged branches can only be stacked on.
///
/// Empty where stacking wouldn't be a complete fix, as [`dependency_branch`] says, or where
/// the changes depend on one stack.
pub fn stackings(
    ws: &Workspace,
    changes: &[RejectedChange],
    merged: &[gix::refs::FullName],
) -> Vec<Stacking> {
    use but_graph::workspace::{StackCommitFlags, StackSegment};
    let merged = |segment: &StackSegment| {
        (segment.ref_name()).is_some_and(|name| merged.iter().any(|m| m.as_ref() == name))
    };
    let pushed = |segment: &StackSegment| {
        (segment.commits.iter()).any(|commit| {
            commit
                .flags
                .contains(StackCommitFlags::ReachableByMatchingRemote)
        })
    };
    let stacks = stacks_depended_on(ws, changes).unwrap_or_default();
    if stacks.len() < 2 {
        return Vec::new();
    }
    // The one that stays last first, so that of two stacks the first depended on moves.
    let mut found: Vec<Stacking> = (stacks.iter().rev())
        .filter_map(|&(stays, depended_on)| {
            let stays_segments = &ws.stacks.get(stays)?.segments;
            // What goes above `onto` is rebased onto what's moved there, so with a merged
            // branch among them it goes on top of the stack, which has all of it below.
            let above = stays_segments.get(..depended_on)?;
            let onto_ix = match above.iter().any(&merged) {
                true => 0,
                false => depended_on,
            };
            let onto = stays_segments.get(onto_ix)?.ref_name()?.to_owned();
            let rebased = stays_segments.get(..onto_ix)?;
            // The others, whole, in the workspace's order, each bottom branch first.
            let mut moved: Vec<&StackSegment> = Vec::new();
            for &(stack, _) in stacks.iter().filter(|(stack, _)| *stack != stays) {
                moved.extend(ws.stacks.get(stack)?.segments.iter().rev());
            }
            if moved.iter().any(|segment| merged(segment)) {
                return None;
            }
            let branches = (moved.iter())
                .map(|segment| segment.ref_name().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>()?;
            let pushed = (rebased.iter().chain(moved.iter().copied()))
                .filter(|segment| pushed(segment))
                .filter_map(|segment| segment.ref_name().map(ToOwned::to_owned))
                .collect();
            Some(Stacking {
                onto,
                top: branches.last()?.clone(),
                branches,
                pushed,
            })
        })
        .collect();
    found.sort_by_key(|stacking| (stacking.pushed.len(), stacking.branches.len()));
    found
}

/// The stack holding `branch` and its segment in it, counted from the tip.
fn position_of(ws: &Workspace, branch: &gix::refs::FullNameRef) -> Option<(usize, usize)> {
    ws.stacks.iter().enumerate().find_map(|(stack_ix, stack)| {
        let segment_ix = stack
            .segments
            .iter()
            .position(|segment| segment.ref_name() == Some(branch))?;
        Some((stack_ix, segment_ix))
    })
}

/// Find the hunks of `dependencies` that belong to `spec`.
///
/// Matching is by path, then by hunk overlap: a rejected spec covering specific
/// hunks only reports the dependent hunks that overlap them, while a whole-file
/// spec (no hunks) reports every dependent hunk for the path. Overlap is used
/// rather than exact equality because the dependency hunks are computed without
/// context lines, so their boundaries rarely match the spec's hunks exactly.
fn dependencies_for_spec(
    ws: &Workspace,
    dependencies: &HunkDependencies,
    spec: &DiffSpec,
) -> Vec<HunkDependency> {
    let spec_path = spec.path.as_bstr();
    let mut result = Vec::new();
    for (dep_path, dep_hunk, locks) in &dependencies.diffs {
        // `dep_path` is a lossy-UTF8 key, so a path with invalid UTF8 simply
        // won't match and falls back to the reason-only summary.
        if dep_path.as_bytes().as_bstr() != spec_path {
            continue;
        }
        if locks.is_empty() {
            continue;
        }
        let hunk = HunkHeader::from(dep_hunk);
        let overlaps = spec.hunk_headers.is_empty()
            || spec
                .hunk_headers
                .iter()
                .any(|spec_hunk| hunks_overlap(spec_hunk, &hunk));
        if !overlaps {
            continue;
        }
        let commits = locks
            .iter()
            .map(|lock| DependencyCommit {
                commit_id: lock.commit_id,
                branch: branch_of_commit(ws, lock.commit_id, stack_of(lock.target)),
            })
            .collect();
        result.push(HunkDependency {
            hunk,
            diff: dep_hunk.diff.clone(),
            commits,
        });
    }
    result
}

/// The stack a lock points at, if it is identifiable.
fn stack_of(target: HunkLockTarget) -> Option<StackId> {
    match target {
        HunkLockTarget::Stack(id) => Some(id),
        HunkLockTarget::Unidentified => None,
    }
}

/// Resolve the branch owning `commit_id`, searching `prefer` first (the stack
/// a lock points at) and then the remaining workspace stacks.
pub fn branch_of_commit(
    ws: &Workspace,
    commit_id: gix::ObjectId,
    prefer: Option<StackId>,
) -> Option<gix::refs::FullName> {
    let ordered = ws
        .stacks
        .iter()
        .filter(|stack| prefer.is_none() || stack.id == prefer)
        .chain(
            ws.stacks
                .iter()
                .filter(|stack| prefer.is_some() && stack.id != prefer),
        );
    for stack in ordered {
        for segment in &stack.segments {
            if segment.commits.iter().any(|commit| commit.id == commit_id)
                && let Some(ref_name) = segment.ref_name()
            {
                return Some(ref_name.to_owned());
            }
        }
    }
    None
}

/// All branches whose workspace commits touch `path` — the dependency
/// suspects when hunk-level locks cannot pinpoint one. Callers filter out the
/// branch being committed to. Only runs on failure paths, so the per-commit
/// tree diffs are acceptable.
fn branches_touching_path(
    repo: &gix::Repository,
    ws: &Workspace,
    path: &BStr,
) -> Vec<gix::refs::FullName> {
    let mut branches = Vec::new();
    for stack in &ws.stacks {
        for segment in &stack.segments {
            let touches = segment.commits.iter().any(|commit| {
                but_core::diff::tree_changes(repo, commit.parent_ids.first().copied(), commit.id)
                    .map(|changes| changes.iter().any(|change| change.path == path))
                    .unwrap_or(false)
            });
            if touches
                && let Some(ref_name) = segment.ref_name()
                && !branches
                    .iter()
                    .any(|name: &gix::refs::FullName| name.as_ref() == ref_name)
            {
                branches.push(ref_name.to_owned());
            }
        }
    }
    branches
}

/// Whether two hunks touch the same lines on the new (worktree) side, which is
/// the coordinate space both the spec hunk and the dependency hunk share.
///
/// Zero-length ranges (pure deletions) are treated as covering a single line so
/// that they still match an overlapping hunk.
fn hunks_overlap(a: &HunkHeader, b: &HunkHeader) -> bool {
    let end_a = a.new_start.saturating_add(a.new_lines.max(1));
    let end_b = b.new_start.saturating_add(b.new_lines.max(1));
    a.new_start < end_b && b.new_start < end_a
}
