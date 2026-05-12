use anyhow::{Context as _, Result};
use bstr::ByteSlice as _;
use but_core::{
    RepositoryExt,
    commit::{Headers, conflict_entries_from_merge_outcome},
};
use but_oxidize::{ObjectIdExt as _, OidExt as _};
use gitbutler_cherry_pick::{ConflictedTreeKey, GixRepositoryExt as _};

/// Merge two commits together
///
/// The `target_commit` and `incoming_commit` must have a common ancestor.
///
/// If there is a merge conflict, we will **auto-resolve** to favor *our* side, the `incoming_commit`.
pub fn merge_commits(
    gix_repository: &gix::Repository,
    target_commit: gix::ObjectId,
    incoming_commit: gix::ObjectId,
    resulting_name: &str,
) -> Result<gix::ObjectId> {
    let target_commit = gix_repository.find_commit(target_commit)?;
    let incoming_commit = gix_repository.find_commit(incoming_commit)?;
    let merge_base = gix_repository.merge_base(target_commit.id, incoming_commit.id)?;
    let merge_base = gix_repository.find_commit(merge_base.detach())?;

    let base_tree = gix_repository.find_real_tree(&merge_base, Default::default())?;
    // We want to use the auto-resolution when computing the merge, but for
    // reconstructing it later, we want the "theirsiest" and "oursiest" trees
    let target_tree = gix_repository.find_real_tree(&target_commit, ConflictedTreeKey::Theirs)?;
    let incoming_tree = gix_repository.find_real_tree(&incoming_commit, ConflictedTreeKey::Ours)?;

    let target_merge_tree = gix_repository.find_real_tree(&target_commit, Default::default())?;
    let incoming_merge_tree =
        gix_repository.find_real_tree(&incoming_commit, Default::default())?;
    let repo = git2::Repository::open(gix_repository.path())?;
    let gix_repo = gix_repository.clone().for_tree_diffing()?;
    let mut merge_result = gix_repo.merge_trees(
        base_tree.detach(),
        incoming_merge_tree.detach(),
        target_merge_tree.detach(),
        gix_repo.default_merge_labels(),
        gix_repo.merge_options_force_ours()?,
    )?;
    let merged_tree_id = merge_result.tree.write()?;

    let tree_oid;
    let forced_resolution = gix::merge::tree::TreatAsUnresolved::forced_resolution();
    let is_conflicted = if merge_result.has_unresolved_conflicts(forced_resolution) {
        let conflicted_files = conflict_entries_from_merge_outcome(
            gix_repository,
            merged_tree_id.detach(),
            &merge_result,
            forced_resolution,
        )?;

        // convert files into a string and save as a blob
        let conflicted_files_string = toml::to_string(&conflicted_files)?;
        let conflicted_files_blob = repo.blob(conflicted_files_string.as_bytes())?;

        // start from the auto-resolution tree, then persist conflict metadata alongside it
        let auto_resolution_tree = repo.find_tree(merged_tree_id.to_git2())?;
        let mut tree_writer = repo.treebuilder(Some(&auto_resolution_tree))?;

        // save the state of the conflict, so we can recreate it later
        tree_writer.insert(
            &*ConflictedTreeKey::Ours,
            incoming_tree.detach().to_git2(),
            0o040000,
        )?;
        tree_writer.insert(
            &*ConflictedTreeKey::Theirs,
            target_tree.detach().to_git2(),
            0o040000,
        )?;
        tree_writer.insert(
            &*ConflictedTreeKey::Base,
            base_tree.detach().to_git2(),
            0o040000,
        )?;
        tree_writer.insert(
            &*ConflictedTreeKey::AutoResolution,
            merged_tree_id.to_git2(),
            0o040000,
        )?;
        tree_writer.insert(
            &*ConflictedTreeKey::ConflictFiles,
            conflicted_files_blob,
            0o100644,
        )?;

        tree_oid = tree_writer.write().context("failed to write tree")?;
        true
    } else {
        tree_oid = merged_tree_id.to_git2();
        false
    };

    let message = if is_conflicted {
        but_core::commit::add_conflict_markers(resulting_name.as_bytes().as_bstr())
    } else {
        resulting_name.into()
    };

    let (author, committer) = gix_repository.commit_signatures()?;
    let commit_oid = crate::commit_with_signature_gix(
        gix_repository,
        None,
        author,
        committer,
        message.as_ref(),
        tree_oid.to_gix(),
        &[target_commit.id, incoming_commit.id],
        Some({
            #[expect(
                deprecated,
                reason = "We should use a synthetic ID instead, but that needs the existing commit id if available"
            )]
            Headers::new_with_random_change_id()
        }),
    )
    .context("failed to create commit")?;

    Ok(commit_oid)
}
