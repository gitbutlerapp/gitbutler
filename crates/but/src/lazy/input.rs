use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind};

use super::app::{CommitModalFocus, LazyApp, Panel, RewordModalFocus};

impl LazyApp {
    pub(super) fn handle_input(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        if self.show_absorb_modal {
            self.handle_absorb_modal_input(key, modifiers);
            return;
        }

        if self.show_squash_modal {
            self.handle_squash_modal_input(key, modifiers);
            return;
        }

        if self.show_branch_rename_modal {
            self.handle_branch_rename_modal_input(key, modifiers);
            return;
        }

        if self.show_diff_modal {
            self.handle_diff_modal_input(key, modifiers);
            return;
        }

        if self.show_uncommit_modal {
            self.handle_uncommit_modal_input(key, modifiers);
            return;
        }

        if self.show_reword_modal {
            self.handle_reword_modal_input(key, modifiers);
            return;
        }

        if self.show_commit_modal {
            self.handle_commit_modal_input(key, modifiers);
            return;
        }

        if self.show_update_modal {
            self.handle_update_modal_input(key);
            return;
        }

        if self.show_restore_modal {
            self.handle_restore_modal_input(key);
            return;
        }

        if self.show_help {
            match key {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                    self.show_help = false;
                    self.help_scroll = 0;
                    self.command_log.push("Closed help".to_string());
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = self.help_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1);
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
                self.command_log.push("Quit requested".to_string());
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                self.command_log.push("Interrupted (Ctrl+C)".to_string());
            }
            KeyCode::Tab => {
                self.next_panel();
                self.command_log
                    .push(format!("Switched to {:?}", self.active_panel_name()));
            }
            KeyCode::BackTab => {
                self.prev_panel();
                self.command_log
                    .push(format!("Switched to {:?}", self.active_panel_name()));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next();
                self.update_main_view();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_prev();
                self.update_main_view();
            }
            KeyCode::Char('1') => {
                if self.upstream_info.is_some() {
                    self.active_panel = Panel::Upstream;
                    self.command_log.push("Switched to Upstream".to_string());
                    self.update_main_view();
                }
            }
            KeyCode::Char('2') => {
                self.active_panel = Panel::Status;
                self.command_log.push("Switched to Status".to_string());
                self.update_main_view();
            }
            KeyCode::Char('3') => {
                self.active_panel = Panel::Oplog;
                self.command_log.push("Switched to Oplog".to_string());
                self.update_main_view();
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                self.help_scroll = 0;
                self.command_log.push("Opened help".to_string());
            }
            KeyCode::Char('r') => {
                if matches!(self.active_panel, Panel::Oplog)
                    && self.oplog_state.selected().is_some()
                {
                    self.open_restore_modal();
                } else if let Err(e) = self.refresh() {
                    self.command_log.push(format!("Refresh failed: {}", e));
                }
            }
            KeyCode::Char('@') => {
                self.command_log_visible = !self.command_log_visible;
                if self.command_log_visible {
                    self.command_log.push("Command log shown".to_string());
                } else {
                    self.command_log.push("Command log hidden".to_string());
                }
            }
            KeyCode::Char('f') => {
                if matches!(self.active_panel, Panel::Upstream) {
                    if let Err(e) = self.fetch_upstream() {
                        self.command_log.push(format!("Fetch failed: {}", e));
                    }
                } else if matches!(self.active_panel, Panel::Status) {
                    self.open_diff_modal();
                }
            }
            KeyCode::Char('c') => {
                if matches!(self.active_panel, Panel::Status) {
                    self.open_commit_modal();
                }
            }
            KeyCode::Char('e') => {
                if matches!(self.active_panel, Panel::Status) {
                    if self.get_selected_commit().is_some() {
                        self.open_reword_modal();
                    } else if self.get_selected_branch().is_some() {
                        self.open_branch_rename_modal();
                    }
                }
            }
            KeyCode::Char('a') => {
                if matches!(self.active_panel, Panel::Status) {
                    self.open_absorb_modal();
                }
            }
            KeyCode::Char('s') => {
                if matches!(self.active_panel, Panel::Status) {
                    self.open_squash_modal();
                }
            }
            KeyCode::Char('u') => {
                if matches!(self.active_panel, Panel::Status) {
                    self.open_uncommit_modal();
                } else if matches!(self.active_panel, Panel::Upstream) {
                    self.open_upstream_update_modal();
                }
            }
            KeyCode::Char('d') => {
                self.details_selected = !self.details_selected;
                if self.details_selected {
                    if !matches!(self.active_panel, Panel::Status) {
                        self.active_panel = Panel::Status;
                        self.command_log.push("Switched to Status".to_string());
                    }
                    self.command_log.push("Details pane selected".to_string());
                } else {
                    if !matches!(self.active_panel, Panel::Status) {
                        self.active_panel = Panel::Status;
                        self.command_log.push("Switched to Status".to_string());
                    }
                    self.command_log.push("Details pane deselected".to_string());
                }
            }
            KeyCode::Char('l') => {
                if self.details_selected {
                    self.details_scroll = self.details_scroll.saturating_add(1);
                } else if matches!(self.active_panel, Panel::Status) {
                    if self.select_prev_branch() {
                        self.update_main_view();
                    }
                }
            }
            KeyCode::Char('h') => {
                if self.details_selected {
                    self.details_scroll = self.details_scroll.saturating_sub(1);
                } else if matches!(self.active_panel, Panel::Status) {
                    if self.select_next_branch() {
                        self.update_main_view();
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) {
        if mouse.kind != MouseEventKind::Down(crossterm::event::MouseButton::Left) {
            return;
        }

        let col = mouse.column;
        let row = mouse.row;

        if let Some(area) = self.status_area {
            if col >= area.x
                && col < area.x + area.width
                && row >= area.y
                && row < area.y + area.height
            {
                self.active_panel = Panel::Status;

                let relative_row = row.saturating_sub(area.y + 1);
                let total_items = self.count_status_items();

                if (relative_row as usize) < total_items {
                    self.status_state.select(Some(relative_row as usize));
                    self.update_main_view();
                }
                return;
            }
        }

        if let Some(area) = self.upstream_area {
            if col >= area.x
                && col < area.x + area.width
                && row >= area.y
                && row < area.y + area.height
            {
                self.active_panel = Panel::Upstream;
                self.update_main_view();
                return;
            }
        }

        if let Some(area) = self.oplog_area {
            if col >= area.x
                && col < area.x + area.width
                && row >= area.y
                && row < area.y + area.height
            {
                self.active_panel = Panel::Oplog;

                let relative_row = row.saturating_sub(area.y + 1);

                if (relative_row as usize) < self.oplog_entries.len() {
                    self.oplog_state.select(Some(relative_row as usize));
                    self.update_main_view();
                }
                return;
            }
        }

        if let Some(area) = self.details_area {
            if col >= area.x
                && col < area.x + area.width
                && row >= area.y
                && row < area.y + area.height
            {
                self.details_selected = true;
                return;
            }
        }
    }

    fn handle_commit_modal_input(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc => {
                self.cancel_commit_modal();
            }
            KeyCode::Char('[') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.cancel_commit_modal();
            }
            KeyCode::Tab => {
                self.commit_modal_focus = match self.commit_modal_focus {
                    CommitModalFocus::BranchSelect => CommitModalFocus::Files,
                    CommitModalFocus::Files => CommitModalFocus::Subject,
                    CommitModalFocus::NewBranchName => CommitModalFocus::Subject,
                    CommitModalFocus::Subject => CommitModalFocus::Message,
                    CommitModalFocus::Message => CommitModalFocus::BranchSelect,
                };
            }
            KeyCode::Up | KeyCode::Char('k')
                if matches!(self.commit_modal_focus, CommitModalFocus::BranchSelect) =>
            {
                if self.commit_selected_branch_idx > 0 {
                    self.commit_selected_branch_idx -= 1;
                    self.rebuild_commit_file_list();
                    self.commit_selected_file_idx = 0;
                }
            }
            KeyCode::Down | KeyCode::Char('j')
                if matches!(self.commit_modal_focus, CommitModalFocus::BranchSelect) =>
            {
                if self.commit_selected_branch_idx < self.commit_branch_options.len() - 1 {
                    self.commit_selected_branch_idx += 1;
                    self.rebuild_commit_file_list();
                    self.commit_selected_file_idx = 0;
                }
            }
            KeyCode::Up | KeyCode::Char('k')
                if matches!(self.commit_modal_focus, CommitModalFocus::Files) =>
            {
                self.move_commit_file_cursor(-1);
            }
            KeyCode::Down | KeyCode::Char('j')
                if matches!(self.commit_modal_focus, CommitModalFocus::Files) =>
            {
                self.move_commit_file_cursor(1);
            }
            KeyCode::Char(' ') if matches!(self.commit_modal_focus, CommitModalFocus::Files) => {
                self.toggle_current_commit_file();
            }
            KeyCode::Char('o') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.commit_only_mode = !self.commit_only_mode;
                self.rebuild_commit_file_list();
                self.commit_selected_file_idx = 0;
            }
            KeyCode::Char('m') | KeyCode::Char('M')
                if modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.submit_commit_modal();
            }
            KeyCode::Char(c) => match self.commit_modal_focus {
                CommitModalFocus::Subject => self.commit_subject.push(c),
                CommitModalFocus::Message => self.commit_message.push(c),
                _ => {}
            },
            KeyCode::Backspace => match self.commit_modal_focus {
                CommitModalFocus::Subject => {
                    self.commit_subject.pop();
                }
                CommitModalFocus::Message => {
                    self.commit_message.pop();
                }
                _ => {}
            },
            KeyCode::Enter => {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    self.submit_commit_modal();
                } else if matches!(self.commit_modal_focus, CommitModalFocus::Message) {
                    self.commit_message.push('\n');
                }
            }
            _ => {}
        }
    }

    fn handle_update_modal_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Char('n') => {
                self.show_update_modal = false;
                self.command_log
                    .push("Canceled upstream update".to_string());
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                if let Err(e) = self.perform_upstream_update() {
                    self.command_log.push(format!("Update failed: {}", e));
                }
                self.show_update_modal = false;
            }
            _ => {}
        }
    }

    fn handle_restore_modal_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc | KeyCode::Char('n') => self.cancel_restore_modal(),
            KeyCode::Enter | KeyCode::Char('y') => self.confirm_restore_modal(),
            _ => {}
        }
    }

    fn handle_reword_modal_input(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc => self.cancel_reword_modal(),
            KeyCode::Char('[') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.cancel_reword_modal();
            }
            KeyCode::Tab => {
                self.reword_modal_focus = match self.reword_modal_focus {
                    RewordModalFocus::Subject => RewordModalFocus::Message,
                    RewordModalFocus::Message => RewordModalFocus::Subject,
                };
            }
            KeyCode::Char('m') | KeyCode::Char('M')
                if modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.submit_reword_modal();
            }
            KeyCode::Char(c) => match self.reword_modal_focus {
                RewordModalFocus::Subject => self.reword_subject.push(c),
                RewordModalFocus::Message => self.reword_message.push(c),
            },
            KeyCode::Backspace => match self.reword_modal_focus {
                RewordModalFocus::Subject => {
                    self.reword_subject.pop();
                }
                RewordModalFocus::Message => {
                    self.reword_message.pop();
                }
            },
            KeyCode::Enter => {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    self.submit_reword_modal();
                } else if matches!(self.reword_modal_focus, RewordModalFocus::Message) {
                    self.reword_message.push('\n');
                }
            }
            _ => {}
        }
    }

    fn handle_uncommit_modal_input(&mut self, key: KeyCode, _modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc | KeyCode::Char('n') => self.cancel_uncommit_modal(),
            KeyCode::Enter | KeyCode::Char('y') => self.confirm_uncommit_modal(),
            _ => {}
        }
    }

    fn handle_diff_modal_input(&mut self, key: KeyCode, _modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc | KeyCode::Char('q') => self.close_diff_modal(),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_diff_modal(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_diff_modal(-1),
            KeyCode::Char('h') | KeyCode::Left => self.select_prev_diff_file(),
            KeyCode::Char('l') | KeyCode::Right => self.select_next_diff_file(),
            KeyCode::Char(']') => self.jump_diff_hunk_forward(),
            KeyCode::Char('[') => self.jump_diff_hunk_backward(),
            _ => {}
        }
    }

    fn handle_branch_rename_modal_input(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc => self.cancel_branch_rename_modal(),
            KeyCode::Enter => self.submit_branch_rename_modal(),
            KeyCode::Char('m') | KeyCode::Char('M')
                if modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.submit_branch_rename_modal();
            }
            KeyCode::Char(c) => self.branch_rename_input.push(c),
            KeyCode::Backspace => {
                self.branch_rename_input.pop();
            }
            _ => {}
        }
    }

    fn handle_squash_modal_input(&mut self, key: KeyCode, _modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc | KeyCode::Char('n') => self.cancel_squash_modal(),
            KeyCode::Enter | KeyCode::Char('y') => self.confirm_squash_modal(),
            _ => {}
        }
    }

    fn handle_absorb_modal_input(&mut self, key: KeyCode, _modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc | KeyCode::Char('n') => self.cancel_absorb_modal(),
            KeyCode::Enter | KeyCode::Char('y') => self.confirm_absorb_modal(),
            _ => {}
        }
    }
}
