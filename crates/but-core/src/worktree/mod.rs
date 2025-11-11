/// Functions related to workspace checkouts.
pub mod checkout;

use std::{io::Read, path::Path};

use bstr::BStr;
pub use checkout::function::{safe_checkout, safe_checkout_from_head};
use gix::filter::plumbing::pipeline::convert::ToGitOutcome;

/// Read a worktree file into `buf` after converting it to what Git *would* store.
/// Useful if `buf` should be turned into a blob.
/// `md` is used to know how to read the entry, and we assume that it was pre-filtered
/// so we only hit items we can handle.
pub fn worktree_file_to_git_in_buf(
    buf: &mut Vec<u8>,
    md: &gix::index::fs::Metadata,
    rela_path: &BStr,
    path: &Path,
    pipeline: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
) -> anyhow::Result<()> {
    buf.clear();
    if md.is_symlink() {
        buf.extend_from_slice(&gix::path::os_string_into_bstring(
            std::fs::read_link(path)?.into(),
        )?);
    } else {
        let to_git = pipeline.convert_to_git(
            std::fs::File::open(path)?,
            &gix::path::from_bstr(rela_path),
            index,
        )?;
        match to_git {
            ToGitOutcome::Unchanged(mut file) => {
                file.read_to_end(buf)?;
            }
            ToGitOutcome::Process(mut stream) => {
                stream.read_to_end(buf)?;
            }
            ToGitOutcome::Buffer(buf2) => buf.extend_from_slice(buf2),
        };
    }
    Ok(())
}
