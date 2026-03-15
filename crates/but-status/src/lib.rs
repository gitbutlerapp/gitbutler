/// Gets the status of a given repository.
pub fn get_status(repo: &gix::Repository) -> anyhow::Result<Vec<gix::status::Item>> {
    use gix::{dir::walk::EmissionMode, status::tree_index::TrackRenames};

    let status_changes = repo
        .status(gix::progress::Discard)?
        .tree_index_track_renames(TrackRenames::Disabled)
        .index_worktree_rewrites(None)
        .index_worktree_submodules(gix::status::Submodule::Given {
            ignore: gix::submodule::config::Ignore::Dirty,
            check_dirty: true,
        })
        .index_worktree_options_mut(|opts| {
            if let Some(opts) = opts.dirwalk_options.as_mut() {
                opts.set_emit_ignored(None)
                    .set_emit_pruned(false)
                    .set_emit_tracked(false)
                    .set_emit_untracked(EmissionMode::Matching)
                    .set_emit_collapsed(None);
            }
        })
        .into_iter(None)?
        .filter_map(|change| change.ok())
        .collect::<Vec<_>>();

    Ok(status_changes)
}
