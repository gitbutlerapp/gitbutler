use anyhow::{Context as _, Result, bail};
use bstr::BString;
use but_core::{RefMetadata, RepositoryExt as _};
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Step, SuccessfulRebase, mutate::InsertSide,
};
use gix::prelude::ObjectIdExt as _;
use serde::{Deserialize, Serialize};

use crate::{WorktreeId, db::get_worktree_meta, git::git_worktree_remove};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
/// This gets used as a public API in the CLI so be careful when modifying.
pub enum WorktreeIntegrationStatus {
    NoMergeBaseFound,
    WorktreeIsBare,
    /// The worktree's tree is identical to its base; there are no changes to
    /// bring back into the workspace.
    NothingToIntegrate,
    /// If we were to integrate this worktree back into the project, it would
    /// cause the workspace to conflict.
    ///
    /// If this is true, the worktree can't be integrated.
    CausesWorkspaceConflicts,
    Integratable {
        /// The cherry pick produced when integrating will be conflicted.
        cherry_pick_conflicts: bool,
        /// Commits above where this worktree will be cherry-picked are going to
        /// end up conflicted.
        commits_above_conflict: bool,
        /// Whether the uncommitted changes in the main checkout would conflict
        /// with the integration result. If this is true, integration aborts to
        /// keep those changes intact.
        working_dir_conflicts: bool,
    },
}

/// Determines whether a worktree can be integrated into `target`.
pub fn worktree_integration_status<M: RefMetadata>(
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    meta: &mut M,
    id: &WorktreeId,
    target: &gix::refs::FullNameRef,
) -> Result<WorktreeIntegrationStatus> {
    Ok(worktree_integration_inner(repo, ws, meta, id, target)?.0)
}

/// Integrates a worktree if it's integratable: the worktree's state is
/// squashed into a single commit which becomes the new tip of `target`,
/// then the linked worktree checkout is removed.
pub fn worktree_integrate<M: RefMetadata>(
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    meta: &mut M,
    id: &WorktreeId,
    target: &gix::refs::FullNameRef,
) -> Result<()> {
    let result = worktree_integration_inner(repo, ws, meta, id, target)?;
    let (WorktreeIntegrationStatus::Integratable { .. }, Some(rebase)) = result else {
        match result.0 {
            WorktreeIntegrationStatus::NoMergeBaseFound => {
                bail!("Cannot integrate worktree: no merge base found with {target}")
            }
            WorktreeIntegrationStatus::WorktreeIsBare => {
                bail!("Cannot integrate worktree: it is bare")
            }
            WorktreeIntegrationStatus::NothingToIntegrate => bail!(
                "Worktree has no changes to integrate. Use `but worktree destroy` to remove it"
            ),
            WorktreeIntegrationStatus::CausesWorkspaceConflicts => {
                bail!("Cannot integrate worktree: it would cause conflicts in the workspace")
            }
            WorktreeIntegrationStatus::Integratable { .. } => {
                bail!("Worktree failed integration checks")
            }
        }
    };

    // Persists the new commits, updates refs, and safely updates the main
    // checkout. Uncommitted changes that would conflict abort the operation
    // before any ref is touched.
    rebase
        .materialize()
        .context("Failed to integrate worktree into the workspace")?;

    git_worktree_remove(repo.common_dir(), id, true)?;

    Ok(())
}

/// Performs the workspace integration in-memory using the graph editor,
/// returning the status, and the un-materialized rebase if it's integratable.
fn worktree_integration_inner<'ws, 'meta, M: RefMetadata>(
    repo: &gix::Repository,
    ws: &'ws mut but_graph::Workspace,
    meta: &'meta mut M,
    id: &WorktreeId,
    target: &gix::refs::FullNameRef,
) -> Result<(
    WorktreeIntegrationStatus,
    Option<SuccessfulRebase<'ws, 'meta, M>>,
)> {
    if !ws.refname_is_segment(target) {
        bail!("Branch {} not found in workspace", target.shorten());
    }

    let git_worktree = repo
        .worktrees()?
        .into_iter()
        .find(|w| w.id() == id.as_bstr())
        .with_context(|| format!("Worktree '{id}' not found"))?;
    let worktree_repo = git_worktree.into_repo()?;
    let worktree_head = worktree_repo.head()?;
    let Some(worktree_head_id) = worktree_head.id() else {
        return Ok((WorktreeIntegrationStatus::WorktreeIsBare, None));
    };

    // Find the base which we will use for the "cherry pick".
    let target_tip = repo.find_reference(target)?.id().detach();
    let wt_meta = get_worktree_meta(repo, id)?;
    let base = {
        // If we have worktree metadata and the base hasn't been dropped entirely
        // from history, we will use that.
        if let Some(wt_meta) = wt_meta
            && repo.find_object(wt_meta.base).is_ok()
        {
            wt_meta.base
        } else {
            let Ok(merge_base) = repo.merge_base(target_tip, worktree_head_id.detach()) else {
                return Ok((WorktreeIntegrationStatus::NoMergeBaseFound, None));
            };
            merge_base.detach()
        }
    };

    // The squashed state of the worktree, including its uncommitted changes.
    #[expect(
        deprecated,
        reason = "no alternative yet for snapshotting a worktree into a tree"
    )]
    let wd_tree = worktree_repo.create_wd_tree(0)?;
    if wd_tree == repo.find_commit(base)?.tree_id()? {
        return Ok((WorktreeIntegrationStatus::NothingToIntegrate, None));
    }

    // State needed later which can't be read while the editor borrows `ws`.
    let ws_commit_id = ws.graph.managed_entrypoint_commit(repo)?.map(|c| c.id);
    let head_id = repo.head_id().ok().map(|id| id.detach());
    let head_tree_id = repo.head_tree_id_or_empty()?.detach();
    #[expect(
        deprecated,
        reason = "no alternative yet for snapshotting a worktree into a tree"
    )]
    let main_wd_tree = repo.create_wd_tree(0)?;

    let author = repo
        .author()
        .transpose()
        .ok()
        .flatten()
        .context("Failed to find author signature")?
        .to_owned()?;

    let mut editor = Editor::create(ws, meta, repo)?;

    // Create the squash commit in the editor's in-memory repository; it is
    // only persisted if the result gets materialized.
    let squash_commit = gix::objs::Commit {
        tree: wd_tree,
        parents: [base].into(),
        author: author.clone(),
        committer: author,
        encoding: None,
        message: BString::from("Integrated worktree"),
        extra_headers: vec![],
    };
    let squash_id = editor.repo().write_object(&squash_commit)?.detach();

    // The squash commit becomes the new tip of `target`; everything that
    // pointed at the old tip (including the workspace commit) follows.
    let squash_selector = editor.insert(
        target,
        Step::new_untracked_pick(squash_id),
        InsertSide::Below,
    )?;

    let rebase = match editor.rebase() {
        Ok(rebase) => rebase,
        Err(err) => {
            // The workspace commit is the only non-conflictable pick in the
            // graph, so failing to rebuild the workspace with the squash
            // inserted means the workspace would conflict.
            tracing::debug!("worktree integration rebase failed: {err:#}");
            return Ok((WorktreeIntegrationStatus::CausesWorkspaceConflicts, None));
        }
    };

    // Inspect the in-memory result for conflicts.
    let in_memory_repo = rebase.repo();
    let new_squash_id = rebase.lookup_pick(squash_selector)?;
    let cherry_pick_conflicts =
        but_core::Commit::from_id(new_squash_id.attach(in_memory_repo))?.is_conflicted();

    let mappings = rebase.history.commit_mappings();
    let commits_above_conflict = mappings.iter().any(|(old, new)| {
        Some(*old) != ws_commit_id
            && *new != new_squash_id
            && but_core::Commit::from_id(new.attach(in_memory_repo))
                .is_ok_and(|c| c.is_conflicted())
    });

    // Predict whether the safe checkout of the new workspace state would
    // conflict with uncommitted changes in the main worktree.
    let working_dir_conflicts = if main_wd_tree == head_tree_id {
        false
    } else {
        match head_id.and_then(|head| mappings.get(&head).copied()) {
            // HEAD is not rewritten, the checkout won't touch anything.
            None => false,
            Some(new_head_id) => {
                let new_head_tree = in_memory_repo.find_commit(new_head_id)?.tree_id()?.detach();
                !in_memory_repo.merges_cleanly(head_tree_id, main_wd_tree, new_head_tree)?
            }
        }
    };

    Ok((
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts,
            commits_above_conflict,
            working_dir_conflicts,
        },
        Some(rebase),
    ))
}
