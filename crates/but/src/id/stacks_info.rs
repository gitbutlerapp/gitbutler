use std::collections::{HashMap, HashSet};

use bstr::BString;
use but_workspace::branch::Stack;

use crate::id::{
    RemoteCommitWithId, SegmentWithId, ShortId, StackWithId, UNASSIGNED, WorkspaceCommitWithId,
    id_usage::{IdUsage, UintId},
};

fn stacks_info_without_short_ids(stacks: Vec<Stack>) -> StacksInfo {
    let mut stacks_info = StacksInfo {
        stacks: Vec::with_capacity(stacks.len()),
        id_usage: IdUsage::default(),
        short_ids_to_count: HashMap::new(),
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
                is_auto_id: false,
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

fn populate_branch_short_ids(
    stacks: &mut [StackWithId],
    id_usage: &mut IdUsage,
    short_ids_to_count: &mut HashMap<ShortId, u8>,
    uncommitted_short_filenames: &HashSet<BString>,
) -> anyhow::Result<()> {
    // Fill the `short_ids_to_count` and `id_usage` data structures.
    let mut maybe_mark_used = |candidate| {
        if let Some(short_id) = UintId::from_name(candidate)
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
            })
        {
            short_ids_to_count
                .entry(short_id)
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
        }
    };
    maybe_mark_used(UNASSIGNED.as_bytes());
    for uncommitted_short_filename in uncommitted_short_filenames.iter() {
        maybe_mark_used(uncommitted_short_filename);
    }
    for segment in stacks.iter().flat_map(|stack| stack.segments.iter()) {
        let Some(branch_name) = segment.branch_name() else {
            continue;
        };
        for candidate in branch_name.windows(2).chain(branch_name.windows(3)) {
            maybe_mark_used(candidate);
        }
    }

    // Populate branch short IDs in `stacks`.
    for segment in stacks.iter_mut().flat_map(|stack| stack.segments.iter_mut()) {
        let Some(branch_name) = segment.branch_name() else {
            // The branch CliId is its name, so if this segment doesn't have a
            // name, it doesn't need an ID.
            continue;
        };
        (segment.short_id, segment.is_auto_id) = 'short_id: {
            // Find first non-conflicting pair or triple (i.e. used in
            // exactly one branch) and use it.
            for candidate in branch_name.windows(2).chain(branch_name.windows(3)) {
                if let Ok(short_id) = str::from_utf8(candidate)
                    && let Some(1) = short_ids_to_count.get(short_id)
                {
                    break 'short_id (short_id.to_owned(), false);
                }
            }
            // If none available, use next available ID.
            (id_usage.next_available()?.to_short_id(), true)
        };
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

fn populate_commit_short_ids(stacks: &mut [StackWithId]) {
    let mut commit_id_and_short_id_pairs = stacks
        .iter_mut()
        .flat_map(|stack| stack.segments.iter_mut())
        .flat_map(|segment| {
            let SegmentWithId {
                workspace_commits,
                remote_commits,
                ..
            } = segment;
            workspace_commits
                .iter_mut()
                .map(|workspace_commit| (workspace_commit.commit_id(), &mut workspace_commit.short_id))
                .chain(
                    remote_commits
                        .iter_mut()
                        .map(|remote_commit| (remote_commit.commit_id(), &mut remote_commit.short_id)),
                )
        })
        .collect::<Vec<_>>();
    commit_id_and_short_id_pairs.sort();

    let mut common_with_previous_len = 0;
    let mut remaining = commit_id_and_short_id_pairs.as_mut_slice();
    while let Some(((commit_id, short_id), rest)) = remaining.split_first_mut() {
        let common_with_next_len = rest.first().map_or(0, |(next_commit_id, _next_short_id)| {
            common_nybble_len(commit_id.as_bytes(), next_commit_id.as_bytes())
        });
        short_id.push_str(
            &commit_id
                .to_hex_with_len(1 + 1.max(common_with_previous_len).max(common_with_next_len))
                .to_string(),
        );
        common_with_previous_len = common_with_next_len;
        remaining = rest;
    }
}

pub(crate) struct StacksInfo {
    pub(crate) stacks: Vec<StackWithId>,
    pub(crate) id_usage: IdUsage,
    // Map from an acceptable short ID to how many times it appears among
    // uncommitted short filenames and substrings of branch names. If a
    // string doesn't appear in this map, it is not an acceptable short ID,
    // and if a string's count is more than 1, it's ambiguous.
    //
    // Note that this map's keys do not necessarily need to start with g-z,
    // unlike [UintId], as long as the key cannot be confused with a commit
    // ID.
    pub(crate) short_ids_to_count: HashMap<ShortId, u8>,
}

impl StacksInfo {
    pub(crate) fn new(stacks: Vec<Stack>, uncommitted_short_filenames: &HashSet<BString>) -> anyhow::Result<Self> {
        let mut stacks_info = stacks_info_without_short_ids(stacks);
        populate_branch_short_ids(
            &mut stacks_info.stacks,
            &mut stacks_info.id_usage,
            &mut stacks_info.short_ids_to_count,
            uncommitted_short_filenames,
        )?;
        populate_commit_short_ids(&mut stacks_info.stacks);
        Ok(stacks_info)
    }
}
