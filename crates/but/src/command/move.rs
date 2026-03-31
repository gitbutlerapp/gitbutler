use anyhow::Context as _;

use crate::{CliId, IdMap, utils::OutputChannel};

pub(crate) fn handle(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    source: &str,
    target: &str,
    after: bool,
) -> anyhow::Result<()> {
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;
    let source_id =
        resolve_single(&id_map, ctx, source, "Source").context("Failed to move commit.")?;
    let target_id =
        resolve_single(&id_map, ctx, target, "Target").context("Failed to move commit.")?;

    let branch_route = matches!(
        (&source_id, &target_id),
        (
            CliId::Branch { .. },
            CliId::Branch { .. } | CliId::Unassigned { .. }
        )
    );

    let move_result = if branch_route {
        let (
            CliId::Branch {
                name: source_name, ..
            },
            target_id,
        ) = (&source_id, &target_id)
        else {
            unreachable!("branch_route guarantees source is a branch")
        };
        if after {
            // TODO: Allow to move branch after another below another branch.
            Err(anyhow::anyhow!(
                "The --after flag only makes sense when moving a commit to another commit."
            ))
        } else {
            match target_id {
                CliId::Unassigned { .. } => {
                    super::branch::tear_off_branch_by_name(ctx, source_name, out)
                }
                CliId::Branch {
                    name: target_name, ..
                } => super::branch::move_branch_by_name(ctx, source_name, target_name, out),
                _ => unreachable!("branch_route guarantees target is branch or unassigned"),
            }
        }
    } else {
        super::commit::r#move::handle_resolved(ctx, out, &source_id, &target_id, after)
    };

    if branch_route {
        move_result.context("Failed to move branch.")
    } else {
        move_result.context("Failed to move commit.")
    }
}

fn resolve_single(
    id_map: &IdMap,
    ctx: &but_ctx::Context,
    selector: &str,
    kind: &str,
) -> anyhow::Result<CliId> {
    let matches = id_map.parse_using_context(selector, ctx)?;
    if matches.is_empty() {
        anyhow::bail!(
            "{kind} '{selector}' not found. If you just performed a Git operation, try running 'but status' to refresh."
        );
    }
    if matches.len() > 1 {
        anyhow::bail!(
            "{kind} '{selector}' is ambiguous. Try using more characters to disambiguate."
        );
    }
    Ok(matches[0].clone())
}
