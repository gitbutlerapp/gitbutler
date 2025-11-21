use anyhow::Result;
use but_oxidize::ObjectIdExt;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;

use super::app::{LazyApp, Panel, SquashTargetInfo};

impl LazyApp {
    pub(super) fn open_squash_modal(&mut self) {
        if !matches!(self.active_panel, Panel::Status) {
            self.command_log
                .push("Select a commit in the Status panel to squash".to_string());
            return;
        }

        let Some(commit) = self.get_selected_commit().cloned() else {
            self.command_log
                .push("No commit selected to squash".to_string());
            return;
        };

        let (stack_id, branch_name, destination) = {
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

            let Some(commit_index) = branch
                .commits
                .iter()
                .position(|c| c.full_id == commit.full_id)
            else {
                self.command_log
                    .push("Unable to locate commit position".to_string());
                return;
            };

            if commit_index + 1 >= branch.commits.len() {
                self.command_log
                    .push("Cannot squash the last commit in a branch".to_string());
                return;
            }

            (
                stack_id,
                branch.name.clone(),
                branch.commits[commit_index + 1].clone(),
            )
        };

        self.reset_squash_modal_state();

        self.squash_target = Some(SquashTargetInfo {
            stack_id,
            branch_name,
            source_short_id: commit.id.clone(),
            source_full_id: commit.full_id.clone(),
            source_message: commit.message.clone(),
            destination_short_id: destination.id.clone(),
            destination_full_id: destination.full_id.clone(),
            destination_message: destination.message.clone(),
        });
        self.show_squash_modal = true;
        self.command_log.push(format!(
            "Preparing to squash {} into {}",
            commit.id, destination.id
        ));
    }

    pub(super) fn cancel_squash_modal(&mut self) {
        self.reset_squash_modal_state();
        self.command_log.push("Canceled squash".to_string());
    }

    pub(super) fn confirm_squash_modal(&mut self) {
        match self.perform_squash() {
            Ok(true) => self.reset_squash_modal_state(),
            Ok(false) => {}
            Err(e) => self.command_log.push(format!("Failed to squash: {}", e)),
        }
    }

    fn perform_squash(&mut self) -> Result<bool> {
        let Some(target) = self.squash_target.as_ref() else {
            return Ok(false);
        };

        let project = gitbutler_project::get(self.project_id)?;
        let ctx =
            &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

        let source_oid = gix::ObjectId::from_hex(target.source_full_id.as_bytes())?;
        let destination_oid = gix::ObjectId::from_hex(target.destination_full_id.as_bytes())?;

        self.command_log.push(format!(
            "gitbutler_branch_actions::squash_commits(stack={:?}, source={}, destination={})",
            target.stack_id, target.source_short_id, target.destination_short_id
        ));

        gitbutler_branch_actions::squash_commits(
            ctx,
            target.stack_id,
            vec![source_oid.to_git2()],
            destination_oid.to_git2(),
        )?;

        self.command_log.push(format!(
            "Squashed {} into {}",
            target.source_short_id, target.destination_short_id
        ));

        self.load_data_with_project(&project)?;
        self.update_main_view();
        Ok(true)
    }

    fn reset_squash_modal_state(&mut self) {
        self.show_squash_modal = false;
        self.squash_target = None;
    }
}
