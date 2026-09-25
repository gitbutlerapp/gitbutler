use std::collections::{BTreeMap, HashSet};

use bstr::BString;
use but_core::ChangeId;
use but_graph::workspace::{Stack, StackSegment, WorktreeStack};

use crate::id::{
    LaneId, LaneWithId, OLD_UNCOMMITTED, RemoteCommitWithId, SegmentWithId, ShortId,
    WorkspaceCommitWithId,
    id_usage::{IdUsage, UintId},
    unique_prefix_lengths,
};

fn lane_without_short_ids(
    lane: LaneId,
    segments: Vec<StackSegment>,
    commit_id_to_change_id: &gix::hashtable::HashMap<gix::ObjectId, ChangeId>,
) -> LaneWithId {
    let mut lane_with_id = LaneWithId {
        lane,
        segments: Vec::with_capacity(segments.len()),
        short_id: None,
    };
    for mut segment in segments {
        let workspace_commits = std::mem::take(&mut segment.commits)
            .into_iter()
            .map(|commit| WorkspaceCommitWithId {
                short_id: ShortId::default(),
                change_id: commit_id_to_change_id
                    .get(&commit.id)
                    .cloned()
                    .map(Into::into),
                inner: commit,
            })
            .collect::<Vec<_>>();
        let remote_commits = std::mem::take(&mut segment.commits_on_remote)
            .into_iter()
            .map(|commit| RemoteCommitWithId {
                short_id: ShortId::default(),
                inner: commit,
            })
            .collect::<Vec<_>>();
        lane_with_id.segments.push(SegmentWithId {
            short_id: ShortId::default(),
            inner: segment,
            workspace_commits,
            remote_commits,
            lane: lane_with_id.lane.clone(),
        });
    }
    lane_with_id
}

fn stacks_info_without_short_ids(
    stacks: Vec<Stack>,
    worktrees: Vec<WorktreeStack>,
    commit_id_to_change_id: &gix::hashtable::HashMap<gix::ObjectId, ChangeId>,
) -> StacksInfo {
    let mut stacks_info = StacksInfo {
        stacks: Vec::with_capacity(stacks.len() + worktrees.len()),
        id_usage: IdUsage::default(),
        non_hex_used_short_ids: HashSet::new(),
    };
    for stack in stacks {
        stacks_info.stacks.push(lane_without_short_ids(
            LaneId::Stack(stack.id),
            stack.segments,
            commit_id_to_change_id,
        ));
    }
    for worktree in worktrees {
        stacks_info.stacks.push(lane_without_short_ids(
            LaneId::Worktree(worktree.name),
            worktree.segments,
            commit_id_to_change_id,
        ));
    }
    stacks_info
}

fn mark_name_short_id_used(
    candidate: &[u8],
    id_usage: &mut IdUsage,
    non_hex_used_short_ids: &mut HashSet<ShortId>,
) -> Option<ShortId> {
    let short_id = UintId::from_name(candidate)
        .map(|uint_id| {
            id_usage.mark_used(uint_id);
            uint_id.to_short_id()
        })
        .or_else(|| {
            // If it's not a valid UintId, it's still acceptable if it
            // cannot be confused for a commit ID (and is valid UTF-8).
            if candidate.iter().all(|c| c.is_ascii_alphanumeric())
                && !candidate.iter().all(|c| c.is_ascii_hexdigit())
            {
                String::from_utf8(candidate.to_vec()).ok()
            } else {
                None
            }
        })?;

    non_hex_used_short_ids
        .insert(short_id.clone())
        .then_some(short_id)
}

fn allocate_generated_short_id(
    id_usage: &mut IdUsage,
    non_hex_used_short_ids: &mut HashSet<ShortId>,
) -> anyhow::Result<ShortId> {
    // `IdUsage` advances past generated IDs, so also retain their textual form for later
    // name-derived allocations, which detect collisions through this shared set.
    loop {
        let short_id = id_usage.next_available()?.to_short_id();
        if non_hex_used_short_ids.insert(short_id.clone()) {
            return Ok(short_id);
        }
    }
}

fn allocate_name_short_id(
    name: &[u8],
    id_usage: &mut IdUsage,
    non_hex_used_short_ids: &mut HashSet<ShortId>,
) -> anyhow::Result<ShortId> {
    // Find the first non-conflicting pair or triple and use it.
    for candidate in name.windows(2).chain(name.windows(3)) {
        if let Some(short_id) = mark_name_short_id_used(candidate, id_usage, non_hex_used_short_ids)
        {
            return Ok(short_id);
        }
    }
    // If none are available, use the next generated ID.
    allocate_generated_short_id(id_usage, non_hex_used_short_ids)
}

fn populate_branch_short_ids(
    stacks: &mut [LaneWithId],
    id_usage: &mut IdUsage,
    non_hex_used_short_ids: &mut HashSet<ShortId>,
    uncommitted_short_filenames: &HashSet<BString>,
) -> anyhow::Result<()> {
    // Keep the old uncommitted-area ID out of both generated and reverse-hex short-ID namespaces
    // so upgrading cannot silently reassign it to another resource.
    let _ = mark_name_short_id_used(OLD_UNCOMMITTED.as_bytes(), id_usage, non_hex_used_short_ids);
    for uncommitted_short_filename in uncommitted_short_filenames {
        let _ =
            mark_name_short_id_used(uncommitted_short_filename, id_usage, non_hex_used_short_ids);
    }

    // Populate branch short IDs in `stacks`.
    for segment in stacks
        .iter_mut()
        .flat_map(|stack| stack.segments.iter_mut())
    {
        if let Some(branch_name) = segment.branch_name() {
            segment.short_id =
                allocate_name_short_id(branch_name, id_usage, non_hex_used_short_ids)?;
        } else {
            // This segment is anonymous, so we have no name to base the ID on. We just assign it a
            // generic ID, which allows some rudimentary stuff to work (e.g. `but status`).
            segment.short_id = allocate_generated_short_id(id_usage, non_hex_used_short_ids)?;
        }
    }

    Ok(())
}

/// Append the shortest unambiguous hash prefix to every short ID in `commits`.
///
/// All commits sharing a CLI ID namespace must come in one call - the prefix length is derived
/// from each commit's neighbours in hash order, so one left out could later print a prefix that
/// is no longer unique.
pub(crate) fn populate_commit_short_ids(commits: Vec<(gix::ObjectId, &mut ShortId)>) {
    let mut commit_id_to_short_ids = BTreeMap::<gix::ObjectId, Vec<&mut ShortId>>::new();
    for (commit_id, short_id) in commits {
        commit_id_to_short_ids
            .entry(commit_id)
            .or_default()
            .push(short_id);
    }
    let hexes: Vec<String> = commit_id_to_short_ids
        .keys()
        .map(|commit_id| commit_id.to_string())
        .collect();
    let lengths = unique_prefix_lengths(&hexes.iter().map(String::as_bytes).collect::<Vec<_>>());
    for ((short_ids, hex), len) in commit_id_to_short_ids
        .into_values()
        .zip(&hexes)
        .zip(lengths)
    {
        for short_id in short_ids {
            *short_id = hex[..len].to_owned();
        }
    }
}

pub(crate) struct StacksInfo {
    pub(crate) stacks: Vec<LaneWithId>,
    pub(crate) id_usage: IdUsage,
    /// The set of short IDs allocated to items when building the [`StacksInfo`].
    ///
    /// Note that this map's keys do not necessarily need to start with g-z,
    /// unlike [UintId], as long as the key cannot be confused with a commit
    /// ID.
    pub(crate) non_hex_used_short_ids: HashSet<ShortId>,
}

impl StacksInfo {
    pub(crate) fn new(
        stacks: Vec<Stack>,
        worktrees: Vec<WorktreeStack>,
        uncommitted_short_filenames: &HashSet<BString>,
        commit_id_to_change_id: &gix::hashtable::HashMap<gix::ObjectId, ChangeId>,
    ) -> anyhow::Result<Self> {
        let mut stacks_info =
            stacks_info_without_short_ids(stacks, worktrees, commit_id_to_change_id);
        populate_branch_short_ids(
            &mut stacks_info.stacks,
            &mut stacks_info.id_usage,
            &mut stacks_info.non_hex_used_short_ids,
            uncommitted_short_filenames,
        )?;
        Ok(stacks_info)
    }
}
