use but_core::{DryRun, sync::RepoExclusive};

use crate::{
    theme::{self, Paint},
    utils::OutputChannel,
};

pub(crate) fn move_branch_by_name_with_perm(
    ctx: &mut but_ctx::Context,
    branch_name: &str,
    target_branch_name: &str,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> anyhow::Result<()> {
    let t = theme::get();

    let branch_ref_name_str = &format!("refs/heads/{branch_name}");
    let target_ref_name_str = &format!("refs/heads/{target_branch_name}");

    but_api::branch::move_branch_with_perm(
        ctx,
        branch_ref_name_str.try_into()?,
        target_ref_name_str.try_into()?,
        DryRun::No,
        perm,
    )?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Moved branch {} on top of {}.",
            t.local_branch.paint(branch_name),
            t.local_branch.paint(target_branch_name),
        )?;
    }

    Ok(())
}

pub(crate) fn tear_off_branch_by_name_with_perm(
    ctx: &mut but_ctx::Context,
    branch_name: &str,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> anyhow::Result<()> {
    let t = theme::get();
    let branch_ref_name_str = &format!("refs/heads/{branch_name}");

    but_api::branch::tear_off_branch_with_perm(
        ctx,
        branch_ref_name_str.try_into()?,
        DryRun::No,
        perm,
    )?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Unstacked branch {}.",
            t.local_branch.paint(branch_name)
        )?;
    }

    Ok(())
}
