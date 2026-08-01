use anyhow::Context as _;
use bstr::{BStr, BString};
use but_core::{DiffSpec, HunkHeader};

use crate::{
    CliId,
    id::{CommitId, CommittedFileId, IdAndHunk, UncommittedHunkOrFile},
};

#[derive(Debug)]
pub struct DiffSpecBuilder<'a> {
    repo: &'a gix::Repository,
    context_lines: u32,
    worktree_changes: Option<Vec<but_core::ui::TreeChange>>,
    diff_specs: Vec<DiffSpec>,
}

impl<'a> DiffSpecBuilder<'a> {
    pub fn new(repo: &'a gix::Repository, context_lines: u32) -> Self {
        Self {
            repo,
            context_lines,
            worktree_changes: None,
            diff_specs: Default::default(),
        }
    }

    #[expect(dead_code)]
    pub fn push_changes_from_id(&mut self, id: &CliId) -> anyhow::Result<()> {
        match id {
            CliId::UncommittedHunkOrFile(uncommitted) => {
                self.push_changes_from_uncommitted(uncommitted)
            }
            CliId::PathPrefix { id: _, hunks } => self.push_changes_from_path_prefix(hunks),
            CliId::CommittedFile {
                committed_file:
                    CommittedFileId {
                        commit_id,
                        path,
                        change_id: _,
                    },
                id: _,
            } => self.push_changes_from_committed_file(*commit_id, path.as_ref()),
            CliId::Branch(branch) => {
                anyhow::bail!("Cannot compute diff specs for branch `{}`", branch.name)
            }
            CliId::Commit {
                commit:
                    CommitId {
                        commit_id,
                        change_id: _,
                    },
                id: _,
            } => self.push_changes_from_commit(*commit_id),
            CliId::Uncommitted { id: _ } => self.push_changes_from_uncommitted_area(),
            CliId::Stack { .. } => {
                anyhow::bail!("Cannot compute diff specs for stacks")
            }
        }
    }

    pub fn push_changes_from_uncommitted(
        &mut self,
        uncommitted: &UncommittedHunkOrFile,
    ) -> anyhow::Result<()> {
        let hunks = uncommitted.hunks.iter().cloned();
        self.push_hunks(hunks.map(|id_and_hunk| id_and_hunk.hunk))
    }

    pub fn push_changes_from_path_prefix(
        &mut self,
        hunks: &nonempty::NonEmpty<IdAndHunk>,
    ) -> anyhow::Result<()> {
        self.push_hunks(hunks.iter().map(|id_and_hunk| id_and_hunk.hunk.clone()))
    }

    pub fn push_changes_from_committed_file(
        &mut self,
        commit_id: gix::ObjectId,
        path: &BStr,
    ) -> anyhow::Result<()> {
        self.push_changes_from_path_in_commit(path, commit_id, "First parent")
    }

    pub fn push_changes_from_path_in_commit(
        &mut self,
        path: &BStr,
        commit_id: gix::ObjectId,
        parent_context: &'static str,
    ) -> anyhow::Result<()> {
        let specs = self.diff_specs_for_path_in_commit(path, commit_id, parent_context)?;
        self.diff_specs.extend(specs);
        Ok(())
    }

    pub fn push_changes_from_commit(&mut self, commit_id: gix::ObjectId) -> anyhow::Result<()> {
        let specs = self.diff_specs_for_commit(commit_id, "First parent")?;
        self.diff_specs.extend(specs);
        Ok(())
    }

    pub fn push_changes_from_uncommitted_area(&mut self) -> anyhow::Result<()> {
        let changes = self.worktree_changes()?.to_vec();
        let hunks = but_core::hunks_from_changes(self.repo, changes.clone(), self.context_lines);
        self.push_hunks_with_changes(hunks, &changes);
        Ok(())
    }

    pub fn push_hunks(
        &mut self,
        hunks: impl IntoIterator<Item = but_core::SingleHunk>,
    ) -> anyhow::Result<()> {
        let changes = self.worktree_changes()?.to_vec();
        self.push_hunks_with_changes(hunks, &changes);
        Ok(())
    }

    #[expect(dead_code)]
    pub fn push_changes_from_single_hunk(&mut self, path: BString, header: HunkHeader) {
        self.diff_specs.push(DiffSpec {
            previous_path: None,
            path,
            hunk_headers: Vec::from([header]),
        });
    }

    pub fn into_diff_specs(self) -> Vec<DiffSpec> {
        but_workspace::flatten_diff_specs(self.diff_specs)
    }

    /// Reconciles the builder's [`DiffSpec`]s by sorting, coalescing and deduplicating all of the
    /// specs based on worktree changes. This only works reliably if the [`DiffSpec`]s are sourced
    /// from the worktree changes. The end result is that there is at most one [`DiffSpec`] per file
    /// and no duplicated hunks.
    ///
    /// WARNING: Does not support overlapping hunks - results may get very strange if such hunks are
    /// in the specs. The implementation naively assumes that hunk equality checks are sufficient
    /// for reconciling changes, whereas with overlapping hunks that is not the case.
    pub fn reconcile_worktree_diff_specs(&mut self) -> anyhow::Result<()> {
        use bstr::ByteSlice;
        use std::collections::HashMap;

        // This looks a bit odd, but we need to populate the worktree_changes cache without holding
        // onto the mut self reference. Otherwise we cant sort the diff_specs later.
        self.worktree_changes()?;
        let worktree_changes = self
            .worktree_changes
            .as_deref()
            .expect("BUG: worktree_changes cache should be populated!");

        #[derive(Hash, Eq, PartialEq)]
        struct DiffSpecKey<'a> {
            path: &'a BStr,
            previous_path: Option<&'a BStr>,
        }

        let mut diff_spec_order: HashMap<DiffSpecKey<'_>, usize> = HashMap::new();
        for (i, change) in worktree_changes.iter().enumerate() {
            use but_core::ui::TreeStatus;

            let previous_path = match &change.status {
                TreeStatus::Rename {
                    previous_path_bytes,
                    ..
                } => Some(previous_path_bytes.as_bstr()),
                _ => None,
            };

            let key = DiffSpecKey {
                path: change.path_bytes.as_bstr(),
                previous_path,
            };
            diff_spec_order.insert(key, i);
        }

        self.diff_specs.sort_by_key(|item| {
            let key = DiffSpecKey {
                path: item.path.as_bstr(),
                previous_path: item.previous_path.as_ref().map(|p| p.as_bstr()),
            };
            *diff_spec_order
                .get(&key)
                .expect("BUG: diff_spec_order did not contain all DiffSpecs")
        });

        let mut reconciled_changes: Vec<DiffSpec> = vec![];
        for change in self.diff_specs.iter() {
            match reconciled_changes.last_mut() {
                Some(last) if last.path == change.path => {
                    for hunk in change.hunk_headers.iter() {
                        match last.hunk_headers.binary_search(hunk) {
                            Ok(_) => (),
                            Err(i) => last.hunk_headers.insert(i, *hunk),
                        }
                    }
                }
                Some(_) | None => {
                    let mut change = change.clone();
                    change.hunk_headers.sort();
                    change.hunk_headers.dedup();
                    reconciled_changes.push(change)
                }
            }
        }

        self.diff_specs = reconciled_changes;

        Ok(())
    }

    fn worktree_changes(&mut self) -> anyhow::Result<&[but_core::ui::TreeChange]> {
        if self.worktree_changes.is_none() {
            self.worktree_changes = Some(but_core::diff::ui::worktree_changes(self.repo)?.changes);
        }
        Ok(self.worktree_changes.as_deref().unwrap_or_default())
    }

    fn push_hunks_with_changes(
        &mut self,
        hunks: impl IntoIterator<Item = but_core::SingleHunk>,
        changes: &[but_core::ui::TreeChange],
    ) {
        self.diff_specs
            .extend(but_core::diff_specs_with_changes(hunks, changes));
    }

    fn diff_specs_for_path_in_commit(
        &self,
        path: &BStr,
        source_id: gix::ObjectId,
        parent_context: &'static str,
    ) -> anyhow::Result<Vec<DiffSpec>> {
        let source_commit = self.repo.find_commit(source_id)?;
        let source_commit_parent_id = source_commit.parent_ids().next().context(parent_context)?;

        let tree_changes = but_core::diff::tree_changes(
            self.repo,
            Some(source_commit_parent_id.detach()),
            source_id,
        )?;
        Ok(tree_changes
            .into_iter()
            .filter(|tc| tc.path == path)
            .map(Into::into)
            .collect())
    }

    fn diff_specs_for_commit(
        &self,
        source_id: gix::ObjectId,
        parent_context: &'static str,
    ) -> anyhow::Result<Vec<DiffSpec>> {
        let source_commit = self.repo.find_commit(source_id)?;
        let source_commit_parent_id = source_commit.parent_ids().next().context(parent_context)?;

        let tree_changes = but_core::diff::tree_changes(
            self.repo,
            Some(source_commit_parent_id.detach()),
            source_id,
        )?;
        Ok(tree_changes.into_iter().map(Into::into).collect())
    }
}
