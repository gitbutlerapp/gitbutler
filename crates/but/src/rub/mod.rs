use std::path::Path;

use anyhow::bail;
use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

use crate::id::CliId;

pub(crate) fn handle(
    repo_path: &Path,
    _json: bool,
    source_str: &str,
    target_str: &str,
) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::default())?;
    let (source, target) = ids(ctx, source_str, target_str)?;

    match (&source, &target) {
        (CliId::UncommittedFile { .. }, CliId::UncommittedFile { .. }) => {
            bail!(makes_no_sense_error(&source, &target))
        }
        (CliId::UncommittedFile { .. }, CliId::Unassigned) => {
            bail!(not_implemented("Unassign file", &source, &target))
        }
        (CliId::UncommittedFile { .. }, CliId::Commit { .. }) => {
            bail!(not_implemented("Amend file into commit", &source, &target))
        }
        (CliId::UncommittedFile { .. }, CliId::Branch { .. }) => {
            bail!(not_implemented("Assign file to branch", &source, &target))
        }
        (CliId::Unassigned, CliId::UncommittedFile { .. }) => {
            bail!(makes_no_sense_error(&source, &target))
        }
        (CliId::Unassigned, CliId::Unassigned) => {
            bail!(makes_no_sense_error(&source, &target))
        }
        (CliId::Unassigned, CliId::Commit { .. }) => {
            bail!(not_implemented("Amend all unassigned", &source, &target))
        }
        (CliId::Unassigned, CliId::Branch { .. }) => {
            bail!(not_implemented("Assign all unassigned", &source, &target))
        }
        (CliId::Commit { .. }, CliId::UncommittedFile { .. }) => {
            bail!(makes_no_sense_error(&source, &target))
        }
        (CliId::Commit { .. }, CliId::Unassigned) => {
            bail!(not_implemented("Uncommit", &source, &target))
        }
        (CliId::Commit { .. }, CliId::Commit { .. }) => {
            bail!(not_implemented("Squash commits", &source, &target))
        }
        (CliId::Commit { .. }, CliId::Branch { .. }) => {
            bail!(not_implemented("Move commit to branch", &source, &target))
        }
        (CliId::Branch { .. }, CliId::UncommittedFile { .. }) => {
            bail!(makes_no_sense_error(&source, &target))
        }
        (CliId::Branch { .. }, CliId::Unassigned) => {
            bail!(not_implemented("Unassign all", &source, &target))
        }
        (CliId::Branch { .. }, CliId::Commit { .. }) => {
            bail!(not_implemented("Amend all assignments", &source, &target))
        }
        (CliId::Branch { .. }, CliId::Branch { .. }) => {
            bail!(not_implemented("Move all assignments", &source, &target))
        }
    }
}

fn not_implemented(desc: &str, source: &CliId, target: &CliId) -> String {
    format!(
        "{} is not implemented yet. Source {} is {} and target {} is {}.",
        desc,
        source.to_string().blue().underline(),
        source.kind().yellow(),
        target.to_string().blue().underline(),
        target.kind().yellow()
    )
}
fn makes_no_sense_error(source: &CliId, target: &CliId) -> String {
    format!(
        "Operation doesn't make sense. Source {} is {} and target {} is {}.",
        source.to_string().blue().underline(),
        source.kind().yellow(),
        target.to_string().blue().underline(),
        target.kind().yellow()
    )
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
