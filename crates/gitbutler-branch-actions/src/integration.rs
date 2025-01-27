use std::cmp::Ordering;
use std::collections::HashMap;
use std::{path::PathBuf, vec};

use anyhow::{anyhow, Context, Result};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch::{self, GITBUTLER_WORKSPACE_REFERENCE};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_diff::diff_files_into_hunks;
use gitbutler_error::error::Marker;
use gitbutler_operating_modes::{OPEN_WORKSPACE_REFS, WORKSPACE_BRANCH_REF};
use gitbutler_oxidize::{git2_to_gix_object_id, gix_to_git2_oid, GixRepositoryExt};
use gitbutler_project::access::{WorktreeReadPermission, WorktreeWritePermission};
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gitbutler_repo::rebase::cherry_rebase_group;
use gitbutler_repo::SignaturePurpose;
use gitbutler_stack::{Stack, StackId, VirtualBranchesHandle};
use tracing::instrument;

use crate::compute_workspace_dependencies;
use crate::{branch_manager::BranchManagerExt, conflicts, VirtualBranchesExt};

const WORKSPACE_HEAD: &str = "Workspace Head";
const GITBUTLER_INTEGRATION_COMMIT_TITLE: &str = "GitButler Integration Commit";
pub const GITBUTLER_WORKSPACE_COMMIT_TITLE: &str = "GitButler Workspace Commit";

/// Creates and returns a merge commit of all active branch heads.
///
/// This is the base against which we diff the working directory to understand
/// what files have been modified.
///
/// This should be used to update the `gitbutler/workspace` ref with, which is usually
/// done from [`update_workspace_commit()`], after any of its input changes.
/// This is namely the conflicting state, or any head of the virtual branches.
#[instrument(level = tracing::Level::DEBUG, skip(ctx))]
pub(crate) fn get_workspace_head(ctx: &CommandContext) -> Result<git2::Oid> {
    let vb_state = ctx.project().virtual_branches();
    let target = vb_state
        .get_default_target()
        .context("failed to get target")?;
    let repo: &git2::Repository = ctx.repo();

    let mut stacks: Vec<Stack> = vb_state.list_stacks_in_workspace()?;

    let target_commit = repo.find_commit(target.sha)?;
    let mut workspace_tree = repo.find_real_tree(&target_commit, Default::default())?;
    let mut workspace_tree_id = git2_to_gix_object_id(workspace_tree.id());

    if conflicts::is_conflicting(ctx, None)? {
        let merge_parent = conflicts::merge_parent(ctx)?.ok_or(anyhow!("No merge parent"))?;
        let first_stack = stacks.first().ok_or(anyhow!("No branches"))?;

        let merge_base = repo.merge_base(first_stack.head(), merge_parent)?;
        workspace_tree = repo.find_commit(merge_base)?.tree()?;
    } else {
        let gix_repo = ctx.gix_repository_for_merging()?;
        let (merge_options_fail_fast, conflict_kind) = gix_repo.merge_options_fail_fast()?;
        let merge_tree_id = git2_to_gix_object_id(repo.find_commit(target.sha)?.tree_id());
        for stack in stacks.iter_mut() {
            let branch_head = repo.find_commit(stack.head())?;
            let branch_tree_id =
                git2_to_gix_object_id(repo.find_real_tree(&branch_head, Default::default())?.id());

            let mut merge = gix_repo.merge_trees(
                merge_tree_id,
                workspace_tree_id,
                branch_tree_id,
                gix_repo.default_merge_labels(),
                merge_options_fail_fast.clone(),
            )?;

            if !merge.has_unresolved_conflicts(conflict_kind) {
                workspace_tree_id = merge.tree.write()?.detach();
            } else {
                // This branch should have already been unapplied during the "update" command but for some reason that failed
                tracing::warn!("Merge conflict between base and {:?}", stack.name);
                stack.in_workspace = false;
                vb_state.set_stack(stack.clone())?;
            }
        }
        workspace_tree = repo.find_tree(gix_to_git2_oid(workspace_tree_id))?;
    }

    let committer = gitbutler_repo::signature(SignaturePurpose::Committer)?;
    let author = gitbutler_repo::signature(SignaturePurpose::Author)?;
    let mut heads: Vec<git2::Commit<'_>> = stacks
        .iter()
        .filter(|b| b.head() != target.sha)
        .map(|b| repo.find_commit(b.head()))
        .filter_map(Result::ok)
        .collect();

    if heads.is_empty() {
        heads = vec![target_commit]
    }

    // TODO: Why does commit only accept a slice of commits? Feels like we
    //       could make use of AsRef with the right traits.
    let head_refs: Vec<&git2::Commit<'_>> = heads.iter().collect();

    let workspace_head_id = repo.commit(
        None,
        &author,
        &committer,
        WORKSPACE_HEAD,
        &workspace_tree,
        head_refs.as_slice(),
    )?;
    Ok(workspace_head_id)
}

// Before switching the user to our gitbutler workspace branch we save
// the current branch into a text file. It is used in generating the commit
// message for workspace branch, as a helpful hint about how to get back
// to where you were.
struct PreviousHead {
    head: String,
    sha: String,
}

fn read_workspace_file(path: &PathBuf) -> Result<Option<PreviousHead>> {
    if let Ok(prev_data) = std::fs::read_to_string(path) {
        let parts: Vec<&str> = prev_data.split(':').collect();
        let prev_head = parts[0].to_string();
        let prev_sha = parts[1].to_string();
        Ok(Some(PreviousHead {
            head: prev_head,
            sha: prev_sha,
        }))
    } else {
        Ok(None)
    }
}

fn write_workspace_file(head: &git2::Reference, path: PathBuf) -> Result<()> {
    let sha = head.target().unwrap().to_string();
    std::fs::write(path, format!(":{}", sha))?;
    Ok(())
}
#[instrument(level = tracing::Level::DEBUG, skip(vb_state, ctx), err(Debug))]
pub fn update_workspace_commit(
    vb_state: &VirtualBranchesHandle,
    ctx: &CommandContext,
) -> Result<git2::Oid> {
    let target = vb_state
        .get_default_target()
        .context("failed to get target")?;

    let repo: &git2::Repository = ctx.repo();

    // get current repo head for reference
    let head_ref = repo.head()?;
    let workspace_filepath = repo.path().join("workspace");
    let mut prev_branch = read_workspace_file(&workspace_filepath)?;
    if let Some(branch) = &prev_branch {
        if branch.head != GITBUTLER_WORKSPACE_REFERENCE.to_string() {
            // we are moving from a regular branch to our gitbutler workspace branch, write a file to
            // .git/workspace with the previous head and name
            write_workspace_file(&head_ref, workspace_filepath)?;
            prev_branch = Some(PreviousHead {
                head: head_ref.target().unwrap().to_string(),
                sha: head_ref.target().unwrap().to_string(),
            });
        }
    }

    let vb_state = ctx.project().virtual_branches();

    // get all virtual branches, we need to try to update them all
    let virtual_branches: Vec<Stack> = vb_state
        .list_stacks_in_workspace()
        .context("failed to list virtual branches")?;

    let workspace_head = repo.find_commit(get_workspace_head(ctx)?)?;

    // message that says how to get back to where they were
    let mut message = GITBUTLER_WORKSPACE_COMMIT_TITLE.to_string();
    message.push_str("\n\n");
    if !virtual_branches.is_empty() {
        message.push_str("This is a merge commit the virtual branches in your workspace.\n\n");
    } else {
        message.push_str("This is placeholder commit and will be replaced by a merge of your");
        message.push_str("virtual branches.\n\n");
    }
    message.push_str(
        "Due to GitButler managing multiple virtual branches, you cannot switch back and\n",
    );
    message.push_str("forth between git branches and virtual branches easily. \n\n");

    message.push_str("If you switch to another branch, GitButler will need to be reinitialized.\n");
    message.push_str("If you commit on this branch, GitButler will throw it away.\n\n");
    if !virtual_branches.is_empty() {
        message.push_str("Here are the branches that are currently applied:\n");
        for branch in &virtual_branches {
            message.push_str(" - ");
            message.push_str(branch.name.as_str());
            message.push_str(format!(" ({})", &branch.refname()?).as_str());
            message.push('\n');

            if branch.head() != target.sha {
                message.push_str("   branch head: ");
                message.push_str(&branch.head().to_string());
                message.push('\n');
            }
            for file in &branch.ownership.claims {
                message.push_str("   - ");
                message.push_str(&file.file_path.display().to_string());
                message.push('\n');
            }
        }
    }
    if let Some(prev_branch) = prev_branch {
        message.push_str("\nYour previous branch was: ");
        message.push_str(&prev_branch.head);
        message.push_str("\n\n");
        message.push_str("The sha for that commit was: ");
        message.push_str(&prev_branch.sha);
        message.push_str("\n\n");
    }
    message.push_str("For more information about what we're doing here, check out our docs:\n");
    message.push_str("https://docs.gitbutler.com/features/virtual-branches/integration-branch\n");

    let committer = gitbutler_repo::signature(SignaturePurpose::Committer)?;
    let author = gitbutler_repo::signature(SignaturePurpose::Author)?;

    // It would be nice if we could pass an `update_ref` parameter to this function, but that
    // requires committing to the tip of the branch, and we're mostly replacing the tip.

    let parents = workspace_head.parents().collect::<Vec<_>>();
    let workspace_tree = workspace_head.tree()?;

    let final_commit = repo.commit(
        None,
        &author,
        &committer,
        &message,
        &workspace_tree,
        parents.iter().collect::<Vec<_>>().as_slice(),
    )?;

    // Create or replace the workspace branch reference, then set as HEAD.
    repo.reference(
        &GITBUTLER_WORKSPACE_REFERENCE.clone().to_string(),
        final_commit,
        true,
        "updated workspace commit",
    )?;
    repo.set_head(&GITBUTLER_WORKSPACE_REFERENCE.clone().to_string())?;

    let mut index = repo.index()?;
    index.read_tree(&workspace_tree)?;
    index.write()?;

    // finally, update the refs/gitbutler/ heads to the states of the current virtual branches
    for branch in &virtual_branches {
        let wip_tree = repo.find_tree(branch.tree)?;
        let mut branch_head = repo.find_commit(branch.head())?;
        let head_tree = branch_head.tree()?;

        // create a wip commit if there is wip
        if head_tree.id() != wip_tree.id() {
            let mut message = "GitButler WIP Commit".to_string();
            message.push_str("\n\n");
            message.push_str("This is a WIP commit for the virtual branch '");
            message.push_str(branch.name.as_str());
            message.push_str("'\n\n");
            message.push_str("This commit is used to store the state of the virtual branch\n");
            message.push_str("while you are working on it. It is not meant to be used for\n");
            message.push_str("anything else.\n\n");
            let branch_head_oid = repo.commit(
                None,
                &committer,
                &committer,
                &message,
                &wip_tree,
                &[&branch_head],
                // None,
            )?;
            branch_head = repo.find_commit(branch_head_oid)?;
        }

        repo.reference(
            &branch.refname()?.to_string(),
            branch_head.id(),
            true,
            "update virtual branch",
        )?;
    }

    Ok(final_commit)
}

pub fn verify_branch(ctx: &CommandContext, perm: &mut WorktreeWritePermission) -> Result<()> {
    verify_current_branch_name(ctx)
        .and_then(verify_head_is_set)
        .and_then(|()| verify_head_is_clean(ctx, perm))
        .context(Marker::VerificationFailure)?;
    Ok(())
}

fn verify_head_is_set(ctx: &CommandContext) -> Result<()> {
    match ctx.repo().head().context("failed to get head")?.name() {
        Some(refname) if OPEN_WORKSPACE_REFS.contains(&refname) => Ok(()),
        Some(head_name) => Err(invalid_head_err(head_name)),
        None => Err(anyhow!(
            "project in detached head state. Please checkout {} to continue",
            GITBUTLER_WORKSPACE_REFERENCE.branch()
        )),
    }
}

// Returns an error if repo head is not pointing to the workspace branch.
fn verify_current_branch_name(ctx: &CommandContext) -> Result<&CommandContext> {
    match ctx.repo().head()?.name() {
        Some(head) => {
            let head_name = head.to_string();
            if !OPEN_WORKSPACE_REFS.contains(&head_name.as_str()) {
                return Err(invalid_head_err(&head_name));
            }
            Ok(ctx)
        }
        None => Err(anyhow!("Repo HEAD is unavailable")),
    }
}

#[derive(Debug)]
pub enum WorkspaceState {
    OffWorkspaceCommit {
        workspace_commit: git2::Oid,
        extra_commits: Vec<git2::Oid>,
    },
    OnWorkspaceCommit,
}

pub fn workspace_state(
    ctx: &CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<WorkspaceState> {
    let repository = ctx.repo();
    let vb_handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let default_target = vb_handle.get_default_target()?;

    let head_commit = repository.head()?.peel_to_commit()?;
    let commits = repository.log(
        head_commit.id(),
        LogUntil::Commit(default_target.sha),
        false,
    )?;

    let workspace_index = commits
        .iter()
        .position(|commit| {
            commit.message().is_some_and(|message| {
                message.starts_with(GITBUTLER_WORKSPACE_COMMIT_TITLE)
                    || message.starts_with(GITBUTLER_INTEGRATION_COMMIT_TITLE)
            })
        })
        .context("")?;
    let workspace_commit = &commits[workspace_index];
    let extra_commits = commits[..workspace_index].to_vec();

    if extra_commits.is_empty() {
        // no extra commits found, so we're good
        return Ok(WorkspaceState::OnWorkspaceCommit);
    }

    Ok(WorkspaceState::OffWorkspaceCommit {
        workspace_commit: workspace_commit.id(),
        extra_commits: extra_commits
            .iter()
            .map(git2::Commit::id)
            .collect::<Vec<_>>(),
    })
}

// TODO(ST): Probably there should not be an implicit vbranch creation here.
fn verify_head_is_clean(ctx: &CommandContext, perm: &mut WorktreeWritePermission) -> Result<()> {
    let repository = ctx.repo();
    let head_commit = repository.head()?.peel_to_commit()?;

    let WorkspaceState::OffWorkspaceCommit {
        workspace_commit,
        extra_commits,
    } = dbg!(workspace_state(ctx, perm.read_permission())?)
    else {
        return Ok(());
    };

    let best_stack_id = find_best_stack_for_changes(ctx, perm, head_commit.id(), workspace_commit)?;

    if let Some(best_stack_id) = best_stack_id {
        let vb_handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
        let mut stack = vb_handle.get_stack_in_workspace(best_stack_id)?;

        let new_head = cherry_rebase_group(repository, stack.head(), &extra_commits, false)?;

        stack.set_stack_head(
            ctx,
            new_head,
            Some(repository.find_commit(new_head)?.tree_id()),
        )?;

        update_workspace_commit(&vb_handle, ctx)?;
    } else {
        // There is no stack which can hold the commits so we should just unroll those changes
        repository.reference(WORKSPACE_BRANCH_REF, workspace_commit, true, "")?;
        repository.set_head(WORKSPACE_BRANCH_REF)?;
    }

    Ok(())
}

fn find_best_stack_for_changes(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
    head_commit: git2::Oid,
    workspace_commit: git2::Oid,
) -> Result<Option<StackId>> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let default_target = vb_state.get_default_target()?;
    let repository = ctx.repo();
    let stacks = vb_state.list_stacks_in_workspace()?;

    let head_commit = repository.find_commit(head_commit)?;

    let diffs = gitbutler_diff::trees(
        ctx.repo(),
        &repository.find_commit(workspace_commit)?.tree()?,
        &head_commit.tree()?,
        true,
    )?;
    let base_diffs: HashMap<_, _> = diff_files_into_hunks(&diffs).collect();
    let workspace_dependencies =
        compute_workspace_dependencies(ctx, &default_target.sha, &base_diffs, &stacks)?;

    match workspace_dependencies.commit_dependent_diffs.len().cmp(&1) {
        Ordering::Greater => {
            // The commits are locked to multiple stacks. We can't correctly assign it
            // to any one stack, so the commits should be undone.
            Ok(None)
        }
        Ordering::Equal => {
            // There is one stack which the commits are locked to, so the commits
            // should be added to that particular stack.
            let stack_id = workspace_dependencies
                .commit_dependent_diffs
                .keys()
                .next()
                .expect("Values was asserted length 1 above");
            Ok(Some(*stack_id))
        }
        Ordering::Less => {
            // We should return the branch selected for changes, or create a new default branch.
            let mut stacks = vb_state.list_stacks_in_workspace()?;
            stacks.sort_by_key(|stack| stack.selected_for_changes.unwrap_or(0));

            if let Some(stack) = stacks.last() {
                return Ok(Some(stack.id));
            }

            let branch_manager = ctx.branch_manager();
            let new_stack = branch_manager
                .create_virtual_branch(
                    &BranchCreateRequest {
                        name: Some(head_commit.message_bstr().to_string()),
                        ..Default::default()
                    },
                    perm,
                )
                .context("failed to create virtual branch")?;

            Ok(Some(new_stack.id))
        }
    }
}

fn invalid_head_err(head_name: &str) -> anyhow::Error {
    anyhow!(
        "project is on {head_name}. Please checkout {} to continue",
        GITBUTLER_WORKSPACE_REFERENCE.branch()
    )
}
