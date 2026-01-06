//! Utility types related to discarding changes in the worktree.

use std::collections::HashMap;

use but_rebase::{RebaseOutput, RebaseStep};

// Re-export from non-legacy location for backward compatibility
pub use crate::tree_manipulation::{ChangesSource, create_tree_without_diff};

/// Takes a rebase output and returns the commit mapping with any extra
/// mapping overrides provided.
///
/// This will only include commits that have actually changed. If a commit was
/// mapped to itself it will not be included in the resulting HashMap.
///
/// Overrides are used to handle the case where the caller of the rebase engine
/// has manually replaced a particular commit with a rewritten one. This is
/// needed because a manually re-written commit that ends up matching the
/// base when the rebase occurs will end up showing up as a no-op in the
/// resulting commit_mapping.
///
/// Overrides should be provided as a vector that contains tuples of object
/// ids, where the first item is the before object_id, and the second item is
/// the after object_id.
pub(crate) fn rebase_mapping_with_overrides(
    rebase_output: &RebaseOutput,
    overrides: impl IntoIterator<Item = (gix::ObjectId, gix::ObjectId)>,
) -> HashMap<gix::ObjectId, gix::ObjectId> {
    let mut mapping = rebase_output
        .commit_mapping
        .iter()
        .filter(|(_, old, new)| old != new)
        .map(|(_, old, new)| (*old, *new))
        .collect::<HashMap<_, _>>();

    for (old, new) in overrides {
        if old != new {
            mapping.insert(old, new);
        }
    }

    mapping
}

pub fn replace_pick_with_commit(
    steps: &mut Vec<RebaseStep>,
    target_commit_id: gix::ObjectId,
    replacement_commit_id: gix::ObjectId,
) -> anyhow::Result<()> {
    let mut found = false;
    for step in steps {
        if step.commit_id() != Some(&target_commit_id) {
            continue;
        }
        let RebaseStep::Pick { commit_id, .. } = step else {
            continue;
        };
        found = true;
        *commit_id = replacement_commit_id;
    }

    if found {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to replace pick step {} with {}",
            target_commit_id,
            replacement_commit_id
        ))
    }
}

pub fn replace_pick_with_multiple_commits(
    steps: &mut Vec<RebaseStep>,
    target_commit_id: gix::ObjectId,
    replacement_commit_ids: &[(gix::ObjectId, Option<String>)],
) -> anyhow::Result<()> {
    let mut found = false;
    let mut new_steps =
        Vec::with_capacity(steps.len() + replacement_commit_ids.len().saturating_sub(1));
    for step in steps.drain(..) {
        if step.commit_id() == Some(&target_commit_id) {
            let RebaseStep::Pick { .. } = step else {
                new_steps.push(step);
                continue;
            };
            found = true;
            for (replacement_commit_id, new_message) in replacement_commit_ids {
                new_steps.push(RebaseStep::Pick {
                    commit_id: *replacement_commit_id,
                    new_message: new_message.clone().map(|msg| msg.into()),
                });
            }
        } else {
            new_steps.push(step);
        }
    }
    *steps = new_steps;

    if found {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to replace pick step {} with multiple commits",
            target_commit_id
        ))
    }
}
