use anyhow::{Result, bail};
// A happy little module for uploading stacks.
use but_oxidize::{ObjectIdExt, OidExt as _, git2_signature_to_gix_signature};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_headers::HasCommitHeaders as _;
use gitbutler_oplog::reflog::{ReflogCommits, set_reference_to_oplog};
use gitbutler_repo::{commit_message::CommitMessage, signature};
use gitbutler_stack::{Stack, StackId, VirtualBranchesHandle};
use gitbutler_user::User;
use gix::bstr::ByteSlice;
use rand::Rng;

use crate::cloud::{RemoteKind, push_to_gitbutler_server, remote};

/// Pushes all the branches in a stack, starting at the specified top_branch.
pub fn push_stack_to_review(
    ctx: &CommandContext,
    user: &User,
    stack_id: StackId,
    top_branch: String,
) -> Result<String> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let mut stack = vb_state.get_stack(stack_id)?;
    let repo = ctx.gix_repo()?;
    // We set the stack immediatly after because it might be an old stack that
    // dosn't yet have review_ids assigned. When reading they will have been
    // assigned new review_ids, so we just need to persist them here.
    vb_state.set_stack(stack.clone())?;

    let branch_heads = branch_heads(&vb_state, &mut stack, &repo, top_branch)?;
    let Some(top_branch) = branch_heads.first() else {
        bail!("No branches to be pushed.");
    };
    let Some(review_base_id) = vb_state.upsert_last_pushed_base(&repo)? else {
        bail!("This is impossible. If you got here, I'm sorry.");
    };
    set_reference_to_oplog(ctx.project().git_dir(), ReflogCommits::new(ctx.project())?)?;

    let target_commit_id = vb_state.get_default_target()?.sha.to_gix();
    let git2_repository = ctx.repo();
    let mut revwalk = git2_repository.revwalk()?;
    revwalk.push(top_branch.id.to_git2())?;
    revwalk.hide(target_commit_id.to_git2())?;
    let commits = revwalk
        .filter_map(|commit| Some(commit.ok()?.to_gix()))
        .collect::<Vec<_>>();

    let review_head = format_stack_for_review(&repo, &commits, &branch_heads, review_base_id)?;

    let refspec = format_refspec(&review_head);

    let remote = remote(ctx, RemoteKind::Oplog)?;
    push_to_gitbutler_server(ctx, Some(user), &[&refspec], remote)?;

    let Some(head_review) = branch_heads.first() else {
        bail!("No head review id. Congratulations, this is not possible")
    };

    Ok(head_review.review_id.clone())
}

fn format_refspec(sha: &gix::ObjectId) -> String {
    format!("+{sha}:refs/review/publish")
}

struct BranchHead {
    name: String,
    review_id: String,
    id: gix::ObjectId,
}

/// The worlds most secure random string generator because uuids are "not cool"
fn generate_review_id() -> String {
    let mut rng = rand::rng();
    let digit = rng.sample(rand::distr::Uniform::new_inclusive(0, 9).unwrap());
    let letters = (0..8)
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .collect::<String>();
    format!("{digit}{letters}")
}

/// Fetch the stack heads in order and attach a review_id if not already present.
fn branch_heads(
    vb_state: &VirtualBranchesHandle,
    stack: &mut Stack,
    repo: &gix::Repository,
    top_branch: String,
) -> Result<Vec<BranchHead>> {
    let mut heads = vec![];

    let mut top_head_found = false;

    // Heads is listed from parent-most to child-most, but we are wanting to
    // find the parent-most match, and any branches that are parent-er to the
    // found branch, so we iterate in reverse.
    for head in stack.heads.iter_mut().rev() {
        if !top_head_found {
            if head.name() == &top_branch {
                top_head_found = true;
            } else {
                continue;
            }
        }

        let head_oid = head.head_oid(repo)?;
        let review_id = head.review_id.clone().unwrap_or_else(generate_review_id);
        head.review_id = Some(review_id.clone());

        heads.push(BranchHead {
            id: head_oid,
            review_id: review_id.clone(),
            name: head.name().to_owned(),
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
    repo: &gix::Repository,
    commits: &[gix::ObjectId],
    heads: &[BranchHead],
    base_commit: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let mut previous_commit = base_commit;

    for commit_id in commits.iter().rev() {
        let commit = repo.find_commit(*commit_id)?;
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

        previous_commit = repo.write_object(object)?.detach();
    }

    Ok(previous_commit)
}
