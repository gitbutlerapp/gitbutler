use crate::{CliId, IdMap, id::parser::parse_sources, utils::OutputChannel};
use anyhow::{Context, bail};

/// Move a branch on top of another
pub fn move_branch(
    mut ctx: but_ctx::Context,
    branch: &str,
    target_branch: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(&mut ctx, None)?;
    let branch_name = resolve_branch_information(&mut ctx, &id_map, branch)
        .context("Failed to determine information for the branch to move.")?;
    let target_branch_name = resolve_branch_information(&mut ctx, &id_map, target_branch)
        .context("Failed to determine information for the target branch.")?;

    let branch_ref_name_str = &format!("refs/heads/{branch_name}");
    let target_ref_name_str = &format!("refs/heads/{target_branch_name}");

    but_api::branch::move_branch(
        &mut ctx,
        branch_ref_name_str.try_into()?,
        target_ref_name_str.try_into()?,
    )?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Moved branch '{branch_name}' on top of '{target_branch_name}'."
        )?;
    }

    Ok(())
}

pub fn tear_off_branch(
    mut ctx: but_ctx::Context,
    branch: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(&mut ctx, None)?;
    let branch_name = resolve_branch_information(&mut ctx, &id_map, branch)
        .context("Failed to determine information for the branch to tear off.")?;

    let branch_ref_name_str = &format!("refs/heads/{branch_name}");

    but_api::branch::tear_off_branch(&mut ctx, branch_ref_name_str.try_into()?)?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Tore off branch '{branch_name}'.")?;
    }

    Ok(())
}

fn resolve_branch_information(
    ctx: &mut but_ctx::Context,
    id_map: &IdMap,
    branch_selector: &str,
) -> anyhow::Result<String> {
    let branch_cli_ids = parse_sources(ctx, id_map, branch_selector)?;
    let branch_name = match &branch_cli_ids.as_slice() {
        &[single_clid] => {
            match single_clid {
                CliId::Branch { name, .. } => name,
                CliId::Commit { .. } => {
                    bail!(
                        "Unable to resolve branch information from commit selector: {branch_selector}"
                    );
                }
                CliId::CommittedFile { .. } => {
                    bail!(
                        "Unable to resolve branch information from committed file selector: {branch_selector}"
                    );
                }
                CliId::PathPrefix { .. } => {
                    bail!(
                        "Unable to resolve branch information from path prefix selector: {branch_selector}"
                    );
                }
                CliId::Stack { .. } => {
                    // TODO: Should we select the top branch?
                    bail!(
                        "Unable to resolve branch information from stack selector: {branch_selector}"
                    );
                }
                CliId::Unassigned { .. } => {
                    bail!(
                        "Unable to resolve branch information from unassigned area selector: {branch_selector}"
                    );
                }
                CliId::Uncommitted(..) => {
                    bail!(
                        "Unable to resolve branch information from uncommitted change selector: {branch_selector}"
                    );
                }
            }
        }
        _ => {
            // If there's 0 or more than one CLI ID found, we can't determine the branch information reliably.
            bail!("Unable to resolve the branch information from selector: {branch_selector}");
        }
    };

    Ok(branch_name.clone())
}
