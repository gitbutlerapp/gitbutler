//! Derive the stored [`RefLayout`](crate::ref_layout::RefLayout) from the [`SegmentData`] —
//! the build's ONE topology→truth derivation. The segments are linearized into an entry graph (per
//! segment: its name ref, then per commit its refs and the commit), each reference's
//! placement (`on`, the NAME below it, its entering edges) is read off that topology, and
//! the layout authors its groups from those parts. The positions computed here are
//! scaffolding, not output: nothing per-ref is stored, the layout's queries answer
//! positions back from the groups.

use std::collections::HashMap;

use anyhow::{Context as _, Result};

use super::IdMap;
use super::segment_data::{OrderedTargets, SegmentData};
use crate::ref_layout::{MaterializedWsParents, Placement, RefFacts, RefLayout, RefPart};

/// One entry in a row's linearization.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// Index into `ref_table`.
    Ref(usize),
    /// Index into `commits`.
    Commit(usize),
    /// The placeholder for a row with neither name nor commits.
    None,
}

/// The scaffolding placements are read off: every segment linearized into entries —
/// the segment's name ref first, then per commit its passive refs and the commit
/// itself — wired into one graph by the parent edges. Positions fall out of walks over
/// it (down the first edges for `on` and below, up for the entering edges). Shared by
/// the phases of [`positions_from_segments`], and dropped once the layout is authored.
struct EntryGraph<'a> {
    entries: Vec<Entry>,
    parents: Vec<Vec<usize>>,
    /// `(name, reachable, names-a-segment: Some(segment-is-empty) | None for passive refs)`
    ref_table: Vec<(gix::refs::FullName, bool, Option<bool>)>,
    commits: Vec<(gix::ObjectId, &'a [gix::ObjectId])>,
    /// Commit `c`'s entry handle, positionally — the id-keyed map only serves parent-id lookups.
    commit_entry_at: Vec<usize>,
    commit_entry: IdMap<usize>,
    reachable_commits: Vec<gix::ObjectId>,
    head_refs: Vec<gix::refs::FullName>,
    segment_rows: Vec<Vec<usize>>,
}

/// A ref's position over the entry topology, in entry handles.
struct DerivedPosition {
    on: usize,
    below: Option<usize>,
    ambiguous: bool,
    entering: Vec<(usize, usize)>,
}

/// Derive the stored layout from the segments in `data`. The managed-workspace decision comes from
/// `cg`, which recognises managed commits by message.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn derive_ref_layout(data: &SegmentData, cg: &crate::CommitGraph) -> Result<RefLayout> {
    let entrypoint_sidx = data
        .entrypoint_sidx
        .context("BUG: must always set the entrypoint")?;
    let workspace_commit_id = data.entrypoint.filter(|&id| cg.is_managed_ws_commit(id));
    Ok(positions_from_segments(
        cg,
        &data.segments,
        &data.extra_refs,
        &data.parent_ordered_targets(cg),
        entrypoint_sidx,
        workspace_commit_id,
    ))
}

/// Compute the layout from the segments: per-segment entry rows (segment ref, then per commit its refs then the
/// commit), the parent fixup (a commit whose group-flattened parents disagree with its raw
/// parent list is rewired directly, bypassing groups — the ws commit and partially-traversed
/// commits keep their wiring), position derivation, and the parent resolution.
///
/// `workspace_commit_id` is the managed entrypoint commit, which takes its resolved CHAIN
/// materialized parents instead of its real ones.
fn positions_from_segments(
    cg: &crate::CommitGraph,
    segments: &[super::segment_data::Segment],
    extra_refs: &HashMap<usize, Vec<gix::refs::FullName>>,
    targets_of_segment: &OrderedTargets,
    entrypoint_sidx: usize,
    workspace_commit_id: Option<gix::ObjectId>,
) -> RefLayout {
    let mut rows = build_entry_graph(
        cg,
        segments,
        extra_refs,
        targets_of_segment,
        entrypoint_sidx,
    );
    wire_segment_edges(&mut rows, targets_of_segment);
    apply_parent_fixup(&mut rows, workspace_commit_id);
    let ref_entries = collect_ref_entries(&rows.entries);
    let mut positions = derive_positions(&rows, &ref_entries);
    let ResolvedParents {
        materialized_ws_parents,
        full_incoming,
    } = resolve_parents(&rows, workspace_commit_id, &mut positions);
    // The oracle in id space: entry handles become commit ids, sorted like the rows'
    // entering edges. This is the edge universe the EDITOR sees after ingest (the ws
    // commit's materialized parents, unresolvable parents dropped) — so a group classified
    // `All` here is `All` there.
    let incoming_by_id: HashMap<gix::ObjectId, Vec<(gix::ObjectId, usize)>> = full_incoming
        .iter()
        .filter_map(|(&handle, edges)| {
            let Entry::Commit(c) = rows.entries[handle] else {
                return None;
            };
            let mut edges: Vec<_> = edges
                .iter()
                .filter_map(|&(child, parent_number)| {
                    let Entry::Commit(cc) = rows.entries[child] else {
                        return None;
                    };
                    Some((rows.commits[cc].0, parent_number))
                })
                .collect();
            edges.sort_unstable();
            edges.dedup();
            Some((rows.commits[c].0, edges))
        })
        .collect();
    let incoming = |on: gix::ObjectId| -> Vec<(gix::ObjectId, usize)> {
        incoming_by_id.get(&on).cloned().unwrap_or_default()
    };
    assemble_layout(
        rows,
        &ref_entries,
        &positions,
        materialized_ws_parents,
        &incoming,
    )
}

/// Entry build: one row of entries per segment, in table order.
fn build_entry_graph<'a>(
    cg: &'a crate::CommitGraph,
    segments: &[super::segment_data::Segment],
    extra_refs: &HashMap<usize, Vec<gix::refs::FullName>>,
    targets_of_segment: &OrderedTargets,
    entrypoint_sidx: usize,
) -> EntryGraph<'a> {
    // A ref that names a segment (or is a named segment's remote-tracking counterpart)
    // stays out of the commits' own ref lists, as does every remote-category ref.
    let segment_names: std::collections::HashSet<&gix::refs::FullName> = segments
        .iter()
        .flat_map(|r| {
            r.name
                .as_ref()
                .into_iter()
                .chain(r.remote_tracking_ref_name.as_ref())
        })
        .collect();
    let commit_ref_names = |h: usize| -> Vec<&gix::refs::FullName> {
        let mut names: Vec<&gix::refs::FullName> = cg
            .node_at(h)
            .refs
            .iter()
            .map(|ri| &ri.ref_name)
            .filter(|name| {
                !segment_names.contains(name)
                    && name.category() != Some(gix::reference::Category::RemoteBranch)
            })
            .collect();
        if let Some(extra) = extra_refs.get(&h) {
            // Same filter as above: a displaced name a later pass (the ad-hoc same-tip
            // rebuild) turned into a segment name must not ALSO ride as a commit ref.
            for name in extra {
                if !segment_names.contains(name) && !names.contains(&name) {
                    names.push(name);
                }
            }
            names.sort();
        }
        names
    };
    // Reach: every segment the entrypoint's segment descends into through the parent-ordered edges.
    let mut reachable_sidx_at = vec![false; segments.len()];
    let mut queue = vec![entrypoint_sidx];
    while let Some(ri) = queue.pop() {
        if !std::mem::replace(&mut reachable_sidx_at[ri], true) {
            queue.extend(targets_of_segment.of(ri));
        }
    }
    let head_name = segments[entrypoint_sidx].ref_name().map(|n| n.to_owned());

    let mut entries: Vec<Entry> = Vec::new();
    let mut parents: Vec<Vec<usize>> = Vec::new();
    let mut ref_table: Vec<(gix::refs::FullName, bool, Option<bool>)> = Vec::new();
    let mut commits: Vec<(gix::ObjectId, &[gix::ObjectId])> = Vec::new();
    let mut commit_entry_at: Vec<usize> = Vec::new();
    let mut commit_entry = IdMap::<usize>::default();
    let mut reachable_commits = Vec::new();
    let mut head_refs = Vec::new();
    let mut segment_rows = Vec::new();

    for (ri, data) in segments.iter().enumerate() {
        let reachable = reachable_sidx_at[ri];
        let mut row: Vec<usize> = vec![];
        let push = |entries: &mut Vec<Entry>, parents: &mut Vec<Vec<usize>>, entry| {
            entries.push(entry);
            parents.push(vec![]);
            entries.len() - 1
        };

        if let Some(reference) = data.ref_name() {
            if head_name.as_ref().map(|n| n.as_ref()) == Some(reference) {
                head_refs.push(reference.to_owned());
            }
            ref_table.push((
                reference.to_owned(),
                reachable,
                Some(data.commits.is_empty()),
            ));
            let n = push(&mut entries, &mut parents, Entry::Ref(ref_table.len() - 1));
            row.push(n);
        }
        for &h in &data.commits {
            let commit = cg.node_at(h);
            if reachable {
                reachable_commits.push(commit.id);
            }
            for name in commit_ref_names(h) {
                ref_table.push((name.clone(), reachable, None));
                let n = push(&mut entries, &mut parents, Entry::Ref(ref_table.len() - 1));
                if let Some(&previous) = row.last() {
                    parents[previous].push(n);
                }
                row.push(n);
            }
            commits.push((commit.id, commit.parent_ids.as_slice()));
            let n = push(&mut entries, &mut parents, Entry::Commit(commits.len() - 1));
            commit_entry_at.push(n);
            commit_entry.insert(commit.id, n);
            if let Some(&previous) = row.last() {
                parents[previous].push(n);
            }
            row.push(n);
        }
        if row.is_empty() {
            row.push(push(&mut entries, &mut parents, Entry::None));
        }
        segment_rows.push(row);
    }

    EntryGraph {
        entries,
        parents,
        ref_table,
        commits,
        commit_entry_at,
        commit_entry,
        reachable_commits,
        head_refs,
        segment_rows,
    }
}

/// The parent-ordered edges land on each row's LAST entry, pointing at the target row's FIRST entry.
fn wire_segment_edges(rows: &mut EntryGraph<'_>, targets_of_segment: &OrderedTargets) {
    let EntryGraph {
        parents,
        segment_rows,
        ..
    } = rows;
    let first_entry_of_row: Vec<usize> = segment_rows
        .iter()
        .map(|row| *row.first().expect("every row has an entry"))
        .collect();
    for (ri, row) in segment_rows.iter().enumerate() {
        let source = *row.last().expect("every row has an entry");
        for &target in targets_of_segment.of(ri) {
            parents[source].push(first_entry_of_row[target]);
        }
    }
}

/// The fixup: flatten a commit's group parents in parent number order; on disagreement with the
/// RAW parent list, rewire directly to present commits (groups lose their edges). The ws
/// commit and partially-traversed commits keep their segment wiring. The flattening
/// compares in-place against the raw list — no per-commit id collection.
fn apply_parent_fixup(rows: &mut EntryGraph<'_>, workspace_commit_id: Option<gix::ObjectId>) {
    let EntryGraph {
        entries,
        parents,
        commits,
        commit_entry_at,
        commit_entry,
        ..
    } = rows;
    let mut stack: Vec<usize> = Vec::new();
    let flatten_matches = |entries: &[Entry],
                           parents: &[Vec<usize>],
                           commits: &[(gix::ObjectId, &[gix::ObjectId])],
                           start: usize,
                           want: &[gix::ObjectId],
                           stack: &mut Vec<usize>|
     -> bool {
        stack.clear();
        stack.extend(parents[start].iter().rev().copied());
        let mut matched = 0;
        while let Some(n) = stack.pop() {
            match entries[n] {
                Entry::Commit(c) => {
                    if want.get(matched) != Some(&commits[c].0) {
                        return false;
                    }
                    matched += 1;
                }
                Entry::Ref(_) | Entry::None => {
                    stack.extend(parents[n].iter().rev().copied());
                }
            }
        }
        matched == want.len()
    };
    for (ci, &(id, raw_parents)) in commits.iter().enumerate() {
        if Some(id) == workspace_commit_id {
            continue;
        }
        let preserved =
            !raw_parents.is_empty() && raw_parents.iter().any(|p| !commit_entry.contains_key(p));
        if preserved {
            continue;
        }
        let n = commit_entry_at[ci];
        if flatten_matches(entries, parents, commits, n, raw_parents, &mut stack) {
            continue;
        }
        parents[n] = raw_parents
            .iter()
            .filter_map(|p| commit_entry.get(p).copied())
            .collect();
    }
}

/// Every `Entry::Ref`, as `(entry handle, ref-table index)`.
fn collect_ref_entries(entries: &[Entry]) -> Vec<(usize, usize)> {
    entries
        .iter()
        .enumerate()
        .filter_map(|(n, entry)| match entry {
            Entry::Ref(r) => Some((n, *r)),
            _ => None,
        })
        .collect()
}

/// Follow first-edges from `start` down to the first commit entry. Returns that commit
/// (`None` when the path ends unborn) and the first ref entry passed on the way — a
/// ref's `below` neighbour. Fuel: a simple path cannot exceed the entry count — running
/// out means a cycle, which the entry graph must never contain.
fn descend_to_commit(
    entries: &[Entry],
    parents: &[Vec<usize>],
    start: usize,
) -> (Option<usize>, Option<usize>) {
    let mut cursor = start;
    let mut below = None;
    for fuel in (0..=entries.len()).rev() {
        debug_assert!(
            fuel != 0,
            "BUG: cycle in the entry graph (first-edge descent)"
        );
        if fuel == 0 {
            break;
        }
        match entries[cursor] {
            Entry::Commit(_) => return (Some(cursor), below),
            Entry::Ref(_) if cursor != start && below.is_none() => below = Some(cursor),
            _ => {}
        }
        let Some(&next) = parents[cursor].first() else {
            break;
        };
        cursor = next;
    }
    (None, below)
}

/// Positions from the post-fixup topology, BEFORE parents are resolved past the ref
/// entries: descend first-edges for `on` and
/// below, ascend for the entering edges and the convergence signal. The reverse adjacency
/// is CSR-shaped (offsets into one flat run) — a Vec per entry costs an allocation per commit.
fn derive_positions(
    rows: &EntryGraph<'_>,
    ref_entries: &[(usize, usize)],
) -> HashMap<usize, DerivedPosition> {
    let EntryGraph {
        entries, parents, ..
    } = rows;
    let mut incoming_start = vec![0usize; entries.len() + 1];
    for adjacency in parents {
        for &parent in adjacency {
            incoming_start[parent + 1] += 1;
        }
    }
    for i in 0..entries.len() {
        incoming_start[i + 1] += incoming_start[i];
    }
    let mut incoming_flat: Vec<(usize, usize)> =
        vec![(0, 0); *incoming_start.last().expect("n+1 offsets")];
    {
        let mut fill = incoming_start.clone();
        for (child, adjacency) in parents.iter().enumerate() {
            for (parent_number, &parent) in adjacency.iter().enumerate() {
                incoming_flat[fill[parent]] = (child, parent_number);
                fill[parent] += 1;
            }
        }
    }
    let incoming = |n: usize| &incoming_flat[incoming_start[n]..incoming_start[n + 1]];
    let is_commit = |n: usize| matches!(entries[n], Entry::Commit(_));
    let mut positions = HashMap::<usize, DerivedPosition>::new();
    for &(ref_entry, _) in ref_entries {
        let (on, below) = descend_to_commit(entries, parents, ref_entry);
        let Some(on) = on else {
            continue; // unborn: no stored position
        };
        let mut cursor = ref_entry;
        let mut entering_edges = Vec::new();
        let mut ambiguous = false;
        for fuel in (0..=entries.len()).rev() {
            debug_assert!(fuel != 0, "BUG: cycle in the entry graph (entering ascent)");
            if fuel == 0 {
                break;
            }
            let entering = incoming(cursor);
            let picks: Vec<_> = entering
                .iter()
                .copied()
                .filter(|&(child, _)| is_commit(child))
                .collect();
            if !picks.is_empty() {
                ambiguous = entering.len() > 1;
                entering_edges = picks;
                break;
            }
            let mut others = entering.iter().filter(|&&(child, _)| !is_commit(child));
            match (others.next(), others.next()) {
                (Some(&(child, _)), None) => cursor = child,
                _ => break,
            }
        }
        positions.insert(
            ref_entry,
            DerivedPosition {
                on,
                below,
                ambiguous,
                entering: entering_edges,
            },
        );
    }
    positions
}

/// What resolving every commit's parents through the entry graph yields: the ws
/// commit's MATERIALIZED parents (one per ref chain, so empty chains over one base
/// yield duplicate entries the real commit does not have), and the full incoming edge
/// set per commit entry — the `GroupCarry::All` oracle. An unborn parent drops out of
/// the list, and the captured entering edges are renumbered to the FINAL parent
/// parent numbers — only here does an entry-graph position become a real parent number.
struct ResolvedParents {
    materialized_ws_parents: Option<MaterializedWsParents>,
    full_incoming: HashMap<usize, Vec<(usize, usize)>>,
}

fn resolve_parents(
    rows: &EntryGraph<'_>,
    workspace_commit_id: Option<gix::ObjectId>,
    positions: &mut HashMap<usize, DerivedPosition>,
) -> ResolvedParents {
    let EntryGraph {
        entries,
        parents,
        commits,
        commit_entry_at,
        ..
    } = rows;
    let resolve = |start: usize| -> Option<usize> { descend_to_commit(entries, parents, start).0 };
    // Per edge source: its vacated positions, ascending (filled in order below).
    let mut dropped: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut materialized_ws_parents: Option<MaterializedWsParents> = None;
    // The full incoming edge set per commit entry, in the SAME (post-resolution) edge
    // universe the entering edges live in — the `GroupCarry::All` oracle: a group carries
    // All exactly when its entering equals its commit's full set.
    let mut full_incoming: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for (ci, &(id, _)) in commits.iter().enumerate() {
        let n = commit_entry_at[ci];
        // Only the ws commit keeps its resolved ids — everyone else just detects
        // vacated positions.
        let is_ws = Some(id) == workspace_commit_id;
        let mut resolved = Vec::new();
        let mut vacated_below = 0usize;
        for (position, &parent) in parents[n].iter().enumerate() {
            match resolve(parent) {
                Some(handle) => {
                    // The entry-graph position minus its vacancies IS the parent number.
                    let parent_slot = position - vacated_below;
                    full_incoming
                        .entry(handle)
                        .or_default()
                        .push((n, parent_slot));
                    if is_ws {
                        let Entry::Commit(c) = entries[handle] else {
                            unreachable!("resolve returns commits");
                        };
                        resolved.push(commits[c].0);
                    }
                }
                None => {
                    dropped.entry(n).or_default().push(position);
                    vacated_below += 1;
                }
            }
        }
        if is_ws {
            materialized_ws_parents = Some(MaterializedWsParents {
                commit: id,
                parents: resolved,
            });
        }
    }
    for position in positions.values_mut() {
        for (edge_source, parent_number) in position.entering.iter_mut() {
            if let Some(vacated) = dropped.get(edge_source) {
                *parent_number -= vacated.partition_point(|v| v < parent_number);
            }
        }
    }
    ResolvedParents {
        materialized_ws_parents,
        full_incoming,
    }
}

/// The stored shape: ref-table order, entry handles translated to ref ordinals and commit
/// ids, entering edges sorted.
fn assemble_layout(
    rows: EntryGraph<'_>,
    ref_entries: &[(usize, usize)],
    positions: &HashMap<usize, DerivedPosition>,
    materialized_ws_parents: Option<MaterializedWsParents>,
    incoming: &dyn Fn(gix::ObjectId) -> Vec<(gix::ObjectId, usize)>,
) -> RefLayout {
    let EntryGraph {
        entries,
        ref_table,
        commits,
        mut reachable_commits,
        head_refs,
        ..
    } = rows;
    let mut entry_of_ref = vec![0usize; ref_table.len()];
    for &(n, r) in ref_entries {
        entry_of_ref[r] = n;
    }
    // One (facts, placement) pair per reference, table order — the parts the layout
    // authors its groups from. The placement's below is the NAME of the ref underneath:
    // groups reference by name, never by ordinal.
    let parts: Vec<(gix::refs::FullName, RefFacts, Option<EntryPlacement>)> = ref_table
        .into_iter()
        .enumerate()
        .map(|(r, (name, reachable, naming))| {
            let placement = positions.get(&entry_of_ref[r]).map(|position| {
                let Entry::Commit(c) = entries[position.on] else {
                    unreachable!("positions sit on commits");
                };
                let below = position.below.map(|b| {
                    let Entry::Ref(br) = entries[b] else {
                        unreachable!("below entries are refs");
                    };
                    br
                });
                let mut entering: Vec<(gix::ObjectId, usize)> = position
                    .entering
                    .iter()
                    .map(|&(child, parent_number)| {
                        let Entry::Commit(cc) = entries[child] else {
                            unreachable!("entering edges come from commits");
                        };
                        (commits[cc].0, parent_number)
                    })
                    .collect();
                entering.sort_unstable();
                EntryPlacement {
                    on: commits[c].0,
                    below,
                    entering,
                }
            });
            let facts = RefFacts {
                reachable,
                names_segment: naming.is_some(),
                names_empty_segment: naming.unwrap_or_default(),
                ambiguous: positions
                    .get(&entry_of_ref[r])
                    .is_some_and(|position| position.ambiguous),
            };
            (name, facts, placement)
        })
        .collect();
    // below as a name: resolvable only now that every ref's table ordinal is known.
    let names: Vec<gix::refs::FullName> = parts.iter().map(|(name, ..)| name.clone()).collect();
    let parts = parts
        .into_iter()
        .map(|(name, facts, placement)| RefPart {
            name,
            facts,
            placement: placement.map(|p| Placement {
                on: p.on,
                below: p.below.map(|b| names[b].clone()),
                entering: p.entering,
            }),
        })
        .collect();

    reachable_commits.sort_unstable();
    let mut layout = RefLayout::from_parts(parts, incoming);
    layout.materialized_ws_parents = materialized_ws_parents;
    layout.head_refs = head_refs;
    layout.reachable_commits = reachable_commits;
    // empty_chain_anchors is stamped by the caller from the plan's chain knowledge.
    layout
}

/// A reference's resolved placement, in id/ordinal space, before below-ordinals become
/// names.
struct EntryPlacement {
    on: gix::ObjectId,
    below: Option<usize>,
    entering: Vec<(gix::ObjectId, usize)>,
}
