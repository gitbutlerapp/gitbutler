//! Cherry picking generalised for N to M cases.

use crate::commit::DateMode;
use anyhow::{Context as _, Result, bail};
use bstr::BString;
use but_core::{
    RepositoryExt as _,
    commit::{
        HEADERS_CONFLICTED_FIELD, Headers, SignCommit, TreeKind,
        conflict_entries_from_merge_outcome,
    },
};
use gix::{objs::tree::EntryKind, prelude::ObjectIdExt as _};

/// Describes the outcome of cherrypick.
#[derive(Debug, Clone)]
pub enum CherryPickOutcome {
    /// Successfully cherry picked cleanly.
    Commit(gix::ObjectId),
    /// Successfully cherry picked and created a conflicted commit.
    ConflictedCommit(gix::ObjectId),
    /// No cherry pick was required since all the parents remained the same.
    Identity(gix::ObjectId),
    /// Represents the cases where either the source or the target commits failed to
    /// merge cleanly.
    FailedToMergeBases {
        /// Whther the merge operation performed on the list of bases failed.
        base_merge_failed: bool,
        /// The shas of the commits that we were trying to cherry pick from.
        bases: Option<Vec<gix::ObjectId>>,
        /// Whther the merge operation performed on the list of ontos failed.
        onto_merge_failed: bool,
        /// The shas of the commits that we were trying to cherry pick onto.
        ontos: Option<Vec<gix::ObjectId>>,
    },
}

/// Controls how parent trees are merged during cherry-pick.
///
/// When cherry-picking a commit with multiple parents, both the old parents
/// (base) and the new parents (ontos) are merged pairwise. This enum controls
/// the merge options used for that pairwise merging.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TreeMergeMode {
    /// Merge with rename detection enabled (default).
    #[default]
    WithRenames,
    /// Merge with rename detection disabled.
    /// Useful when parents are independent and renames detected across them
    /// would be false positives.
    WithoutRenames,
}

/// Controls when a commit is cherry-picked during a rebase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickMode {
    /// Cherry-picks the commit only if it's necessary because of changes to the commit or its
    /// parents.
    IfChanged,
    /// Forces a cherry-pick on a commit. This is for example useful in combination with
    /// [`SignCommit`] to sign/unsign commits that are otherwise unchanged.
    Force,
}

/// Cherry pick, but supports cherry-picking merge commits.
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
///
/// Special case: when a synthetic merge template has no original parents, an
/// empty tree, and exactly two new parents whose trees conflict with each
/// other, we materialize that conflict as a GitButler conflicted merge commit
/// instead of returning [`CherryPickOutcome::FailedToMergeBases`].
///
/// `repo`
/// The repository that owns `target`, `ontos`, and any newly written commit or tree objects.
///
/// `target`
/// The commit to cherry-pick, possibly including conflicted-tree metadata or a synthetic merge template.
///
/// `ontos`
/// The parent commits that `target` should be replayed onto in the result graph.
///
/// `pick_mode`
/// Controls whether the cherry-pick may short-circuit to identity or must recreate the commit.
///
/// `tree_merge_mode`
/// Controls how parent trees are merged while constructing merge-commit bases and ontos.
///
/// `sign_commit`
/// Controls whether the newly written commit should be signed.
pub fn cherry_pick(
    repo: &gix::Repository,
    target: gix::ObjectId,
    ontos: &[gix::ObjectId],
    pick_mode: PickMode,
    tree_merge_mode: TreeMergeMode,
    sign_commit: SignCommit,
) -> Result<CherryPickOutcome> {
    let target = but_core::Commit::from_id(target.attach(repo))?;

    if ontos == target.parents.as_slice() && pick_mode != PickMode::Force {
        // We don't need to rebase
        return Ok(CherryPickOutcome::Identity(target.id.detach()));
    }

    let base_t = find_base_tree(&target, tree_merge_mode)?;
    // We always want the "theirs-ist" side of the target if it's conflicted.
    let target_t = find_real_tree(&target, TreeKind::Theirs)?;
    // We want to cherry-pick onto the merge result.
    let onto_t = merged_tree_from_commits(repo, ontos, tree_merge_mode, TreeKind::AutoResolution)?;

    match (&base_t, &onto_t) {
        (MergeOutcome::NoCommit, MergeOutcome::NoCommit) if pick_mode == PickMode::Force => {
            // We should only end up here when trying to force-pick a parentless commit. At that
            // point, it's safe to simply recreate that commit outright.
            //
            // Currently, the only known use case for this is to forcibly sign/unsign root commits.
            let commit = crate::commit::create(
                repo,
                target.inner,
                DateMode::CommitterUpdateAuthorKeep,
                sign_commit,
            )?;
            Ok(CherryPickOutcome::Commit(commit))
        }
        (MergeOutcome::NoCommit, MergeOutcome::NoCommit) => {
            // We shouldn't actually ever hit this because it should be handled
            // by the ontos & parents comparison or the PickMode::Force case.
            Ok(CherryPickOutcome::Identity(target.id.detach()))
        }
        (MergeOutcome::Conflict { commits: bases }, MergeOutcome::Conflict { commits: ontos }) => {
            Ok(CherryPickOutcome::FailedToMergeBases {
                base_merge_failed: true,
                bases: Some(bases.to_vec()),
                onto_merge_failed: true,
                ontos: Some(ontos.to_vec()),
            })
        }
        (MergeOutcome::Conflict { commits }, _) => Ok(CherryPickOutcome::FailedToMergeBases {
            base_merge_failed: true,
            bases: Some(commits.to_vec()),
            onto_merge_failed: false,
            ontos: None,
        }),
        (_, MergeOutcome::Conflict { commits }) => {
            if let Some(outcome) = maybe_materialize_conflicted_onto_merge(
                repo,
                &target,
                ontos,
                &base_t,
                target_t.detach(),
                tree_merge_mode,
                sign_commit,
            )? {
                Ok(outcome)
            } else {
                Ok(CherryPickOutcome::FailedToMergeBases {
                    base_merge_failed: false,
                    bases: None,
                    onto_merge_failed: true,
                    ontos: Some(commits.to_vec()),
                })
            }
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
                    sign_commit,
                )?;
                Ok(CherryPickOutcome::ConflictedCommit(
                    conflicted_commit.detach(),
                ))
            } else {
                Ok(CherryPickOutcome::Commit(
                    commit_from_unconflicted_tree(ontos, target, tree_id, sign_commit)?.detach(),
                ))
            }
        }
    }
}

/// Materialize the narrow synthetic-merge case where building the merged
/// `onto` tree conflicts before the normal final cherry-pick merge can run.
///
/// This helper only handles parentless, non-conflicted, empty-tree templates
/// that are being replayed onto exactly two commits with rename detection
/// enabled. In that situation we can treat the conflicting `onto` merge itself
/// as the desired result and wrap it in GitButler's conflicted-commit format.
///
/// `repo`
/// The repository that owns all commits and trees involved in the merge attempt.
///
/// `target`
/// The synthetic merge template commit that is being replayed onto `ontos`.
///
/// `ontos`
/// The two commits whose full trees should become the merge parents of the result.
///
/// `base_t`
/// The already-computed original-base merge outcome for `target`, used to confirm this is the parentless template case.
///
/// `target_tree_id`
/// The resolved tree of `target`, used to confirm the template starts from the empty tree.
///
/// `tree_merge_mode`
/// The merge mode requested by the caller; only `TreeMergeMode::WithRenames` is handled here.
///
/// `sign_commit`
/// Controls whether the conflicted or clean merge commit written by this helper should be signed.
fn maybe_materialize_conflicted_onto_merge(
    repo: &gix::Repository,
    target: &but_core::Commit<'_>,
    ontos: &[gix::ObjectId],
    base_t: &MergeOutcome,
    target_tree_id: gix::ObjectId,
    tree_merge_mode: TreeMergeMode,
    sign_commit: SignCommit,
) -> Result<Option<CherryPickOutcome>> {
    let empty_tree = gix::ObjectId::empty_tree(repo.object_hash());
    if !matches!(base_t, MergeOutcome::NoCommit)
        || target.is_conflicted()
        || !target.parents.is_empty()
        || target_tree_id != empty_tree
        || ontos.len() != 2
        || tree_merge_mode != TreeMergeMode::WithRenames
    {
        return Ok(None);
    }

    let base_tree_id = peel_to_tree_or_empty(repo, merge_base(repo, ontos[0], ontos[1])?)?.detach();
    let ours_tree_id = but_core::Commit::from_id(ontos[0].attach(repo))?
        .tree_id_or_auto_resolution()?
        .detach();
    let theirs_tree_id = but_core::Commit::from_id(ontos[1].attach(repo))?
        .tree_id_or_auto_resolution()?
        .detach();

    let mut outcome = repo.merge_trees(
        base_tree_id,
        ours_tree_id,
        theirs_tree_id,
        repo.default_merge_labels(),
        repo.merge_options_force_ours()?,
    )?;
    let tree_id = outcome.tree.write()?;
    let conflict_kind = gix::merge::tree::TreatAsUnresolved::forced_resolution();
    if !outcome.has_unresolved_conflicts(conflict_kind) {
        return Ok(Some(CherryPickOutcome::Commit(
            commit_from_unconflicted_tree(ontos, target.clone(), tree_id, sign_commit)?.detach(),
        )));
    }

    let conflicted_commit = commit_from_conflicted_tree(
        ontos,
        target.clone(),
        tree_id,
        outcome,
        conflict_kind,
        base_tree_id,
        ours_tree_id,
        theirs_tree_id,
        sign_commit,
    )?;
    Ok(Some(CherryPickOutcome::ConflictedCommit(
        conflicted_commit.detach(),
    )))
}

#[derive(Debug, Clone)]
enum MergeOutcome {
    Success(gix::ObjectId),
    NoCommit,
    Conflict {
        /// The commits that we were trying to merge together
        commits: Vec<gix::ObjectId>,
    },
}

impl MergeOutcome {
    fn object_id(&self) -> Option<gix::ObjectId> {
        match self {
            Self::Success(oid) => Some(*oid),
            _ => None,
        }
    }
}

fn find_base_tree(
    target: &but_core::Commit,
    tree_merge_mode: TreeMergeMode,
) -> Result<MergeOutcome> {
    if target.is_conflicted() {
        Ok(MergeOutcome::Success(
            find_real_tree(target, TreeKind::Base)?.detach(),
        ))
    } else {
        merged_tree_from_commits(
            target.id.repo,
            &target.parents,
            tree_merge_mode,
            TreeKind::AutoResolution,
        )
    }
}

/// Merge together many commits, making use of the preferred tree.
fn merged_tree_from_commits(
    repo: &gix::Repository,
    commits: &[gix::ObjectId],
    tree_merge_mode: TreeMergeMode,
    preference: TreeKind,
) -> Result<MergeOutcome> {
    let mut to_merge = commits.to_vec();
    to_merge.reverse();
    let Some(first) = to_merge.pop() else {
        // Handle the case where no commits are given.
        return Ok(MergeOutcome::NoCommit);
    };
    let mut sum = find_real_tree(&but_core::Commit::from_id(first.attach(repo))?, preference)?;

    let mut base: Option<Option<gix::ObjectId>> = None;

    while let Some(commit) = to_merge.pop() {
        if let Some(base_commit) = base {
            if let Some(base_commit) = base_commit {
                base = Some(merge_base(repo, base_commit, commit)?);
            }
        } else {
            base = Some(merge_base(repo, first, commit)?);
        }
        let Some(base) = base else {
            bail!("BUG: Base is None, this should never happen");
        };

        let commit = but_core::Commit::from_id(commit.attach(repo))?;
        let tree = find_real_tree(&commit, preference)?;
        // When parents are independent, rename detection can produce false
        // positives across them, silently swallowing clean file deletions.
        let (options, conflicts) = match tree_merge_mode {
            TreeMergeMode::WithRenames => repo.merge_options_fail_fast()?,
            TreeMergeMode::WithoutRenames => repo.merge_options_no_rewrites_fail_fast()?,
        };

        let mut output = repo.merge_trees(
            peel_to_tree_or_empty(repo, base)?,
            sum,
            tree,
            repo.default_merge_labels(),
            options,
        )?;

        if output.has_unresolved_conflicts(conflicts) {
            return Ok(MergeOutcome::Conflict {
                commits: commits.into(),
            });
        }

        sum = output.tree.write()?;
    }

    Ok(MergeOutcome::Success(sum.detach()))
}

fn merge_base(
    repo: &gix::Repository,
    first: gix::ObjectId,
    second: gix::ObjectId,
) -> Result<Option<gix::ObjectId>> {
    match repo.merge_base(first, second) {
        Ok(oid) => Ok(Some(oid.detach())),
        // It's very possible we'll see scenarios where there are two parents
        // that have no common ancestor. We should handle that well by using the
        // empty tree as the base.
        Err(gix::repository::merge_base::Error::FindMergeBase(_))
        | Err(gix::repository::merge_base::Error::NotFound { .. }) => Ok(None),
        Err(e) => bail!(e),
    }
}

fn peel_to_tree_or_empty(
    repo: &'_ gix::Repository,
    id: Option<gix::ObjectId>,
) -> Result<gix::Id<'_>> {
    Ok(match id {
        Some(id) => find_real_tree(
            &but_core::Commit::from_id(id.attach(repo))?,
            TreeKind::AutoResolution,
        )?,
        None => gix::ObjectId::empty_tree(gix::hash::Kind::Sha1).attach(repo),
    })
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
    sign_commit: SignCommit,
) -> anyhow::Result<gix::Id<'repo>> {
    let repo = to_rebase.id.repo;

    let headers = to_rebase.headers();
    let change_id = to_rebase.change_id();
    let mut new_commit = to_rebase.inner;
    new_commit.tree = resolved_tree_id.detach();

    // Ensure the commit isn't thinking it's conflicted.
    new_commit.message = but_core::commit::strip_conflict_markers(new_commit.message.as_ref());
    if let Some(pos) = new_commit
        .extra_headers()
        .find_pos(HEADERS_CONFLICTED_FIELD)
    {
        new_commit.extra_headers.remove(pos);
    } else if headers.is_none() {
        let headers = Headers::from_change_id(change_id);
        new_commit
            .extra_headers
            .extend(Vec::<(BString, BString)>::from(&headers));
    }

    new_commit.parents = parents.into();

    Ok(crate::commit::create(
        repo,
        new_commit,
        DateMode::CommitterUpdateAuthorKeep,
        sign_commit,
    )?
    .attach(repo))
}

#[expect(clippy::too_many_arguments)]
fn commit_from_conflicted_tree<'repo>(
    parents: &[gix::ObjectId],
    mut to_rebase: but_core::Commit<'repo>,
    resolved_tree_id: gix::Id<'repo>,
    cherry_pick: gix::merge::tree::Outcome<'_>,
    treat_as_unresolved: gix::merge::tree::TreatAsUnresolved,
    base_tree_id: gix::ObjectId,
    ours_tree_id: gix::ObjectId,
    theirs_tree_id: gix::ObjectId,
    sign_commit: SignCommit,
) -> anyhow::Result<gix::Id<'repo>> {
    let repo = resolved_tree_id.repo;

    let conflicted_files = conflict_entries_from_merge_outcome(
        repo,
        resolved_tree_id.detach(),
        &cherry_pick,
        treat_as_unresolved,
    )?;

    // convert files into a string and save as a blob
    let conflicted_files_string = toml::to_string(&conflicted_files)?;
    let conflicted_files_blob = repo.write_blob(conflicted_files_string.as_bytes())?;

    let mut tree = repo.find_tree(resolved_tree_id)?.edit()?;

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

    let mut headers = to_rebase
        .headers()
        .unwrap_or_else(|| Headers::from_change_id(to_rebase.change_id()));
    headers.conflicted = None;
    to_rebase.tree = tree.write().context("failed to write tree")?.detach();
    to_rebase.parents = parents.into();

    // Add conflict markers to the commit message
    to_rebase.inner.message =
        but_core::commit::add_conflict_markers(to_rebase.inner.message.as_ref());

    to_rebase.set_headers(&headers);
    Ok(crate::commit::create(
        repo,
        to_rebase.inner,
        DateMode::CommitterUpdateAuthorKeep,
        sign_commit,
    )?
    .attach(repo))
}
