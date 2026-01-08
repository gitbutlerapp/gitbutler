use but_ctx::Context;

use crate::{CliId, IdMap, command::legacy::diff::show::Filter, utils::OutputChannel};

mod display;
mod show;

// Note: To use the DiffDisplay trait in other modules,
// import it with: use crate::command::diff::display::DiffDisplay;

pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    target_str: Option<&str>,
) -> anyhow::Result<()> {
    let wt_changes = but_api::legacy::diff::changes_in_worktree(ctx)?;
    let mut id_map = IdMap::new_from_context(ctx, Some(wt_changes.assignments.clone()))?;
    id_map.add_committed_file_info_from_context(ctx)?;

    if let Some(entity) = target_str {
        let id = id_map
            .resolve_entity_to_ids(entity)? // TODO: look up plain names
            .first() // TODO: handle ambiguity
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No ID found for entity"))?;

        match id {
            CliId::Uncommitted(id) => {
                show::worktree(wt_changes, id_map, out, Some(Filter::Uncommitted(id)))
            }
            CliId::Unassigned { .. } => {
                show::worktree(wt_changes, id_map, out, Some(Filter::Unassigned))
            }
            CliId::CommittedFile {
                commit_id, path, ..
            } => show::commit(ctx, out, commit_id, Some(path)),
            CliId::Branch { name, .. } => show::branch(ctx, out, name),
            CliId::Commit { commit_id: id, .. } => show::commit(ctx, out, id, None),
        }
    } else {
        show::worktree(wt_changes, id_map, out, None)
    }
}
