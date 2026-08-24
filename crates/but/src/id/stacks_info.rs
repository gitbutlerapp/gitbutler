use std::collections::{BTreeMap, HashSet};

use bstr::BString;
use but_core::ChangeId;
use but_graph::workspace::Stack;

use crate::id::{
    RemoteCommitWithId, SegmentWithId, ShortId, StackWithId, UNCOMMITTED, WorkspaceCommitWithId,
    id_usage::{IdUsage, UintId},
};

fn stacks_info_without_short_ids(
    stacks: Vec<Stack>,
    commit_id_to_change_id: &gix::hashtable::HashMap<gix::ObjectId, ChangeId>,
) -> StacksInfo {
    let mut stacks_info = StacksInfo {
        stacks: Vec::with_capacity(stacks.len()),
        id_usage: IdUsage::default(),
        non_hex_used_short_ids: HashSet::new(),
    };
    for stack in stacks {
        let mut stack_with_id = StackWithId {
            id: stack.id,
            segments: Vec::with_capacity(stack.segments.len()),
        };
        for mut segment in stack.segments {
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
            stack_with_id.segments.push(SegmentWithId {
                short_id: ShortId::default(),
                inner: segment,
                workspace_commits,
                remote_commits,
                stack_id: stack.id,
            });
        }
        stacks_info.stacks.push(stack_with_id);
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

pub(crate) fn allocate_name_short_id(
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
    stacks: &mut [StackWithId],
    id_usage: &mut IdUsage,
    non_hex_used_short_ids: &mut HashSet<ShortId>,
    uncommitted_short_filenames: &HashSet<BString>,
) -> anyhow::Result<()> {
    let _ = mark_name_short_id_used(UNCOMMITTED.as_bytes(), id_usage, non_hex_used_short_ids);
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

/// Returns the length of the longest common *nybble* prefix.
fn common_nybble_len(a: &[u8], b: &[u8]) -> usize {
    let mut byte_len = 0usize;
    let extra_nybble = loop {
        let (Some(a_byte), Some(b_byte)) = (a.get(byte_len), b.get(byte_len)) else {
            break 0;
        };
        if a_byte != b_byte {
            break if a_byte & 0xf0 == b_byte & 0xf0 { 1 } else { 0 };
        }
        byte_len += 1;
    };
    byte_len * 2 + extra_nybble
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
    // Ideally we would use BTreeMap cursors, but those are still experimental,
    // so convert to a Vec for now.
    let mut commit_id_to_short_ids: Vec<_> = commit_id_to_short_ids.into_iter().collect();

    let mut common_with_previous_len = 0;
    let mut remaining = commit_id_to_short_ids.as_mut_slice();
    while let Some(((commit_id, short_ids), rest)) = remaining.split_first_mut() {
        let common_with_next_len = rest.first().map_or(0, |(next_commit_id, _next_short_id)| {
            common_nybble_len(commit_id.as_bytes(), next_commit_id.as_bytes())
        });
        for short_id in short_ids.iter_mut() {
            short_id.push_str(
                &commit_id
                    .to_hex_with_len(1 + common_with_previous_len.max(common_with_next_len))
                    .to_string(),
            );
        }
        common_with_previous_len = common_with_next_len;
        remaining = rest;
    }
}

pub(crate) struct StacksInfo {
    pub(crate) stacks: Vec<StackWithId>,
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
        uncommitted_short_filenames: &HashSet<BString>,
        commit_id_to_change_id: &gix::hashtable::HashMap<gix::ObjectId, ChangeId>,
    ) -> anyhow::Result<Self> {
        let mut stacks_info = stacks_info_without_short_ids(stacks, commit_id_to_change_id);
        populate_branch_short_ids(
            &mut stacks_info.stacks,
            &mut stacks_info.id_usage,
            &mut stacks_info.non_hex_used_short_ids,
            uncommitted_short_filenames,
        )?;
        // Commit short IDs are assigned by the caller, which also knows the linked worktrees'
        // commits that share the same namespace.
        Ok(stacks_info)
    }
}
