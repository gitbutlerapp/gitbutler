use anyhow::Result;
use but_oxidize::{ObjectIdExt, OidExt};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;

use super::app::{LazyApp, Panel, RewordModalFocus, RewordTargetInfo};

impl LazyApp {
    pub(super) fn open_reword_modal(&mut self) {
        if !matches!(self.active_panel, Panel::Status) {
            self.command_log
                .push("Select a commit in the Status panel to edit".to_string());
            return;
        }

        let Some(commit) = self.get_selected_commit().cloned() else {
            self.command_log
                .push("No commit selected to edit".to_string());
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

        self.reset_reword_modal_state();

        let (subject, body) = Self::split_commit_message(&commit.message);
        self.reword_subject = subject;
        self.reword_message = body;
        self.reword_modal_focus = RewordModalFocus::Subject;
        self.reword_target = Some(RewordTargetInfo {
            stack_id,
            branch_name,
            commit_short_id: commit.id.clone(),
            commit_full_id: commit.full_id.clone(),
            original_message: commit.message.clone(),
        });
        self.show_reword_modal = true;
        self.command_log
            .push(format!("Editing commit message for {}", commit.id));
    }

    fn reset_reword_modal_state(&mut self) {
        self.show_reword_modal = false;
        self.reword_subject.clear();
        self.reword_message.clear();
        self.reword_modal_focus = RewordModalFocus::Subject;
        self.reword_target = None;
    }

    pub(super) fn cancel_reword_modal(&mut self) {
        self.reset_reword_modal_state();
        self.command_log
            .push("Canceled commit message editing".to_string());
    }

    pub(super) fn submit_reword_modal(&mut self) {
        match self.perform_reword_commit() {
            Ok(true) => self.reset_reword_modal_state(),
            Ok(false) => {}
            Err(e) => {
                self.command_log
                    .push(format!("Failed to update commit message: {}", e));
            }
        }
    }

    fn perform_reword_commit(&mut self) -> Result<bool> {
        let Some(target) = self.reword_target.as_ref() else {
            return Ok(false);
        };

        let new_message = Self::compose_reword_message(&self.reword_subject, &self.reword_message);

        if new_message.trim().is_empty() {
            self.command_log
                .push("Commit message cannot be empty".to_string());
            return Ok(false);
        }

        if new_message.trim() == target.original_message.trim() {
            self.command_log
                .push("Commit message unchanged".to_string());
            return Ok(false);
        }

        let project = gitbutler_project::get(self.project_id)?;
        let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

        let oid = gix::ObjectId::from_hex(target.commit_full_id.as_bytes())?;
        let git2_oid = oid.to_git2();

        self.command_log.push(format!(
            "gitbutler_branch_actions::update_commit_message(stack={:?}, commit={})",
            target.stack_id, target.commit_short_id
        ));

        let new_commit_oid = gitbutler_branch_actions::update_commit_message(
            &ctx,
            target.stack_id,
            git2_oid,
            &new_message,
        )?;

        self.command_log.push(format!(
            "Rebased stack '{}' with updated commit {}",
            target.branch_name,
            new_commit_oid.to_gix().to_hex_with_len(7)
        ));

        self.load_data_with_project(&project)?;
        self.update_main_view();
        Ok(true)
    }

    fn split_commit_message(message: &str) -> (String, String) {
        if let Some((subject, rest)) = message.split_once("\n\n") {
            (subject.to_string(), rest.to_string())
        } else {
            let mut lines = message.lines();
            let subject = lines.next().unwrap_or("").to_string();
            let body = lines.collect::<Vec<_>>().join("\n");
            (subject, body)
        }
    }

    fn compose_reword_message(subject: &str, body: &str) -> String {
        let subject = subject.trim_end();
        if body.trim().is_empty() {
            subject.to_string()
        } else {
            format!("{}\n\n{}", subject, body)
        }
    }
}
