/// The information extracted from [`resolve_tree`](function::resolve_tree()).
#[must_use]
pub struct Outcome<'repo> {
    /// The cherry-pick result as merge between the target worktree and the snapshot, **possibly with conflicts**.
    ///
    /// This tree, may be checked out to the working tree, with or without conflicts - it's entirely left to the caller.
    /// It's `None` if the was no worktree change.
    pub worktree_cherry_pick: Option<gix::merge::tree::Outcome<'repo>>,
    /// If an index was stored in the snapshot, this is the reconstructed index, including conflicts.
    /// Note that it has no information from disk whatsoever and should not be written like that.
    ///
    /// It's `None` if there were no index-only changes.
    pub index: Option<gix::index::State>,
    /// Reference edits that when applied in a transaction will set the workspace back to where it was. Only available
    /// if it was part of the snapshot to begin with.
    pub workspace_references: Option<Vec<gix::refs::transaction::RefEdit>>,
    /// The metadata to be applied to the ref-metadata store.
    pub metadata: Option<MetadataEdits>,
}

/// Edits for application to the [reference metadata store](crate::RefMetadata).
pub struct MetadataEdits {
    /// The workspace metadata stored in the snapshot.
    pub workspace: (gix::refs::FullName, crate::ref_metadata::Workspace),
    /// The branch metadata stored in snapshots.
    pub branches: Vec<(gix::refs::FullName, crate::ref_metadata::Branch)>,
}

/// Options for use in [super::resolve_tree()].
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// If set, the non-default options to use when cherry-picking the worktree changes onto the target tree.
    ///
    /// If `None`, perform the merge just like Git, which will include conflict markers!
    pub worktree_cherry_pick: Option<gix::merge::tree::Options>,
}

pub(super) mod function {
    use std::collections::BTreeSet;

    use crate::RepositoryExt;
    use anyhow::{Context as _, bail};
    use bstr::ByteSlice;
    use gix::index::entry::{Flags, Stage};

    use super::{Options, Outcome};

    /// Given the `snapshot_tree` as previously returned via [`crate::snapshot::create_tree::Outcome::snapshot_tree`], extract data and…
    ///
    /// * …cherry-pick the worktree changes onto the `target_worktree_tree_id`, which is assumed to represent the future working directory state
    ///   and which either contains the worktree changes or *preferably* is the `HEAD^{tree}` as the working directory is clean.
    /// * …reconstruct the index to write into `.git/index` (including conflicts), assuming that the current `.git/index` is clean, *or* is the index from which the snapshot was taken.
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
        let head_tree = snapshot_tree.lookup_entry_by_path("HEAD")?;
        let worktree = snapshot_tree.lookup_entry_by_path("worktree")?;
        let index = snapshot_tree.lookup_entry_by_path("index")?;
        let index_conflicts = snapshot_tree.lookup_entry_by_path("index-conflicts")?;

        let worktree_cherry_pick = match (&head_tree, worktree) {
            (Some(worktree_base), Some(worktree)) => {
                let base = worktree_base.object_id();
                let ours = target_worktree_tree_id;
                let theirs = worktree.object_id();

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
            }
            (None, Some(_worktree)) => {
                bail!(
                    "Snapshot tree {id} needs a 'HEAD' entry if it has a 'worktree' entry",
                    id = snapshot_tree.id
                )
            }
            (None, None) | (Some(_), None) => None,
        };

        let index = match (&head_tree, index, index_conflicts) {
            (_, Some(index_tree), Some(index_conflicts)) => {
                let (mut index, _path) = repo.index_from_tree(&index_tree.id())?.into_parts();
                replace_entries_with_their_restored_conflicts(&mut index, index_conflicts.id())?;
                Some(index)
            }
            (_, Some(index_tree), None) => {
                let (index, _path) = repo.index_from_tree(&index_tree.id())?.into_parts();
                Some(index)
            }
            (Some(worktree_base), None, Some(index_conflicts)) => {
                let (mut index, _path) = repo.index_from_tree(&worktree_base.id())?.into_parts();
                replace_entries_with_their_restored_conflicts(&mut index, index_conflicts.id())?;
                Some(index)
            }
            (None, None, Some(_index_conflicts)) => bail!(
                "Snapshot tree {id} needs a 'HEAD' entry if it has only a 'index-conflicts' entry",
                id = snapshot_tree.id
            ),
            (_, None, None) => None,
        };

        Ok(Outcome {
            worktree_cherry_pick,
            index,
            workspace_references: None,
            metadata: None,
        })
    }

    #[expect(clippy::indexing_slicing)]
    fn replace_entries_with_their_restored_conflicts(
        index: &mut gix::index::State,
        conflict_tree: gix::Id,
    ) -> anyhow::Result<()> {
        let conflict_tree = conflict_tree.object()?.try_into_tree()?;
        // Since we don't expect a lot of entries, quickly record everything.
        let mut recorder = gix::traverse::tree::Recorder::default();
        conflict_tree.traverse().depthfirst(&mut recorder)?;

        let mut to_remove = BTreeSet::new();
        for record in &recorder.records {
            if record.mode.is_tree() {
                continue;
            }
            let rela_path = &record.filepath;
            let rslash_pos = rela_path
                .rfind("/")
                .context("BUG: expecting <path>/<stage>")?;
            let stage: usize = rela_path[rslash_pos + 1..]
                .to_str()?
                .parse()
                .context("Failed to parse stage that should only be [1,2,3]")?;
            let rela_path = rela_path[..rslash_pos].as_bstr();
            let stage = match stage {
                0 => bail!("Unconflicted stage for '{rela_path}' is unexpected"),
                1 => Stage::Base,
                2 => Stage::Ours,
                3 => Stage::Theirs,
                other => bail!("Invalid stage '{other}' for '{rela_path}'"),
            };

            index.dangerously_push_entry(
                Default::default(),
                record.oid,
                Flags::from_stage(stage),
                record.mode.into(),
                rela_path,
            );

            to_remove.insert(rela_path);
        }
        index.remove_entries(|_idx, path, entry| {
            entry.flags.stage() == Stage::Unconflicted && to_remove.contains(path)
        });

        index.sort_entries();
        Ok(())
    }
}
