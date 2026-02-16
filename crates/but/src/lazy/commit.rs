use anyhow::{Result, bail};
use bstr::ByteSlice;
use but_core::DiffSpec;
use but_ctx::Context;

use super::app::{App, CommitBranchOption, CommitFileOption, CommitModalFocus};
use crate::command::legacy::status::assignment::FileAssignment;

impl App {
    /// Open the commit modal, populating branch options and file list.
    pub(super) fn open_commit_modal(&mut self, ctx: &Context) {
        self.reset_commit_modal();

        // Build branch options from existing stacks
        for stack in &self.stacks {
            for branch in &stack.branches {
                self.commit_branch_options.push(CommitBranchOption {
                    stack_id: stack.id,
                    branch_name: branch.name.clone(),
                    is_new_branch: false,
                });
            }
        }

        // Add "new branch" option
        let new_name = but_api::legacy::workspace::canned_branch_name(ctx)
            .unwrap_or_else(|_| "new-branch".to_string());
        self.commit_branch_options.push(CommitBranchOption {
            stack_id: None,
            branch_name: new_name,
            is_new_branch: true,
        });

        // Pre-select the branch that's currently selected in the status panel
        if let Some(branch_name) = self.selected_branch_name() {
            if let Some(idx) = self
                .commit_branch_options
                .iter()
                .position(|o| !o.is_new_branch && o.branch_name == branch_name)
            {
                self.commit_selected_branch = idx;
            }
        }

        // If there are staged (assigned) files, default to staged-only mode
        let has_staged = self
            .stacks
            .get(self.commit_selected_branch)
            .map(|s| s.branches.iter().any(|b| !b.files.is_empty()))
            .unwrap_or(false);
        self.commit_staged_only = has_staged;

        self.rebuild_commit_files();
        self.show_commit_modal = true;
        self.commit_focus = CommitModalFocus::Subject;
        self.command_log.push("Opened commit modal".to_string());
    }

    /// Rebuild the file list based on the currently selected branch and staged-only toggle.
    pub(super) fn rebuild_commit_files(&mut self) {
        let selected = match self.commit_branch_options.get(self.commit_selected_branch) {
            Some(opt) => opt,
            None => return,
        };

        let mut files: Vec<CommitFileOption> = Vec::new();

        if selected.is_new_branch {
            // For new branch, show all unassigned files
            for f in &self.unassigned_files {
                files.push(CommitFileOption {
                    path: f.path.to_str_lossy().into_owned(),
                    selected: true,
                });
            }
        } else if self.commit_staged_only {
            // Only show files assigned to this stack
            if let Some(stack) = self.stacks.iter().find(|s| s.id == selected.stack_id) {
                for branch in &stack.branches {
                    for f in &branch.files {
                        files.push(CommitFileOption {
                            path: f.path.to_str_lossy().into_owned(),
                            selected: true,
                        });
                    }
                }
            }
        } else {
            // Show unassigned + files assigned to this stack
            for f in &self.unassigned_files {
                files.push(CommitFileOption {
                    path: f.path.to_str_lossy().into_owned(),
                    selected: true,
                });
            }
            if let Some(stack) = self.stacks.iter().find(|s| s.id == selected.stack_id) {
                for branch in &stack.branches {
                    for f in &branch.files {
                        files.push(CommitFileOption {
                            path: f.path.to_str_lossy().into_owned(),
                            selected: true,
                        });
                    }
                }
            }
        }

        self.commit_files = files;
        self.commit_file_cursor = 0;
    }

    pub(super) fn cancel_commit_modal(&mut self) {
        self.show_commit_modal = false;
        self.command_log.push("Commit cancelled".to_string());
    }

    pub(super) fn submit_commit(&mut self, ctx: &mut Context) {
        match self.perform_commit(ctx) {
            Ok(()) => {
                self.show_commit_modal = false;
                self.refresh(ctx);
            }
            Err(e) => {
                self.command_log.push(format!("Commit failed: {e}"));
            }
        }
    }

    fn perform_commit(&mut self, ctx: &mut Context) -> Result<()> {
        let message = if self.commit_message.is_empty() {
            self.commit_subject.clone()
        } else {
            format!("{}\n\n{}", self.commit_subject, self.commit_message)
        };

        if message.trim().is_empty() {
            bail!("Empty commit message");
        }

        let selected_paths: Vec<String> = self
            .commit_files
            .iter()
            .filter(|f| f.selected)
            .map(|f| f.path.clone())
            .collect();

        if selected_paths.is_empty() {
            bail!("No files selected");
        }

        let opt = &self.commit_branch_options[self.commit_selected_branch];
        let branch_name = opt.branch_name.clone();
        let is_new = opt.is_new_branch;

        // Resolve or create the stack
        let stack_id = if is_new {
            self.command_log
                .push(format!("Creating branch '{branch_name}'"));
            let (new_id, _) = but_api::legacy::stack::create_reference(
                ctx,
                but_api::legacy::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor: None,
                },
            )?;
            new_id.ok_or_else(|| anyhow::anyhow!("Failed to create branch"))?
        } else {
            opt.stack_id
                .ok_or_else(|| anyhow::anyhow!("No stack ID for branch"))?
        };

        // Get branch tip for parent_id
        let details = but_api::legacy::workspace::stack_details(ctx, Some(stack_id))?;
        let parent_id = details
            .branch_details
            .iter()
            .find(|b| b.name.to_str_lossy() == branch_name)
            .map(|b| but_api::json::HexHash::from(b.tip));

        // Build DiffSpecs from selected files
        let diff_specs = self.build_diff_specs(&selected_paths);

        self.command_log.push(format!(
            "Committing {} file(s) to '{branch_name}'",
            selected_paths.len()
        ));

        but_api::legacy::workspace::create_commit_from_worktree_changes(
            ctx,
            stack_id,
            parent_id,
            diff_specs,
            message,
            branch_name,
        )?;

        self.command_log.push("Commit created".to_string());
        Ok(())
    }

    fn build_diff_specs(&self, selected_paths: &[String]) -> Vec<DiffSpec> {
        let mut specs = Vec::new();

        let all_files: Vec<&FileAssignment> = self
            .unassigned_files
            .iter()
            .chain(
                self.stacks
                    .iter()
                    .flat_map(|s| s.branches.iter().flat_map(|b| b.files.iter())),
            )
            .collect();

        for fa in all_files {
            let path_str = fa.path.to_str_lossy();
            if selected_paths.iter().any(|p| p == path_str.as_ref()) {
                specs.push(DiffSpec {
                    previous_path: None,
                    path: fa.path.clone(),
                    hunk_headers: fa
                        .assignments
                        .iter()
                        .filter_map(|a| a.inner.hunk_header)
                        .collect(),
                });
            }
        }

        specs
    }

    pub(super) fn toggle_commit_file(&mut self) {
        if let Some(f) = self.commit_files.get_mut(self.commit_file_cursor) {
            f.selected = !f.selected;
            // Don't allow deselecting everything
            if self.commit_files.iter().all(|f| !f.selected) {
                self.commit_files[self.commit_file_cursor].selected = true;
            }
        }
    }

    pub(super) fn has_uncommitted_changes(&self) -> bool {
        !self.unassigned_files.is_empty()
            || self
                .stacks
                .iter()
                .any(|s| s.branches.iter().any(|b| !b.files.is_empty()))
    }

    fn reset_commit_modal(&mut self) {
        self.commit_branch_options.clear();
        self.commit_selected_branch = 0;
        self.commit_files.clear();
        self.commit_file_cursor = 0;
        self.commit_subject.clear();
        self.commit_message.clear();
        self.commit_staged_only = false;
        self.commit_focus = CommitModalFocus::BranchSelect;
    }

    fn selected_branch_name(&self) -> Option<String> {
        match self.selected_status_item()? {
            super::app::StatusItem::Branch { stack, branch } => {
                Some(self.stacks[stack].branches[branch].name.clone())
            }
            super::app::StatusItem::AssignedFile { stack, branch, .. } => {
                Some(self.stacks[stack].branches[branch].name.clone())
            }
            super::app::StatusItem::Commit { stack, branch, .. } => {
                Some(self.stacks[stack].branches[branch].name.clone())
            }
            _ => None,
        }
    }
}
