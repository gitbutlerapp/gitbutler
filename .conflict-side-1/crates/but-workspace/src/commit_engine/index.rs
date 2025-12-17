use bstr::BStr;

// TODO(gix): this could be a platform in Gix which supports these kinds of edits while assuring
//       consistency. It could use some tricks to not have worst-case performance like this has.
//       It really is index-add that we need.
// If `md` is `None`, we will write a null-stat that will trigger Git to recheck the content each time, i.e. the index isn't considered fresh.
// Otherwise, if `md` is `Some()`, Git will always be made to think that the worktree file matches the state in the index, so that better be the case.
pub fn upsert_index_entry(
    index: &mut gix::index::State,
    rela_path: &BStr,
    md: Option<&gix::index::fs::Metadata>,
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
        #[expect(clippy::indexing_slicing)]
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
            md.map(gix::index::entry::Stat::from_fs)
                .unwrap_or_else(|| Ok(Default::default()))?,
            id,
            add_flags,
            mode,
            rela_path,
        );
        true
    };
    Ok(needs_sort)
}

// TODO(gix)
// TODO(performance): make an efficient version of this available in `gix`,
//                    right now we need 4 lookups for each deletion, and possibly 4 rewrites of the vec
pub(crate) fn delete_entry_by_path_bounded_stages(
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
