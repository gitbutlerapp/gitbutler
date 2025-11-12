use std::path::Path;

use crate::commit_engine::index::{delete_entry_by_path_bounded_stages, upsert_index_entry};
use bstr::{BStr, ByteSlice};

/// Turn `rhs` into `lhs` by modifying `rhs`. This will leave `rhs` intact as much as possible.
/// Note that conflicting entries will be replaced by an addition or edit automatically.
/// extensions that might be affected by these changes, for a lack of finesse with our edits.
pub fn apply_lhs_to_rhs(
    workdir: &Path,
    lhs: &gix::index::State,
    rhs: &mut gix::index::State,
) -> anyhow::Result<()> {
    let mut num_sorted_entries = rhs.entries().len();
    let mut needs_sorting = false;

    let mut changes = Vec::new();
    gix::diff::index(
        lhs,
        rhs,
        |change| -> Result<_, std::convert::Infallible> {
            changes.push(change.into_owned());
            Ok(gix::diff::index::Action::Continue)
        },
        None::<gix::diff::index::RewriteOptions<'_, gix::Repository>>,
        &mut gix::pathspec::Search::from_specs(None, None, workdir)?,
        &mut |_, _, _, _| unreachable!("no pathspec is used"),
    )?;

    use gix::diff::index::Change;
    for change in changes {
        match change {
            Change::Addition { location, .. } => {
                delete_entry_by_path_bounded(rhs, location.as_bstr(), &mut num_sorted_entries);
            }
            Change::Deletion {
                location,
                entry_mode,
                id,
                ..
            }
            | Change::Modification {
                location,
                previous_entry_mode: entry_mode,
                previous_id: id,
                ..
            } => {
                // We cannot be sure that the index is representing the worktree, so Git has to be forced to recalculate the hashes.
                let assume_index_does_not_match_worktree = None;
                needs_sorting |= upsert_index_entry(
                    rhs,
                    location.as_bstr(),
                    assume_index_does_not_match_worktree,
                    id.into_owned(),
                    entry_mode,
                    gix::index::entry::Flags::empty(),
                    &mut num_sorted_entries,
                )?;
            }
            Change::Rewrite { .. } => {
                unreachable!("rewrites tracking was disabled")
            }
        }
    }

    if needs_sorting {
        rhs.sort_entries();
    }
    rhs.remove_tree();
    rhs.remove_resolve_undo();
    Ok(())
}

fn delete_entry_by_path_bounded(
    index: &mut gix::index::State,
    rela_path: &BStr,
    num_sorted_entries: &mut usize,
) {
    use gix::index::entry::Stage;
    delete_entry_by_path_bounded_stages(
        index,
        rela_path,
        num_sorted_entries,
        &[Stage::Unconflicted, Stage::Base, Stage::Ours, Stage::Theirs],
    );
}
