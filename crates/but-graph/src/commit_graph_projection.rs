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

use std::collections::{HashMap, HashSet};

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
    /// The remote-tracking branch ref for this segment's branch, if any (e.g. `refs/remotes/origin/A`).
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// Commits the remote-tracking branch is ahead by (only on the remote, not locally), tip-first.
    pub commits_on_remote: Vec<gix::ObjectId>,
}

impl SegmentRun {
    /// A run with the given name and commits; enrichment fields (remote tracking, remote-ahead commits)
    /// are filled later from gathered data.
    fn new(ref_name: Option<gix::refs::FullName>, commits: Vec<gix::ObjectId>) -> Self {
        SegmentRun {
            ref_name,
            commits,
            remote_tracking_ref_name: None,
            commits_on_remote: Vec::new(),
        }
    }
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
    /// One per stack top, in order.
    pub stacks: Vec<GatheredStack>,
}

/// A single stack's gathered facts: its target-relative base, whether it is metadata-tracked, and its
/// first-parent spine sliced into segments.
#[derive(Debug, Clone)]
pub struct GatheredStack {
    /// The stack's target-relative base (the commit below its last segment).
    pub base: Option<gix::ObjectId>,
    /// Whether the stack has a metadata branch list (an in-workspace tracked branch). A tracked stack
    /// is kept even when empty (a placeholder); an untracked, fully-integrated stack is dropped.
    pub tracked: bool,
    /// The stack's segments, tip-first.
    pub segments: Vec<SegmentRun>,
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
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> ProjectionData {
    let stack_tops: Vec<_> = cg.parents(workspace_commit).collect();
    // Each stack's base is its OWN merge base with the target (origin/main): a stack's commits stop
    // where it forks from the target, so commits shared with the target/remote are excluded (they
    // belong to `commits_on_remote`/outside, not the segment). Without a target, fall back to the
    // global merge base — where the stacks converge. A lone stack has no convergence, so its base is
    // the root (`None`); `merge_base` of one top would degenerately return the top itself.
    let global_base = if stack_tops.len() >= 2 {
        merge_base(cg, &stack_tops)
    } else {
        None
    };
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
    // The other stacks' tops. A commit that is a sibling stack's top is absorbed (that stack owns it),
    // rather than starting a nested segment inside this stack. A non-top ref, by contrast, forms a
    // shared segment. (Passing all tops is fine: this stack's own top sits at spine index 0, which is
    // never a mid-spine split candidate.)
    let all_tops: HashSet<gix::ObjectId> = stack_tops.iter().copied().collect();
    let stacks = stack_tops
        .iter()
        .enumerate()
        .map(|(i, &top)| {
            let stack_base = match target {
                // The base is the first-parent FORK POINT — the first commit on the stack's
                // first-parent spine that is contained in the target. The general merge-base can sit
                // off the first-parent line (reached via a merge's second parent), which would let the
                // spine overshoot past its real base.
                Some(t) => fork_point(cg, top, t).or(global_base),
                None => global_base,
            };
            let mut segments = match &aligned[i] {
                // Metadata-driven: the stack's branch list defines the segments and their names.
                Some(branches) => segment_by_branches(
                    cg,
                    top,
                    stack_base,
                    branches,
                    &meta_branches,
                    &all_tops,
                    remote_tracking,
                ),
                // No metadata: fall back to slicing at each disambiguated local-branch ref on the spine.
                None => segment_runs(
                    cg,
                    top,
                    stack_base,
                    &meta_branches,
                    &all_tops,
                    remote_tracking,
                ),
            };
            // A checkout inside a stack forces a segment boundary at the entrypoint commit — there is
            // always a segment starting there, even where nothing would otherwise split.
            segments = split_at_entrypoint(
                cg,
                segments,
                cg.entrypoint(),
                cg.entrypoint_ref(),
                &meta_branches,
                remote_tracking,
            );
            // Enrichment: attach each named segment's remote-tracking branch (a pure lookup by ref).
            for seg in &mut segments {
                seg.remote_tracking_ref_name = seg
                    .ref_name
                    .as_ref()
                    .and_then(|rn| remote_tracking.get(rn).cloned());
            }
            // Each segment's remote tip (tip-first, so index order = stack depth).
            let remote_tips: Vec<Option<gix::ObjectId>> = segments
                .iter()
                .map(|s| {
                    s.remote_tracking_ref_name
                        .as_ref()
                        .and_then(|rn| cg.commit_by_ref(rn.as_ref()))
                })
                .collect();
            for (i, seg) in segments.iter_mut().enumerate() {
                if let Some(remote_ref) = &seg.remote_tracking_ref_name
                    && let Some(local_tip) = seg.commits.first().copied()
                {
                    // Exclude commits owned by remotes of segments BELOW this one (deeper in the stack):
                    // a shared remote commit belongs to the lowest segment that reaches it, so a higher
                    // segment must not double-list it.
                    let below: Vec<gix::ObjectId> =
                        remote_tips[i + 1..].iter().flatten().copied().collect();
                    seg.commits_on_remote = commits_on_remote(cg, remote_ref, local_tip, &below);
                }
            }
            GatheredStack {
                base: stack_base,
                tracked: aligned[i].is_some(),
                segments,
            }
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
        // Drop an UNTRACKED stack with no commits: its tip is at/below its base, i.e. fully integrated
        // into the target (an integrated sibling). A metadata-tracked stack is kept even when empty (a
        // placeholder branch), and empty *branches* within an otherwise non-empty stack are kept.
        .filter(|s| s.tracked || s.segments.iter().any(|seg| !seg.commits.is_empty()))
        .map(|s| StackView {
            segments: s.segments,
            base: s.base,
        })
        .collect()
}

/// Gather, then build.
pub fn project(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    target: Option<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Vec<StackView> {
    build(gather(
        cg,
        workspace_commit,
        stack_branches,
        target,
        remote_tracking,
    ))
}

/// Self-contained entry: build a [`CommitGraph`] straight from `repo` and project it, deriving the
/// enrichment inputs (each in-workspace stack's branch list, the target, and the remote-tracking map)
/// from the repository and its ref metadata — the same inputs the segment-graph path takes. This is
/// the shape in which the projection can replace the segment graph: given only `(repo, meta)`, produce
/// the display stacks.
pub fn project_from_repository<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
) -> anyhow::Result<Vec<StackView>> {
    let cg = CommitGraph::from_repository(repo)?;
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let ws_commit = repo
        .find_reference(&ws_ref)?
        .peel_to_commit()?
        .id()
        .detach();

    let ws_meta = meta.workspace(ws_ref.as_ref())?;
    // Each in-workspace stack's ordered branch refs.
    let stack_branches: Vec<Vec<gix::refs::FullName>> = ws_meta
        .stacks
        .iter()
        .filter(|s| s.is_in_workspace())
        .map(|s| s.branches.iter().map(|b| b.ref_name.clone()).collect())
        .collect();
    // The target that bounds each stack's base: the metadata target ref, else `origin/main`.
    let target = ws_meta
        .project_meta()
        .target_ref
        .or_else(|| "refs/remotes/origin/main".try_into().ok())
        .and_then(|tr| {
            Some(
                repo.find_reference(&tr)
                    .ok()?
                    .peel_to_commit()
                    .ok()?
                    .id()
                    .detach(),
            )
        });
    let remote_tracking = remote_tracking_from_repository(repo, &ws_meta.project_meta())?;

    Ok(project(
        &cg,
        ws_commit,
        Some(&stack_branches),
        target,
        &remote_tracking,
    ))
}

/// Local branch -> its remote-tracking branch, mirroring the walk's
/// `lookup_remote_tracking_branch_or_deduce_it`:
/// 1. A branch CONFIGURED in git (`branch.<name>.remote`/`merge`) tracks that remote branch.
/// 2. Otherwise the relationship is deduced by name (`refs/remotes/<remote>/<X>` for `refs/heads/<X>`),
///    but ONLY against remotes the workspace configuration implies — the `push_remote` (highest
///    priority: "the push-remote overrides the remote we use for listing, even if a fetch remote is
///    available"), then the remote of the configured `target_ref`. A workspace with neither deduces
///    NO name-based relationships at all.
pub(crate) fn remote_tracking_from_repository(
    repo: &gix::Repository,
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<HashMap<gix::refs::FullName, gix::refs::FullName>> {
    let mut remotes: Vec<String> = Vec::new();
    if let Some(push_remote) = project_meta.push_remote.as_deref() {
        remotes.push(push_remote.to_string());
    }
    if let Some(target_ref) = project_meta.target_ref.as_ref()
        && let Some((remote, _short)) =
            but_core::extract_remote_name_and_short_name(target_ref.as_ref(), &repo.remote_names())
        && !remotes.contains(&remote)
    {
        remotes.push(remote);
    }

    let remote_refs: Vec<gix::refs::FullName> = repo
        .references()?
        .all()?
        .filter_map(Result::ok)
        .filter(|r| r.name().as_bstr().starts_with(b"refs/remotes/"))
        .map(|r| r.name().to_owned())
        .collect();
    let mut map = HashMap::new();
    // Name-deduction against the symbolic remotes.
    for remote in remotes {
        let prefix = format!("refs/remotes/{remote}/");
        for name in &remote_refs {
            if let Some(short) = name.as_bstr().strip_prefix(prefix.as_bytes()) {
                let local = format!("refs/heads/{}", String::from_utf8_lossy(short));
                if let Ok(local_ref) = gix::refs::FullName::try_from(local) {
                    // The first (highest-priority) remote to claim a local branch wins.
                    map.entry(local_ref).or_insert_with(|| name.clone());
                }
            }
        }
    }
    // Git-configured tracking branches win over name-deduction.
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let local = reference.name().to_owned();
        if let Some(Ok(rt)) =
            repo.branch_remote_tracking_ref_name(local.as_ref(), gix::remote::Direction::Fetch)
            && remote_refs.iter().any(|r| r.as_ref() == rt.as_ref())
        {
            map.insert(local, rt.into_owned());
        }
    }
    Ok(map)
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
    meta_branches: &HashSet<gix::refs::FullName>,
    sibling_tops: &HashSet<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
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
            SegmentRun::new(ref_name, commits)
        })
        // A metadata segment may still enclose a SHARED segment below it — a commit carrying a
        // (non-sibling-top) branch ref that another stack also passes through, e.g. a shared base
        // branch. Split those out; a sibling stack's own top is left absorbed.
        .flat_map(|run| {
            split_run_at_shared_refs(cg, run, meta_branches, sibling_tops, remote_tracking)
        })
        .collect()
}

/// Force a segment boundary at the `entrypoint` commit: the segment enclosing it is split so the
/// entrypoint begins its own segment (unless it already starts one). The upper part keeps the original
/// name; the lower part is named by normal disambiguation of the entrypoint commit (anonymous if
/// ambiguous). Mirrors "there is always a segment starting at the entrypoint".
fn split_at_entrypoint(
    cg: &CommitGraph,
    segments: Vec<SegmentRun>,
    entrypoint: Option<gix::ObjectId>,
    entrypoint_ref: Option<&gix::refs::FullName>,
    meta_branches: &HashSet<gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Vec<SegmentRun> {
    let Some(ep) = entrypoint else {
        return segments;
    };
    // A checked-out ref names the entrypoint segment (even if otherwise ambiguous); else disambiguate.
    let ep_name = || {
        entrypoint_ref
            .cloned()
            .or_else(|| disambiguated_branch_ref(cg, ep, meta_branches, remote_tracking))
    };
    let mut out = Vec::with_capacity(segments.len() + 1);
    for seg in segments {
        match seg.commits.iter().position(|&c| c == ep) {
            // Split only when the entrypoint sits BELOW the segment's tip (pos 0 is already a boundary).
            Some(pos) if pos > 0 => {
                let (above, below) = seg.commits.split_at(pos);
                out.push(SegmentRun::new(seg.ref_name, above.to_vec()));
                out.push(SegmentRun::new(ep_name(), below.to_vec()));
            }
            _ => out.push(seg),
        }
    }
    out
}

/// Split one segment run wherever a commit below its first carries a disambiguated local-branch ref
/// that is not a sibling stack top — a shared segment. The first sub-run keeps the run's own name;
/// each split sub-run takes the disambiguated ref's name.
fn split_run_at_shared_refs(
    cg: &CommitGraph,
    run: SegmentRun,
    meta_branches: &HashSet<gix::refs::FullName>,
    sibling_tops: &HashSet<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Vec<SegmentRun> {
    let mut runs = Vec::new();
    let mut current = SegmentRun::new(run.ref_name, Vec::new());
    for (i, &c) in run.commits.iter().enumerate() {
        if i > 0
            && !sibling_tops.contains(&c)
            && let Some(rn) = disambiguated_branch_ref(cg, c, meta_branches, remote_tracking)
        {
            runs.push(std::mem::replace(
                &mut current,
                SegmentRun::new(Some(rn), Vec::new()),
            ));
        }
        current.commits.push(c);
    }
    runs.push(current);
    runs
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
    sibling_tops: &HashSet<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Vec<SegmentRun> {
    let mut runs = Vec::new();
    let mut current = SegmentRun::new(
        disambiguated_branch_ref(cg, top, meta_branches, remote_tracking),
        Vec::new(),
    );
    let mut id = Some(top);
    while let Some(c) = id {
        if Some(c) == base {
            break;
        }
        if c != top
            && !sibling_tops.contains(&c)
            && let Some(rn) = disambiguated_branch_ref(cg, c, meta_branches, remote_tracking)
        {
            // `c` is the tip of a new segment.
            runs.push(std::mem::replace(
                &mut current,
                SegmentRun::new(Some(rn), Vec::new()),
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

/// The unambiguous local-branch name at `c`, mirroring the segment graph's disambiguation: prefer the
/// single branch *with metadata*, else the single branch *with a remote-tracking branch*, else the
/// single branch overall. When several branches compete and none (or more than one) is distinguished
/// at a tier, the commit is ambiguous and gets no name (`None`) — it does not start a new segment,
/// folding into the run above it.
fn disambiguated_branch_ref(
    cg: &CommitGraph,
    c: gix::ObjectId,
    meta_branches: &HashSet<gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Option<gix::refs::FullName> {
    let branches: Vec<gix::refs::FullName> = cg
        .refs_at(c)
        .into_iter()
        .filter(is_plain_local_branch)
        .collect();
    let unique_by = |pred: &dyn Fn(&gix::refs::FullName) -> bool| {
        let mut it = branches.iter().filter(|rn| pred(rn));
        it.next().filter(|_| it.next().is_none()).cloned()
    };
    unique_by(&|rn| meta_branches.contains(rn))
        .or_else(|| unique_by(&|rn| remote_tracking.contains_key(rn)))
        .or_else(|| unique_by(&|_| true))
}

/// A plain (non-`gitbutler/*`) local branch ref.
fn is_plain_local_branch(rn: &gix::refs::FullName) -> bool {
    let rn = rn.as_ref();
    rn.category() == Some(Category::LocalBranch)
        && !rn.as_bstr().starts_with_str("refs/heads/gitbutler/")
}

/// The commits a segment's remote-tracking branch is ahead by: walking the remote tip's first-parent
/// spine and collecting commits until one is reachable from the segment's local tip. Empty when the
/// remote ref is absent from the graph or the remote is not ahead.
fn commits_on_remote(
    cg: &CommitGraph,
    remote_ref: &gix::refs::FullName,
    local_tip: gix::ObjectId,
    other_remote_tips: &[gix::ObjectId],
) -> Vec<gix::ObjectId> {
    let Some(remote_tip) = cg.commit_by_ref(remote_ref.as_ref()) else {
        return Vec::new();
    };
    // Every commit reachable from the remote tip but NOT locally — a full reachability difference, so
    // commits on a merge's second parent (only on the remote) are included, not just the first-parent
    // spine. Commits owned by ANOTHER remote segment (reachable from its tip) are excluded so a shared
    // remote commit isn't double-listed. Order newest-first by generation (the segment graph's
    // gen-then-time; we lack commit time, so tie-break by id for determinism).
    let mut excluded = ancestors(cg, local_tip);
    for &other in other_remote_tips {
        excluded.extend(ancestors(cg, other));
    }
    let mut out: Vec<gix::ObjectId> = ancestors(cg, remote_tip)
        .into_iter()
        .filter(|c| !excluded.contains(c))
        .collect();
    out.sort_by_key(|&c| {
        (
            std::cmp::Reverse(cg.node(c).map(|n| n.generation).unwrap_or(0)),
            c,
        )
    });
    out
}

/// The first-parent fork point of `top` against `target`: walking `top`'s first-parent spine, the
/// first commit that is contained in the target's history (an ancestor of, or equal to, `target`).
/// This is where the stack's first-parent line rejoins the target — its base. Unlike the general
/// merge-base it never lands off the first-parent spine (which would let the spine overshoot).
fn fork_point(
    cg: &CommitGraph,
    top: gix::ObjectId,
    target: gix::ObjectId,
) -> Option<gix::ObjectId> {
    let target_ancestors = ancestors(cg, target);
    let mut id = Some(top);
    while let Some(c) = id {
        if target_ancestors.contains(&c) {
            return Some(c);
        }
        id = cg.first_parent(c);
    }
    None
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

        let stacks = project(&cg, oid(0xff), None, None, &Default::default());
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
        let stacks = project(&cg, oid(0xff), None, None, &Default::default());
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
        let stacks = project(&cg, oid(0xff), Some(&branches), None, &Default::default());
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
