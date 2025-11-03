use std::str::FromStr;

use anyhow::bail;
use but_rules::Operation;
use but_settings::AppSettings;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_project::Project;

use crate::rub::branch_name_to_stack_id;
pub(crate) fn handle(
    project: &Project,
    _json: bool,
    target_str: &str,
    delete: bool,
) -> anyhow::Result<()> {
    let ctx = &mut CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;
    let target_result = crate::id::CliId::from_str(ctx, target_str)?;
    if target_result.len() != 1 {
        return Err(anyhow::anyhow!(
            "Target {} is ambiguous: {:?}",
            target_str,
            target_result
        ));
    }
    // Hack - delete all other rules
    for rule in but_rules::list_rules(ctx)? {
        but_rules::delete_rule(ctx, &rule.id())?;
    }
    match target_result[0].clone() {
        crate::id::CliId::Branch { name } => mark_branch(ctx, name, delete),
        crate::id::CliId::Commit { oid } => mark_commit(ctx, oid, delete),
        _ => bail!("Nope"),
    }
}

fn mark_commit(ctx: &mut CommandContext, oid: gix::ObjectId, delete: bool) -> anyhow::Result<()> {
    if delete {
        let rules = but_rules::list_rules(ctx)?;
        for rule in rules {
            if rule.target_commit_id() == Some(oid.to_string()) {
                but_rules::delete_rule(ctx, &rule.id())?;
            }
        }
        println!("Mark was removed");
        return Ok(());
    }
    let repo = ctx.gix_repo()?;
    let commit = repo.find_commit(oid)?;
    let change_id = commit.change_id().ok_or_else(|| {
        anyhow::anyhow!("Commit {} does not have a Change-Id, cannot mark it", oid)
    })?;
    let action = but_rules::Action::Explicit(Operation::Amend { change_id });
    let req = but_rules::CreateRuleRequest {
        trigger: but_rules::Trigger::FileSytemChange,
        filters: vec![but_rules::Filter::PathMatchesRegex(regex::Regex::new(
            ".*",
        )?)],
        action,
    };
    but_rules::create_rule(ctx, req)?;
    println!("Changes will be amended into commit → {}", &oid.to_string());
    Ok(())
}

fn mark_branch(ctx: &mut CommandContext, branch_name: String, delete: bool) -> anyhow::Result<()> {
    let stack_id = branch_name_to_stack_id(ctx, Some(&branch_name))?;
    if delete {
        let rules = but_rules::list_rules(ctx)?;
        for rule in rules {
            if rule.target_stack_id() == stack_id.map(|s| s.to_string()) {
                but_rules::delete_rule(ctx, &rule.id())?;
            }
        }
        println!("Mark was removed");
        return Ok(());
    }
    // TODO: if there are other marks of this kind, get rid of them
    let stack_id = stack_id.expect("Cant find stack for this branch");
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

pub(crate) fn stack_marked(ctx: &mut CommandContext, stack_id: StackId) -> anyhow::Result<bool> {
    let rules = but_rules::list_rules(ctx)?
        .iter()
        .any(|r| r.target_stack_id() == Some(stack_id.to_string()) && r.session_id().is_none());
    Ok(rules)
}

pub(crate) fn commit_marked(ctx: &mut CommandContext, commit_id: String) -> anyhow::Result<bool> {
    let repo = ctx.gix_repo()?;
    let commit = repo.find_commit(gix::ObjectId::from_str(&commit_id)?)?;
    let change_id = commit.change_id().ok_or_else(|| {
        anyhow::anyhow!(
            "Commit {} does not have a Change-Id, cannot mark it",
            commit_id
        )
    })?;
    let rules = but_rules::list_rules(ctx)?
        .iter()
        .any(|r| r.target_commit_id() == Some(change_id.clone()));
    Ok(rules)
}

pub(crate) fn unmark(project: &Project, _json: bool) -> anyhow::Result<()> {
    let ctx = &mut CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    let rules = but_rules::list_rules(ctx)?;
    let rule_count = rules.len();

    if rule_count == 0 {
        println!("No marks to remove");
        return Ok(());
    }

    for rule in rules {
        but_rules::delete_rule(ctx, &rule.id())?;
    }

    println!(
        "Removed {} mark{}",
        rule_count,
        if rule_count == 1 { "" } else { "s" }
    );
    Ok(())
}
