use std::path::Path;

use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

use crate::id::CliId;

pub(crate) fn handle(
    repo_path: &Path,
    _json: bool,
    source: &str,
    target: &str,
) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::default())?;
    let (source, target) = ids(ctx, source, target)?;
    dbg!(source);
    dbg!(target);
    Ok(())
}

fn ids(ctx: &mut CommandContext, source: &str, target: &str) -> anyhow::Result<(CliId, CliId)> {
    let source_result = crate::id::CliId::from_str(ctx, source)?;
    if source_result.len() != 1 {
        return Err(anyhow::anyhow!(
            "Source {} is ambiguous: {:?}",
            source,
            source_result
        ));
    }
    let target_result = crate::id::CliId::from_str(ctx, target)?;
    if target_result.len() != 1 {
        return Err(anyhow::anyhow!(
            "Target {} is ambiguous: {:?}",
            target,
            target_result
        ));
    }
    Ok((source_result[0].clone(), target_result[0].clone()))
}
