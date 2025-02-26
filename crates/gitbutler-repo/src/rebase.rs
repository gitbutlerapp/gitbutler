use std::{collections::HashSet, path::PathBuf};

use crate::{
    logging::{LogUntil, RepositoryExt as _},
    RepositoryExt as _,
};
use anyhow::{Context, Result};
use but_rebase::cherry_pick::{EmptyCommit, PickMode};
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt};
use gitbutler_command_context::{gix_repository_for_merging, CommandContext};
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_oxidize::{GixRepositoryExt, ObjectIdExt, OidExt};
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// cherry-pick based rebase, which handles empty commits
/// this function takes a commit range and generates a Vector of commit oids
/// and then passes them to `cherry_rebase_group` to rebase them onto the target commit
///
/// Returns the new head commit id
pub fn cherry_rebase(
    ctx: &CommandContext,
    target_commit_oid: git2::Oid,
    to_commit_oid: git2::Oid,
    from_commit_oid: git2::Oid,
) -> Result<Option<git2::Oid>> {
    // get a list of the commits to rebase
    let ids_to_rebase = ctx
        .repo()
        .l(from_commit_oid, LogUntil::Commit(to_commit_oid), false)?;

    if ids_to_rebase.is_empty() {
        return Ok(None);
    }

    let new_head_id =
        cherry_rebase_group(ctx.repo(), target_commit_oid, &ids_to_rebase, false, false)?;

    Ok(Some(new_head_id))
}

/// takes a vector of commit oids and rebases them onto a target commit and returns the
/// new head commit oid if it's successful
/// the difference between this and a libgit2 based rebase is that this will successfully
/// rebase empty commits (two commits with identical trees)
///
/// the commit id's to rebase should be ordered such that the child most commit is first
#[instrument(level = tracing::Level::DEBUG, skip(repository, ids_to_rebase))]
pub fn cherry_rebase_group(
    repository: &git2::Repository,
    target_commit_oid: git2::Oid,
    ids_to_rebase: &[git2::Oid],
    always_rebase: bool,
    allow_empty_commit: bool,
) -> Result<git2::Oid> {
    let repo = gix_repository_for_merging(repository.path())?;
    let new_commit_id = cherry_pick_many(
        &repo,
        target_commit_oid.to_gix(),
        ids_to_rebase.iter().map(|id| id.to_gix()),
        if always_rebase {
            PickMode::Unconditionally
        } else {
            PickMode::SkipIfNoop
        },
        if allow_empty_commit {
            EmptyCommit::Keep
        } else {
            EmptyCommit::UsePrevious
        },
    )?;
    Ok(new_commit_id.to_git2())
}

/// Place `commits_to_rebase` onto `base` in-order, i.e. `base -> 0 -> 1 -> N`, so that that last
/// commit in `commits_to_rebase` is the last commit to rebase.
/// If `commits_to_rebase` is empty, `base` is returned unaltered.
///
/// `pick_mode` and `empty_commit` control how to deal with no-ops and epty commits.
///
/// Returns the id of the top-most, rebased commit.
///
/// Note that each rewritten commit will have headers injected, among which is a change id.
///
/// ### Superseded!
///
/// This is just use to unify code, cherry-pick-many is fully replaced by `but_rebase::rebase()`
#[instrument(level = tracing::Level::DEBUG, skip(repo, commits_to_rebase))]
fn cherry_pick_many(
    repo: &gix::Repository,
    base: gix::ObjectId,
    commits_to_rebase: impl DoubleEndedIterator<Item = gix::ObjectId>,
    pick_mode: PickMode,
    empty_commit: EmptyCommit,
) -> anyhow::Result<gix::ObjectId> {
    let mut cursor = base;
    let mut maybe_previous_base = None;
    for to_rebase_id in commits_to_rebase.rev() {
        cursor = but_rebase::cherry_pick_one(
            repo,
            maybe_previous_base,
            cursor,
            to_rebase_id,
            pick_mode,
            empty_commit,
        )?;
        maybe_previous_base = Some(to_rebase_id);
    }
    Ok(cursor)
}

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
    let repository = git2::Repository::open(gix_repository.path())?;
    let target_commit = repository.find_commit(target_commit.to_git2())?;
    let incoming_commit = repository.find_commit(incoming_commit.to_git2())?;
    let merge_base = repository.merge_base(target_commit.id(), incoming_commit.id())?;
    let merge_base = repository.find_commit(merge_base)?;

    let base_tree = repository.find_real_tree(&merge_base, Default::default())?;
    // We want to use the auto-resolution when computing the merge, but for
    // reconstructing it later, we want the "theirsiest" and "oursiest" trees
    let target_tree = repository.find_real_tree(&target_commit, ConflictedTreeKey::Theirs)?;
    let incoming_tree = repository.find_real_tree(&incoming_commit, ConflictedTreeKey::Ours)?;

    let target_merge_tree = repository.find_real_tree(&target_commit, Default::default())?;
    let incoming_merge_tree = repository.find_real_tree(&incoming_commit, Default::default())?;
    let gix_repo = gix_repository_for_merging(repository.path())?;
    let mut merge_result = gix_repo.merge_trees(
        base_tree.id().to_gix(),
        incoming_merge_tree.id().to_gix(),
        target_merge_tree.id().to_gix(),
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
        let conflicted_files_blob = repository.blob(conflicted_files_string.as_bytes())?;

        // create a treewriter
        let mut tree_writer = repository.treebuilder(None)?;

        // save the state of the conflict, so we can recreate it later
        tree_writer.insert(&*ConflictedTreeKey::Ours, incoming_tree.id(), 0o040000)?;
        tree_writer.insert(&*ConflictedTreeKey::Theirs, target_tree.id(), 0o040000)?;
        tree_writer.insert(&*ConflictedTreeKey::Base, base_tree.id(), 0o040000)?;
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
        let readme_blob = repository.blob(readme_content)?;
        tree_writer.insert("README.txt", readme_blob, 0o100644)?;

        tree_oid = tree_writer.write().context("failed to write tree")?;
        conflicted_files.to_headers()
    } else {
        tree_oid = merged_tree_id.to_git2();
        CommitHeadersV2::default()
    };

    let (author, committer) = repository.signatures()?;
    let commit_oid = crate::RepositoryExt::commit_with_signature(
        &repository,
        None,
        &author,
        &committer,
        resulting_name,
        &repository
            .find_tree(tree_oid)
            .context("failed to find tree")?,
        &[&target_commit, &incoming_commit],
        Some(commit_headers),
    )
    .context("failed to create commit")?;

    Ok(commit_oid.to_gix())
}
pub fn gitbutler_merge_commits<'repository>(
    repository: &'repository git2::Repository,
    target_commit: git2::Commit<'repository>,
    incoming_commit: git2::Commit<'repository>,
    target_branch_name: &str,
    incoming_branch_name: &str,
) -> Result<git2::Commit<'repository>> {
    let gix_repository = gix::open(repository.path())?;
    let result_oid = merge_commits(
        &gix_repository,
        target_commit.id().to_gix(),
        incoming_commit.id().to_gix(),
        &format!(
            "Merge `{}` into `{}`",
            incoming_branch_name, target_branch_name
        ),
    )?;

    Ok(repository.find_commit(result_oid.to_git2())?)
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntries {
    pub ancestor_entries: Vec<PathBuf>,
    our_entries: Vec<PathBuf>,
    their_entries: Vec<PathBuf>,
}

impl ConflictEntries {
    pub fn has_entries(&self) -> bool {
        !self.ancestor_entries.is_empty()
            || !self.our_entries.is_empty()
            || !self.their_entries.is_empty()
    }

    pub fn total_entries(&self) -> usize {
        let set = self
            .ancestor_entries
            .iter()
            .chain(self.our_entries.iter())
            .chain(self.their_entries.iter())
            .collect::<HashSet<_>>();

        set.len()
    }

    /// Assure that the returned headers will always indicate a conflict.
    /// This is a fail-safe in case this instance has no paths stored as auto-resolution
    /// removed the path that would otherwise be conflicting.
    /// In other words: conflicting index entries aren't reliable when conflicts were resolved
    /// with the 'ours' strategy.
    fn to_headers(&self) -> CommitHeadersV2 {
        CommitHeadersV2 {
            conflicted: Some({
                let entries = self.total_entries();
                if entries > 0 {
                    entries as u64
                } else {
                    1
                }
            }),
            ..Default::default()
        }
    }
}
