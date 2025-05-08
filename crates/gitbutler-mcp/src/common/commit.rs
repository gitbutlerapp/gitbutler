use anyhow::{bail, Result};
use but_settings::AppSettings;
use gitbutler_branch::{BranchCreateRequest, BranchIdentity, BranchUpdateRequest};
use gitbutler_branch_actions::{get_branch_listing_details, list_branches, BranchManagerExt};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{Stack, VirtualBranchesHandle};

pub fn commit(project: Project, full_prompt: String, summary: String) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    let list_result = gitbutler_branch_actions::list_virtual_branches(&ctx)?;

    // just get the first stack for now
    let stack = VirtualBranchesHandle::new(project.gb_dir())
        .list_stacks_in_workspace()?
        .into_iter()
        .next()
        .ok_or(anyhow::anyhow!("No stacks found in the project directory"))?;

    dbg!(&stack);

    if !list_result.skipped_files.is_empty() {
        eprintln!(
            "{} files could not be processed (binary or large size)",
            list_result.skipped_files.len()
        )
    }

    dbg!(&list_result);

    let target_branch = list_result
        .branches
        .iter()
        .next()
        .expect("A populated branch exists for a branch we can list");
    if target_branch.ownership.claims.is_empty() {
        bail!(
            "Branch has no change to commit{hint}",
            hint = {
                let candidate_names = list_result
                    .branches
                    .iter()
                    .filter_map(|b| (!b.ownership.claims.is_empty()).then_some(b.name.as_str()))
                    .collect::<Vec<_>>();
                let mut candidates = candidate_names.join(", ");
                if !candidate_names.is_empty() {
                    candidates = format!(
                        ". {candidates} {have} changes.",
                        have = if candidate_names.len() == 1 {
                            "has"
                        } else {
                            "have"
                        }
                    )
                };
                candidates
            }
        )
    }

    let message = full_prompt + "\n\n" + &summary;
    dbg!(&message);

    let _oid = gitbutler_branch_actions::create_commit(
        &ctx,
        stack.id,
        &message,
        Some(&target_branch.ownership),
    )?;

    dbg!("Commit created successfully");
    dbg!(_oid);

    Ok(())
}
