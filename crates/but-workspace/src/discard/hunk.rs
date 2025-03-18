use crate::commit_engine::HunkHeader;
use bstr::BStr;
use but_core::TreeChange;

/// Discard `hunks_to_discard` in the resource at `wt_change`, whose previous version is
///
/// The general idea is to rebuild the file, but without the `hunks_to_discard`, write it into the worktree replacing
/// the previous file.
///
/// Note that an index update isn't necessary, as for the purpose of the application it might as well not exist.
pub fn restore_state_to_worktree(
    _wt_change: &TreeChange,
    _pipeline: &mut gix::filter::Pipeline<'_>,
    _rela_path: &BStr,
    _hunks_to_discard: &[HunkHeader],
    _path_check: &mut gix::status::plumbing::SymlinkCheck,
) -> anyhow::Result<()> {
    todo!()
}
