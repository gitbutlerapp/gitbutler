use crate::commit::CommitterMode;
use anyhow::{bail, Result};
use but_core::commit::TreeKind;
use gitbutler_oxidize::GixRepositoryExt;
use gix::prelude::ObjectIdExt;

/// Perform a three-base merge for each of the parents in `target_merge_commit` which serves as template for the merge.
/// This means that after merging, we will use it unchanged to create a new, possibly signed commit, after adjusting its
/// tree to point to the merge result of its parents.
/// If `target_merge_commit` only has two parents, it will be a normal merge.
///
/// The specialty of the octopus merge is that the merge-base is calculated once using [`gix::Repository::merge_base_octopus()`]
/// and then re-used when merging subsequent `parent-commit^{tree}` into each other in a three-way merge, reusing the previous
/// result as *ours* until all parents are merged in.
///
/// Conflicts will cause the operation to fail, there is no hiding of conflicts as merge commits typically are
/// workspace tips which are implicit in the application. For consistency, there is no special treatment of merge-commits
/// which are part of the branches or stacks.
///
/// ### About Signing
///
/// Merges are special, and we will *not* sign it if it wasn't yet signed. That way workspace commits will naturally
/// remain unsigned.
/// However, if we re-merge a commit that was signed before it's likely a user-commit that should be treated accordingly.
/// Thanks to this logic, the caller shouldn't have to steer signing.
pub fn octopus(
    repo: &gix::Repository,
    mut target_merge_commit: gix::objs::Commit,
    graph: &mut gix::revwalk::Graph<
        '_,
        '_,
        gix::revwalk::graph::Commit<gix::revision::plumbing::merge_base::Flags>,
    >,
) -> Result<gix::ObjectId> {
    if target_merge_commit.parents.len() < 2 {
        bail!("An octopus merge commits must have at least two parents");
    }
    let parents_to_merge = target_merge_commit.parents.iter().copied();
    let merge_base = but_core::Commit::from_id(
        repo.merge_base_octopus_with_graph(parents_to_merge.clone(), graph)?,
    )?
    .tree_id_by_kind_or_ours(TreeKind::Base)?
    .detach();
    let mut trees_to_merge = parents_to_merge
        .clone()
        .map(|commit_id| -> Result<_> {
            // TODO: as long as only cherry-picking is creating these trees, THEIRS
            //       is the original 'to_rebase'. However, if that changes we must know
            //       what created the special merge commit.
            Ok(but_core::Commit::from_id(commit_id.attach(repo))?
                .tree_id_by_kind_or_ours(TreeKind::Theirs)?
                .detach())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter();
    let mut ours = trees_to_merge.next().expect("two or more trees");
    let (merge_options, unresolved) = repo.merge_options_fail_fast()?;
    for tree_to_merge in trees_to_merge {
        let mut merge = repo.merge_trees(
            merge_base,
            ours,
            tree_to_merge,
            repo.default_merge_labels(),
            merge_options.clone(),
        )?;
        if merge.has_unresolved_conflicts(unresolved) {
            bail!(
                "Encountered conflict when merging commits {}",
                parents_to_merge
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        ours = merge.tree.write()?.detach();
    }
    target_merge_commit.tree = ours;
    if target_merge_commit
        .extra_headers()
        .pgp_signature()
        .is_some()
    {
        crate::commit::create(repo, target_merge_commit, CommitterMode::Update)
    } else {
        crate::commit::update_committer(repo, &mut target_merge_commit)?;
        Ok(repo.write_object(target_merge_commit)?.detach())
    }
}
