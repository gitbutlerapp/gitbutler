use anyhow::Context as _;
use bstr::{BStr, BString};
use but_core::{DiffSpec, HunkHeader, ref_metadata::StackId};
use but_hunk_assignment::HunkAssignment;

use crate::{
    CliId,
    id::{ShortId, UncommittedHunkOrFile},
};

#[cfg(feature = "legacy")]
use crate::command::legacy::status::assignment::FileAssignment;

#[derive(Debug)]
pub struct DiffSpecBuilder<'a> {
    db: &'a mut but_db::DbHandle,
    repo: &'a gix::Repository,
    workspace: &'a but_graph::Workspace,
    context_lines: u32,
    scope_to_stack: Option<StackId>,
    worktree_changes: Option<Vec<but_core::ui::TreeChange>>,
    diff_specs: Vec<DiffSpec>,
}

impl<'a> DiffSpecBuilder<'a> {
    pub fn new(
        db: &'a mut but_db::DbHandle,
        repo: &'a gix::Repository,
        workspace: &'a but_graph::Workspace,
        context_lines: u32,
    ) -> Self {
        Self {
            db,
            repo,
            workspace,
            context_lines,
            scope_to_stack: None,
            worktree_changes: None,
            diff_specs: Default::default(),
        }
    }

    #[expect(dead_code)] // TODO: remove this when we're removing assignments
    pub fn with_scope_to_stack(mut self, scope_to_stack: Option<StackId>) -> Self {
        self.scope_to_stack = scope_to_stack;
        self
    }

    #[expect(dead_code)]
    pub fn push_changes_from_id(&mut self, id: &CliId) -> anyhow::Result<()> {
        match id {
            CliId::UncommittedHunkOrFile(uncommitted) => {
                self.push_changes_from_uncommitted(uncommitted)
            }
            CliId::PathPrefix {
                id,
                hunk_assignments,
            } => self.push_changes_from_path_prefix(id, hunk_assignments),
            CliId::CommittedFile {
                commit_id,
                path,
                id: _,
            } => self.push_changes_from_committed_file(*commit_id, path.as_ref()),
            CliId::Branch { name, id, stack_id } => {
                self.push_changes_from_branch(name, id, *stack_id)
            }
            CliId::Commit { commit_id, id } => self.push_changes_from_commit(*commit_id, id),
            CliId::Uncommitted { id: _ } => self.push_changes_from_uncommitted_area(),
            CliId::Stack { id: _, stack_id } => self.push_changes_from_stack(*stack_id),
        }
    }

    pub fn push_changes_from_uncommitted(
        &mut self,
        uncommitted: &UncommittedHunkOrFile,
    ) -> anyhow::Result<()> {
        let scope_to_stack = self.scope_to_stack;
        let assignments = uncommitted
            .hunk_assignments
            .iter()
            .filter(|assignment| assignment.stack_id == scope_to_stack)
            .cloned();
        self.push_hunk_assignments(assignments)
    }

    pub fn push_changes_from_path_prefix(
        &mut self,
        _id: &ShortId,
        hunk_assignments: &nonempty::NonEmpty<(ShortId, HunkAssignment)>,
    ) -> anyhow::Result<()> {
        self.push_hunk_assignments(
            hunk_assignments
                .iter()
                .map(|(_id, assignment)| assignment.clone()),
        )
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

    pub fn push_changes_from_branch(
        &mut self,
        name: &str,
        _id: &ShortId,
        _stack_id: Option<StackId>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Cannot compute diff specs for branch `{name}`")
    }

    pub fn push_changes_from_commit(
        &mut self,
        commit_id: gix::ObjectId,
        _id: &ShortId,
    ) -> anyhow::Result<()> {
        let specs = self.diff_specs_for_commit(commit_id, "First parent")?;
        self.diff_specs.extend(specs);
        Ok(())
    }

    pub fn push_changes_from_uncommitted_area(&mut self) -> anyhow::Result<()> {
        let changes = self.worktree_changes()?.to_vec();
        let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
            self.db.hunk_assignments_mut()?,
            self.repo,
            self.workspace,
            Some(changes.clone()),
            self.context_lines,
        )?;
        let assignments = assignments
            .into_iter()
            .filter(|assignment| assignment.stack_id.is_none());
        self.push_hunk_assignments_with_changes(assignments, &changes);
        Ok(())
    }

    pub fn push_changes_from_stack(&mut self, stack_id: StackId) -> anyhow::Result<()> {
        let changes = self.worktree_changes()?.to_vec();
        let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
            self.db.hunk_assignments_mut()?,
            self.repo,
            self.workspace,
            Some(changes.clone()),
            self.context_lines,
        )?;
        let assignments = assignments
            .into_iter()
            .filter(|assignment| assignment.stack_id == Some(stack_id));
        self.push_hunk_assignments_with_changes(assignments, &changes);
        Ok(())
    }

    pub fn push_hunk_assignments(
        &mut self,
        assignments: impl IntoIterator<Item = HunkAssignment>,
    ) -> anyhow::Result<()> {
        let changes = self.worktree_changes()?.to_vec();
        self.push_hunk_assignments_with_changes(assignments, &changes);
        Ok(())
    }

    #[cfg(feature = "legacy")]
    pub fn push_file_assignments(&mut self, files: &[FileAssignment]) -> anyhow::Result<()> {
        let changes = self.worktree_changes()?.to_vec();
        self.diff_specs
            .extend(diff_specs_from_file_assignments_status_aware(
                files, &changes,
            ));
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

    fn push_hunk_assignments_with_changes(
        &mut self,
        assignments: impl IntoIterator<Item = HunkAssignment>,
        changes: &[but_core::ui::TreeChange],
    ) {
        self.diff_specs.extend(
            but_hunk_assignment::diff_specs_from_assignments_with_changes(assignments, changes),
        );
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

#[cfg(feature = "legacy")]
fn diff_specs_from_file_assignments_status_aware(
    files_to_commit: &[FileAssignment],
    changes: &[but_core::ui::TreeChange],
) -> Vec<DiffSpec> {
    files_to_commit
        .iter()
        .map(|fa| {
            let (previous_path, is_addition_or_deletion) = changes
                .iter()
                .find(|change| change.path_bytes == fa.path)
                .map(|change| match &change.status {
                    but_core::ui::TreeStatus::Rename {
                        previous_path_bytes,
                        ..
                    } => (Some(previous_path_bytes.clone()), false),
                    but_core::ui::TreeStatus::Addition { .. }
                    | but_core::ui::TreeStatus::Deletion { .. } => (None, true),
                    but_core::ui::TreeStatus::Modification { .. } => (None, false),
                })
                .unwrap_or((None, false));

            let hunk_headers = if is_addition_or_deletion {
                Vec::new()
            } else {
                fa.assignments
                    .iter()
                    .filter_map(|assignment| assignment.inner.hunk_header)
                    .collect()
            };

            DiffSpec {
                previous_path,
                path: fa.path.clone(),
                hunk_headers,
            }
        })
        .collect()
}
