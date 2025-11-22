use anyhow::Result;
use but_oxidize::ObjectIdExt;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;

use super::app::{LazyApp, Panel, UncommitTargetInfo};

impl LazyApp {
    pub(super) fn open_uncommit_modal(&mut self) {
        if !matches!(self.active_panel, Panel::Status) {
            self.command_log
                .push("Select a commit in the Status panel to uncommit".to_string());
            return;
        }

        let Some(commit) = self.get_selected_commit().cloned() else {
            self.command_log
                .push("No commit selected to uncommit".to_string());
            return;
        };

        let Some((Some(stack_id), branch)) = self.find_branch_context(|candidate| {
            candidate
                .commits
                .iter()
                .any(|c| c.full_id == commit.full_id)
        }) else {
            self.command_log
                .push("Unable to resolve stack for selected commit".to_string());
            return;
        };

        let branch_name = branch.name.clone();

        self.reset_uncommit_modal_state();

        self.uncommit_target = Some(UncommitTargetInfo {
            stack_id,
            branch_name,
            commit_short_id: commit.id.clone(),
            commit_full_id: commit.full_id.clone(),
            commit_message: commit.message.clone(),
        });
        self.show_uncommit_modal = true;
        self.command_log
            .push(format!("Preparing to uncommit {}", commit.id));
    }

    pub(super) fn cancel_uncommit_modal(&mut self) {
        self.reset_uncommit_modal_state();
        self.command_log.push("Canceled uncommit".to_string());
    }

    pub(super) fn confirm_uncommit_modal(&mut self) {
        match self.perform_uncommit() {
            Ok(true) => self.reset_uncommit_modal_state(),
            Ok(false) => {}
            Err(e) => {
                self.command_log.push(format!("Failed to uncommit: {}", e));
            }
        }
    }

    fn perform_uncommit(&mut self) -> Result<bool> {
        let Some(target) = self.uncommit_target.as_ref() else {
            return Ok(false);
        };

        let project = gitbutler_project::get(self.project_id)?;
        let ctx =
            &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

        let oid = gix::ObjectId::from_hex(target.commit_full_id.as_bytes())?;

        self.command_log.push(format!(
            "gitbutler_branch_actions::undo_commit(stack={:?}, commit={})",
            target.stack_id, target.commit_short_id
        ));

        gitbutler_branch_actions::undo_commit(ctx, target.stack_id, oid.to_git2())?;

        self.command_log.push(format!(
            "Uncommitted {} from branch {}",
            target.commit_short_id, target.branch_name
        ));

        self.load_data_with_project(&project)?;
        self.update_main_view();
        Ok(true)
    }

    fn reset_uncommit_modal_state(&mut self) {
        self.show_uncommit_modal = false;
        self.uncommit_target = None;
    }
}
