use anyhow::Context;
use bstr::{BString, ByteVec};
use std::path::Path;

pub(crate) fn checkout_repo_worktree(
    parent_worktree_dir: &Path,
    mut repo: gix::Repository,
) -> anyhow::Result<()> {
    // No need to cache anything, it's just single-use for the most part.
    repo.object_cache_size(0);
    let mut index = repo.index_from_tree(&repo.head_tree_id_or_empty()?)?;
    if index.entries().is_empty() {
        // The worktree directory is created later, so we don't have to deal with it here.
        return Ok(());
    }
    for entry in index.entries_mut().iter_mut().filter(|e| {
        e.mode
            .contains(gix::index::entry::Mode::DIR | gix::index::entry::Mode::COMMIT)
    }) {
        entry.flags.insert(gix::index::entry::Flags::SKIP_WORKTREE);
    }

    let mut opts =
        repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
    opts.destination_is_initially_empty = true;
    opts.keep_going = true;

    let checkout_destination = repo.workdir().context("non-bare repository")?.to_owned();
    if !checkout_destination.exists() {
        std::fs::create_dir(&checkout_destination)?;
    }
    let sm_repo_dir = gix::path::relativize_with_prefix(
        repo.path().strip_prefix(parent_worktree_dir)?,
        checkout_destination.strip_prefix(parent_worktree_dir)?,
    )
    .into_owned();
    let out = gix::worktree::state::checkout(
        &mut index,
        checkout_destination.clone(),
        repo,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        opts,
    )?;

    let mut buf = BString::from("gitdir: ");
    buf.extend_from_slice(&gix::path::os_string_into_bstring(sm_repo_dir.into())?);
    buf.push_byte(b'\n');
    std::fs::write(checkout_destination.join(".git"), &buf)?;

    tracing::debug!(directory = ?checkout_destination, outcome = ?out, "submodule checkout result");
    Ok(())
}
