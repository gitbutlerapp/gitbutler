//! Cherry picking generalised for N to M cases.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use bstr::BString;
use but_core::RepositoryExt as _;
use but_core::commit::{HEADERS_CONFLICTED_FIELD, HeadersV2, TreeKind};
use gix::{objs::tree::EntryKind, prelude::ObjectIdExt as _};

use crate::{cherry_pick::function::ConflictEntries, commit::DateMode};

/// Describes the outcome of cherrypick.
#[derive(Debug, Clone, Copy)]
pub enum CherryPickOutcome {
    /// Successfully cherry picked cleanly.
    Commit(gix::ObjectId),
    /// Successfully cherry picked and created a conflicted commit.
    ConflictedCommit(gix::ObjectId),
    /// No cherry pick was required since all the parents remained the same.
    Identity(gix::ObjectId),
    /// Represents the cases where either the source or the target commits failed to
    /// merge cleanly.
    FailedToMergeBases,
}

/// Cherry pick, but supports supports cherry-picking merge commits.
///
/// When cherry-picking a commit onto two or more commits, we first find the
/// merge of the two "onto" commits, and then cherry-pick onto that.
///
/// When cherry-picking a commit with two or more parents, we first find the
/// merge of the parents, and use that as the "base".
///
/// EG, if I've got commit X, with the parents A and B, and I want to cherry
/// pick onto C and D.
/// - I first find the merge of A and B, which I'll call N.
/// - Then, I find the merge of C and D, which I'll call M.
/// - Then, I do the three way merge where N is the "base", M is "ours", and X
///   is "theirs".
///
/// Except in the case where X is conflicted. In that case we then make use of
/// X's "base" sub-tree as the base.
pub fn cherry_pick(
    repo: &gix::Repository,
    target: gix::ObjectId,
    ontos: &[gix::ObjectId],
) -> Result<CherryPickOutcome> {
    let target = but_core::Commit::from_id(target.attach(repo))?;
    if ontos == target.parents.as_slice() {
        // We don't need to rebase
        return Ok(CherryPickOutcome::Identity(target.id.detach()));
    }

    let base_t = find_base_tree(&target)?;
    // We always want the "theirs-ist" side of the target if it's conflicted.
    let target_t = find_real_tree(&target, TreeKind::Theirs)?;
    // We want to cherry-pick onto the merge result.
    let onto_t = tree_from_merging_commits(repo, ontos, TreeKind::AutoResolution)?;

    match (base_t, onto_t) {
        (MergeOutcome::NoCommit, MergeOutcome::NoCommit) => {
            // We shouldn't actually ever hit this because it should be handled
            // by the ontos & parents comparison.
            Ok(CherryPickOutcome::Identity(target.id.detach()))
        }
        (MergeOutcome::Conflict, _) | (_, MergeOutcome::Conflict) => {
            // TODO(cto): We can handle the specific case where (the base is
            // _not_ conflicted & the ontos _is_ conflicted & the target == base
            // & ontos.len() == 2) by writing out the ontos conflict as a
            // conflicted merge commit, without expanding our conflict
            // representation.
            Ok(CherryPickOutcome::FailedToMergeBases)
        }
        (
            MergeOutcome::Success(_) | MergeOutcome::NoCommit,
            MergeOutcome::Success(_) | MergeOutcome::NoCommit,
        ) => {
            let empty_tree = gix::ObjectId::empty_tree(gix::hash::Kind::Sha1);
            let base_t = base_t.object_id().unwrap_or(empty_tree);
            let onto_t = onto_t.object_id().unwrap_or(empty_tree);

            let mut outcome = repo.merge_trees(
                base_t,
                onto_t,
                target_t,
                repo.default_merge_labels(),
                repo.merge_options_force_ours()?,
            )?;
            let tree_id = outcome.tree.write()?;

            let conflict_kind = gix::merge::tree::TreatAsUnresolved::forced_resolution();
            if outcome.has_unresolved_conflicts(conflict_kind) {
                let conflicted_commit = commit_from_conflicted_tree(
                    ontos,
                    target,
                    tree_id,
                    outcome,
                    conflict_kind,
                    base_t,
                    onto_t,
                    target_t.detach(),
                )?;
                Ok(CherryPickOutcome::ConflictedCommit(
                    conflicted_commit.detach(),
                ))
            } else {
                Ok(CherryPickOutcome::Commit(
                    commit_from_unconflicted_tree(ontos, target, tree_id)?.detach(),
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MergeOutcome {
    Success(gix::ObjectId),
    NoCommit,
    Conflict,
}

impl MergeOutcome {
    fn object_id(&self) -> Option<gix::ObjectId> {
        match self {
            Self::Success(oid) => Some(*oid),
            _ => None,
        }
    }
}

fn find_base_tree(target: &but_core::Commit) -> Result<MergeOutcome> {
    if target.is_conflicted() {
        Ok(MergeOutcome::Success(
            find_real_tree(target, TreeKind::Base)?.detach(),
        ))
    } else {
        tree_from_merging_commits(target.id.repo, &target.parents, TreeKind::AutoResolution)
    }
}

/// Merge together many commits, making use of the preferenced tree.
fn tree_from_merging_commits(
    repo: &gix::Repository,
    commits: &[gix::ObjectId],
    preference: TreeKind,
) -> Result<MergeOutcome> {
    let mut to_merge = commits.to_vec();
    let Some(sum) = to_merge.pop() else {
        // Handle the case where no commits are given.
        return Ok(MergeOutcome::NoCommit);
    };
    let mut sum = find_real_tree(&but_core::Commit::from_id(sum.attach(repo))?, preference)?;

    let base_tree = match repo.merge_base_octopus(commits.to_owned()) {
        Ok(oid) => {
            let commit = but_core::Commit::from_id(oid)?;
            find_real_tree(&commit, TreeKind::AutoResolution)?.detach()
        }
        // It's very possible we'll see scenarios where there are two parents
        // that have no common ancestor. We should handle that well by using the
        // empty tree as the base.
        Err(gix::repository::merge_base_octopus::Error::MergeBaseOctopus(
            gix::repository::merge_base_octopus_with_graph::Error::NoMergeBase,
        )) => gix::ObjectId::empty_tree(gix::hash::Kind::Sha1),
        Err(e) => bail!(e),
    };

    while let Some(commit) = to_merge.pop() {
        let commit = but_core::Commit::from_id(commit.attach(repo))?;
        let tree = find_real_tree(&commit, preference)?;
        let (options, conflicts) = repo.merge_options_fail_fast()?;

        let mut output =
            repo.merge_trees(base_tree, sum, tree, repo.default_merge_labels(), options)?;

        if output.has_unresolved_conflicts(conflicts) {
            return Ok(MergeOutcome::Conflict);
        }

        sum = output.tree.write()?;
    }

    Ok(MergeOutcome::Success(sum.detach()))
}

fn find_real_tree<'repo>(
    commit: &but_core::Commit<'repo>,
    side: TreeKind,
) -> anyhow::Result<gix::Id<'repo>> {
    Ok(if commit.is_conflicted() {
        let tree = commit.id.repo.find_tree(commit.tree)?;
        let conflicted_side = tree
            .find_entry(side.as_tree_entry_name())
            .context("Failed to get conflicted side of commit")?;
        conflicted_side.id()
    } else {
        commit.tree_id_or_auto_resolution()?
    })
}

fn commit_from_unconflicted_tree<'repo>(
    parents: &[gix::ObjectId],
    to_rebase: but_core::Commit<'repo>,
    resolved_tree_id: gix::Id<'repo>,
) -> anyhow::Result<gix::Id<'repo>> {
    let repo = to_rebase.id.repo;

    let headers = to_rebase.headers();
    let to_rebase_is_conflicted = headers.as_ref().is_some_and(|hdr| hdr.is_conflicted());
    let mut new_commit = to_rebase.inner;
    new_commit.tree = resolved_tree_id.detach();

    // Ensure the commit isn't thinking it's conflicted.
    if to_rebase_is_conflicted {
        if let Some(pos) = new_commit
            .extra_headers()
            .find_pos(HEADERS_CONFLICTED_FIELD)
        {
            new_commit.extra_headers.remove(pos);
        }
    } else if headers.is_none() {
        new_commit
            .extra_headers
            .extend(Vec::<(BString, BString)>::from(&HeadersV2::from_config(
                &repo.config_snapshot(),
            )));
    }
    new_commit.parents = parents.into();

    Ok(crate::commit::create(repo, new_commit, DateMode::CommitterUpdateAuthorKeep)?.attach(repo))
}

#[allow(clippy::too_many_arguments)]
fn commit_from_conflicted_tree<'repo>(
    parents: &[gix::ObjectId],
    mut to_rebase: but_core::Commit<'repo>,
    resolved_tree_id: gix::Id<'repo>,
    cherry_pick: gix::merge::tree::Outcome<'_>,
    treat_as_unresolved: gix::merge::tree::TreatAsUnresolved,
    base_tree_id: gix::ObjectId,
    ours_tree_id: gix::ObjectId,
    theirs_tree_id: gix::ObjectId,
) -> anyhow::Result<gix::Id<'repo>> {
    let repo = resolved_tree_id.repo;
    // in case someone checks this out with vanilla Git, we should warn why it looks like this
    let readme_content =
        b"You have checked out a GitButler Conflicted commit. You probably didn't mean to do this.";
    let readme_blob = repo.write_blob(readme_content)?;

    let conflicted_files =
        extract_conflicted_files(resolved_tree_id, cherry_pick, treat_as_unresolved)?;

    // convert files into a string and save as a blob
    let conflicted_files_string = toml::to_string(&conflicted_files)?;
    let conflicted_files_blob = repo.write_blob(conflicted_files_string.as_bytes())?;

    let mut tree = repo.empty_tree().edit()?;

    tree.upsert(
        TreeKind::Ours.as_tree_entry_name(),
        EntryKind::Tree,
        ours_tree_id,
    )?;
    tree.upsert(
        TreeKind::Theirs.as_tree_entry_name(),
        EntryKind::Tree,
        theirs_tree_id,
    )?;
    tree.upsert(
        TreeKind::Base.as_tree_entry_name(),
        EntryKind::Tree,
        base_tree_id,
    )?;
    tree.upsert(
        TreeKind::AutoResolution.as_tree_entry_name(),
        EntryKind::Tree,
        resolved_tree_id,
    )?;
    tree.upsert(".conflict-files", EntryKind::Blob, conflicted_files_blob)?;
    tree.upsert("README.txt", EntryKind::Blob, readme_blob)?;

    let mut headers = to_rebase
        .headers()
        .unwrap_or_else(|| HeadersV2::from_config(&repo.config_snapshot()));
    headers.conflicted = conflicted_files.conflicted_header_field();
    to_rebase.tree = tree.write().context("failed to write tree")?.detach();
    to_rebase.parents = parents.into();

    to_rebase.set_headers(&headers);
    Ok(
        crate::commit::create(repo, to_rebase.inner, DateMode::CommitterUpdateAuthorKeep)?
            .attach(repo),
    )
}

fn extract_conflicted_files(
    merged_tree_id: gix::Id<'_>,
    merge_result: gix::merge::tree::Outcome<'_>,
    treat_as_unresolved: gix::merge::tree::TreatAsUnresolved,
) -> anyhow::Result<ConflictEntries> {
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
