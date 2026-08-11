use std::path::PathBuf;

use anyhow::{Context as _, Result};
use but_ctx::{Context, access::RepoExclusive};
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_branch::GITBUTLER_WORKSPACE_REFERENCE;
use gitbutler_repo::{SignaturePurpose, commit_without_signature_gix, signature_gix};
use tracing::instrument;

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
pub fn update_workspace_commit(
    ctx: &mut Context,
    checkout_new_worktree: bool,
) -> Result<gix::ObjectId> {
    let mut guard = ctx.exclusive_worktree_access();
    update_workspace_commit_with_perm(ctx, checkout_new_worktree, guard.write_permission())
}

/// Update `gitbutler/workspace` while reusing caller-held exclusive repository access.
#[instrument(level = "debug", skip(ctx, perm), err(Debug))]
pub fn update_workspace_commit_with_perm(
    ctx: &Context,
    checkout_new_worktree: bool,
    perm: &mut RepoExclusive,
) -> Result<gix::ObjectId> {
    let ws = ctx.workspace_from_head_uncached(perm.read_permission())?;
    update_workspace_commit_from_workspace(ctx, checkout_new_worktree, &ws, perm)
}

pub(crate) fn update_workspace_commit_from_workspace(
    ctx: &Context,
    checkout_new_worktree: bool,
    ws: &but_graph::Workspace,
    _perm: &mut RepoExclusive,
) -> Result<gix::ObjectId> {
    let target_base_oid = ws
        .stored_target_commit_id()
        .context("failed to get target base oid")?;

    #[expect(deprecated, reason = "workspace checkout/index boundary")]
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

    let workspace_head = gix_repo.find_commit(
        but_workspace::legacy::remerged_workspace_commit_v2(ctx, ws)?,
    )?;

    // message that says how to get back to where they were
    let mut message = GITBUTLER_WORKSPACE_COMMIT_TITLE.to_string();
    message.push_str("\n\n");
    if !ws.stacks.is_empty() {
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
    if !ws.stacks.is_empty() {
        message.push_str("Here are the branches that are currently applied:\n");
        for stack in &ws.stacks {
            let Some(ref_name) = stack.ref_name() else {
                continue;
            };
            message.push_str(" - ");
            message.push_str(&ref_name.shorten().to_string());
            message.push_str(format!(" ({ref_name})").as_str());
            message.push('\n');

            let head = stack.tip_skip_empty().unwrap_or(target_base_oid);
            if head != target_base_oid {
                message.push_str("   branch head: ");
                message.push_str(&head.to_string());
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

    let checkout_res = if checkout_new_worktree && prev_head_id.is_some() {
        let res = but_core::worktree::safe_checkout_from_head(
            final_commit,
            &gix_repo,
            but_core::worktree::checkout::Options {
                skip_head_update: true,
                ..Default::default()
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
    if let Err(e) = gitbutler_repo::managed_hooks::install_managed_hooks(&gix_repo) {
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

    ctx.invalidate_workspace_cache()?;

    Ok(final_commit)
}
