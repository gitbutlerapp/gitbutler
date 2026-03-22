use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use but_core::worktree::checkout::UncommitedWorktreeChanges;
use but_ctx::{Context, access::RepoExclusive};
use but_error::Marker;
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_branch::{self, BranchCreateRequest, GITBUTLER_WORKSPACE_REFERENCE};
use gitbutler_operating_modes::is_well_known_workspace_ref;
use gitbutler_repo::{
    SignaturePurpose, commit_with_signature_gix, commit_without_signature_gix,
    first_parent_commit_ids_until, signature_gix,
};
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use tracing::instrument;

use crate::{VirtualBranchesExt, branch_manager::BranchManagerExt};

const GITBUTLER_INTEGRATION_COMMIT_TITLE: &str = "GitButler Integration Commit";
pub const GITBUTLER_WORKSPACE_COMMIT_TITLE: &str = "GitButler Workspace Commit";

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

fn write_workspace_file(head_target: gix::ObjectId, path: PathBuf) -> Result<()> {
    let sha = head_target.to_string();
    std::fs::write(path, format!(":{sha}"))?;
    Ok(())
}

/// Update `gitbutler/workspace` using the current virtual branch state from `ctx`.
///
/// Prefer this helper unless the caller is already carrying a `VirtualBranchesHandle` through a
/// stack/target mutation flow. In those cases use [`update_workspace_commit_with_vb_state()`] so
/// the dependency on that handle stays explicit at the call-site.
pub fn update_workspace_commit(
    ctx: &Context,
    checkout_new_worktree: bool,
) -> Result<gix::ObjectId> {
    update_workspace_commit_with_vb_state(&ctx.virtual_branches(), ctx, checkout_new_worktree)
}

/// Update `gitbutler/workspace` using the caller-provided virtual branch state handle.
///
/// This variant exists for flows that have just mutated stacks or the default target through an
/// already-existing `VirtualBranchesHandle`. Passing that handle keeps it obvious that the
/// workspace commit must be rebuilt from the same state mutation sequence instead of implicitly
/// reacquiring a fresh handle from `ctx`.
///
/// Most callers should use [`update_workspace_commit()`]. Reach for this helper only when the
/// handle is already part of the operation itself, such as branch creation, base-branch changes,
/// or rebases that update stack metadata before refreshing the workspace commit.
#[instrument(level = "debug", skip(vb_state, ctx), err(Debug))]
pub fn update_workspace_commit_with_vb_state(
    vb_state: &VirtualBranchesHandle,
    ctx: &Context,
    checkout_new_worktree: bool,
) -> Result<gix::ObjectId> {
    let target = vb_state
        .get_default_target()
        .context("failed to get target")?;

    let repo = &*ctx.git2_repo.get()?;
    let gix_repo = ctx.repo.get()?.clone();

    // get current repo head for reference
    let head_ref = repo.head()?;
    let workspace_filepath = repo.path().join("workspace");
    let mut prev_branch = read_workspace_file(&workspace_filepath)?;
    if let Some(branch) = &prev_branch
        && branch.head != GITBUTLER_WORKSPACE_REFERENCE.to_string()
    {
        // we are moving from a regular branch to our gitbutler workspace branch, write a file to
        // .git/workspace with the previous head and name
        write_workspace_file(
            head_ref.target().map(|oid| oid.to_gix()).unwrap(),
            workspace_filepath,
        )?;
        prev_branch = Some(PreviousHead {
            head: head_ref.target().unwrap().to_string(),
            sha: head_ref.target().unwrap().to_string(),
        });
    }
    let prev_head_id = head_ref.target();

    // get all virtual branches, we need to try to update them all
    let virtual_branches: Vec<Stack> = vb_state
        .list_stacks_in_workspace()
        .context("failed to list virtual branches")?;

    let workspace_head =
        gix_repo.find_commit(but_workspace::legacy::remerged_workspace_commit_v2(ctx)?)?;

    // message that says how to get back to where they were
    let mut message = GITBUTLER_WORKSPACE_COMMIT_TITLE.to_string();
    message.push_str("\n\n");
    if !virtual_branches.is_empty() {
        message.push_str("This is a merge commit the virtual branches in your workspace.\n\n");
    } else {
        message.push_str("This is placeholder commit and will be replaced by a merge of your ");
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
            message.push_str(&branch.name());
            message.push_str(format!(" ({})", &branch.refname()?).as_str());
            message.push('\n');

            if branch.head_oid(ctx)? != target.sha {
                message.push_str("   branch head: ");
                message.push_str(&branch.head_oid(ctx)?.to_string());
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
    message.push_str("https://docs.gitbutler.com/features/branch-management/integration-branch\n");

    let committer = signature_gix(SignaturePurpose::Committer);
    let author = signature_gix(SignaturePurpose::Author);

    // It would be nice if we could pass an `update_ref` parameter to this function, but that
    // requires committing to the tip of the branch, and we're mostly replacing the tip.

    let parents = workspace_head
        .parent_ids()
        .map(|id| id.detach())
        .collect::<Vec<_>>();
    let workspace_tree = workspace_head.tree_id()?.detach();

    let final_commit = commit_without_signature_gix(
        &gix_repo,
        None,
        author,
        committer,
        message.as_str().into(),
        workspace_tree,
        &parents,
        None,
    )?;

    let checkout_res = if checkout_new_worktree && let Some(prev_head_id) = prev_head_id {
        let res = but_core::worktree::safe_checkout(
            prev_head_id.to_gix(),
            final_commit,
            &gix_repo,
            but_core::worktree::checkout::Options {
                uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                skip_head_update: true,
            },
        );
        Some(res)
    } else {
        None
    };

    // Create or replace the workspace branch reference, then set as HEAD.
    repo.reference(
        &GITBUTLER_WORKSPACE_REFERENCE.clone().to_string(),
        final_commit.to_git2(),
        true,
        "updated workspace commit",
    )?;
    repo.set_head(&GITBUTLER_WORKSPACE_REFERENCE.clone().to_string())?;

    // Install managed hooks to prevent accidental git commits on workspace branch
    if let Err(e) = gitbutler_repo::managed_hooks::install_managed_hooks_gix(&gix_repo) {
        tracing::warn!("Failed to install managed hooks: {}", e);
    }

    let mut index = repo.index()?;
    index.read_tree(&repo.find_tree(workspace_tree.to_git2())?)?;
    index.write()?;

    // Everything is written out already, so if we fail here, we do so to surface the error
    // that prevented the checkout to be performed. The operation is still successful, on reload.
    if let Some(res) = checkout_res {
        res?;
    }

    Ok(final_commit)
}

pub fn verify_branch(ctx: &Context, perm: &mut RepoExclusive) -> Result<()> {
    verify_current_branch_name(ctx)
        .and_then(verify_head_is_set)
        .and_then(|()| verify_head_is_clean(ctx, perm))
        .context(Marker::VerificationFailure)?;
    Ok(())
}

fn verify_head_is_set(ctx: &Context) -> Result<()> {
    match ctx
        .repo
        .get()?
        .head()
        .context("failed to get head")?
        .referent_name()
    {
        Some(refname) => {
            if is_well_known_workspace_ref(refname) {
                Ok(())
            } else {
                Err(invalid_head_err(refname))
            }
        }
        None => Err(anyhow!(
            "project in detached head state. Please checkout {} to continue",
            GITBUTLER_WORKSPACE_REFERENCE.branch()
        )),
    }
}

// Returns an error if repo head is not pointing to the workspace branch.
fn verify_current_branch_name(ctx: &Context) -> Result<&Context> {
    match ctx.repo.get()?.head()?.referent_name() {
        Some(head) => {
            if !is_well_known_workspace_ref(head) {
                return Err(invalid_head_err(head));
            }
            Ok(ctx)
        }
        None => Err(anyhow!("Repo HEAD is unavailable")),
    }
}

// TODO(ST): Probably there should not be an implicit vbranch creation here.
fn verify_head_is_clean(ctx: &Context, perm: &mut RepoExclusive) -> Result<()> {
    let git2_repo = &*ctx.git2_repo.get()?;
    let gix_repo = ctx.repo.get()?.clone();
    let head_commit_id = gix_repo.head_id()?.detach();

    let mut vb_handle = VirtualBranchesHandle::new(ctx.project_data_dir());
    let default_target = vb_handle
        .get_default_target()
        .context("failed to get default target")?;

    let commit_ids = first_parent_commit_ids_until(&gix_repo, head_commit_id, default_target.sha)
        .context("failed to get log")?;
    let workspace_index = commit_ids
        .iter()
        .position(|commit_id| {
            gix_repo
                .find_commit(*commit_id)
                .ok()
                .and_then(|commit| {
                    commit.message_raw().ok().map(|message| {
                        message.starts_with(GITBUTLER_WORKSPACE_COMMIT_TITLE.as_bytes())
                            || message.starts_with(GITBUTLER_INTEGRATION_COMMIT_TITLE.as_bytes())
                    })
                })
                .unwrap_or(false)
        })
        .context("GitButler workspace commit not found")?;
    let workspace_commit = git2_repo.find_commit(commit_ids[workspace_index].to_git2())?;
    let mut extra_commit_ids = commit_ids[..workspace_index].to_vec();
    extra_commit_ids.reverse();

    if extra_commit_ids.is_empty() {
        // no extra commits found, so we're good
        return Ok(());
    }

    git2_repo
        .reset(workspace_commit.as_object(), git2::ResetType::Soft, None)
        .context("failed to reset to workspace commit")?;

    let branch_manager = ctx.branch_manager();
    let mut new_branch = branch_manager
        .create_virtual_branch(
            &BranchCreateRequest {
                name: extra_commit_ids
                    .last()
                    .map(|commit_id| {
                        gix_repo
                            .find_commit(*commit_id)?
                            .message_raw()
                            .map(|message| message.to_string())
                            .with_context(|| format!("failed to read extra commit {commit_id}"))
                    })
                    .transpose()?,
                ..Default::default()
            },
            perm,
        )
        .context("failed to create virtual branch")?;

    // rebasing the extra commits onto the new branch
    let mut head = new_branch.head_oid(ctx)?;
    for commit_id in extra_commit_ids {
        let commit = gix_repo
            .find_commit(commit_id)
            .with_context(|| format!("failed to find extra commit {commit_id}"))?;
        let commit = commit.decode()?;
        let rebased_commit_oid = commit_with_signature_gix(
            &gix_repo,
            None,
            commit.author()?.into(),
            commit.committer()?.into(),
            commit.message,
            commit.tree(),
            &[head],
            None,
        )
        .context(format!(
            "failed to rebase commit {commit_id} onto new branch"
        ))?;

        head = rebased_commit_oid;
        new_branch.set_stack_head(&mut vb_handle, &gix_repo, head)?;
    }
    Ok(())
}

fn invalid_head_err(head_name: &gix::refs::FullNameRef) -> anyhow::Error {
    anyhow!(
        "project is on {head_name}. Please checkout {} to continue",
        GITBUTLER_WORKSPACE_REFERENCE.branch(),
        head_name = head_name.shorten(),
    )
}
