//! Cherry-pick commits into a workspace graph.

use std::collections::HashSet;

use anyhow::bail;
use but_core::{RefMetadata, commit::Headers};
use but_rebase::commit::DateMode;
use but_rebase::graph_rebase::{
    CommitIndex, CommitSpec, Editor, EditorIndex, RebasedEditor, anchor::Anchor, mutate::InsertSide,
};

/// Cherry-pick commits above or below a commit, or below a reference, in the
/// workspace graph.
///
/// Sources are read from the object database, so they may live anywhere in the repository,
/// including on branches that aren't part of the workspace. Duplicates are dropped, keeping the
/// first occurrence, and the rest are applied in the order given rather than reordered, like
/// `git cherry-pick`: the first source lands at `side` of `relative_to`, and each later one
/// directly above the one before it.
/// Child commits, and the target commit, if applicable, are rebased atop the cherry-picked commits.
pub fn cherry_pick_commits<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    source_commits: impl IntoIterator<Item = gix::ObjectId>,
    relative_to: Anchor,
    side: InsertSide,
) -> anyhow::Result<(RebasedEditor<'meta, M>, Vec<CommitIndex>)> {
    let mut seen = HashSet::new();
    let sources = source_commits
        .into_iter()
        .filter(|id| seen.insert(*id))
        .collect::<Vec<_>>();
    if sources.is_empty() {
        bail!("No commits were provided to cherry-pick")
    }
    if matches!(
        (&relative_to, side),
        (Anchor::Reference(_), InsertSide::Above)
    ) {
        bail!("Cannot cherry-pick above a reference")
    }

    let target = editor.resolve_anchor(relative_to)?;

    let mut inserted_handles = Vec::with_capacity(sources.len());
    let mut previous_handle = None;
    for source in sources {
        // Give the copy its own change ID, retaining all other metadata.
        let mut template = editor.find_commit(source)?;
        let mut headers = Headers::try_from_commit(&template.inner).unwrap_or_default();
        headers.change_id = Headers::from_config(&editor.repo().config_snapshot()).change_id;
        headers.set_in_commit(&mut template.inner);
        let template_id = editor.new_commit(template, DateMode::CommitterUpdateAuthorKeep)?;

        let (anchor, insert_side) = match previous_handle {
            Some(handle) => (EditorIndex::from(handle), InsertSide::Above),
            None => (target, side),
        };
        let handle =
            editor.insert_commit(anchor, CommitSpec::untracked(template_id), insert_side)?;
        inserted_handles.push(handle);
        previous_handle = Some(handle);
    }

    Ok((editor.rebase()?, inserted_handles))
}
