use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, anyhow, bail};
use bstr::{BString, ByteSlice};
use but_api::{
    json::HexHash,
    legacy::{diff, stack, workspace},
};
use but_core::{DiffSpec, HunkHeader, ref_metadata::StackId};
use but_hunk_assignment::HunkAssignment;
use but_workspace::ui::StackDetails;
use gitbutler_project::Project;

use crate::status::assignment::FileAssignment;

use super::app::{CommitBranchOption, CommitFileOption, CommitModalFocus, LazyApp};

impl LazyApp {
    pub(super) fn open_commit_modal(&mut self) {
        self.reset_commit_modal_state();

        for stack in &self.stacks {
            if let Some(stack_id) = stack.id {
                for branch in &stack.branches {
                    self.commit_branch_options.push(CommitBranchOption {
                        stack_id: Some(stack_id),
                        branch_name: branch.name.clone(),
                        is_new_branch: false,
                    });
                }
            }
        }

        let canned_name = but_api::legacy::workspace::canned_branch_name(self.project_id)
            .unwrap_or_else(|_| "new-branch".to_string());

        self.commit_branch_options.push(CommitBranchOption {
            stack_id: None,
            branch_name: format!("New: {}", canned_name),
            is_new_branch: true,
        });

        self.commit_new_branch_name = canned_name;

        if let Some((stack_id, selected_branch)) = self.get_selected_branch_context() {
            let stack_id_ref = stack_id.as_ref();
            let mut preferred_idx = None;

            if stack_id_ref.is_some() {
                preferred_idx = self.commit_branch_options.iter().position(|opt| {
                    !opt.is_new_branch
                        && opt.stack_id.as_ref() == stack_id_ref
                        && opt.branch_name == selected_branch.name
                });

                if preferred_idx.is_none() {
                    preferred_idx = self.commit_branch_options.iter().position(|opt| {
                        !opt.is_new_branch && opt.stack_id.as_ref() == stack_id_ref
                    });
                }
            }

            if preferred_idx.is_none() {
                preferred_idx = self
                    .commit_branch_options
                    .iter()
                    .position(|opt| !opt.is_new_branch && opt.branch_name == selected_branch.name);
            }

            if let Some(idx) = preferred_idx {
                self.commit_selected_branch_idx = idx;
            }
        }

        self.show_commit_modal = true;
        self.commit_modal_focus = CommitModalFocus::BranchSelect;
        self.rebuild_commit_file_list();
        self.command_log.push("Opened commit modal".to_string());
    }

    pub(super) fn reset_commit_modal_state(&mut self) {
        self.commit_subject.clear();
        self.commit_message.clear();
        self.commit_new_branch_name.clear();
        self.commit_only_mode = false;
        self.commit_branch_options.clear();
        self.commit_selected_branch_idx = 0;
        self.commit_modal_focus = CommitModalFocus::BranchSelect;
        self.commit_files.clear();
        self.commit_selected_file_idx = 0;
        self.commit_selected_file_paths.clear();
    }

    pub(super) fn dismiss_commit_modal(&mut self) {
        self.reset_commit_modal_state();
        self.show_commit_modal = false;
    }

    pub(super) fn cancel_commit_modal(&mut self) {
        self.dismiss_commit_modal();
        self.command_log.push("Canceled commit".to_string());
    }

    fn perform_commit(&mut self) -> Result<()> {
        let full_message = if self.commit_message.is_empty() {
            self.commit_subject.clone()
        } else {
            format!("{}\n\n{}", self.commit_subject, self.commit_message)
        };

        if full_message.trim().is_empty() {
            self.command_log
                .push("Commit failed: empty message".to_string());
            return Ok(());
        }

        if self.commit_selected_file_paths.is_empty() {
            self.command_log
                .push("Commit failed: no files selected".to_string());
            return Err(anyhow!("No files selected"));
        }

        let selected = &self.commit_branch_options[self.commit_selected_branch_idx];
        let branch_name = if selected.is_new_branch {
            self.commit_new_branch_name.clone()
        } else {
            selected.branch_name.clone()
        };

        self.command_log.push(format!(
            "workspace commit: branch={}, only_mode={}, new_branch={}",
            branch_name, self.commit_only_mode, selected.is_new_branch
        ));

        let project = gitbutler_project::get(self.project_id)?;
        match self.execute_lazy_commit(
            &project,
            full_message,
            branch_name,
            selected.stack_id,
            selected.is_new_branch,
        ) {
            Ok(_) => Ok(()),
            Err(e) => {
                self.command_log.push(format!("Commit error: {}", e));
                Err(e)
            }
        }
    }

    pub(super) fn submit_commit_modal(&mut self) {
        if let Err(e) = self.perform_commit() {
            self.command_log.push(format!("Commit failed: {}", e));
        } else {
            self.dismiss_commit_modal();
        }
    }

    fn execute_lazy_commit(
        &mut self,
        project: &Project,
        commit_message: String,
        branch_name: String,
        stack_id_hint: Option<StackId>,
        create_branch: bool,
    ) -> Result<()> {
        let (target_stack_id, stack_details) =
            self.resolve_target_stack(project, stack_id_hint, &branch_name, create_branch)?;

        let target_branch = stack_details
            .branch_details
            .iter()
            .find(|branch| branch.name.to_str_lossy() == branch_name)
            .ok_or_else(|| anyhow!("Branch '{}' not found in stack", branch_name))?;

        let files_to_commit = self.collect_files_to_commit(target_stack_id)?;

        if files_to_commit.is_empty() {
            bail!("No changes to commit");
        }

        let diff_specs = Self::files_to_diff_specs(&files_to_commit);

        self.command_log.push(format!(
            "workspace::create_commit_from_worktree_changes(stack: {}, branch: {})",
            target_stack_id, branch_name
        ));

        workspace::create_commit_from_worktree_changes(
            self.project_id,
            target_stack_id,
            Some(HexHash::from(target_branch.tip)),
            diff_specs,
            commit_message,
            branch_name.clone(),
        )?;

        self.command_log
            .push(format!("Commit created successfully on '{}'", branch_name));

        self.load_data_with_project(project)?;
        self.update_main_view();
        Ok(())
    }

    fn resolve_target_stack(
        &mut self,
        project: &Project,
        stack_id_hint: Option<StackId>,
        branch_name: &str,
        create_branch: bool,
    ) -> Result<(StackId, StackDetails)> {
        if create_branch {
            self.command_log.push(format!(
                "but_api::legacy::stack::create_reference(new_name={})",
                branch_name
            ));
            let (new_stack_id_opt, _) = stack::create_reference(
                project.id,
                stack::create_reference::Request {
                    new_name: branch_name.to_string(),
                    anchor: None,
                },
            )?;
            let new_stack_id = new_stack_id_opt
                .ok_or_else(|| anyhow!("Failed to create new branch '{}'", branch_name))?;
            self.command_log.push(format!(
                "but_api::legacy::workspace::stack_details({:?})",
                new_stack_id
            ));
            let details = workspace::stack_details(project.id, Some(new_stack_id))?;
            return Ok((new_stack_id, details));
        }

        let stack_id = stack_id_hint
            .ok_or_else(|| anyhow!("Missing stack ID for branch '{}'", branch_name))?;
        self.command_log.push(format!(
            "but_api::legacy::workspace::stack_details({:?})",
            stack_id
        ));
        let details = workspace::stack_details(project.id, Some(stack_id))?;
        Ok((stack_id, details))
    }

    fn collect_files_to_commit(&mut self, stack_id: StackId) -> Result<Vec<FileAssignment>> {
        self.command_log
            .push("but_api::legacy::diff::changes_in_worktree()".to_string());
        let worktree_changes = diff::changes_in_worktree(self.project_id)?;
        let assignments_by_file = Self::group_assignments_by_file(worktree_changes.assignments);

        let mut files_to_commit = Vec::new();
        if !self.commit_only_mode {
            let unassigned =
                crate::status::assignment::filter_by_stack_id(assignments_by_file.values(), &None);
            files_to_commit.extend(unassigned);
        }

        let stack_assignments = crate::status::assignment::filter_by_stack_id(
            assignments_by_file.values(),
            &Some(stack_id),
        );
        files_to_commit.extend(stack_assignments);

        if !self.commit_selected_file_paths.is_empty() {
            files_to_commit.retain(|file| {
                let key = Self::file_key_from_path(&file.path);
                self.commit_selected_file_paths.contains(&key)
            });
        }

        Ok(files_to_commit)
    }

    fn group_assignments_by_file(
        assignments: Vec<HunkAssignment>,
    ) -> BTreeMap<BString, FileAssignment> {
        let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
        for assignment in assignments {
            by_file
                .entry(assignment.path_bytes.clone())
                .or_default()
                .push(assignment);
        }

        let mut assignments_by_file = BTreeMap::new();
        for (path, grouped) in by_file {
            assignments_by_file.insert(
                path.clone(),
                FileAssignment::from_assignments(&path, &grouped),
            );
        }

        assignments_by_file
    }

    pub(super) fn rebuild_commit_file_list(&mut self) {
        let stack_id = self
            .commit_branch_options
            .get(self.commit_selected_branch_idx)
            .and_then(|opt| opt.stack_id);
        let files = self.gather_commit_candidate_files(stack_id);

        let mut retained = HashSet::new();
        for file in &files {
            let key = Self::file_key_from_path(&file.path);
            if self.commit_selected_file_paths.contains(&key) {
                retained.insert(key);
            }
        }

        if retained.is_empty() {
            for file in &files {
                retained.insert(Self::file_key_from_path(&file.path));
            }
        }

        self.commit_selected_file_paths = retained;
        self.commit_files = files
            .into_iter()
            .map(|file| CommitFileOption { file })
            .collect();

        if self.commit_selected_file_idx >= self.commit_files.len() {
            self.commit_selected_file_idx = self.commit_files.len().saturating_sub(1);
        }
    }

    fn gather_commit_candidate_files(&self, stack_id: Option<StackId>) -> Vec<FileAssignment> {
        let mut files = Vec::new();
        if !self.commit_only_mode {
            files.extend(self.unassigned_files.iter().cloned());
        }

        if let Some(stack_id) = stack_id {
            if let Some(stack) = self.stacks.iter().find(|s| s.id == Some(stack_id)) {
                for branch in &stack.branches {
                    files.extend(branch.assignments.iter().cloned());
                }
            }
        }

        files
    }

    pub(super) fn move_commit_file_cursor(&mut self, direction: i32) {
        if self.commit_files.is_empty() {
            return;
        }
        if direction > 0 {
            if self.commit_selected_file_idx + 1 >= self.commit_files.len() {
                self.commit_selected_file_idx = 0;
            } else {
                self.commit_selected_file_idx += 1;
            }
        } else if self.commit_selected_file_idx == 0 {
            self.commit_selected_file_idx = self.commit_files.len() - 1;
        } else {
            self.commit_selected_file_idx -= 1;
        }
    }

    pub(super) fn toggle_current_commit_file(&mut self) {
        if let Some(entry) = self.commit_files.get(self.commit_selected_file_idx) {
            let key = Self::file_key_from_path(&entry.file.path);
            if self.commit_selected_file_paths.contains(&key) {
                self.commit_selected_file_paths.remove(&key);
                if self.commit_selected_file_paths.is_empty() {
                    self.commit_selected_file_paths.insert(key);
                }
            } else {
                self.commit_selected_file_paths.insert(key);
            }
        }
    }

    fn file_key_from_path(path: &BString) -> String {
        path.to_str_lossy().into_owned()
    }

    fn files_to_diff_specs(files: &[FileAssignment]) -> Vec<DiffSpec> {
        files
            .iter()
            .map(|fa| {
                let hunk_headers: Vec<HunkHeader> = fa
                    .assignments
                    .iter()
                    .filter_map(|assignment| assignment.inner.hunk_header)
                    .collect();

                DiffSpec {
                    previous_path: None,
                    path: fa.path.clone(),
                    hunk_headers,
                }
            })
            .collect()
    }
}
