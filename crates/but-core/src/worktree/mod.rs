/// Functions related to workspace checkouts.
pub mod checkout;

use std::{io::Read, path::Path};

use bstr::{BStr, BString};
pub use checkout::function::safe_checkout_from_head;
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

/// A linked worktree of a repository that can be used, i.e. it is checked out and its
/// `HEAD` resolves.
#[derive(Debug, Clone)]
pub struct Linked {
    /// The worktree checkout directory.
    pub path: std::path::PathBuf,
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`,
    /// which survives `git worktree move`.
    pub name: BString,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    pub head: gix::ObjectId,
}

/// Enumerate the linked worktrees of `repo`, returning the names of ALL of them
/// (so callers tracking per-worktree state can see one that is unusable today but may
/// become usable later) along with the usable ones.
///
/// Worktrees that are broken (pruned checkout, unresolvable or unborn `HEAD`) and
/// worktrees checked out on the workspace ref are not usable and thus not returned.
///
/// `repo` must be the main worktree, so none of the linked worktrees enumerated
/// here can be the repository's own.
pub fn linked(repo: &gix::Repository) -> anyhow::Result<(Vec<BString>, Vec<Linked>)> {
    let mut all_names = Vec::new();
    let mut out = Vec::new();
    for proxy in repo.worktrees()? {
        let name: BString = proxy.id().to_owned();
        all_names.push(name.clone());
        let path = match proxy.base() {
            Ok(path) => path,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // Missing administrative data - the worktree is prunable.
                continue;
            }
            Err(err) => {
                tracing::warn!(%name, ?err, "Skipping linked worktree whose checkout location cannot be read");
                continue;
            }
        };
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                // The `gitdir` file points at something that is not a directory - prunable.
                continue;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // The checkout was deleted without `git worktree remove` - prunable.
                continue;
            }
            Err(err) => {
                tracing::warn!(%name, ?err, "Skipping linked worktree whose checkout cannot be inspected");
                continue;
            }
        }
        let wt_repo = match proxy.into_repo_with_possibly_inaccessible_worktree() {
            Ok(wt_repo) => wt_repo,
            Err(err) => {
                // Unlike the prunable states above, this is never expected.
                tracing::warn!(%name, ?err, "Skipping linked worktree whose repository cannot be opened");
                continue;
            }
        };
        let mut head = match wt_repo.head() {
            Ok(head) => head,
            Err(err) => {
                tracing::warn!(%name, ?err, "Skipping linked worktree with an unreadable HEAD");
                continue;
            }
        };
        let ref_name = head.referent_name().map(ToOwned::to_owned);
        if ref_name
            .as_ref()
            .is_some_and(|name| crate::is_workspace_ref_name(name.as_ref()))
        {
            // The workspace ref is fully managed by GitButler already.
            continue;
        }
        let commit = match head.peel_to_commit() {
            Ok(commit) => commit,
            Err(gix::head::peel::to_commit::Error::PeelToObject(
                gix::head::peel::to_object::Error::Unborn { .. },
            )) => {
                // A worktree on an unborn branch has nothing to list yet.
                continue;
            }
            Err(err) => {
                tracing::warn!(%name, ?err, "Skipping linked worktree whose HEAD cannot be peeled to a commit");
                continue;
            }
        };
        out.push(Linked {
            path,
            name,
            ref_name,
            head: commit.id,
        });
    }
    Ok((all_names, out))
}
