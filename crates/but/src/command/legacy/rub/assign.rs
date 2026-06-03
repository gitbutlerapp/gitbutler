use bstr::BString;
use but_core::{HunkHeader, ref_metadata::StackId};
use but_ctx::Context;
use but_hunk_assignment::{HunkAssignmentRequest, HunkAssignmentTarget};

pub(crate) fn do_assignments(
    ctx: &Context,
    reqs: Vec<HunkAssignmentRequest>,
) -> anyhow::Result<()> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    but_hunk_assignment::assign(db.hunk_assignments_mut()?, &repo, &ws, reqs, context_lines)?;
    Ok(())
}

pub(crate) fn branch_name_to_stack_id(
    ctx: &Context,
    branch_name: Option<&str>,
) -> anyhow::Result<Option<StackId>> {
    let stack_id = if let Some(branch_name) = branch_name {
        crate::legacy::workspace::applied_stacks(ctx)?
            .iter()
            .find(|s| s.contains_branch(branch_name))
            .and_then(|s| s.id)
    } else {
        None
    };
    Ok(stack_id)
}

pub(crate) fn stack_id_to_branch_name(ctx: &Context, stack_id: StackId) -> Option<String> {
    crate::legacy::workspace::applied_stacks(ctx)
        .ok()?
        .into_iter()
        .find(|s| s.id.as_ref() == Some(&stack_id))
        .and_then(|s| s.top_branch_name().map(ToOwned::to_owned))
}

/// Normalize a branch name to a full ref name (e.g. "foo" → "refs/heads/foo").
/// If the name is already a full ref, it is returned as-is.
fn to_full_ref_name(name: &str) -> anyhow::Result<gix::refs::FullName> {
    let full = if name.starts_with("refs/") {
        name.to_string()
    } else {
        format!("refs/heads/{name}")
    };
    gix::refs::FullName::try_from(full).map_err(|e| anyhow::anyhow!("invalid ref name: {e}"))
}

/// Normalize a branch name for stack lookup.
/// Local branch refs like "refs/heads/foo" are converted to "foo" because stack heads
/// are stored as shortened branch names.
fn normalize_branch_name_for_lookup(name: &str) -> &str {
    name.strip_prefix("refs/heads/").unwrap_or(name)
}

pub(crate) fn to_assignment_request(
    ctx: &mut Context,
    assignments: impl Iterator<Item = (Option<HunkHeader>, BString)>,
    branch_name: Option<&str>,
) -> anyhow::Result<Vec<HunkAssignmentRequest>> {
    let normalized = branch_name.map(normalize_branch_name_for_lookup);
    let stack_id = branch_name_to_stack_id(ctx, normalized)?;
    let target = match (stack_id, branch_name) {
        (Some(_), Some(name)) => Some(HunkAssignmentTarget::Branch {
            branch_ref_bytes: to_full_ref_name(name)?.into_inner(),
        }),
        (Some(stack_id), None) => Some(HunkAssignmentTarget::Stack { stack_id }),
        _ => None,
    };

    let mut reqs = Vec::new();
    for (hunk_header, path_bytes) in assignments {
        reqs.push(HunkAssignmentRequest {
            hunk_header,
            path_bytes,
            target: target.clone(),
        });
    }
    Ok(reqs)
}
