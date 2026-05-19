use but_action::Source;
use but_ctx::Context;
use serde::Serialize;

use crate::utils::OutputChannel;

pub(crate) fn handle_changes(
    ctx: &mut Context,
    out: &mut OutputChannel,
    handler: impl Into<but_action::ActionHandler>,
    change_description: &str,
) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let handler = handler.into();
    let response = but_action::record_uncommitted_changes_with_perm(
        ctx,
        change_description,
        None,
        handler,
        Source::ButCli,
        None,
        perm,
    )?;
    print_json_or_human(&response, out)
}

impl From<crate::args::actions::Handler> for but_action::ActionHandler {
    fn from(val: crate::args::actions::Handler) -> Self {
        match val {
            crate::args::actions::Handler::Simple => but_action::ActionHandler::HandleChangesSimple,
        }
    }
}

pub(crate) fn list_actions(
    ctx: &Context,
    out: &mut OutputChannel,
    offset: i64,
    limit: i64,
) -> anyhow::Result<()> {
    let db = ctx.db.get_cache()?;
    let response = but_action::list_actions(&db, offset, limit)?;
    print_json_or_human(&response, out)
}

pub(crate) fn print_json_or_human<T>(this: &T, out: &mut OutputChannel) -> anyhow::Result<()>
where
    T: ?Sized + Serialize + std::fmt::Debug,
{
    if let Some(out) = out.for_json() {
        out.write_value(this)?;
    } else if let Some(out) = out.for_human() {
        writeln!(out, "{this:#?}")?;
    }
    Ok(())
}
