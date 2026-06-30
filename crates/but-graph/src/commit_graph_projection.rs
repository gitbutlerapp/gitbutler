//! SPIKE (commit-graph-experiment): build the display projection — stacks of segments — straight
//! from a [`CommitGraph`], in two clean phases:
//!
//! 1. [`gather`] — a *pure* read of the commit graph that produces immutable [`ProjectionData`]:
//!    the workspace commit, the stack tops (its parents, **in order** — the stack order for free),
//!    the base where they converge, and each stack's first-parent spine sliced into segments.
//! 2. [`build`] — a *single pass* that assembles the output from that data.
//!
//! This deliberately replaces the segment graph's collect → enrich → prune → mark *mutation* passes:
//! every fact the build needs is computed up front as data, then the stacks are constructed once.
//! Enrichment that today runs as extra passes (remote reachability, integrated/archived pruning,
//! target/lower-bound) becomes additional *fields gathered in phase 1*, not passes in phase 2.
//!
//! Scope of this spike: the core stack/segment grouping for a managed, multi-stack workspace.
//! Boundary rules mirrored from the real projection: a stack top is a workspace-commit parent;
//! a new segment begins at a commit carrying a non-special local-branch ref (`refs/heads/gitbutler/*`
//! continues through); the spine stops at the base. The entrypoint/sibling-segment splits and the
//! enrichment passes are intentionally out of scope here.

#![allow(dead_code)]

use std::collections::HashSet;

use bstr::ByteSlice;
use gix::reference::Category;

use crate::CommitGraph;

/// One segment of a stack: the local-branch ref at its tip (if named) and its commits, tip-first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentRun {
    /// The non-special local-branch ref pointing at the segment's tip, if any.
    pub ref_name: Option<gix::refs::FullName>,
    /// The segment's commits, tip-first.
    pub commits: Vec<gix::ObjectId>,
}

/// A stack: its segments, tip-first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackView {
    /// The stack's segments, tip-first.
    pub segments: Vec<SegmentRun>,
}

/// Immutable facts gathered from the commit graph in phase 1, before any output is built.
#[derive(Debug, Clone)]
pub struct ProjectionData {
    /// The managed workspace (octopus merge) commit.
    pub workspace_commit: gix::ObjectId,
    /// Stack tops = the workspace commit's parents, IN ORDER (the stack order, for free).
    pub stack_tops: Vec<gix::ObjectId>,
    /// Where the stacks converge — the merge base of the tops. Segments stop here.
    pub base: Option<gix::ObjectId>,
    /// Per stack top, its first-parent spine sliced into segments at local-branch refs.
    pub stacks: Vec<Vec<SegmentRun>>,
}

/// Phase 1 — GATHER: read the commit graph (and, if given, each stack's ordered branch names) and
/// compute every fact, with no mutation. `stack_branches` is enrichment *data* — the in-workspace
/// stacks' branch lists, in the same order as the stack tops (the workspace commit's parent array
/// is kept in metadata order). It lets empty branches (no unique commits) be placed; it does not
/// drive a second pass. Passing a minimal `[[FullName]]` keeps this decoupled from the metadata type.
pub fn gather(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    target: Option<gix::ObjectId>,
) -> ProjectionData {
    let stack_tops: Vec<_> = cg.parents(workspace_commit).collect();
    // The base is the merge base of the stack tops AND the target (origin/main). The target is what
    // bounds a single stack — without it, merge_base of one top is the top itself.
    let anchors: Vec<_> = stack_tops.iter().copied().chain(target).collect();
    let base = merge_base(cg, &anchors);
    let stacks = stack_tops
        .iter()
        .enumerate()
        .map(|(i, &top)| {
            let spine = segment_runs(cg, top, base);
            match stack_branches.and_then(|b| b.get(i)) {
                Some(branches) => reconcile_with_branches(spine, branches),
                None => spine,
            }
        })
        .collect();
    ProjectionData {
        workspace_commit,
        stack_tops,
        base,
        stacks,
    }
}

/// Phase 2 — BUILD: assemble the output in a single pass from the gathered data.
pub fn build(data: ProjectionData) -> Vec<StackView> {
    data.stacks
        .into_iter()
        .map(|segments| StackView { segments })
        .collect()
}

/// Gather, then build.
pub fn project(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    target: Option<gix::ObjectId>,
) -> Vec<StackView> {
    build(gather(cg, workspace_commit, stack_branches, target))
}

/// Walk the stack's ordered `branches`, taking each commit-bearing segment from `spine` when its ref
/// matches, and emitting an empty segment for a branch the spine doesn't cover (an empty branch).
/// Spine segments whose ref isn't among `branches` are appended as-is.
fn reconcile_with_branches(
    spine: Vec<SegmentRun>,
    branches: &[gix::refs::FullName],
) -> Vec<SegmentRun> {
    let mut spine = spine.into_iter().peekable();
    let mut out = Vec::new();
    for branch in branches {
        if spine
            .peek()
            .is_some_and(|s| s.ref_name.as_ref() == Some(branch))
        {
            out.push(spine.next().expect("peeked"));
        } else {
            out.push(SegmentRun {
                ref_name: Some(branch.clone()),
                commits: Vec::new(),
            });
        }
    }
    out.extend(spine);
    out
}

/// Walk `top`'s first-parent spine down to (excluding) `base`, slicing into segments wherever a
/// commit carries a non-special local-branch ref — that ref names the new segment.
fn segment_runs(
    cg: &CommitGraph,
    top: gix::ObjectId,
    base: Option<gix::ObjectId>,
) -> Vec<SegmentRun> {
    let mut runs = Vec::new();
    let mut current = SegmentRun {
        ref_name: local_branch_ref(cg, top),
        commits: Vec::new(),
    };
    let mut id = Some(top);
    while let Some(c) = id {
        if Some(c) == base {
            break;
        }
        if c != top
            && let Some(rn) = local_branch_ref(cg, c)
        {
            // `c` is the tip of a new segment.
            runs.push(std::mem::replace(
                &mut current,
                SegmentRun {
                    ref_name: Some(rn),
                    commits: Vec::new(),
                },
            ));
        }
        current.commits.push(c);
        id = cg.first_parent(c);
    }
    runs.push(current);
    runs
}

/// The first non-special local-branch ref pointing at `c`, if any.
fn local_branch_ref(cg: &CommitGraph, c: gix::ObjectId) -> Option<gix::refs::FullName> {
    cg.refs_at(c).into_iter().find(|rn| {
        let rn = rn.as_ref();
        rn.category() == Some(Category::LocalBranch)
            && !rn.as_bstr().starts_with_str("refs/heads/gitbutler/")
    })
}

/// The merge base of `tops` — the highest-generation commit that is an ancestor of all of them.
fn merge_base(cg: &CommitGraph, tops: &[gix::ObjectId]) -> Option<gix::ObjectId> {
    let mut common: Option<HashSet<gix::ObjectId>> = None;
    for &top in tops {
        let anc = ancestors(cg, top);
        common = Some(match common {
            None => anc,
            Some(c) => c.intersection(&anc).copied().collect(),
        });
    }
    common?
        .into_iter()
        .max_by_key(|id| cg.node(*id).map(|n| n.generation).unwrap_or(0))
}

/// All ancestors of `start` (inclusive), walking every parent.
fn ancestors(cg: &CommitGraph, start: gix::ObjectId) -> HashSet<gix::ObjectId> {
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(c) = stack.pop() {
        if seen.insert(c) {
            stack.extend(cg.parents(c));
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Commit, CommitFlags, RefInfo};

    fn oid(b: u8) -> gix::ObjectId {
        let mut bytes = [0u8; 20];
        bytes[0] = b;
        gix::ObjectId::from_bytes_or_panic(&bytes)
    }

    fn commit(b: u8, parents: &[u8], ref_name: Option<&str>) -> Commit {
        Commit {
            id: oid(b),
            parent_ids: parents.iter().map(|&p| oid(p)).collect(),
            flags: CommitFlags::empty(),
            refs: ref_name
                .into_iter()
                .map(|n| RefInfo {
                    ref_name: n.try_into().expect("valid ref"),
                    commit_id: None,
                    worktree: None,
                })
                .collect(),
        }
    }

    /// Shape a projection into `[stack][segment] = (ref_name, [commit ids])` for assertions.
    fn shape(stacks: &[StackView]) -> Vec<Vec<(Option<String>, Vec<gix::ObjectId>)>> {
        stacks
            .iter()
            .map(|s| {
                s.segments
                    .iter()
                    .map(|seg| {
                        (
                            seg.ref_name.as_ref().map(|r| r.as_bstr().to_string()),
                            seg.commits.clone(),
                        )
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn stacks_order_segment_split_and_base_all_from_the_commit_graph() {
        // Workspace merges stack A (a2 -> a1) and stack B (b1), all on base b0.
        // a1 carries a second branch `child`, so stack A must split into two segments.
        let cg = CommitGraph::from_commits(
            [
                commit(0xff, &[0xa2, 0xb1], None), // workspace octopus merge
                commit(0xa2, &[0xa1], Some("refs/heads/A")),
                commit(0xa1, &[0xb0], Some("refs/heads/child")),
                commit(0xb1, &[0xb0], Some("refs/heads/B")),
                commit(0xb0, &[], None), // shared base
            ],
            Some(oid(0xff)),
        );

        let stacks = project(&cg, oid(0xff), None, None);
        assert_eq!(
            shape(&stacks),
            vec![
                // Stack A: tops first (parent order), split at the mid-spine `child` ref, stops at b0.
                vec![
                    (Some("refs/heads/A".into()), vec![oid(0xa2)]),
                    (Some("refs/heads/child".into()), vec![oid(0xa1)]),
                ],
                // Stack B: single segment down to the base.
                vec![(Some("refs/heads/B".into()), vec![oid(0xb1)])],
            ]
        );
    }

    #[test]
    fn special_gitbutler_refs_do_not_split_a_segment() {
        // a1 carries a special ref — the segment continues through it instead of splitting.
        let cg = CommitGraph::from_commits(
            [
                commit(0xff, &[0xa2, 0xb1], None),
                commit(0xa2, &[0xa1], Some("refs/heads/A")),
                commit(0xa1, &[0xb0], Some("refs/heads/gitbutler/edit")),
                commit(0xb1, &[0xb0], Some("refs/heads/B")),
                commit(0xb0, &[], None),
            ],
            Some(oid(0xff)),
        );
        let stacks = project(&cg, oid(0xff), None, None);
        // Stack A stays one segment spanning a2 and a1 (the special ref didn't split it).
        assert_eq!(
            shape(&stacks)[0],
            vec![(Some("refs/heads/A".into()), vec![oid(0xa2), oid(0xa1)])]
        );
    }

    #[test]
    fn empty_branches_from_metadata_are_placed_after_their_commit_bearing_segment() {
        // Stacks A (a1) and B (b1) on base b0; metadata says stack B also has an empty `below`.
        let cg = CommitGraph::from_commits(
            [
                commit(0xff, &[0xa1, 0xb1], None),
                commit(0xa1, &[0xb0], Some("refs/heads/A")),
                commit(0xb1, &[0xb0], Some("refs/heads/B")),
                commit(0xb0, &[], None),
            ],
            Some(oid(0xff)),
        );
        let branches: Vec<Vec<gix::refs::FullName>> = vec![
            vec!["refs/heads/A".try_into().expect("valid")],
            vec![
                "refs/heads/B".try_into().expect("valid"),
                "refs/heads/below".try_into().expect("valid"),
            ],
        ];
        let stacks = project(&cg, oid(0xff), Some(&branches), None);
        assert_eq!(
            shape(&stacks),
            vec![
                vec![(Some("refs/heads/A".into()), vec![oid(0xa1)])],
                vec![
                    (Some("refs/heads/B".into()), vec![oid(0xb1)]),
                    // `below` has no unique commits, so it lands as an empty segment.
                    (Some("refs/heads/below".into()), vec![]),
                ],
            ]
        );
    }
}
