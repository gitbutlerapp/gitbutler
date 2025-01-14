#![forbid(rust_2018_idioms)]

use anyhow::{bail, Result};
// A happy little module for uploading stacks.

use gitbutler_command_context::CommandContext;
use gitbutler_oplog::reflog::{set_reference_to_oplog, ReflogCommits};
use gitbutler_oxidize::OidExt as _;
use gitbutler_stack::{
    stack_context::{CommandContextExt, StackContext},
    Stack, StackId, VirtualBranchesHandle,
};
use gitbutler_user::User;
use gix::bstr::{BString, ByteSlice, ByteVec as _};

use crate::cloud::{push_to_gitbutler_server, remote, RemoteKind};

pub fn push_stack_to_review(ctx: &CommandContext, user: &User, stack_id: StackId) -> Result<()> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stack = vb_state.get_stack(stack_id)?;
    let repository = ctx.gix_repository()?;
    // We set the stack immediatly after because it might be an old stack that
    // dosn't yet have review_ids assigned. When reading they will have been
    // assigned new review_ids, so we just need to persist them here.
    vb_state.set_stack(stack.clone())?;
    let stack_context = ctx.to_stack_context()?;

    let branch_heads = branch_heads(&stack, &stack_context)?;
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

/// Fetch the stack heads in order
fn branch_heads(stack: &Stack, stack_context: &StackContext<'_>) -> Result<Vec<BranchHead>> {
    let mut heads = vec![];
    for head in &stack.heads {
        let head_oid = head.head_oid(stack_context, stack)?.to_gix();
        heads.push(BranchHead {
            id: head_oid,
            review_id: head.review_id.clone(),
            name: head.name.to_owned(),
        })
    }

    Ok(heads)
}

struct CommitMessage {
    title: BString,
    body: BString,
    trailers: Vec<(BString, BString)>,
}

impl CommitMessage {
    // I tried not allocating but couldn't get it right. This works fine
    fn to_bstring(&self) -> BString {
        let mut out = BString::default();
        out.push_str(self.title.clone());
        out.push_str(b"\n\n");
        out.push_str(self.body.clone());
        out.push_str(b"\n\n");
        out.push_str(self.trailers_as_bstring());
        out
    }

    fn trailers_as_bstring(&self) -> BString {
        let mut out = BString::default();
        for (index, trailer) in self.trailers.iter().enumerate() {
            let trailer = gix::bstr::join(": ", [&trailer.0, &trailer.1]);
            out.push_str(trailer);

            if index != self.trailers.len() - 1 {
                out.push_str(b"\n")
            }
        }
        out
    }

    fn new(commit: gix::objs::CommitRef<'_>) -> Self {
        let message_ref = commit.message();
        let body_ref = message_ref.body();

        CommitMessage {
            title: commit.message().title.to_owned(),
            body: body_ref
                .map(|body_ref| body_ref.without_trailer().as_bstr().to_owned())
                .unwrap_or_default(),
            trailers: body_ref
                .map(|body_ref| {
                    body_ref
                        .trailers()
                        .map(|trailer| (trailer.token.to_owned(), trailer.value.to_owned()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        }
    }
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

    for (index, commit_id) in commits.iter().enumerate().rev() {
        let commit = repository.find_commit(*commit_id)?;
        let decoded_commit = commit.decode()?;
        let mut message = CommitMessage::new(decoded_commit.clone());
        let mut object: gix::objs::Commit = decoded_commit.into();

        // The parent-most commit is the last one in the array
        if index == commits.len() - 1 {
            message
                .trailers
                .push((b"Base-Commit".into(), base_commit.to_string().into()));
        }

        message
            .trailers
            .push((b"Original-Commit".into(), commit_id.to_string().into()));

        'heads: for stack_head in heads {
            if stack_head.id != *commit_id {
                continue 'heads;
            }

            let value = format!("{}, {}", stack_head.name, stack_head.review_id)
                .as_bytes()
                .as_bstr()
                .to_owned();
            message.trailers.push((b"Branch-Head".into(), value))
        }

        object.message = message.to_bstring();
        // Remove the signing header so we don't have an invalid signature
        object.extra_headers.retain(|entry| entry.0 != "gpgsig");
        object.parents = [previous_commit].into();

        previous_commit = repository.write_object(object)?.detach();
    }

    Ok(previous_commit)
}
