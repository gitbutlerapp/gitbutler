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
    /// Where the stack rests — its own target-relative base (the commit below its last segment).
    /// Matches the segment graph's `Stack::base()`.
    pub base: Option<gix::ObjectId>,
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
    /// Per stack top, its target-relative base and first-parent spine sliced into segments.
    pub stacks: Vec<(Option<gix::ObjectId>, Vec<SegmentRun>)>,
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
    // Each stack's base is its OWN merge base with the target (origin/main): a stack's commits stop
    // where it forks from the target, so commits shared with the target/remote are excluded (they
    // belong to `commits_on_remote`/outside, not the segment). Without a target, fall back to the
    // global merge base of all tops (which also fixes a single top being its own merge base).
    let global_base = merge_base(cg, &stack_tops);
    // Metadata stacks aren't necessarily in the same order as the stack tops (the ws-commit parent
    // array), so match each top to the branch list one of its commits carries before zipping.
    let aligned = align_branches_to_tops(cg, &stack_tops, global_base, stack_branches);
    // Every branch known to the metadata, used to disambiguate a commit carrying competing refs
    // (mirrors the segment graph's `disambiguate_refs_by_branch_metadata`): a branch *with* metadata
    // wins over one without.
    let meta_branches: HashSet<gix::refs::FullName> = stack_branches
        .into_iter()
        .flatten()
        .flatten()
        .cloned()
        .collect();
    let stacks = stack_tops
        .iter()
        .enumerate()
        .map(|(i, &top)| {
            let stack_base = match target {
                Some(t) => merge_base(cg, &[top, t]),
                None => global_base,
            };
            let segments = match &aligned[i] {
                // Metadata-driven: the stack's branch list defines the segments and their names.
                Some(branches) => segment_by_branches(cg, top, stack_base, branches),
                // No metadata: fall back to slicing at each disambiguated local-branch ref on the spine.
                None => segment_runs(cg, top, stack_base, &meta_branches),
            };
            (stack_base, segments)
        })
        .collect();
    ProjectionData {
        workspace_commit,
        stack_tops,
        base: global_base,
        stacks,
    }
}

/// Phase 2 — BUILD: assemble the output in a single pass from the gathered data.
pub fn build(data: ProjectionData) -> Vec<StackView> {
    data.stacks
        .into_iter()
        // Drop a stack with no commits at all: its tip is at/below its base, i.e. fully integrated
        // into the target (all its commits are shared with, or reachable from, the target). Empty
        // *branches* within an otherwise non-empty stack are kept.
        .filter(|(_, segments)| segments.iter().any(|s| !s.commits.is_empty()))
        .map(|(base, segments)| StackView { segments, base })
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

/// Match each stack top to the branch list one of its spine commits carries, returning the lists in
/// stack-top order. A top whose spine carries none of any list's branches (e.g. an anonymous top)
/// takes a leftover list, in order. Returns all-`None` when there is no metadata.
fn align_branches_to_tops(
    cg: &CommitGraph,
    stack_tops: &[gix::ObjectId],
    base: Option<gix::ObjectId>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
) -> Vec<Option<Vec<gix::refs::FullName>>> {
    let Some(lists) = stack_branches else {
        return vec![None; stack_tops.len()];
    };
    let mut used = vec![false; lists.len()];
    let mut out: Vec<Option<Vec<gix::refs::FullName>>> = vec![None; stack_tops.len()];
    for (ti, &top) in stack_tops.iter().enumerate() {
        let spine_refs: HashSet<gix::refs::FullName> = first_parent_spine(cg, top, base)
            .iter()
            .flat_map(|&c| cg.refs_at(c))
            .collect();
        if let Some(bi) = lists
            .iter()
            .enumerate()
            .position(|(bi, branches)| !used[bi] && branches.iter().any(|b| spine_refs.contains(b)))
        {
            out[ti] = Some(lists[bi].clone());
            used[bi] = true;
        }
    }
    // Unmatched tops (anonymous) take the leftover lists in order.
    let mut leftover = (0..lists.len()).filter(|bi| !used[*bi]);
    for slot in out.iter_mut().filter(|s| s.is_none()) {
        if let Some(bi) = leftover.next() {
            *slot = Some(lists[bi].clone());
        }
    }
    out
}

/// Slice `top`'s first-parent spine into segments by the stack's ordered `branches` — metadata-driven,
/// not by arbitrary refs on commits. The first branch owns the tip; each later branch starts where its
/// ref appears on the spine (and is an empty segment if its ref appears nowhere). A segment's display
/// name is its branch iff that ref is on the segment's first commit (else it is anonymous, `None`);
/// empty segments keep their branch name.
fn segment_by_branches(
    cg: &CommitGraph,
    top: gix::ObjectId,
    base: Option<gix::ObjectId>,
    branches: &[gix::refs::FullName],
) -> Vec<SegmentRun> {
    let spine = first_parent_spine(cg, top, base);
    // Where each branch begins on the spine: the top branch at 0, each later branch at its ref (or
    // past the end — an empty segment — if its ref isn't on the spine).
    let positions: Vec<usize> = branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if i == 0 {
                0
            } else {
                spine
                    .iter()
                    .position(|&c| commit_has_ref(cg, c, b))
                    .unwrap_or(spine.len())
            }
        })
        .collect();
    branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let start = positions[i];
            let end = positions
                .get(i + 1)
                .copied()
                .unwrap_or(spine.len())
                .max(start);
            let commits = spine.get(start..end).unwrap_or(&[]).to_vec();
            // Name the segment after its branch when the branch's ref is on the tip, or the tip has no
            // competing refs (e.g. the branch's ref is on a commit outside the workspace). If the tip
            // carries *other* refs, the segment is ambiguous and shown anonymously.
            let ref_name = match commits.first() {
                None => Some(b.clone()),
                Some(&c) if commit_has_ref(cg, c, b) || cg.refs_at(c).is_empty() => Some(b.clone()),
                Some(_) => None,
            };
            SegmentRun { ref_name, commits }
        })
        .collect()
}

/// `top`'s first-parent commits down to (excluding) `base`.
fn first_parent_spine(
    cg: &CommitGraph,
    top: gix::ObjectId,
    base: Option<gix::ObjectId>,
) -> Vec<gix::ObjectId> {
    let mut spine = Vec::new();
    let mut id = Some(top);
    while let Some(c) = id {
        if Some(c) == base {
            break;
        }
        spine.push(c);
        id = cg.first_parent(c);
    }
    spine
}

/// Whether `ref_name` points at `commit`.
fn commit_has_ref(cg: &CommitGraph, commit: gix::ObjectId, ref_name: &gix::refs::FullName) -> bool {
    cg.refs_at(commit).iter().any(|r| r == ref_name)
}

/// Walk `top`'s first-parent spine down to (excluding) `base`, slicing into segments wherever a
/// commit carries a non-special local-branch ref — that ref names the new segment.
fn segment_runs(
    cg: &CommitGraph,
    top: gix::ObjectId,
    base: Option<gix::ObjectId>,
    meta_branches: &HashSet<gix::refs::FullName>,
) -> Vec<SegmentRun> {
    let mut runs = Vec::new();
    let mut current = SegmentRun {
        ref_name: disambiguated_branch_ref(cg, top, meta_branches),
        commits: Vec::new(),
    };
    let mut id = Some(top);
    while let Some(c) = id {
        if Some(c) == base {
            break;
        }
        if c != top
            && let Some(rn) = disambiguated_branch_ref(cg, c, meta_branches)
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
    cg.refs_at(c)
        .into_iter()
        .find(|rn| is_plain_local_branch(rn))
}

/// The unambiguous local-branch name at `c`, mirroring the segment graph's
/// `disambiguate_refs_by_branch_metadata`: prefer the single branch *with* metadata; else fall back
/// to the single branch overall. When several branches compete and none (or more than one) is
/// distinguished by metadata, the commit is ambiguous and gets no name (`None`) — it does not start a
/// new segment, folding into the run above it.
fn disambiguated_branch_ref(
    cg: &CommitGraph,
    c: gix::ObjectId,
    meta_branches: &HashSet<gix::refs::FullName>,
) -> Option<gix::refs::FullName> {
    let branches: Vec<gix::refs::FullName> = cg
        .refs_at(c)
        .into_iter()
        .filter(is_plain_local_branch)
        .collect();
    let mut with_meta = branches.iter().filter(|rn| meta_branches.contains(*rn));
    with_meta
        .next()
        .filter(|_| with_meta.next().is_none())
        .or_else(|| {
            let mut all = branches.iter();
            all.next().filter(|_| all.next().is_none())
        })
        .cloned()
}

/// A plain (non-`gitbutler/*`) local branch ref.
fn is_plain_local_branch(rn: &gix::refs::FullName) -> bool {
    let rn = rn.as_ref();
    rn.category() == Some(Category::LocalBranch)
        && !rn.as_bstr().starts_with_str("refs/heads/gitbutler/")
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
