use but_ctx::Context;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::app::{App, CommitModalFocus, Panel};

impl App {
    pub(super) fn handle_key(&mut self, key: KeyEvent, ctx: &mut Context) {
        // Modal takes priority
        if self.show_commit_modal {
            self.handle_commit_modal_key(key, ctx);
            return;
        }

        // Help overlay takes priority
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                KeyCode::Char('j') | KeyCode::Down => self.help_scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => {
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                _ => {}
            }
            return;
        }

        match key.code {
            // Quit
            KeyCode::Char('q')
            | KeyCode::Char('c')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.should_quit = true;
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }

            // Help
            KeyCode::Char('?') => {
                self.show_help = true;
                self.help_scroll = 0;
            }

            // Panel switching
            KeyCode::Tab => self.next_panel(),
            KeyCode::BackTab => self.prev_panel(),

            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                if self.details_selected {
                    self.details_scroll += 1;
                } else {
                    self.select_next();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.details_selected {
                    self.details_scroll = self.details_scroll.saturating_sub(1);
                } else {
                    self.select_prev();
                }
            }

            // Details pane focus
            KeyCode::Char('l') | KeyCode::Right => {
                if !self.details_selected {
                    self.details_selected = true;
                    self.details_scroll = 0;
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.details_selected {
                    self.details_selected = false;
                }
            }
            KeyCode::Esc => {
                if self.details_selected {
                    self.details_selected = false;
                }
            }

            // Commit
            KeyCode::Char('c') => {
                if self.has_uncommitted_changes() {
                    self.open_commit_modal(ctx);
                }
            }

            // Refresh
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.refresh(ctx);
            }

            // Fetch
            KeyCode::Char('f') => {
                self.fetch_upstream(ctx);
            }

            // Toggle command log
            KeyCode::Char('~') => {
                self.command_log_visible = !self.command_log_visible;
            }

            _ => {}
        }
    }

    fn handle_commit_modal_key(&mut self, key: KeyEvent, ctx: &mut Context) {
        match key.code {
            KeyCode::Esc => {
                self.cancel_commit_modal();
            }
            // Submit with Ctrl+Enter (from anywhere) or Enter on commit button
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.submit_commit(ctx);
            }
            // Tab cycles focus: Branch -> Files -> Subject -> Message -> CommitButton -> Branch
            KeyCode::Tab => {
                self.commit_focus = match self.commit_focus {
                    CommitModalFocus::BranchSelect => CommitModalFocus::Files,
                    CommitModalFocus::Files => CommitModalFocus::Subject,
                    CommitModalFocus::Subject => CommitModalFocus::Message,
                    CommitModalFocus::Message => CommitModalFocus::CommitButton,
                    CommitModalFocus::CommitButton => CommitModalFocus::BranchSelect,
                };
            }
            KeyCode::BackTab => {
                self.commit_focus = match self.commit_focus {
                    CommitModalFocus::BranchSelect => CommitModalFocus::CommitButton,
                    CommitModalFocus::Files => CommitModalFocus::BranchSelect,
                    CommitModalFocus::Subject => CommitModalFocus::Files,
                    CommitModalFocus::Message => CommitModalFocus::Subject,
                    CommitModalFocus::CommitButton => CommitModalFocus::Message,
                };
            }
            _ => match self.commit_focus {
                CommitModalFocus::BranchSelect => match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if self.commit_selected_branch + 1 < self.commit_branch_options.len() {
                            self.commit_selected_branch += 1;
                            self.rebuild_commit_files();
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.commit_selected_branch > 0 {
                            self.commit_selected_branch -= 1;
                            self.rebuild_commit_files();
                        }
                    }
                    // Toggle staged-only with 's'
                    KeyCode::Char('s') => {
                        self.commit_staged_only = !self.commit_staged_only;
                        self.rebuild_commit_files();
                    }
                    _ => {}
                },
                CommitModalFocus::Files => match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if !self.commit_files.is_empty() {
                            self.commit_file_cursor =
                                (self.commit_file_cursor + 1) % self.commit_files.len();
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if !self.commit_files.is_empty() {
                            if self.commit_file_cursor == 0 {
                                self.commit_file_cursor = self.commit_files.len() - 1;
                            } else {
                                self.commit_file_cursor -= 1;
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        self.toggle_commit_file();
                    }
                    // Toggle staged-only with 's'
                    KeyCode::Char('s') => {
                        self.commit_staged_only = !self.commit_staged_only;
                        self.rebuild_commit_files();
                    }
                    _ => {}
                },
                CommitModalFocus::Subject => match key.code {
                    KeyCode::Char(c) => self.commit_subject.push(c),
                    KeyCode::Backspace => {
                        self.commit_subject.pop();
                    }
                    KeyCode::Enter => {
                        // Move to message body on Enter
                        self.commit_focus = CommitModalFocus::Message;
                    }
                    _ => {}
                },
                CommitModalFocus::Message => match key.code {
                    KeyCode::Char(c) => self.commit_message.push(c),
                    KeyCode::Backspace => {
                        self.commit_message.pop();
                    }
                    KeyCode::Enter => {
                        self.commit_message.push('\n');
                    }
                    _ => {}
                },
                CommitModalFocus::CommitButton => match key.code {
                    KeyCode::Enter => {
                        self.submit_commit(ctx);
                    }
                    _ => {}
                },
            },
        }
    }

    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let x = mouse.column;
                let y = mouse.row;

                // Check which panel was clicked
                if let Some(area) = self.upstream_area {
                    if x >= area.x
                        && x < area.x + area.width
                        && y >= area.y
                        && y < area.y + area.height
                    {
                        self.active_panel = Panel::Upstream;
                        self.details_selected = false;
                        return;
                    }
                }
                if let Some(area) = self.status_area {
                    if x >= area.x
                        && x < area.x + area.width
                        && y >= area.y
                        && y < area.y + area.height
                    {
                        self.active_panel = Panel::Status;
                        self.details_selected = false;
                        // Try to select the item at the clicked row
                        let row_in_panel = (y - area.y).saturating_sub(1) as usize;
                        let offset = self.status_state.offset();
                        let target = offset + row_in_panel;
                        let total = self.count_status_items();
                        if target < total && !self.is_separator(target) {
                            self.status_state.select(Some(target));
                            self.details_scroll = 0;
                        }
                        return;
                    }
                }
                if let Some(area) = self.oplog_area {
                    if x >= area.x
                        && x < area.x + area.width
                        && y >= area.y
                        && y < area.y + area.height
                    {
                        self.active_panel = Panel::Oplog;
                        self.details_selected = false;
                        let row_in_panel = (y - area.y).saturating_sub(1) as usize;
                        let offset = self.oplog_state.offset();
                        let target = offset + row_in_panel;
                        if target < self.oplog_entries.len() {
                            self.oplog_state.select(Some(target));
                        }
                        return;
                    }
                }
                if let Some(area) = self.details_area {
                    if x >= area.x
                        && x < area.x + area.width
                        && y >= area.y
                        && y < area.y + area.height
                    {
                        self.details_selected = true;
                        return;
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if self.details_selected {
                    self.details_scroll += 3;
                } else {
                    self.select_next();
                }
            }
            MouseEventKind::ScrollUp => {
                if self.details_selected {
                    self.details_scroll = self.details_scroll.saturating_sub(3);
                } else {
                    self.select_prev();
                }
            }
            _ => {}
        }
    }
}
