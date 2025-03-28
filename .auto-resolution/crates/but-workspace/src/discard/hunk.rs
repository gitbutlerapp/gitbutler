use crate::commit_engine::tree::worktree_file_to_git_in_buf;
use crate::commit_engine::{HunkHeader, apply_hunks};
use anyhow::bail;
use bstr::ByteSlice;
use but_core::{ChangeState, TreeChange, UnifiedDiff};
use gix::filter::plumbing::driver::apply::{Delay, MaybeDelayed};
use gix::filter::plumbing::pipeline::convert::ToWorktreeOutcome;
use gix::prelude::ObjectIdExt;

/// Discard `hunks_to_discard` in the resource at `wt_change`, whose previous version is `previous_state` and is expected to
/// be tracked and readable from the object database. We will always read what's currently on disk as the current version
/// and overwrite it.
///
/// The general idea is to rebuild the file, but without the `hunks_to_discard`, write it into the worktree replacing
/// the previous file. `hunks_to_discard` are hunks based on a diff of what's in Git.
///
/// Note that an index update isn't necessary, as for the purpose of the application it might as well not exist due to the
/// way our worktree-changes function ignores the index entirely.
///
/// Note that `hunks_to_discard` that weren't present in the actual set of worktree changes will remain, so when everything
/// worked `hunks_to_discard` will be empty.
pub fn restore_state_to_worktree(
    wt_change: &TreeChange,
    previous_state: ChangeState,
    hunks_to_discard: &mut Vec<HunkHeader>,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    pipeline: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
    context_lines: u32,
) -> anyhow::Result<()> {
    let repo = pipeline.repo;
    let state_in_worktree = ChangeState {
        id: repo.object_hash().null(),
        kind: previous_state.kind,
    };
    let mut diff_filter = but_core::unified_diff::filter_from_state(
        repo,
        Some(state_in_worktree),
        UnifiedDiff::CONVERSION_MODE,
    )?;
    let UnifiedDiff::Patch { hunks } =
        wt_change.unified_diff_with_filter(repo, context_lines, &mut diff_filter)?
    else {
        bail!("Couldn't obtain diff for worktree changes.")
    };

    let prev_len = hunks_to_discard.len();
    let hunks_to_keep: Vec<HunkHeader> = hunks
        .into_iter()
        .map(Into::into)
        .filter(|hunk| {
            match hunks_to_discard
                .iter()
                .enumerate()
                .find_map(|(idx, hunk_to_discard)| (hunk_to_discard == hunk).then_some(idx))
            {
                None => true,
                Some(idx_to_remove) => {
                    hunks_to_discard.remove(idx_to_remove);
                    false
                }
            }
        })
        .collect();
    if prev_len == hunks_to_discard.len() {
        // We want to keep everything, so nothing has to be done.
        return Ok(());
    }

    let old = previous_state.id.attach(repo).object()?.detach().data;
    let mut new = repo.empty_reusable_buffer();
    let rela_path = wt_change.path.as_bstr();
    let worktree_path = path_check.verified_path_allow_nonexisting(rela_path)?;
    worktree_file_to_git_in_buf(&mut new, rela_path, &worktree_path, pipeline, index)?;

    let base_with_patches = apply_hunks(old.as_bstr(), new.as_bstr(), &hunks_to_keep)?;

    let to_worktree = pipeline.convert_to_worktree(&base_with_patches, rela_path, Delay::Forbid)?;
    match to_worktree {
        ToWorktreeOutcome::Unchanged(buf) => {
            std::fs::write(&worktree_path, buf)?;
        }
        ToWorktreeOutcome::Buffer(buf) => {
            std::fs::write(&worktree_path, buf)?;
        }
        ToWorktreeOutcome::Process(MaybeDelayed::Immediate(mut stream)) => {
            let mut file = std::fs::File::create(&worktree_path)?;
            std::io::copy(&mut stream, &mut file)?;
        }
        ToWorktreeOutcome::Process(MaybeDelayed::Delayed(_)) => unreachable!("disabled"),
    }
    Ok(())
}
