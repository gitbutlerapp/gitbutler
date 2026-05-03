use but_core::{DiffSpec, ref_metadata::StackId};
use but_ctx::{Context, access::RepoExclusive};
use but_hunk_assignment::HunkAssignment;
use but_workspace::commit_engine::{self, CreateCommitOutcome};
use gitbutler_branch_actions::update_workspace_commit;
use gix::ObjectId;
use nonempty::NonEmpty;

use crate::{
    theme::{self, Paint},
    utils::{OutputChannel, shorten_object_id, split_short_id},
};

pub(crate) fn uncommitted_to_commit_with_perm(
    ctx: &mut Context,
    hunk_assignments: NonEmpty<&HunkAssignment>,
    description: String,
    oid: ObjectId,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> anyhow::Result<Option<ObjectId>> {
    let first_hunk_assignment = hunk_assignments.first();
    let stack_id = first_hunk_assignment.stack_id;

    let diff_specs: Vec<DiffSpec> = hunk_assignments
        .into_iter()
        .map(|assignment| assignment.to_owned().into())
        .collect();

    let outcome = amend_diff_specs(ctx, diff_specs, stack_id, oid, perm)?;
    update_workspace_commit(ctx, false)?;
    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let new_commit = outcome
            .new_commit
            .map(|c| {
                let short = shorten_object_id(&repo, c);
                let (lead, rest) = split_short_id(&short, 2);
                let t = theme::get();
                format!("{}{}", t.cli_id.paint(lead), t.cli_id.paint(rest))
            })
            .unwrap_or_default();
        writeln!(out, "Amended {description} → {new_commit}")?;
    }
    Ok(outcome.new_commit)
}

fn amend_diff_specs(
    ctx: &mut Context,
    diff_specs: Vec<DiffSpec>,
    stack_id: Option<StackId>,
    oid: ObjectId,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CreateCommitOutcome> {
    but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
        &*ctx.repo.get()?,
        &ctx.project_data_dir(),
        stack_id,
        commit_engine::Destination::AmendCommit {
            commit_id: oid,
            new_message: None,
        },
        but_workspace::flatten_diff_specs(diff_specs),
        ctx.settings.context_lines,
        perm,
    )
}
