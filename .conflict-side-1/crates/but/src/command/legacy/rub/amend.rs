use but_core::DiffSpec;
use but_ctx::{Context, access::RepoExclusive};
use but_hunk_assignment::HunkAssignment;
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use gitbutler_branch_actions::update_workspace_commit;
use gix::ObjectId;
use nonempty::NonEmpty;

use crate::{
    theme::{self, Paint},
    utils::{
        OutputChannel, diff_specs::DiffSpecBuilder, rejection, shorten_object_id, split_short_id,
    },
};

pub(crate) fn uncommitted_to_commit_with_perm(
    ctx: &mut Context,
    hunk_assignments: NonEmpty<&HunkAssignment>,
    description: String,
    oid: ObjectId,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> anyhow::Result<()> {
    // Resolve the target commit's branch before amending, while `oid` is still
    // valid, so a dependency rejection can suggest stacking onto it.
    let (diff_specs, target_branch) = {
        let context_lines = ctx.settings.context_lines;
        let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
        let target_branch = rejection::branch_of_commit(&ws, oid, None);
        let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
        builder.push_hunk_assignments(hunk_assignments.into_iter().map(ToOwned::to_owned))?;
        (builder.into_diff_specs(), target_branch)
    };

    let (new_commit, rejected) = amend_diff_specs(ctx, diff_specs, oid, perm)?;
    update_workspace_commit(ctx, false)?;
    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let new_commit = new_commit
            .map(|c| {
                let short = shorten_object_id(&repo, c);
                let (lead, rest) = split_short_id(&short, 2);
                let t = theme::get();
                format!("{}{}", t.cli_id.paint(lead), t.cli_id.paint(rest))
            })
            .unwrap_or_default();
        writeln!(out, "Amended {description} → {new_commit}")?;
        rejection::write_rejection_report(out, &rejected, target_branch.as_deref())?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "ok": true,
            "new_commit_id": new_commit.map(|c| c.to_string()),
            "rejected": serde_json::to_value(&rejected).unwrap_or_default(),
        }))?;
    }
    Ok(())
}

fn amend_diff_specs(
    ctx: &mut Context,
    diff_specs: Vec<DiffSpec>,
    oid: ObjectId,
    perm: &mut RepoExclusive,
) -> anyhow::Result<(Option<ObjectId>, Vec<rejection::RejectedChange>)> {
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = but_workspace::commit::commit_amend(
        editor,
        oid,
        but_workspace::flatten_diff_specs(diff_specs),
        ctx.settings.context_lines,
    )?;

    let rejected_specs = outcome.rejected_specs;
    if !rejected_specs.is_empty() {
        tracing::warn!(?rejected_specs, "Failed to commit at least one hunk");
    }
    let new_commit = outcome
        .commit_selector
        .map(|selector| outcome.rebase.lookup_pick(selector))
        .transpose()?;
    outcome.rebase.materialize()?;

    // `materialize()` released the workspace borrow, so we can now look up why
    // the rejected changes were locked and to which branch each one depends on.
    let rejected = rejection::explain_rejections(&repo, &ws, &rejected_specs);
    Ok((new_commit, rejected))
}
