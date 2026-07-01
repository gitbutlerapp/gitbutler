/// How to perform a cherry-pick.
#[derive(Debug, Copy, Clone)]
pub enum PickMode {
    /// No matter what, rebase one commit onto the other, creating a new commit in the process.
    ///
    /// This is useful if the list of commits to rebase is known to actually need a rebase.
    Unconditionally,
    /// Do not actually do anything if the commit to pick is already on the desired parent.
    /// This useful if the list of `commits_to_rebase` includes commits that don't need a change.
    // Note: this is more for older code which provides more commits than would be needed for
    // an operation, for if the UI lists everything because it makes the code easier.
    SkipIfNoop,
}

/// How to deal with commits that are empty after cherry-picking.
#[derive(Debug, Copy, Clone)]
pub enum EmptyCommit {
    /// Keep the empty commit.
    Keep,
    /// Instead of the empty commit, keep only the previous one, effectively
    /// dropping the commit whose tree didn't differ compared to the previous one.
    UsePrevious,
}

use anyhow::{Context as _, bail};
use bstr::BString;
use but_core::commit::{
    HEADERS_CONFLICTED_FIELD, Headers, SignCommit, TreeKind, conflict_entries_from_merge_outcome,
};
use but_error::bail_precondition;
use gix::{object::tree::EntryKind, prelude::ObjectIdExt};

use crate::commit::DateMode;

/// Place `commit_to_rebase` onto `base`.
///
/// `pick_mode` and `empty_commit` control how to deal with no-ops and empty commits.
/// Returns the id of the cherry-picked commit.
///
/// Note that the rewritten commit will have headers injected, among which is a change id.
pub fn cherry_pick_one(
    repo: &gix::Repository,
    base: gix::ObjectId,
    commit_to_rebase: gix::ObjectId,
    pick_mode: PickMode,
    empty_commit: EmptyCommit,
) -> anyhow::Result<gix::ObjectId> {
    let base = but_core::Commit::from_id(base.attach(repo))?;
    let to_rebase = but_core::Commit::from_id(commit_to_rebase.attach(repo))?;
    Ok(cherry_pick_one_inner(base, to_rebase, pick_mode, empty_commit)?.detach())
}

fn cherry_pick_one_inner<'repo>(
    base: but_core::Commit<'repo>,
    commit_to_rebase: but_core::Commit<'repo>,
    pick_mode: PickMode,
    empty_commit: EmptyCommit,
) -> anyhow::Result<gix::Id<'repo>> {
    if commit_to_rebase.parents.len() > 1 {
        bail_precondition!("Cannot yet cherry-pick merge-commits - use rebasing for that")
    }
    if matches!(pick_mode, PickMode::SkipIfNoop)
        && commit_to_rebase.parents.contains(&base.id.detach())
    {
        return Ok(commit_to_rebase.id);
    };

    let mut cherry_pick = cherry_pick_tree(&base, &commit_to_rebase)?;
    let tree_id = cherry_pick.tree.write()?;

    let conflict_kind = gix::merge::tree::TreatAsUnresolved::forced_resolution();
    if cherry_pick.has_unresolved_conflicts(conflict_kind) {
        commit_from_conflicted_tree(base, commit_to_rebase, tree_id, cherry_pick, conflict_kind)
    } else {
        commit_from_unconflicted_tree(base, commit_to_rebase, tree_id, empty_commit)
    }
}

fn set_parent(to_rebase: &mut gix::objs::Commit, new_parent: gix::ObjectId) -> anyhow::Result<()> {
    if to_rebase.parents.len() > 1 {
        bail!(
            "Cherry picks can only be done for single-parent commits. Merge-commits need to be re-merged"
        )
    }
    to_rebase.parents.clear();
    to_rebase.parents.push(new_parent);
    Ok(())
}

/// Rebase `to_rebase` onto `new_base`, dealing with the intricacies of conflicted trees, and return the newly
/// merged tree.
/// Note that all merges are made to succeed, possibly recording the original trees in a special tree.
fn cherry_pick_tree<'repo>(
    new_base: &but_core::Commit<'repo>,
    to_rebase: &but_core::Commit<'repo>,
) -> anyhow::Result<gix::merge::tree::Outcome<'repo>> {
    let repo = to_rebase.id.repo;
    let (base, ours, theirs) = find_cherry_pick_trees(new_base, to_rebase)?;
    use but_core::RepositoryExt;
    repo.merge_trees(
        base,
        ours,
        theirs,
        repo.default_merge_labels(),
        repo.merge_options_force_ours()?,
    )
    .context("failed to merge trees for cherry pick")
}

/// Return `(base, ours, theirs)` suitable for cherry-pick merges from the `new_base` for `to_rebase`.
fn find_cherry_pick_trees<'repo>(
    new_base: &but_core::Commit<'repo>,
    to_rebase: &but_core::Commit<'repo>,
) -> anyhow::Result<(gix::Id<'repo>, gix::Id<'repo>, gix::Id<'repo>)> {
    let repo = to_rebase.id.repo;
    // we need to do a manual 3-way patch merge
    // find the base, which is the parent of to_rebase
    let base = if to_rebase.is_conflicted() {
        // Use to_rebase's recorded base
        find_real_tree(to_rebase, TreeKind::Base)?
    } else {
        let base_commit_id = to_rebase.parents.first().context("no parent")?;
        // Use the parent's auto-resolution
        let base_commit = but_core::Commit::from_id(base_commit_id.attach(repo))?;
        find_real_tree(&base_commit, TreeKind::AutoResolution)?
    };
    // Get the auto-resolution
    let ours = find_real_tree(new_base, TreeKind::AutoResolution)?;
    // Get the original theirs
    let theirs = find_real_tree(to_rebase, TreeKind::Theirs)?;
    Ok((base, ours, theirs))
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
    head: but_core::Commit<'repo>,
    to_rebase: but_core::Commit<'repo>,
    resolved_tree_id: gix::Id<'repo>,
    empty_commit: EmptyCommit,
) -> anyhow::Result<gix::Id<'repo>> {
    let repo = head.id.repo;
    // Remove empty commits
    if matches!(empty_commit, EmptyCommit::UsePrevious)
        && resolved_tree_id == head.tree_id_or_auto_resolution()?
    {
        return Ok(head.id);
    }

    let headers = to_rebase.headers();
    let mut new_commit = to_rebase.inner;
    new_commit.tree = resolved_tree_id.detach();

    // Assure the commit isn't thinking it's conflicted.
    new_commit.message = but_core::commit::strip_conflict_markers(new_commit.message.as_ref());
    if let Some(pos) = new_commit
        .extra_headers()
        .find_pos(HEADERS_CONFLICTED_FIELD)
    {
        new_commit.extra_headers.remove(pos);
    } else if headers.is_none() {
        let headers = Headers::from_config(&repo.config_snapshot());
        new_commit
            .extra_headers
            .extend(Vec::<(BString, BString)>::from(&headers));
    }
    set_parent(&mut new_commit, head.id.detach())?;
    Ok(crate::commit::create(
        repo,
        new_commit,
        DateMode::CommitterUpdateAuthorKeep,
        SignCommit::IfSignCommitsEnabled,
    )?
    .attach(repo))
}

fn commit_from_conflicted_tree<'repo>(
    head: but_core::Commit<'repo>,
    mut to_rebase: but_core::Commit<'repo>,
    resolved_tree_id: gix::Id<'repo>,
    cherry_pick: gix::merge::tree::Outcome<'_>,
    treat_as_unresolved: gix::merge::tree::TreatAsUnresolved,
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

    // save the state of the conflict, so we can recreate it later
    let (base_tree_id, ours_tree_id, theirs_tree_id) = find_cherry_pick_trees(&head, &to_rebase)?;
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
        .unwrap_or_else(|| Headers::from_config(&repo.config_snapshot()));
    headers.conflicted = None;
    to_rebase.tree = tree.write().context("failed to write tree")?.detach();
    set_parent(&mut to_rebase, head.id.detach())?;

    // Add conflict markers to the commit message
    to_rebase.inner.message =
        but_core::commit::add_conflict_markers(to_rebase.inner.message.as_ref());

    to_rebase.set_headers(&headers);
    Ok(crate::commit::create(
        repo,
        to_rebase.inner,
        DateMode::CommitterUpdateAuthorKeep,
        SignCommit::IfSignCommitsEnabled,
    )?
    .attach(repo))
}
