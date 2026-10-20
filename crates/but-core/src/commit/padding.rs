//! Padding commits sit on top of a conflicted commit to make its auto-resolution checkout-able.

use anyhow::bail;

/// The title of every padding commit message.
pub const TITLE: &str = "GitButler Padding Commit";

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
            message: format!("{TITLE}\n").into(),
            extra_headers: Vec::new(),
        })?
        .detach())
}
