use anyhow::Result;

use super::app::{LazyApp, OplogEntry};

impl LazyApp {
    pub(super) fn open_restore_modal(&mut self) {
        if let Some(entry) = self.get_selected_oplog_entry().cloned() {
            self.restore_target = Some(entry);
            self.show_restore_modal = true;
        } else {
            self.command_log
                .push("Select an oplog entry before attempting to restore".to_string());
        }
    }

    pub(super) fn cancel_restore_modal(&mut self) {
        self.show_restore_modal = false;
        self.restore_target = None;
        self.command_log.push("Canceled oplog restore".to_string());
    }

    pub(super) fn confirm_restore_modal(&mut self) {
        if let Some(entry) = self.restore_target.clone() {
            if let Err(err) = self.restore_workspace_to(&entry) {
                self.command_log.push(format!("Restore failed: {}", err));
            }
        }
        self.restore_target = None;
        self.show_restore_modal = false;
    }

    fn restore_workspace_to(&mut self, entry: &OplogEntry) -> Result<()> {
        self.command_log.push(format!(
            "Restoring workspace to snapshot {} ({})",
            entry.id, entry.title
        ));

        self.command_log
            .push("but_api::legacy::oplog::restore_snapshot()".to_string());
        but_api::legacy::oplog::restore_snapshot(self.project_id, entry.full_id.clone())?;

        let project = gitbutler_project::get(self.project_id)?;
        self.load_data_with_project(&project)?;
        self.update_main_view();

        self.command_log
            .push(format!("Workspace restored to snapshot {}", entry.id));
        Ok(())
    }
}
