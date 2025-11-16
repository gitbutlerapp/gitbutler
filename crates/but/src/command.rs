use crate::utils::OutputChannel;
use but_action::Source;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use serde::Serialize;

pub(crate) fn handle_changes(
    project: &Project,
    out: &mut OutputChannel,
    handler: impl Into<but_action::ActionHandler>,
    change_description: &str,
) -> anyhow::Result<()> {
    let ctx = &mut CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;
    let response = but_action::handle_changes(
        ctx,
        change_description,
        None,
        handler.into(),
        Source::ButCli,
        None,
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
    project: &Project,
    out: &mut OutputChannel,
    offset: i64,
    limit: i64,
) -> anyhow::Result<()> {
    let ctx = &mut CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    let response = but_action::list_actions(ctx, offset, limit)?;
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
