use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use itertools::Itertools;

use crate::StackBranch;
use crate::stack_branch::CommitOrChangeId;

pub(crate) fn get_head(heads: &[StackBranch], name: &str) -> Result<(usize, StackBranch)> {
    let (idx, head) = heads
        .iter()
        .enumerate()
        .find(|(_, h)| h.name() == name)
        .ok_or_else(|| anyhow!("Series with name {} not found", name))?;
    Ok((idx, head.clone()))
}

/// Returns the updated list of heads and a boolean indicating if another reference was moved
/// to the top of the stack as a result.
pub(crate) fn remove_head(
    mut heads: Vec<StackBranch>,
    name: String,
    repo: &gix::Repository,
) -> Result<(Vec<StackBranch>, bool)> {
    // find the head that corresponds to the supplied name, together with its index
    let (idx, head) = get_head(&heads, &name)?;
    if heads.len() == 1 {
        bail!("Cannot remove the last branch from the stack")
    }
    // The branch that is being removed is the top (last) one.
    // This means that if there are commits, they need to be moved to the branch underneath.
    let mut moved_another_reference = false;
    if heads.len() - 1 == idx {
        // Getting the preceding head  and setting its target to the target of the head being removed
        let prior_head = heads
            .get_mut(idx - 1)
            .ok_or_else(|| anyhow!("Cannot get the head before the head being removed"))?;
        prior_head.set_head(head.head_oid(repo)?, repo)?;
        moved_another_reference = true;
    }
    heads.remove(idx);
    let _ = head.delete_reference(repo); // TODO: In the future, make this error out. For now, ignoring, since references are not yet 100% authoritative
    Ok((heads, moved_another_reference))
}

/// Takes the list of current existing heads and a new head.
/// Returns new, updated list of heads with the new head added in the correct position.
/// If there are multiple heads pointing to the same patch, it uses `preceding_head` to disambiguate the order.
// TODO: when there is a patch reference for a commit ID and a patch reference for a change ID, recognize if they are equivalent (i.e. point to the same commit)
pub fn add_head(
    existing_heads: Vec<StackBranch>,
    new_head: StackBranch,
    preceding_head: Option<StackBranch>,
    patches: Vec<CommitOrChangeId>,
    repo: &gix::Repository,
) -> Result<Vec<StackBranch>> {
    let existing_heads: Vec<(StackBranch, CommitOrChangeId)> = existing_heads
        .iter()
        .map(|h| {
            let head = h.head_oid(repo)?;
            Ok((h.clone(), head.into()))
        })
        .collect::<Result<_>>()?;

    // Go over all patches in the stack from oldest to newest
    // If `new_head` or the first (bottom of the stack) head in existing_heads points to the patch, add it to the list
    // If there are multiple heads that point to the same patch, the order is disambiguated by specifying the `preceding_head`
    // If `preceding_head` is specified, it must be in the list of existing heads, and it must be a head for the same patch as the `new_head`
    if let Some(preceding_head) = &preceding_head {
        if preceding_head.head_oid(repo)? != new_head.head_oid(repo)? {
            return Err(anyhow!(
                "Preceding head needs to be one that point to the same patch as new_head"
            ));
        }
        let preceding_head: (StackBranch, CommitOrChangeId) = (
            preceding_head.clone(),
            preceding_head.head_oid(repo)?.into(),
        );
        if !existing_heads.contains(&preceding_head) {
            return Err(anyhow!(
                "Preceding head is set but does not exist for specified patch"
            ));
        }
    }
    let archived_heads = existing_heads
        .iter()
        .filter(|h| !patches.contains(&h.1))
        .cloned()
        .collect_vec();
    let mut existing_heads = existing_heads
        .iter()
        .filter(|h| patches.contains(&h.1))
        .cloned()
        .collect_vec();
    let mut updated_heads: Vec<(StackBranch, CommitOrChangeId)> = vec![];
    updated_heads.extend(archived_heads); // add any heads that are below the merge base (archived)
    let mut new_head = Some(new_head);
    for patch in &patches {
        loop {
            let existing_head = existing_heads.first().cloned();
            match (existing_head, &new_head) {
                // Both the new head and the next existing head reference the patch as a target
                (Some(existing_head), Some(new_head_ref))
                    if &existing_head.1 == patch
                        && patch == &new_head_ref.head_oid(repo)?.into() =>
                {
                    if preceding_head.is_none() {
                        updated_heads
                            .push((new_head_ref.clone(), new_head_ref.head_oid(repo)?.into())); // no preceding head specified, so add the new head first
                        new_head = None; // the `new_head` is now consumed
                    } else if preceding_head.as_ref() == updated_heads.last().map(|(h, _)| h) {
                        updated_heads
                            .push((new_head_ref.clone(), new_head_ref.head_oid(repo)?.into())); // preceding_head matches the last entry, so add the new_head next
                        new_head = None; // the `new_head` is now consumed
                    } else {
                        updated_heads.push(existing_head.clone()); // add the next existing head as the next entry
                        existing_heads.remove(0); // consume the next in line from the existing heads
                    }
                }
                // Only the next existing head matches the patch as a target
                (Some(existing_head), _) if &existing_head.1 == patch => {
                    updated_heads.push(existing_head.clone()); // add the nex existing head as the next entry
                    existing_heads.remove(0); // consume the next in line from the existing heads
                }
                // Only the new head matches the patch as a target
                (_, Some(new_head_ref)) if patch == &new_head_ref.head_oid(repo)?.into() => {
                    updated_heads.push((new_head_ref.clone(), new_head_ref.head_oid(repo)?.into())); // add the new head as the next entry
                    new_head = None; // the `new_head` is now consumed
                }
                // Neither the next existing head nor the new head match the patch as a target so continue to the next patch
                _ => {
                    break;
                }
            }
        }
    }
    // the last head must point to the last commit in the stack of patches
    if let Some(last_head) = updated_heads.last() {
        if let Some(last_patch) = patches.last() {
            if &last_head.1 != last_patch {
                // error - invalid state - this would result in orphaned patches
                bail!(
                    "The newest head must point to the newest patch in the stack. The newest patch is {}, while the newest head with name {} points patch {}",
                    last_patch,
                    last_head.0.name(),
                    last_head.1
                );
            }
        } else {
            // error - invalid state (at minimum there should be the merge base here)
            bail!(
                "Error while adding head - there must be at least one patch(commit) in the stack, when including the merge base"
            );
        }
    } else {
        // error - invalid state (an initialized stack must have at least one head)
        bail!("Error while adding head - there must be at least one head in an initialized stack");
    }
    Ok(updated_heads
        .iter()
        .map(|(h, _)| h.clone())
        .collect::<Vec<_>>())
}
