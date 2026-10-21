//! Padding commits sit on top of a conflicted commit to make its auto-resolution checkout-able.

use anyhow::bail;
use bstr::{BStr, ByteSlice};

/// The title of every padding commit message.
pub const TITLE: &str = "GitButler Padding Commit";

const BODY: &str = "\
Hi! I'm a stand-in for the conflicted commit below me, so your editor shows its
auto-resolved changes instead of conflict markers.

There's nothing to do with me. GitButler takes me out again once you resolve the
conflict, and anything you commit on top of me moves onto the conflicted commit.";

/// Return `true` if the title of `commit_message` is [`TITLE`].
pub fn is_padding_by_message(commit_message: &BStr) -> bool {
    let message = gix::objs::commit::MessageRef::from_bytes(commit_message);
    message.title.trim().as_bstr() == TITLE
}

/// Write the padding commit of `conflicted` and return its id.
///
/// It has the auto-resolution tree of `conflicted`, `conflicted` as its only parent,
/// and the author, committer and encoding of `conflicted`, so it is the same commit for the same `conflicted` commit.
/// Fails if `conflicted` is not conflicted.
pub fn create(conflicted: &crate::Commit<'_>) -> anyhow::Result<gix::ObjectId> {
    if !conflicted.is_conflicted() {
        bail!("Commit {} is not conflicted", conflicted.id);
    }
    Ok(conflicted
        .id
        .repo
        .write_object(gix::objs::Commit {
            tree: conflicted.tree_id_or_auto_resolution()?.detach(),
            parents: [conflicted.id.detach()].into(),
            author: conflicted.author.clone(),
            committer: conflicted.committer.clone(),
            encoding: conflicted.encoding.clone(),
            message: format!("{TITLE}\n\n{BODY}\n").into(),
            extra_headers: Vec::new(),
        })?
        .detach())
}
