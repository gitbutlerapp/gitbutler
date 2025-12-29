/// How to perform a cherry-pick.
#[derive(Debug, Copy, Clone)]
pub enum PickMode {
    /// No matter what, rebase one commit onto the other, creating a new commit in the process.
    ///
    /// This is useful if the list of commits to rebase is known to actually need a rebase.
    Unconditionally,
    /// Do not actually do anything if the commit to pick is already on the desired parent.
    /// This useful if the list of `commits_to_rebase` includes commits that don't need a change.
    // Note: this is more for older code which provides more commits than would be needed for
    // an operation, for if the UI lists everything because it makes the code easier.
    SkipIfNoop,
}

/// How to deal with commits that are empty after cherry-picking.
#[derive(Debug, Copy, Clone)]
pub enum EmptyCommit {
    /// Keep the empty commit.
    Keep,
    /// Instead of the empty commit, keep only the previous one, effectively
    /// dropping the commit whose tree didn't differ compared to the previous one.
    UsePrevious,
}

pub(crate) mod function {
    use std::{collections::HashSet, path::PathBuf};

    use anyhow::{Context as _, bail};
    use bstr::BString;
    use but_core::commit::{HEADERS_CONFLICTED_FIELD, HeadersV2, TreeKind};
    use gix::{object::tree::EntryKind, prelude::ObjectIdExt};
    use serde::Serialize;

    use crate::{
        cherry_pick::{EmptyCommit, PickMode},
        commit::DateMode,
    };

    /// Place `commit_to_rebase` onto `base`.
    ///
    /// `pick_mode` and `empty_commit` control how to deal with no-ops and empty commits.
    /// Returns the id of the cherry-picked commit.
    ///
    /// Note that the rewritten commit will have headers injected, among which is a change id.
    pub fn cherry_pick_one(
        repo: &gix::Repository,
        base: gix::ObjectId,
        commit_to_rebase: gix::ObjectId,
        pick_mode: PickMode,
        empty_commit: EmptyCommit,
    ) -> anyhow::Result<gix::ObjectId> {
        let base = but_core::Commit::from_id(base.attach(repo))?;
        let to_rebase = but_core::Commit::from_id(commit_to_rebase.attach(repo))?;
        Ok(cherry_pick_one_inner(base, to_rebase, pick_mode, empty_commit)?.detach())
    }

    fn cherry_pick_one_inner<'repo>(
        base: but_core::Commit<'repo>,
        commit_to_rebase: but_core::Commit<'repo>,
        pick_mode: PickMode,
        empty_commit: EmptyCommit,
    ) -> anyhow::Result<gix::Id<'repo>> {
        if commit_to_rebase.parents.len() > 1 {
            bail!("Cannot yet cherry-pick merge-commits - use rebasing for that")
        }
        if matches!(pick_mode, PickMode::SkipIfNoop)
            && commit_to_rebase.parents.contains(&base.id.detach())
        {
            return Ok(commit_to_rebase.id);
        };

        let mut cherry_pick = cherry_pick_tree(&base, &commit_to_rebase)?;
        let tree_id = cherry_pick.tree.write()?;

        let conflict_kind = gix::merge::tree::TreatAsUnresolved::forced_resolution();
        if cherry_pick.has_unresolved_conflicts(conflict_kind) {
            commit_from_conflicted_tree(base, commit_to_rebase, tree_id, cherry_pick, conflict_kind)
        } else {
            commit_from_unconflicted_tree(base, commit_to_rebase, tree_id, empty_commit)
        }
    }

    fn set_parent(
        to_rebase: &mut gix::objs::Commit,
        new_parent: gix::ObjectId,
    ) -> anyhow::Result<()> {
        if to_rebase.parents.len() > 1 {
            bail!(
                "Cherry picks can only be done for single-parent commits. Merge-commits need to be re-merged"
            )
        }
        to_rebase.parents.clear();
        to_rebase.parents.push(new_parent);
        Ok(())
    }

    /// Rebase `to_rebase` onto `new_base`, dealing with the intricacies of conflicted trees, and return the newly
    /// merged tree.
    /// Note that all merges are made to succeed, possibly recording the original trees in a special tree.
    fn cherry_pick_tree<'repo>(
        new_base: &but_core::Commit<'repo>,
        to_rebase: &but_core::Commit<'repo>,
    ) -> anyhow::Result<gix::merge::tree::Outcome<'repo>> {
        let repo = to_rebase.id.repo;
        let (base, ours, theirs) = find_cherry_pick_trees(new_base, to_rebase)?;
        use but_core::RepositoryExt;
        repo.merge_trees(
            base,
            ours,
            theirs,
            repo.default_merge_labels(),
            repo.merge_options_force_ours()?,
        )
        .context("failed to merge trees for cherry pick")
    }

    /// Return `(base, ours, theirs)` suitable for cherry-pick merges from the `new_base` for `to_rebase`.
    fn find_cherry_pick_trees<'repo>(
        new_base: &but_core::Commit<'repo>,
        to_rebase: &but_core::Commit<'repo>,
    ) -> anyhow::Result<(gix::Id<'repo>, gix::Id<'repo>, gix::Id<'repo>)> {
        let repo = to_rebase.id.repo;
        // we need to do a manual 3-way patch merge
        // find the base, which is the parent of to_rebase
        let base = if to_rebase.is_conflicted() {
            // Use to_rebase's recorded base
            find_real_tree(to_rebase, TreeKind::Base)?
        } else {
            let base_commit_id = to_rebase.parents.first().context("no parent")?;
            // Use the parent's auto-resolution
            let base_commit = but_core::Commit::from_id(base_commit_id.attach(repo))?;
            find_real_tree(&base_commit, TreeKind::AutoResolution)?
        };
        // Get the auto-resolution
        let ours = find_real_tree(new_base, TreeKind::AutoResolution)?;
        // Get the original theirs
        let theirs = find_real_tree(to_rebase, TreeKind::Theirs)?;
        Ok((base, ours, theirs))
    }

    fn find_real_tree<'repo>(
        commit: &but_core::Commit<'repo>,
        side: TreeKind,
    ) -> anyhow::Result<gix::Id<'repo>> {
        Ok(if commit.is_conflicted() {
            let tree = commit.id.repo.find_tree(commit.tree)?;
            let conflicted_side = tree
                .find_entry(side.as_tree_entry_name())
                .context("Failed to get conflicted side of commit")?;
            conflicted_side.id()
        } else {
            commit.tree_id_or_auto_resolution()?
        })
    }

    fn commit_from_unconflicted_tree<'repo>(
        head: but_core::Commit<'repo>,
        to_rebase: but_core::Commit<'repo>,
        resolved_tree_id: gix::Id<'repo>,
        empty_commit: EmptyCommit,
    ) -> anyhow::Result<gix::Id<'repo>> {
        let repo = head.id.repo;
        // Remove empty commits
        if matches!(empty_commit, EmptyCommit::UsePrevious)
            && resolved_tree_id == head.tree_id_or_auto_resolution()?
        {
            return Ok(head.id);
        }

        let headers = to_rebase.headers();
        let to_rebase_is_conflicted = headers.as_ref().is_some_and(|hdr| hdr.is_conflicted());
        let mut new_commit = to_rebase.inner;
        new_commit.tree = resolved_tree_id.detach();

        // Assure the commit isn't thinking it's conflicted.
        if to_rebase_is_conflicted {
            if let Some(pos) = new_commit
                .extra_headers()
                .find_pos(HEADERS_CONFLICTED_FIELD)
            {
                new_commit.extra_headers.remove(pos);
            }
        } else if headers.is_none() {
            new_commit
                .extra_headers
                .extend(Vec::<(BString, BString)>::from(&HeadersV2::from_config(
                    &repo.config_snapshot(),
                )));
        }
        set_parent(&mut new_commit, head.id.detach())?;
        Ok(
            crate::commit::create(repo, new_commit, DateMode::CommitterUpdateAuthorKeep)?
                .attach(repo),
        )
    }

    fn commit_from_conflicted_tree<'repo>(
        head: but_core::Commit<'repo>,
        mut to_rebase: but_core::Commit<'repo>,
        resolved_tree_id: gix::Id<'repo>,
        cherry_pick: gix::merge::tree::Outcome<'_>,
        treat_as_unresolved: gix::merge::tree::TreatAsUnresolved,
    ) -> anyhow::Result<gix::Id<'repo>> {
        let repo = resolved_tree_id.repo;
        // in case someone checks this out with vanilla Git, we should warn why it looks like this
        let readme_content =
            b"You have checked out a GitButler Conflicted commit. You probably didn't mean to do this.";
        let readme_blob = repo.write_blob(readme_content)?;

        let conflicted_files =
            extract_conflicted_files(resolved_tree_id, cherry_pick, treat_as_unresolved)?;

        // convert files into a string and save as a blob
        let conflicted_files_string = toml::to_string(&conflicted_files)?;
        let conflicted_files_blob = repo.write_blob(conflicted_files_string.as_bytes())?;

        let mut tree = repo.empty_tree().edit()?;

        // save the state of the conflict, so we can recreate it later
        let (base_tree_id, ours_tree_id, theirs_tree_id) =
            find_cherry_pick_trees(&head, &to_rebase)?;
        tree.upsert(
            TreeKind::Ours.as_tree_entry_name(),
            EntryKind::Tree,
            ours_tree_id,
        )?;
        tree.upsert(
            TreeKind::Theirs.as_tree_entry_name(),
            EntryKind::Tree,
            theirs_tree_id,
        )?;
        tree.upsert(
            TreeKind::Base.as_tree_entry_name(),
            EntryKind::Tree,
            base_tree_id,
        )?;
        tree.upsert(
            TreeKind::AutoResolution.as_tree_entry_name(),
            EntryKind::Tree,
            resolved_tree_id,
        )?;
        tree.upsert(".conflict-files", EntryKind::Blob, conflicted_files_blob)?;
        tree.upsert("README.txt", EntryKind::Blob, readme_blob)?;

        let mut headers = to_rebase
            .headers()
            .unwrap_or_else(|| HeadersV2::from_config(&repo.config_snapshot()));
        headers.conflicted = conflicted_files.conflicted_header_field();
        to_rebase.tree = tree.write().context("failed to write tree")?.detach();
        set_parent(&mut to_rebase, head.id.detach())?;

        to_rebase.set_headers(&headers);
        Ok(
            crate::commit::create(repo, to_rebase.inner, DateMode::CommitterUpdateAuthorKeep)?
                .attach(repo),
        )
    }

    fn extract_conflicted_files(
        merged_tree_id: gix::Id<'_>,
        merge_result: gix::merge::tree::Outcome<'_>,
        treat_as_unresolved: gix::merge::tree::TreatAsUnresolved,
    ) -> anyhow::Result<ConflictEntries> {
        use gix::index::entry::Stage;
        let repo = merged_tree_id.repo;
        let mut index = repo.index_from_tree(&merged_tree_id)?;
        merge_result.index_changed_after_applying_conflicts(
            &mut index,
            treat_as_unresolved,
            gix::merge::tree::apply_index_entries::RemovalMode::Mark,
        );
        let (mut ancestor_entries, mut our_entries, mut their_entries) =
            (Vec::new(), Vec::new(), Vec::new());
        for entry in index.entries() {
            let stage = entry.stage();
            let storage = match stage {
                Stage::Unconflicted => {
                    continue;
                }
                Stage::Base => &mut ancestor_entries,
                Stage::Ours => &mut our_entries,
                Stage::Theirs => &mut their_entries,
            };

            let path = entry.path(&index);
            storage.push(gix::path::from_bstr(path).into_owned());
        }
        let mut out = ConflictEntries {
            ancestor_entries,
            our_entries,
            their_entries,
        };

        // Since we typically auto-resolve with 'ours', it maybe that conflicting entries don't have an
        // unconflicting counterpart anymore, so they are not applied (which is also what Git does).
        // So to have something to show for - we *must* produce a conflict, extract paths manually.
        // TODO(ST): instead of doing this, don't pre-record the paths. Instead redo the merge without
        //           merge-strategy so that the index entries can be used instead.
        if !out.has_entries() {
            fn push_unique(v: &mut Vec<PathBuf>, change: &gix::diff::tree_with_rewrites::Change) {
                let path = gix::path::from_bstr(change.location()).into_owned();
                if !v.contains(&path) {
                    v.push(path);
                }
            }
            for conflict in merge_result
                .conflicts
                .iter()
                .filter(|c| c.is_unresolved(treat_as_unresolved))
            {
                let (ours, theirs) = conflict.changes_in_resolution();
                push_unique(&mut out.our_entries, ours);
                push_unique(&mut out.their_entries, theirs);
            }
        }
        assert_eq!(
            out.has_entries(),
            merge_result.has_unresolved_conflicts(treat_as_unresolved),
            "Must have entries to indicate conflicting files, or bad things will happen later: {:#?}",
            merge_result.conflicts
        );
        Ok(out)
    }

    #[derive(Default, Debug, Clone, Serialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct ConflictEntries {
        pub(crate) ancestor_entries: Vec<PathBuf>,
        pub(crate) our_entries: Vec<PathBuf>,
        pub(crate) their_entries: Vec<PathBuf>,
    }

    impl ConflictEntries {
        pub(crate) fn has_entries(&self) -> bool {
            !self.ancestor_entries.is_empty()
                || !self.our_entries.is_empty()
                || !self.their_entries.is_empty()
        }

        fn total_entries(&self) -> usize {
            let set = self
                .ancestor_entries
                .iter()
                .chain(self.our_entries.iter())
                .chain(self.their_entries.iter())
                .collect::<HashSet<_>>();

            set.len()
        }

        /// Return the `conflicted` header field value.
        pub(crate) fn conflicted_header_field(&self) -> Option<u64> {
            let entries = self.total_entries();
            Some(if entries > 0 { entries as u64 } else { 1 })
        }
    }
}
