use bstr::{BStr, ByteSlice};
use std::path::Path;

/// Turn `rhs` into `lhs` by modifying `rhs`. This will leave `rhs` intact as much as possible, but will remove
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
                let md = gix::index::fs::Metadata::from_path_no_follow(
                    &workdir.join(gix::path::from_bstr(location.as_bstr())),
                )?;
                needs_sorting |= upsert_index_entry(
                    rhs,
                    location.as_bstr(),
                    &md,
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

// TODO(gix): this could be a platform in Gix which supports these kinds of edits while assuring
//       consistency. It could use some tricks to not have worst-case performance like this has.
//       It really is index-add that we need.
pub fn upsert_index_entry(
    index: &mut gix::index::State,
    rela_path: &BStr,
    md: &gix::index::fs::Metadata,
    id: gix::ObjectId,
    mode: gix::index::entry::Mode,
    add_flags: gix::index::entry::Flags,
    num_sorted_entries: &mut usize,
) -> anyhow::Result<bool> {
    use gix::index::entry::Stage;
    delete_entry_by_path_bounded_stages(
        index,
        rela_path,
        num_sorted_entries,
        &[Stage::Base, Stage::Ours, Stage::Theirs],
    );

    let needs_sort = if let Some(pos) = index.entry_index_by_path_and_stage_bounded(
        rela_path,
        Stage::Unconflicted,
        *num_sorted_entries,
    ) {
        #[allow(clippy::indexing_slicing)]
        let entry = &mut index.entries_mut()[pos];
        // NOTE: it's needed to set the values to 0 here or else 1 in 40 times or so
        //       git status will report the file didn't change even though it did.
        //       This basically forces it to look closely, bad for performance, but at
        //       least correct. Usually it fixes itself as well.
        entry.stat = Default::default();
        entry.flags |= add_flags;
        entry.id = id;
        entry.mode = mode;
        false
    } else {
        index.dangerously_push_entry(
            gix::index::entry::Stat::from_fs(md)?,
            id,
            add_flags,
            mode,
            rela_path,
        );
        true
    };
    Ok(needs_sort)
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

// TODO(gix)
// TODO(performance): make an efficient version of this available in `gix`,
//                    right now we need 4 lookups for each deletion, and possibly 4 rewrites of the vec
fn delete_entry_by_path_bounded_stages(
    index: &mut gix::index::State,
    rela_path: &BStr,
    num_sorted_entries: &mut usize,
    stages: &[gix::index::entry::Stage],
) {
    for stage in stages {
        if let Some(pos) =
            index.entry_index_by_path_and_stage_bounded(rela_path, *stage, *num_sorted_entries)
        {
            index.remove_entry_at_index(pos);
            *num_sorted_entries -= 1;
        }
    }
}
