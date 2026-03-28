use std::path::PathBuf;

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_core::{
    RepositoryExt,
    commit::{ConflictEntries, Headers},
};
use but_oxidize::{ObjectIdExt as _, OidExt as _};
use gitbutler_cherry_pick::{ConflictedTreeKey, GixRepositoryExt as _};

fn extract_conflicted_files(
    merged_tree_id: gix::Id<'_>,
    merge_result: gix::merge::tree::Outcome<'_>,
    treat_as_unresolved: gix::merge::tree::TreatAsUnresolved,
) -> Result<ConflictEntries> {
    use gix::index::entry::Stage;
    let repo = merged_tree_id.repo;
    let mut index = repo.index_from_tree(&merged_tree_id)?;
    merge_result.index_changed_after_applying_conflicts(
        &mut index,
        treat_as_unresolved,
        gix::merge::tree::apply_index_entries::RemovalMode::Mark,
    );
    let (mut ancestor_entries, mut our_entries, mut their_entries) =
        (Vec::new(), Vec::new(), Vec::new());
    for entry in index.entries() {
        let stage = entry.stage();
        let storage = match stage {
            Stage::Unconflicted => {
                continue;
            }
            Stage::Base => &mut ancestor_entries,
            Stage::Ours => &mut our_entries,
            Stage::Theirs => &mut their_entries,
        };

        let path = entry.path(&index);
        storage.push(gix::path::from_bstr(path).into_owned());
    }
    let mut out = ConflictEntries {
        ancestor_entries,
        our_entries,
        their_entries,
    };

    // Since we typically auto-resolve with 'ours', it maybe that conflicting entries don't have an
    // unconflicting counterpart anymore, so they are not applied (which is also what Git does).
    // So to have something to show for - we *must* produce a conflict, extract paths manually.
    // TODO(ST): instead of doing this, don't pre-record the paths. Instead redo the merge without
    //           merge-strategy so that the index entries can be used instead.
    if !out.has_entries() {
        fn push_unique(v: &mut Vec<PathBuf>, change: &gix::diff::tree_with_rewrites::Change) {
            let path = gix::path::from_bstr(change.location()).into_owned();
            if !v.contains(&path) {
                v.push(path);
            }
        }
        for conflict in merge_result
            .conflicts
            .iter()
            .filter(|c| c.is_unresolved(treat_as_unresolved))
        {
            let (ours, theirs) = conflict.changes_in_resolution();
            push_unique(&mut out.our_entries, ours);
            push_unique(&mut out.their_entries, theirs);
        }
    }
    assert_eq!(
        out.has_entries(),
        merge_result.has_unresolved_conflicts(treat_as_unresolved),
        "Must have entries to indicate conflicting files, or bad things will happen later: {:#?}",
        merge_result.conflicts
    );
    Ok(out)
}

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
    let gix_repo = but_core::open_repo_for_merging(gix_repository.path())?;
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
    let commit_headers = if merge_result.has_unresolved_conflicts(forced_resolution) {
        let conflicted_files =
            extract_conflicted_files(merged_tree_id, merge_result, forced_resolution)?;

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

        // in case someone checks this out with vanilla Git, we should warn why it looks like this
        let readme_content =
            b"You have checked out a GitButler Conflicted commit. You probably didn't mean to do this.";
        let readme_blob = repo.blob(readme_content)?;
        tree_writer.insert("CONFLICT-README.txt", readme_blob, 0o100644)?;

        tree_oid = tree_writer.write().context("failed to write tree")?;
        conflicted_files.to_headers()
    } else {
        tree_oid = merged_tree_id.to_git2();
        Headers::new_with_random_change_id()
    };

    let (author, committer) = gix_repository.commit_signatures()?;
    let commit_oid = crate::commit_with_signature_gix(
        gix_repository,
        None,
        author,
        committer,
        resulting_name.as_bytes().as_bstr(),
        tree_oid.to_gix(),
        &[target_commit.id, incoming_commit.id],
        Some(commit_headers),
    )
    .context("failed to create commit")?;

    Ok(commit_oid)
}
pub fn gitbutler_merge_commits<'repo>(
    repo: &'repo git2::Repository,
    target_commit: git2::Commit<'repo>,
    incoming_commit: git2::Commit<'repo>,
    target_branch_name: &str,
    incoming_branch_name: &str,
) -> Result<git2::Commit<'repo>> {
    let gix_repo = gix::open(repo.path())?;
    let result_oid = merge_commits(
        &gix_repo,
        target_commit.id().to_gix(),
        incoming_commit.id().to_gix(),
        &format!("Merge `{incoming_branch_name}` into `{target_branch_name}`"),
    )?;

    Ok(repo.find_commit(result_oid.to_git2())?)
}

trait ToHeaders {
    /// Assure that the returned headers will always indicate a conflict.
    /// This is a fail-safe in case this instance has no paths stored as auto-resolution
    /// removed the path that would otherwise be conflicting.
    /// In other words: conflicting index entries aren't reliable when conflicts were resolved
    /// with the 'ours' strategy.
    fn to_headers(&self) -> Headers;
}

impl ToHeaders for ConflictEntries {
    fn to_headers(&self) -> Headers {
        Headers {
            conflicted: Some({
                let entries = self.total_entries();
                if entries > 0 { entries as u64 } else { 1 }
            }),
            ..Headers::new_with_random_change_id()
        }
    }
}
