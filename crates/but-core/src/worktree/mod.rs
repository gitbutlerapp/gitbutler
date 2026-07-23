/// Functions related to workspace checkouts.
pub mod checkout;

use std::{io::Read, path::Path};

use bstr::BStr;
pub use checkout::function::safe_checkout_from_head;
use gix::filter::plumbing::pipeline::convert::ToGitOutcome;

/// `true` if `content` contains a complete set of Git conflict markers: a `<<<<<<<` line,
/// later followed by a `=======` line, later followed by a `>>>>>>>` line.
///
/// Content containing NUL bytes is considered binary and never reported as marked.
pub fn contains_conflict_markers(content: &[u8]) -> bool {
    fn is_marker_line(line: &[u8], marker: &[u8; 7]) -> bool {
        line.strip_prefix(marker)
            .is_some_and(|rest| rest.is_empty() || matches!(rest[0], b' ' | b'\r'))
    }
    if content.contains(&0) {
        return false;
    }
    let mut after_ours = false;
    let mut after_separator = false;
    for line in content.split(|byte| *byte == b'\n') {
        if is_marker_line(line, b"<<<<<<<") {
            after_ours = true;
            after_separator = false;
        } else if after_ours && is_marker_line(line, b"=======") {
            after_separator = true;
        } else if after_separator && is_marker_line(line, b">>>>>>>") {
            return true;
        }
    }
    false
}

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
