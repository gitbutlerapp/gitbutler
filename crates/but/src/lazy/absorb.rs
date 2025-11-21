use anyhow::Result;

use super::app::{LazyApp, Panel};
use crate::{
    absorb,
    utils::{OutputChannel, OutputFormat},
};

impl LazyApp {
    pub(super) fn open_absorb_modal(&mut self) {
        if !matches!(self.active_panel, Panel::Status) {
            self.command_log
                .push("Switch to the Status panel to absorb changes".to_string());
            return;
        }

        if !self.is_unassigned_header_selected() {
            self.command_log
                .push("Select 'Unassigned Files' to absorb all pending changes".to_string());
            return;
        }

        if self.unassigned_files.is_empty() {
            self.command_log
                .push("No unassigned changes available to absorb".to_string());
            return;
        }

        let (summary, _) = self.summarize_unassigned_files();
        if summary.file_count == 0 {
            self.command_log
                .push("No unassigned changes available to absorb".to_string());
            return;
        }

        self.absorb_summary = Some(summary.clone());
        self.show_absorb_modal = true;
        self.command_log.push(format!(
            "Preparing to absorb {} unassigned file(s)",
            summary.file_count
        ));
    }

    pub(super) fn cancel_absorb_modal(&mut self) {
        self.reset_absorb_modal_state();
        self.command_log.push("Canceled absorb".to_string());
    }

    pub(super) fn confirm_absorb_modal(&mut self) {
        match self.perform_absorb() {
            Ok(true) => self.reset_absorb_modal_state(),
            Ok(false) => {}
            Err(e) => self.command_log.push(format!("Failed to absorb: {}", e)),
        }
    }

    fn perform_absorb(&mut self) -> Result<bool> {
        if self.unassigned_files.is_empty() {
            self.command_log
                .push("No unassigned changes to absorb".to_string());
            return Ok(false);
        }

        let project = gitbutler_project::get(self.project_id)?;
        let mut out = OutputChannel::new_without_pager_non_json(OutputFormat::None);
        self.command_log
            .push("Running absorb on unassigned changes".to_string());
        absorb::handle(&project, &mut out, None)?;

        if let Some(summary) = self.absorb_summary.clone() {
            self.command_log.push(format!(
                "Absorbed {} file(s) (+{} / -{})",
                summary.file_count, summary.total_additions, summary.total_removals
            ));
        } else {
            self.command_log
                .push("Absorbed unassigned changes".to_string());
        }

        self.load_data_with_project(&project)?;
        self.update_main_view();
        Ok(true)
    }

    fn reset_absorb_modal_state(&mut self) {
        self.show_absorb_modal = false;
        self.absorb_summary = None;
    }
}
