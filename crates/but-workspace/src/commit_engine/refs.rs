use crate::commit_engine::UpdatedReference;
use bstr::BString;
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_stack::{CommitOrChangeId, VirtualBranchesState};
use gix::prelude::ObjectIdExt as _;
use gix::refs::transaction::PreviousValue;

use super::StackSegmentId;

/// Rewrite all references as mapped by their target in `refs_by_commit_id` so that those
/// pointing to `old` in `changed_commits` will then point to `new`.
/// Do the same for the virtual refs in `state` place information about all performed updates
/// in `updated_refs`.
/// `workspace_tip` is used, if present, to help build mappings from change-ids to commit-ids *if*
/// no target branch is available.
pub fn rewrite(
    repo: &gix::Repository,
    state: &mut VirtualBranchesState,
    mut refs_by_commit_id: gix::hashtable::HashMap<gix::ObjectId, Vec<gix::refs::FullName>>,
    changed_commits: impl IntoIterator<Item = (gix::ObjectId, gix::ObjectId)>,
    updated_refs: &mut Vec<UpdatedReference>,
    stack_segment: Option<&StackSegmentId>,
) -> anyhow::Result<()> {
    let mut ref_edits = Vec::new();
    let changed_commits: Vec<_> = changed_commits.into_iter().collect();
    let mut stacks_ordered: Vec<_> = state
        .branches
        .values_mut()
        .filter(|stack| stack.in_workspace)
        .collect();
    stacks_ordered.sort_by(|a, b| a.name.cmp(&b.name));
    for (old, new) in changed_commits {
        let old_git2 = old.to_git2();
        let mut already_updated_refs = Vec::<BString>::new();
        for stack in &mut stacks_ordered {
            if let Some(stack_segment) = stack_segment {
                if stack_segment.stack_id != stack.id {
                    continue; // Dont rewrite refs for other stacks
                }
            }
            if stack.head(repo)? == old_git2 {
                // Perhaps skip this - the head will be updated later in this call
                // stack.set_stack_head_without_persisting(repo, new.to_git2(), None)?;
                stack.tree = new
                    .attach(repo)
                    .object()?
                    .into_commit()
                    .tree_id()?
                    .to_git2();
                updated_refs.push(UpdatedReference {
                    old_commit_id: old,
                    new_commit_id: new,
                    reference: but_core::Reference::Virtual(stack.name.clone()),
                });
            }
            let update_up_to_idx =
                stack_segment
                    .map(|s| s.segment_ref.as_ref())
                    .and_then(|up_to_ref| {
                        let short_name = up_to_ref.shorten();
                        stack
                            .heads
                            .iter()
                            .rev()
                            .enumerate()
                            .find_map(|(idx, h)| (h.name == short_name).then_some(idx))
                    });
            for (idx, branch) in stack.heads.iter_mut().rev().enumerate() {
                let id = branch.head_oid(repo)?.to_gix();
                if id == old {
                    if update_up_to_idx.is_some() && Some(idx) > update_up_to_idx {
                        // Make sure the actual refs also don't update (later)
                        already_updated_refs.push(format!("refs/heads/{}", branch.name()).into());
                        continue;
                    }
                    if let Some(full_refname) =
                        branch.set_head(CommitOrChangeId::CommitId(new.to_string()), repo)?
                    {
                        already_updated_refs.push(full_refname)
                    }
                    updated_refs.push(UpdatedReference {
                        old_commit_id: old,
                        new_commit_id: new,
                        reference: but_core::Reference::Virtual(branch.name().clone()),
                    });
                }
            }
        }

        let Some(refs_to_rewrite) = refs_by_commit_id.remove(&old) else {
            continue;
        };

        for name in refs_to_rewrite {
            if already_updated_refs.iter().any(|r| name.as_bstr() == r) {
                continue;
            }
            use gix::refs::{
                Target,
                transaction::{Change, LogChange, RefEdit, RefLog},
            };
            updated_refs.push(UpdatedReference {
                old_commit_id: old,
                new_commit_id: new,
                reference: but_core::Reference::Git(name.clone()),
            });
            ref_edits.push(RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: "Created or amended commit".into(),
                    },
                    expected: PreviousValue::ExistingMustMatch(Target::Object(old)),
                    new: Target::Object(new),
                },
                name,
                deref: false,
            });
        }
    }
    repo.edit_references(ref_edits)?;
    Ok(())
}
