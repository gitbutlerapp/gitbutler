use anyhow::{Result, bail};
use but_ctx::Context;

use crate::{CliId, IdMap, tui::get_text};

pub(crate) fn open_target(ctx: &mut Context, target: &str) -> Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;
    let cli_ids = id_map.parse_using_context(target, ctx)?;
    if cli_ids.is_empty() {
        bail!("ID '{target}' not found")
    }

    if cli_ids.len() > 1 {
        bail!(
            "ID '{target}' is ambiguous. Found {} matches",
            cli_ids.len()
        )
    }

    let cli_id = &cli_ids[0];

    match cli_id {
        CliId::Uncommitted(uncommitted_id) => {
            if !uncommitted_id.is_entire_file {
                bail!("Cannot open part of file")
            }

            let path = &uncommitted_id.hunk_assignments.head.path;
            get_text::launch_editor(path.as_ref())
        }
        _ => bail!("Can only open uncommitted files"),
    }
}
