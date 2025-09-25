use std::path::Path;

use crate::rub::branch_name_to_stack_id;
use anyhow::bail;
use but_rules::Operation;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
pub(crate) fn handle(repo_path: &Path, _json: bool, target_str: &str) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let target_result = crate::id::CliId::from_str(ctx, target_str)?;
    if target_result.len() != 1 {
        return Err(anyhow::anyhow!(
            "Target {} is ambiguous: {:?}",
            target_str,
            target_result
        ));
    }
    match target_result[0].clone() {
        crate::id::CliId::Branch { name } => mark_branch(ctx, name),
        crate::id::CliId::Commit { oid } => mark_commit(oid),
        _ => bail!("Nope"),
    }
}

fn mark_commit(_oid: gix::ObjectId) -> anyhow::Result<()> {
    bail!("Not implemented yet");
}

fn mark_branch(ctx: &mut CommandContext, branch_name: String) -> anyhow::Result<()> {
    let stack_id =
        branch_name_to_stack_id(ctx, Some(&branch_name))?.expect("Cant find stack for this branch");
    let action = but_rules::Action::Explicit(Operation::Assign {
        target: but_rules::StackTarget::StackId(stack_id.to_string()),
    });
    let req = but_rules::CreateRuleRequest {
        trigger: but_rules::Trigger::FileSytemChange,
        filters: vec![but_rules::Filter::PathMatchesRegex(regex::Regex::new(
            ".*",
        )?)],
        action,
    };
    but_rules::create_rule(ctx, req)?;
    println!("Changes will be assigned to → {branch_name}");
    Ok(())
}
