use std::path::Path;

use anyhow::bail;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use serde::Serialize;

pub(crate) fn handle_changes(
    repo_path: &Path,
    json: bool,
    simple: bool,
    change_description: &str,
) -> anyhow::Result<()> {
    if !simple {
        bail!("Only simple mode is supported");
    }
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::default())?;
    let response = but_auto::handle_changes_simple(ctx, change_description)?;
    print(&response, json)
}

fn print<T>(this: &T, json: bool) -> anyhow::Result<()>
where
    T: ?Sized + Serialize + std::fmt::Debug,
{
    if json {
        let json = serde_json::to_string_pretty(&this)?;
        println!("{json}");
    } else {
        println!("{:#?}", this);
    }
    Ok(())
}
