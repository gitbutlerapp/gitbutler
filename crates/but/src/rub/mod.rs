use std::path::Path;

use anyhow::bail;
use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
mod amend;
mod assign;

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
        (CliId::UncommittedFile { path, .. }, CliId::Unassigned) => {
            assign::unassign_file(ctx, path)
        }
        (CliId::UncommittedFile { path, assignment }, CliId::Commit { oid }) => {
            amend::file_to_commit(ctx, path, *assignment, oid)
        }
        (CliId::UncommittedFile { path, .. }, CliId::Branch { name }) => {
            assign::assign_file_to_branch(ctx, path, name)
        }
        (CliId::Unassigned, CliId::UncommittedFile { .. }) => {
            bail!(makes_no_sense_error(&source, &target))
        }
        (CliId::Unassigned, CliId::Unassigned) => {
            bail!(makes_no_sense_error(&source, &target))
        }
        (CliId::Unassigned, CliId::Commit { oid }) => amend::assignments_to_commit(ctx, None, oid),
        (CliId::Unassigned, CliId::Branch { name: to }) => assign::assign_all(ctx, None, Some(to)),
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
        (CliId::Branch { name: from }, CliId::Unassigned) => {
            assign::assign_all(ctx, Some(from), None)
        }
        (CliId::Branch { name }, CliId::Commit { oid }) => {
            amend::assignments_to_commit(ctx, Some(name), oid)
        }
        (CliId::Branch { name: from }, CliId::Branch { name: to }) => {
            assign::assign_all(ctx, Some(from), Some(to))
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
