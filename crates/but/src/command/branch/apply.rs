use but_ctx::Context;
use gix::reference::Category;

use crate::utils::OutputChannel;

/// Apply a branch to the workspace, and return the full ref name to it.
pub fn apply(ctx: Context, branch_name: &str, out: &mut OutputChannel) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;

    let reference = repo.find_reference(branch_name)?;
    let _outcome = but_api::branch::apply(&ctx, reference.name())?;

    if let Some(out) = out.for_human() {
        let short_name = reference.name().shorten();
        let is_remote_reference = reference
            .name()
            .category()
            .is_some_and(|c| c == Category::RemoteBranch);
        if is_remote_reference {
            writeln!(out, "Applied remote branch '{short_name}' to workspace")
        } else {
            writeln!(out, "Applied branch '{short_name}' to workspace")
        }?;
    } else if let Some(out) = out.for_shell() {
        writeln!(out, "{reference_name}", reference_name = reference.name())?;
    }

    if let Some(out) = out.for_json() {
        out.write_value(but_api::json::Reference::from(reference.inner))?;
    }
    Ok(())
}
