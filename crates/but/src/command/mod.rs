use std::path::Path;

use but_action::Source;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use serde::Serialize;

pub(crate) fn handle_changes(
    repo_path: &Path,
    json: bool,
    handler: impl Into<but_action::ActionHandler>,
    change_description: &str,
) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let response = but_action::handle_changes(
        ctx,
        change_description,
        None,
        handler.into(),
        Source::ButCli,
        None,
    )?;
    print(&response, json)
}

impl From<crate::args::actions::Handler> for but_action::ActionHandler {
    fn from(val: crate::args::actions::Handler) -> Self {
        match val {
            crate::args::actions::Handler::Simple => but_action::ActionHandler::HandleChangesSimple,
        }
    }
}

pub(crate) fn list_actions(
    repo_path: &Path,
    json: bool,
    offset: i64,
    limit: i64,
) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let response = but_action::list_actions(ctx, offset, limit)?;
    print(&response, json)
}

pub(crate) fn print<T>(this: &T, json: bool) -> anyhow::Result<()>
where
    T: ?Sized + Serialize + std::fmt::Debug,
{
    if json {
        let json = serde_json::to_string_pretty(&this)?;
        println!("{json}");
    } else {
        println!("{this:#?}");
    }
    Ok(())
}
