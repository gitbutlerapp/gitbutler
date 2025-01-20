#![forbid(rust_2018_idioms)]

use anyhow::{bail, Result};
// A happy little module for uploading stacks.

use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_headers::HasCommitHeaders as _;
use gitbutler_oplog::reflog::{set_reference_to_oplog, ReflogCommits};
use gitbutler_oxidize::{git2_signature_to_gix_signature, OidExt as _};
use gitbutler_repo::{commit_message::CommitMessage, signature};
use gitbutler_stack::{
    stack_context::{CommandContextExt, StackContext},
    Stack, StackId, VirtualBranchesHandle,
};
use gitbutler_user::User;
use gix::bstr::ByteSlice;

use crate::cloud::{push_to_gitbutler_server, remote, RemoteKind};

pub fn push_stack_to_review(ctx: &CommandContext, user: &User, stack_id: StackId) -> Result<()> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let mut stack = vb_state.get_stack(stack_id)?;
    let repository = ctx.gix_repository()?;
    // We set the stack immediatly after because it might be an old stack that
    // dosn't yet have review_ids assigned. When reading they will have been
    // assigned new review_ids, so we just need to persist them here.
    vb_state.set_stack(stack.clone())?;
    let stack_context = ctx.to_stack_context()?;

    let branch_heads = branch_heads(&vb_state, &mut stack, &stack_context)?;
    let Some(review_base_id) = vb_state.upsert_last_pushed_base(&repository)? else {
        bail!("This is impossible. If you got here, I'm sorry.");
    };
    set_reference_to_oplog(&ctx.project().path, ReflogCommits::new(ctx.project())?)?;

    let target_commit_id = vb_state.get_default_target()?.sha.to_gix();
    let commits = repository
        .rev_walk([stack.head().to_gix()])
        .first_parent_only()
        .with_pruned([target_commit_id])
        .sorting(gix::revision::walk::Sorting::BreadthFirst)
        .all()?
        .filter_map(|e| Some(e.ok()?.id().detach()))
        .collect::<Vec<_>>();

    let review_head =
        format_stack_for_review(&repository, &commits, &branch_heads, review_base_id)?;

    let refspec = format_refspec(&review_head);

    let remote = remote(ctx, RemoteKind::Oplog)?;
    push_to_gitbutler_server(ctx, Some(user), &[&refspec], remote)?;

    Ok(())
}

fn format_refspec(sha: &gix::ObjectId) -> String {
    format!("+{}:refs/review/publish", sha)
}

struct BranchHead {
    name: String,
    review_id: String,
    id: gix::ObjectId,
}

/// Fetch the stack heads in order and attach a review_id if not already present.
fn branch_heads(
    vb_state: &VirtualBranchesHandle,
    stack: &mut Stack,
    stack_context: &StackContext<'_>,
) -> Result<Vec<BranchHead>> {
    let mut heads = vec![];

    let stack_clone = stack.clone();

    for head in stack.heads.iter_mut() {
        let head_oid = head.head_oid(stack_context, &stack_clone)?.to_gix();
        let review_id = head
            .review_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        head.review_id = Some(review_id.clone());

        heads.push(BranchHead {
            id: head_oid,
            review_id: review_id.clone(),
            name: head.name.to_owned(),
        })
    }

    vb_state.set_stack(stack.clone())?;

    Ok(heads)
}

/// Rewrites a series of commits in preperation for pusing to gitbutler review.
///
/// The heads and commits should be passed in child-most to parent-most order.
///
/// Returns the head of stack.
fn format_stack_for_review(
    repository: &gix::Repository,
    commits: &[gix::ObjectId],
    heads: &[BranchHead],
    base_commit: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let mut previous_commit = base_commit;

    for commit_id in commits.iter().rev() {
        let commit = repository.find_commit(*commit_id)?;
        let decoded_commit = commit.decode()?;
        let mut message = CommitMessage::new(decoded_commit.clone());
        let mut object: gix::objs::Commit = decoded_commit.into();

        message
            .trailers
            .push(("Original-Commit".into(), commit_id.to_string().into()));

        let change_id = commit
            .gitbutler_headers()
            .map(|headers| headers.change_id)
            .unwrap_or_else(|| commit.id.to_string());
        message
            .trailers
            .push(("Change-Id".into(), change_id.into()));

        'heads: for stack_head in heads.iter().rev() {
            if stack_head.id != *commit_id {
                continue 'heads;
            }

            let value = format!("{}, {}", stack_head.name, stack_head.review_id)
                .as_bytes()
                .as_bstr()
                .to_owned();
            message.trailers.push(("Branch-Head".into(), value))
        }

        object.message = message.to_bstring();
        // Remove the signing header so we don't have an invalid signature
        object.extra_headers.retain(|entry| entry.0 != "gpgsig");
        object.parents = [previous_commit].into();
        object.committer = git2_signature_to_gix_signature(signature(
            gitbutler_repo::SignaturePurpose::Committer,
        )?);

        previous_commit = repository.write_object(object)?.detach();
    }

    Ok(previous_commit)
}
