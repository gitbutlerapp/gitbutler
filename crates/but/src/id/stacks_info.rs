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

fn populate_branch_short_ids(
    stacks: &mut [StackWithId],
    id_usage: &mut IdUsage,
    non_hex_used_short_ids: &mut HashSet<ShortId>,
    uncommitted_short_filenames: &HashSet<BString>,
    worktree_names: &[BString],
) -> anyhow::Result<()> {
    // Fill the `non_hex_used_short_ids` and `id_usage` data structures.
    //
    // Returns None if the candidate was bad, Some(false) if it was already taken and Some(true) if
    // it was successfully "acquired".
    let mut maybe_mark_used = |candidate: &[u8], id_usage: &mut IdUsage| {
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

        Some(non_hex_used_short_ids.insert(short_id))
    };

    maybe_mark_used(UNCOMMITTED.as_bytes(), id_usage);
    for uncommitted_short_filename in uncommitted_short_filenames.iter() {
        maybe_mark_used(uncommitted_short_filename, id_usage);
    }
    // Worktree names participate in exact-name matching just like uncommitted
    // filenames, so reserve them as well - a worktree literally named like a
    // mintable ID (e.g. "k0") must not shadow whatever would otherwise be
    // allocated that ID.
    for worktree_name in worktree_names {
        maybe_mark_used(worktree_name.as_slice(), id_usage);
    }

    // Populate branch short IDs in `stacks`.
    for segment in stacks
        .iter_mut()
        .flat_map(|stack| stack.segments.iter_mut())
    {
        if let Some(branch_name) = segment.branch_name() {
            segment.short_id = 'short_id: {
                // Find first non-conflicting pair or triple (i.e. used in
                // exactly one branch) and use it.
                for candidate in branch_name.windows(2).chain(branch_name.windows(3)) {
                    if let Ok(short_id) = str::from_utf8(candidate)
                        && let Some(true) = maybe_mark_used(candidate, id_usage)
                    {
                        break 'short_id short_id.to_owned();
                    }
                }
                // If none available, use next available ID.
                id_usage.next_available()?.to_short_id()
            };
        } else {
            // This sgement is anonymous, so we have no name to base the ID on. We just assign it a
            // generic ID, which allows some rudimentary stuff to work (e.g. `but status`).
            segment.short_id = id_usage.next_available()?.to_short_id();
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

fn populate_commit_short_ids(stacks: &mut [StackWithId]) {
    let mut commit_id_to_short_ids = BTreeMap::<gix::ObjectId, Vec<&mut ShortId>>::new();
    for stack in stacks.iter_mut() {
        for segment in stack.segments.iter_mut() {
            let SegmentWithId {
                workspace_commits,
                remote_commits,
                ..
            } = segment;
            for workspace_commit in workspace_commits.iter_mut() {
                commit_id_to_short_ids
                    .entry(workspace_commit.commit_id())
                    .or_default()
                    .push(&mut workspace_commit.short_id);
            }
            for remote_commit in remote_commits.iter_mut() {
                commit_id_to_short_ids
                    .entry(remote_commit.commit_id())
                    .or_default()
                    .push(&mut remote_commit.short_id);
            }
        }
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
        worktree_names: &[BString],
    ) -> anyhow::Result<Self> {
        let mut stacks_info = stacks_info_without_short_ids(stacks, commit_id_to_change_id);
        populate_branch_short_ids(
            &mut stacks_info.stacks,
            &mut stacks_info.id_usage,
            &mut stacks_info.non_hex_used_short_ids,
            uncommitted_short_filenames,
            worktree_names,
        )?;
        populate_commit_short_ids(&mut stacks_info.stacks);
        Ok(stacks_info)
    }
}
