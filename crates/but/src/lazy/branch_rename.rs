use but_api::legacy::stack;

use super::app::{BranchRenameTarget, LazyApp, Panel};

impl LazyApp {
    pub(super) fn open_branch_rename_modal(&mut self) {
        if !matches!(self.active_panel, Panel::Status) {
            self.command_log
                .push("Select a branch in the Status panel to rename".to_string());
            return;
        }

        let Some((Some(stack_id), branch_name)) = self
            .get_selected_branch_context()
            .map(|(stack_id, branch)| (stack_id, branch.name.clone()))
        else {
            self.command_log
                .push("No branch selected to rename".to_string());
            return;
        };

        self.branch_rename_input = branch_name.clone();
        self.branch_rename_target = Some(BranchRenameTarget {
            stack_id,
            current_name: branch_name.clone(),
        });
        self.show_branch_rename_modal = true;
        self.command_log
            .push(format!("Renaming branch '{}'", branch_name));
    }

    pub(super) fn cancel_branch_rename_modal(&mut self) {
        self.reset_branch_rename_modal_state();
        self.command_log.push("Branch rename canceled".to_string());
    }

    pub(super) fn submit_branch_rename_modal(&mut self) {
        match self.perform_branch_rename() {
            Ok(true) => self.reset_branch_rename_modal_state(),
            Ok(false) => {}
            Err(e) => {
                self.command_log
                    .push(format!("Branch rename failed: {}", e));
            }
        }
    }

    fn perform_branch_rename(&mut self) -> anyhow::Result<bool> {
        let Some(target) = self.branch_rename_target.as_ref() else {
            return Ok(false);
        };

        let new_name = self.branch_rename_input.trim();
        if new_name.is_empty() {
            self.command_log
                .push("Branch name cannot be empty".to_string());
            return Ok(false);
        }

        if new_name == target.current_name {
            self.command_log.push("Branch name unchanged".to_string());
            return Ok(false);
        }

        stack::update_branch_name(
            self.project_id,
            target.stack_id,
            target.current_name.clone(),
            new_name.to_string(),
        )?;

        self.command_log.push(format!(
            "Renamed branch '{}' to '{}'",
            target.current_name, new_name
        ));

        let project = gitbutler_project::get(self.project_id)?;
        self.load_data_with_project(&project)?;
        self.update_main_view();
        Ok(true)
    }

    fn reset_branch_rename_modal_state(&mut self) {
        self.show_branch_rename_modal = false;
        self.branch_rename_input.clear();
        self.branch_rename_target = None;
    }
}
