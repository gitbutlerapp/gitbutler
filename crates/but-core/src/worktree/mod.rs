pub mod checkout;

use std::{io::Read, path::Path};

use bstr::{BStr, BString};
pub use checkout::function::{
    overwrite_worktree, reset_index, revert_worktree, safe_checkout_from_head,
};
use gix::filter::plumbing::pipeline::convert::ToGitOutcome;

/// Read the target of the symlink at `path` as Git stores it: with `/` separators, even if
/// Windows reports them as `\`.
pub(crate) fn symlink_target_in_git_form(path: &Path) -> anyhow::Result<BString> {
    let target = gix::path::os_string_into_bstring(std::fs::read_link(path)?.into())?;
    Ok(gix::path::to_unix_separators_on_windows(target).into_owned())
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
        buf.extend_from_slice(&symlink_target_in_git_form(path)?);
    } else {
        let to_git = pipeline.convert_to_git(
            std::fs::File::open(path)?,
            &gix::path::from_bstr(rela_path)?,
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

/// Hash the worktree file at `path`, whose metadata is `md`, as the blob Git would store for
/// `rela_path`. A file that no filter changes is hashed as a stream, so its size doesn't matter,
/// and a symlink is hashed as its target.
pub(crate) fn hash_worktree_file_in_git_form(
    md: &gix::index::fs::Metadata,
    rela_path: &BStr,
    path: &Path,
    pipeline: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
) -> anyhow::Result<gix::ObjectId> {
    let (object_hash, blob) = (index.object_hash(), gix::object::Kind::Blob);
    if md.is_symlink() {
        let target = symlink_target_in_git_form(path)?;
        return Ok(gix::objs::compute_hash(object_hash, blob, &target)?);
    }
    let to_git = pipeline.convert_to_git(
        std::fs::File::open(path)?,
        &gix::path::from_bstr(rela_path)?,
        index,
    )?;
    Ok(match to_git {
        ToGitOutcome::Unchanged(mut file) => gix::objs::compute_stream_hash(
            object_hash,
            blob,
            &mut file,
            md.len(),
            &mut gix::progress::Discard,
            &gix::interrupt::IS_INTERRUPTED,
        )?,
        ToGitOutcome::Process(mut stream) => {
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf)?;
            gix::objs::compute_hash(object_hash, blob, &buf)?
        }
        ToGitOutcome::Buffer(buf) => gix::objs::compute_hash(object_hash, blob, buf)?,
    })
}
