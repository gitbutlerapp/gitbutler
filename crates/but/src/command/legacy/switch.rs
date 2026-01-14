use but_ctx::Context;

use crate::utils::OutputChannel;

pub(crate) fn handle(ctx: &mut Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    but_api::legacy::virtual_branches::switch_back_to_workspace(ctx.legacy_project.id)?;
    if let Some(out) = out.for_human() {
        writeln!(out, "Switched back to workspace.")?;
    }
    Ok(())
}
