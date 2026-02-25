use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result, bail};
use bstr::{BString, ByteSlice};
use but_core::{RepositoryExt, TreeChange, commit::Headers, ref_metadata::StackId};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_oxidize::{ObjectIdExt, OidExt, RepoExt, gix_to_git2_index};
use but_workspace::legacy::stack_ext::StackExt;
use git2::build::CheckoutBuilder;
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt as _};
use gitbutler_commit::commit_ext::{CommitExt, CommitMessageBstr as _};
use gitbutler_operating_modes::{
    EDIT_BRANCH_REF, EditModeMetadata, OperatingMode, WORKSPACE_BRANCH_REF, operating_mode,
    read_edit_mode_metadata, write_edit_mode_metadata,
};
use gitbutler_repo::{RepositoryExt as _, SignaturePurpose, signature};
use gitbutler_stack::VirtualBranchesHandle;
use gitbutler_workspace::{
    branch_trees::{WorkspaceState, update_uncommitted_changes_with_tree},
    submodules::has_submodules_configured,
};
use serde::Serialize;

pub mod commands;

const UNCOMMITTED_CHANGES_REF: &str = "refs/gitbutler/edit-uncommitted-changes";

/// Returns an index of the tree of `commit` if it is unconflicted, *or* produce a merged tree
/// if `commit` is conflicted. That tree is turned into an index that records the conflicts that occurred
/// during the merge.
fn get_commit_index(ctx: &Context, commit: &git2::Commit) -> Result<git2::Index> {
    let commit_tree = commit.tree().context("Failed to get commit's tree")?;
    let repo = ctx.repo.get()?;
    let commit = repo.find_commit(commit.id().to_gix())?;
    // Checkout the commit as unstaged changes
    if commit.is_conflicted() {
        let base = commit_tree
            .get_name(".conflict-base-0")
            .context("Failed to get base")?
            .id();
        let ours = commit_tree
            .get_name(".conflict-side-0")
            .context("Failed to get base")?
            .id();
        let theirs = commit_tree
            .get_name(".conflict-side-1")
            .context("Failed to get base")?
            .id();

        let gix_repo = repo.clone().for_tree_diffing()?;
        // Merge without favoring a side this time to get a tree containing the actual conflicts.
        let mut merge_result = gix_repo.merge_trees(
            base.to_gix(),
            ours.to_gix(),
            theirs.to_gix(),
            gix_repo.default_merge_labels(),
            gix_repo.tree_merge_options()?,
        )?;
        let merged_tree_id = merge_result.tree.write()?;
        let mut index = gix_repo.index_from_tree(&merged_tree_id)?;
        if !merge_result.index_changed_after_applying_conflicts(
            &mut index,
            gix::merge::tree::TreatAsUnresolved::git(),
            gix::merge::tree::apply_index_entries::RemovalMode::Mark,
        ) {
            tracing::warn!(
                "There must be an issue with conflict-commit creation as re-merging the conflicting trees didn't yield a conflicting index."
            );
        }
        gix_to_git2_index(&index)
    } else {
        let mut index = git2::Index::new()?;
        index.read_tree(&commit_tree)?;
        Ok(index)
    }
}

/// Returns a commit to be the HEAD of `gitbutler/edit`
///
/// This should a commit who's tree is what the commit getting edited
/// (the editee) is based on.
///
/// If the editee is conflicted:
/// We should checkout `.conflict-side-0`. This is because the resulting merge
/// is always based on top of `.conflict-side-0`, so this is the preferable
/// base.
///
/// If the parent is conflicted:
/// We should checkout the parent's `.auto-resolution` because that is what
/// the editee is based on
///
/// Otherwise:
/// We can simply return the parent commit.
fn find_or_create_base_commit<'a>(
    repository: &'a git2::Repository,
    commit: &git2::Commit<'a>,
) -> Result<git2::Commit<'a>> {
    let gix_repo = repository.to_gix_repo()?;
    let gix_commit = gix_repo.find_commit(commit.id().to_gix())?;
    let is_conflicted = gix_commit.is_conflicted();
    let parent = gix_commit
        .parent_ids()
        .next()
        .context("Expected commit to have a single parent")?
        .object()?
        .into_commit();
    let is_parent_conflicted = parent.is_conflicted();

    // If neither is conflicted, we can use the old parent.
    if !(is_conflicted || is_parent_conflicted) {
        return Ok(commit.parent(0)?);
    };

    let base_tree = if is_conflicted {
        repository.find_real_tree(commit, ConflictedTreeKey::Ours)?
    } else {
        let parent = commit.parent(0)?;
        repository.find_real_tree(&parent, ConflictedTreeKey::AutoResolution)?
    };

    let author_signature = signature(SignaturePurpose::Author)?;
    let committer_signature = signature(SignaturePurpose::Committer)?;
    let base = repository.commit(
        None,
        &author_signature,
        &committer_signature,
        "Conflict base",
        &base_tree,
        &[],
    )?;

    Ok(repository.find_commit(base)?)
}

fn commit_uncommited_changes(ctx: &Context) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;
    let uncommited_changes = repo.create_wd_tree(0)?;
    repo.reference(UNCOMMITTED_CHANGES_REF, uncommited_changes.id(), true, "")?;
    Ok(())
}

fn get_uncommited_changes(ctx: &Context) -> Result<git2::Oid> {
    let repo = &*ctx.git2_repo.get()?;
    let uncommited_changes = repo
        .find_reference(UNCOMMITTED_CHANGES_REF)?
        .peel_to_tree()?
        .id();
    Ok(uncommited_changes)
}

fn sync_configured_submodules(repo: &git2::Repository) {
    let Ok(mut submodules) = repo.submodules() else {
        return;
    };

    for submodule in &mut submodules {
        let _ = submodule.update(true, None);
    }
}

fn configured_submodule_paths(repo: &git2::Repository) -> Vec<String> {
    let mut paths = HashSet::new();

    if let Ok(submodules) = repo.submodules() {
        for submodule in submodules {
            paths.insert(submodule.path().to_string_lossy().to_string());
        }
    }

    // When only .git/modules entries exist, derive logical submodule paths from that layout.
    let modules_root = repo.path().join("modules");
    if modules_root.exists() {
        let mut stack = vec![modules_root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                if path.join("HEAD").is_file()
                    && let Ok(relative) = path.strip_prefix(&modules_root)
                {
                    paths.insert(relative.to_string_lossy().to_string());
                }

                stack.push(path);
            }
        }
    }

    let mut output = paths.into_iter().collect::<Vec<_>>();
    output.sort();
    output
}

fn is_submodule_related_path(path: &str, submodule_paths: &[String]) -> bool {
    submodule_paths
        .iter()
    .any(|sm| path == sm || path.strip_prefix(sm).is_some_and(|rest| rest.starts_with('/')))
}

fn collect_checkout_paths(
    repo: &git2::Repository,
    baseline: &git2::Tree,
    target: &git2::Tree,
    submodule_paths: &[String],
) -> Result<Vec<String>> {
    let mut paths = HashSet::new();
    let diff = repo.diff_tree_to_tree(Some(baseline), Some(target), None)?;

    for delta in diff.deltas() {
        if let Some(path) = delta.old_file().path() {
            let path = path.to_string_lossy().to_string();
            if !is_submodule_related_path(&path, submodule_paths) {
                paths.insert(path);
            }
        }
        if let Some(path) = delta.new_file().path() {
            let path = path.to_string_lossy().to_string();
            if !is_submodule_related_path(&path, submodule_paths) {
                paths.insert(path);
            }
        }
    }

    let mut output = paths.into_iter().collect::<Vec<_>>();
    output.sort();
    Ok(output)
}

fn checkout_edit_branch(ctx: &Context, commit: git2::Commit) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;
    let has_submodules = has_submodules_configured(repo);
    let submodule_paths = if has_submodules {
        configured_submodule_paths(repo)
    } else {
        Vec::new()
    };
    let result = (|| -> Result<()> {
        let current_head_tree = repo.head()?.peel_to_tree()?;

        // Checkout commits's parent
        let commit_parent = find_or_create_base_commit(repo, &commit)?;
        let commit_parent_tree = commit_parent.tree()?;
        let checkout_head_paths = collect_checkout_paths(
            repo,
            &current_head_tree,
            &commit_parent_tree,
            &submodule_paths,
        )?;

        repo.reference(EDIT_BRANCH_REF, commit_parent.id(), true, "")?;
        repo.set_head(EDIT_BRANCH_REF)?;
        let mut checkout_head = CheckoutBuilder::new();
        checkout_head.force();
        if !has_submodules {
            checkout_head.remove_untracked(true);
        } else {
            for path in &checkout_head_paths {
                checkout_head.path(path);
            }
        }
        if !has_submodules || !checkout_head_paths.is_empty() {
            repo.checkout_head(Some(&mut checkout_head))?;
        }

        // Checkout the commit as unstaged changes
        let mut index = get_commit_index(ctx, &commit)?;
        let commit_tree = commit.tree()?;
        let checkout_index_paths = collect_checkout_paths(
            repo,
            &commit_parent_tree,
            &commit_tree,
            &submodule_paths,
        )?;
        let mut checkout_index = CheckoutBuilder::new();
        checkout_index.force().conflict_style_diff3(true);
        if !has_submodules {
            checkout_index.remove_untracked(true);
        } else {
            for path in &checkout_index_paths {
                checkout_index.path(path);
            }
        }

        if !has_submodules || !checkout_index_paths.is_empty() {
            repo.checkout_index(Some(&mut index), Some(&mut checkout_index))?;
        }

        // Keep configured submodule worktrees populated after moving into edit mode.
        if has_submodules {
            sync_configured_submodules(repo);
        }

        Ok(())
    })();
    result
}

pub(crate) fn enter_edit_mode(
    ctx: &Context,
    commit: git2::Commit,
    stack_id: StackId,
    _perm: &mut RepoExclusive,
) -> Result<EditModeMetadata> {
    let edit_mode_metadata = EditModeMetadata {
        commit_oid: commit.id(),
        stack_id,
    };

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    // Validate the stack_id
    vb_state.get_stack_in_workspace(stack_id)?;

    commit_uncommited_changes(ctx)?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;
    checkout_edit_branch(ctx, commit).context("Failed to checkout edit branch")?;

    Ok(edit_mode_metadata)
}

pub(crate) fn abort_and_return_to_workspace(
    ctx: &Context,
    force: bool,
    perm: &mut RepoExclusive,
) -> Result<()> {
    if !force && !changes_from_initial(ctx, perm.read_permission())?.is_empty() {
        bail!(
            "The working tree differs from the original commit. A forced abort is necessary.\nIf you are seeing this message, please report it as a bug. The UI should have prevented this line getting hit."
        );
    }

    let repo = &*ctx.git2_repo.get()?;
    let has_submodules = has_submodules_configured(repo);

    // Checkout gitbutler workspace branch
    repo.set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;

    let uncommited_changes = get_uncommited_changes(ctx)?;
    let uncommited_changes = repo.find_tree(uncommited_changes)?;

    let mut checkout_tree = CheckoutBuilder::new();
    checkout_tree.force();
    if !has_submodules {
        checkout_tree.remove_untracked(true);
    }
    repo.checkout_tree(uncommited_changes.as_object(), Some(&mut checkout_tree))?;

    Ok(())
}

pub(crate) fn save_and_return_to_workspace(ctx: &Context, perm: &mut RepoExclusive) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repo = &*ctx.git2_repo.get()?;
    let gix_repo = &*ctx.repo.get()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;

    // Get important references
    let commit = repo
        .find_commit(edit_mode_metadata.commit_oid)
        .context("Failed to find commit")?;
    let gix_commit = gix_repo.find_commit(commit.id().to_gix())?;
    let commit_obj = gix_commit.decode()?.into_owned()?;

    let mut stack = vb_state.get_stack_in_workspace(edit_mode_metadata.stack_id)?;

    let parents = commit.parents().collect::<Vec<_>>();

    // Write out all the changes, including unstaged changes to a tree for re-committing
    let mut index = repo.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree = repo.create_wd_tree(0)?;

    let (_, committer) = repo.signatures()?;
    let commit_headers = Headers::try_from_commit(&commit_obj).map(|commit_headers| Headers {
        conflicted: None,
        ..commit_headers
    });
    let new_commit_oid = ctx
        .git2_repo
        .get()?
        .commit_with_signature(
            None,
            &commit.author(),
            &committer, // Use a new committer
            &commit.message_bstr().to_str_lossy(),
            &tree,
            &parents.iter().collect::<Vec<_>>(),
            commit_headers,
        )
        .context("Failed to commit new commit")?;

    let gix_repo = repo.to_gix_repo()?;

    let mut steps = stack.as_rebase_steps(ctx)?;
    // swap out the old commit with the new, updated one
    steps.iter_mut().for_each(|step| {
        if let but_rebase::RebaseStep::Pick { commit_id, .. } = step
            && commit.id() == commit_id.to_git2()
        {
            *commit_id = new_commit_oid.to_gix();
        }
    });
    let merge_base = stack.merge_base(ctx)?;
    let mut rebase = but_rebase::Rebase::new(&gix_repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;

    stack.set_heads_from_rebase_output(ctx, output.references)?;

    // Switch branch to gitbutler/workspace
    repo.set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;
    repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

    update_workspace_commit(&vb_state, ctx, false)?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let uncommtied_changes = get_uncommited_changes(ctx)?;

    update_uncommitted_changes_with_tree(
        ctx,
        old_workspace,
        new_workspace,
        Some(uncommtied_changes),
        Some(true),
        perm,
    )?;

    // Currently if the index goes wonky then files don't appear quite right.
    // This just makes sure the index is all good.
    let mut index = repo.index()?;
    index.read_tree(&repo.head()?.peel_to_tree()?)?;
    index.write()?;

    Ok(())
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntryPresence {
    pub ours: bool,
    pub theirs: bool,
    pub ancestor: bool,
}

pub(crate) fn starting_index_state(
    ctx: &Context,
    _perm: &RepoShared,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx) else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let repo = &*ctx.git2_repo.get()?;
    let gix_repo = &*ctx.repo.get()?;

    let commit = repo.find_commit(metadata.commit_oid)?;
    let gix_commit = gix_repo.find_commit(commit.id().to_gix())?;
    let commit_parent_tree = if gix_commit.is_conflicted() {
        repo.find_real_tree(&commit, ConflictedTreeKey::Base)?
    } else {
        commit.parent(0)?.tree()?
    };

    let index = get_commit_index(ctx, &commit)?;

    let conflicts = index
        .conflicts()?
        .filter_map(|conflict| {
            let Ok(conflict) = conflict else {
                return None;
            };

            let path = conflict
                .ancestor
                .as_ref()
                .or(conflict.our.as_ref())
                .or(conflict.their.as_ref())
                .map(|entry| BString::new(entry.path.clone()))?;

            Some((
                path,
                ConflictEntryPresence {
                    ours: conflict.our.is_some(),
                    theirs: conflict.their.is_some(),
                    ancestor: conflict.ancestor.is_some(),
                },
            ))
        })
        .collect::<HashMap<BString, ConflictEntryPresence>>();

    let gix_repo = ctx.repo.get()?;

    let tree_changes = but_core::diff::tree_changes(
        &gix_repo,
        Some(commit_parent_tree.id().to_gix()),
        repo.find_real_tree(&commit, ConflictedTreeKey::Theirs)?
            .id()
            .to_gix(),
    )?;

    let outcome = tree_changes
        .into_iter()
        .map(|tc| (tc.clone(), conflicts.get(&tc.path).cloned()))
        .collect();

    Ok(outcome)
}

pub(crate) fn changes_from_initial(ctx: &Context, _perm: &RepoShared) -> Result<Vec<TreeChange>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx) else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let repo = &*ctx.git2_repo.get()?;
    let commit = repo.find_commit(metadata.commit_oid)?;
    let base = repo
        .find_real_tree(&commit, Default::default())?
        .id()
        .to_gix();
    let head = repo.create_wd_tree(0)?.id().to_gix();

    let gix_repo = ctx.repo.get()?;
    let tree_changes = but_core::diff::tree_changes(&gix_repo, Some(base), head)?;
    Ok(tree_changes)
}
