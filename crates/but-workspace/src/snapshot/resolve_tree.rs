/// The information extracted from [`resolve_tree`](function::resolve_tree()).
pub struct Outcome<'repo> {
    /// The cherry-pick result as merge between the target worktree and the snapshot, **possibly with conflicts**.
    ///
    /// This tree, may be checked out to the working tree, with or without conflicts - it's entirely left to the caller.
    /// It's `None` if the was no worktree change.
    pub worktree_cherry_pick: Option<gix::merge::tree::Outcome<'repo>>,
    /// If an index was stored in the snapshot, this is the reconstructed index, including conflicts.
    ///
    /// It's `None` if there were no index-only changes.
    pub index: Option<gix::index::State>,
    /// Reference edits that when applied in a transaction will set the workspace back to where it was. Only available
    /// if it was part of the snapshot to begin with.
    pub workspace_references: Option<Vec<gix::refs::transaction::RefEdit>>,
    /// The metadata to be applied to the ref-metadata store.
    pub metadata: Option<MetadataEdits>,
}

/// Edits for application via [`but_core::RefMetadata`].
pub struct MetadataEdits {
    /// The workspace metadata stored in the snapshot.
    pub workspace: (gix::refs::FullName, but_core::ref_metadata::Workspace),
    /// The branch metadata stored in snapshots.
    pub branches: Vec<(gix::refs::FullName, but_core::ref_metadata::Branch)>,
}

/// Options for use in [super::resolve_tree()].
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// If set, the non-default options to use when cherry-picking the worktree changes onto the target tree.
    ///
    /// If `None`, perform the merge just like Git.
    pub worktree_cherry_pick: Option<gix::merge::tree::Options>,
}

pub(super) mod function {
    use super::{Options, Outcome};
    use anyhow::Context;
    use gitbutler_oxidize::GixRepositoryExt;

    /// Given the `snapshot_tree` as previously returned via [super::create_tree::Outcome::snapshot_tree], extract data and…
    ///
    /// * …cherry-pick the worktree changes onto the `target_worktree_tree_id`, which is assumed to represent the future working directory state
    ///   and which either contains the worktree changes or *preferably* is the `HEAD^{tree}` as the working directory is clean.
    /// * …reconstruct the index to write into `.git/index`, assuming that the current `.git/index` is clean.
    /// * …produce reference edits to put the workspace refs back into place with.
    /// * …produce metadata that if set will represent the metadata of the entire workspace.
    ///
    /// Note that none of this data is actually manifested in the repository or working tree, they only exists as objects in the Git database,
    /// assuming in-memory objects aren't used in the repository.
    pub fn resolve_tree(
        snapshot_tree: gix::Id<'_>,
        target_worktree_tree_id: gix::ObjectId,
        Options {
            worktree_cherry_pick: worktree_cherry_pick_options,
        }: Options,
    ) -> anyhow::Result<Outcome<'_>> {
        let repo = snapshot_tree.repo;
        let snapshot_tree = snapshot_tree.object()?.try_into_tree()?;
        let worktree_cherry_pick =
            if let Some(worktree_base) = snapshot_tree.lookup_entry_by_path("HEAD")? {
                let base = worktree_base.object_id();
                let ours = target_worktree_tree_id;
                let theirs = snapshot_tree
                    .lookup_entry_by_path("worktree")?
                    .with_context(|| {
                        format!(
                            "Snapshot tree {id} needs a 'worktree' entry if it has a 'HEAD' entry",
                            id = snapshot_tree.id
                        )
                    })?
                    .object_id();

                repo.merge_trees(
                    base,
                    ours,
                    theirs,
                    repo.default_merge_labels(),
                    worktree_cherry_pick_options
                        .map(Ok)
                        .unwrap_or_else(|| repo.tree_merge_options())?,
                )?
                .into()
            } else {
                None
            };
        Ok(Outcome {
            worktree_cherry_pick,
            index: None,
            workspace_references: None,
            metadata: None,
        })
    }
}
