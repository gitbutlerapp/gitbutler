use anyhow::{Context as _, Result};
use gix::merge::tree::{Options, TreatAsUnresolved};

use crate::git2_to_gix_object_id;

pub trait GixRepositoryExt: Sized {
    /// Configure the repository for diff operations between trees.
    /// This means it needs an object cache relative to the amount of files in the repository.
    // TODO(st): make it non-consuming
    fn for_tree_diffing(self) -> Result<Self>;

    /// Returns `true` if the merge between `our_tree` and `their_tree` is free of conflicts.
    /// Conflicts entail content merges with conflict markers, or anything else that doesn't merge cleanly in the tree.
    ///
    /// # Important
    ///
    /// Make sure the repository is configured [`with_object_memory()`](gix::Repository::with_object_memory()).
    fn merges_cleanly_compat(
        &self,
        ancestor_tree: git2::Oid,
        our_tree: git2::Oid,
        their_tree: git2::Oid,
    ) -> Result<bool>;

    /// Just like the above, but with `gix` types.
    fn merges_cleanly(
        &self,
        ancestor_tree: gix::ObjectId,
        our_tree: gix::ObjectId,
        their_tree: gix::ObjectId,
    ) -> Result<bool>;

    /// Return default label names when merging trees.
    ///
    /// Note that these should probably rather be branch names, but that's for another day.
    fn default_merge_labels(&self) -> gix::merge::blob::builtin_driver::text::Labels<'static> {
        gix::merge::blob::builtin_driver::text::Labels {
            ancestor: Some("base".into()),
            current: Some("ours".into()),
            other: Some("theirs".into()),
        }
    }

    /// Tree merge options that enforce undecidable conflicts to be forcefully resolved
    /// to favor ours, both when dealing with content merges and with tree merges.
    fn merge_options_force_ours(&self) -> Result<gix::merge::tree::Options>;

    /// Return options suitable for merging so that the merge stops immediately after the first conflict.
    /// It also returns the conflict kind to use when checking for unresolved conflicts.
    fn merge_options_fail_fast(
        &self,
    ) -> Result<(
        gix::merge::tree::Options,
        gix::merge::tree::TreatAsUnresolved,
    )>;

    /// Just like [`Self::merge_options_fail_fast()`], but additionally don't perform rename tracking.
    /// This is useful if the merge result isn't going to be used, and we are only interested in knowing
    /// if a merge would succeed.
    fn merge_options_no_rewrites_fail_fast(
        &self,
    ) -> Result<(gix::merge::tree::Options, TreatAsUnresolved)>;
}

impl GixRepositoryExt for gix::Repository {
    fn for_tree_diffing(mut self) -> anyhow::Result<Self> {
        let bytes = self.compute_object_cache_size_for_tree_diffs(&***self.index_or_empty()?);
        self.object_cache_size_if_unset(bytes);
        Ok(self)
    }

    fn merges_cleanly_compat(
        &self,
        ancestor_tree: git2::Oid,
        our_tree: git2::Oid,
        their_tree: git2::Oid,
    ) -> Result<bool> {
        self.merges_cleanly(
            git2_to_gix_object_id(ancestor_tree),
            git2_to_gix_object_id(our_tree),
            git2_to_gix_object_id(their_tree),
        )
    }

    fn merges_cleanly(
        &self,
        ancestor_tree: gix::ObjectId,
        our_tree: gix::ObjectId,
        their_tree: gix::ObjectId,
    ) -> Result<bool> {
        let (options, conflict_kind) = self.merge_options_no_rewrites_fail_fast()?;
        let merge_outcome = self
            .merge_trees(
                ancestor_tree,
                our_tree,
                their_tree,
                Default::default(),
                options,
            )
            .context("failed to merge trees")?;
        Ok(!merge_outcome.has_unresolved_conflicts(conflict_kind))
    }

    fn merge_options_force_ours(&self) -> Result<Options> {
        Ok(self
            .tree_merge_options()?
            .with_tree_favor(Some(gix::merge::tree::TreeFavor::Ours))
            .with_file_favor(Some(gix::merge::tree::FileFavor::Ours)))
    }

    fn merge_options_fail_fast(&self) -> Result<(gix::merge::tree::Options, TreatAsUnresolved)> {
        let conflict_kind = TreatAsUnresolved::forced_resolution();
        let options = self
            .tree_merge_options()?
            .with_fail_on_conflict(Some(conflict_kind));
        Ok((options, conflict_kind))
    }

    fn merge_options_no_rewrites_fail_fast(
        &self,
    ) -> Result<(gix::merge::tree::Options, TreatAsUnresolved)> {
        let (options, conflict_kind) = self.merge_options_fail_fast()?;
        Ok((options.with_rewrites(None), conflict_kind))
    }
}
