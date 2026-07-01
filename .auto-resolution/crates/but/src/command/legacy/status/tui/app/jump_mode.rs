use crossterm::event::Event;
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    CliId,
    command::legacy::status::{
        FilesStatusFlag, StatusOutputLine,
        tui::{App, Backstack, Message, Mode, NormalMode, cursor},
    },
};

#[derive(Debug, Clone)]
pub struct JumpMode {
    pub textarea: Box<TextArea<'static>>,
    pub return_mode: Box<Mode>,
    pub return_backstack: Backstack,
}

impl JumpMode {
    pub fn query(&self) -> &str {
        self.textarea
            .lines()
            .first()
            .map(|s| &**s)
            .unwrap_or_default()
            .trim()
    }
}

#[derive(Debug, Clone)]
pub enum JumpMessage {
    Enter,
    Input(Event),
    Previous,
    Next,
    Confirm,
}

fn find_line_by_short_id<'a>(
    query: &str,
    lines: &'a [StatusOutputLine],
    return_mode: &Mode,
    show_files_flag: FilesStatusFlag,
) -> Option<&'a StatusOutputLine> {
    if query.is_empty() {
        return None;
    }

    let mut matches = lines
        .iter()
        .filter(|line| prefix_match(query, line, return_mode, show_files_flag));

    let needle = matches.next()?;

    if matches.next().is_none()
        && let Some(id) = needle.data.cli_id()
        && short_id(id) == query
    {
        Some(needle)
    } else {
        None
    }
}

pub fn prefix_match(
    query: &str,
    line: &StatusOutputLine,
    return_mode: &Mode,
    show_files_flag: FilesStatusFlag,
) -> bool {
    let Some(id) = line.data.cli_id() else {
        return false;
    };
    if !cursor::is_selectable_in_mode(line, return_mode, show_files_flag) {
        return false;
    }
    if query.is_empty() {
        true
    } else {
        short_id(id).starts_with(query)
    }
}

fn short_id(id: &CliId) -> &str {
    match id {
        CliId::UncommittedHunkOrFile(hunk) => &hunk.id,
        CliId::PathPrefix { id, .. }
        | CliId::CommittedFile { id, .. }
        | CliId::Branch { id, .. }
        | CliId::Commit { id, .. }
        | CliId::Uncommitted { id }
        | CliId::Stack { id, .. } => id,
    }
}

impl App {
    pub fn handle_jump(&mut self, message: JumpMessage, messages: &mut Vec<Message>) {
        match message {
            JumpMessage::Enter => self.handle_jump_enter(),
            JumpMessage::Input(event) => self.handle_jump_input(event, messages),
            JumpMessage::Confirm => self.handle_jump_confirm(messages),
            JumpMessage::Previous => self.handle_jump_previous(),
            JumpMessage::Next => self.handle_jump_next(),
        }
    }

    pub fn restore_mode_before_jump(&mut self) -> bool {
        self.mode.update(&mut self.backstack, |backstack, mode| {
            let previous_mode = std::mem::replace(mode, Mode::Normal(NormalMode::default()));
            let Mode::Jump(jump_mode) = previous_mode else {
                *mode = previous_mode;
                return false;
            };

            *mode = *jump_mode.return_mode;
            *backstack = jump_mode.return_backstack;

            true
        })
    }

    fn handle_jump_enter(&mut self) {
        // TODO(david): dont enter if commit file list is open

        match self.flags.show_files {
            FilesStatusFlag::None | FilesStatusFlag::All => {}
            FilesStatusFlag::Commit(..) => return,
        }

        let previous_mode = match &*self.mode {
            Mode::Details(..) => return,
            mode @ (Mode::Normal(..)
            | Mode::Rub(..)
            | Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Commit(..)
            | Mode::Move(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..)
            | Mode::PickChanges(..)
            | Mode::Jump(..)) => mode.clone(),
        };
        let backstack = self.backstack.clone();

        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(self.theme.default);
        textarea.move_cursor(CursorMove::End);

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                *mode = Mode::Jump(JumpMode {
                    textarea: Box::new(textarea),
                    return_mode: Box::new(previous_mode),
                    return_backstack: backstack,
                });
            });
    }

    fn handle_jump_input(&mut self, ev: Event, _messages: &mut Vec<Message>) {
        let Mode::Jump(mode) = self
            .mode
            .get_mut_without_updating_backstack_and_i_promise_not_to_change_state()
        else {
            return;
        };

        mode.textarea.input(ev);

        if let Some(line) = find_line_by_short_id(
            mode.query(),
            &self.status_lines,
            &mode.return_mode,
            self.flags.show_files,
        ) && let Some(data) = line.data.cli_id()
            && let Some(new_cursor) = cursor::Cursor::restore(data, &self.status_lines)
        {
            self.cursor = new_cursor;

            self.details.mark_dirty();

            let return_mode = mode.return_mode.clone();
            let return_backstack = mode.return_backstack.clone();

            self.mode.update(&mut self.backstack, |backstack, mode| {
                *mode = *return_mode;
                *backstack = return_backstack;
            });
        }
    }

    fn handle_jump_confirm(&mut self, _messages: &mut Vec<Message>) {
        let Mode::Jump(mode) = &*self.mode else {
            return;
        };

        let new_cursor = self
            .cursor
            .selected_line(&self.status_lines)
            .filter(|line| {
                prefix_match(mode.query(), line, &mode.return_mode, self.flags.show_files)
            })
            .map(|_| self.cursor)
            .or_else(|| {
                self.status_lines
                    .iter()
                    .find(|line| {
                        prefix_match(mode.query(), line, &mode.return_mode, self.flags.show_files)
                    })
                    .and_then(|line| line.data.cli_id())
                    .and_then(|data| cursor::Cursor::restore(data, &self.status_lines))
            });

        if let Some(new_cursor) = new_cursor {
            self.cursor = new_cursor;
            self.details.mark_dirty();

            let return_mode = mode.return_mode.clone();
            let return_backstack = mode.return_backstack.clone();

            self.mode.update(&mut self.backstack, |backstack, mode| {
                *mode = *return_mode;
                *backstack = return_backstack;
            });
        }
    }

    fn handle_jump_next(&mut self) {
        let Mode::Jump(_) = &*self.mode else {
            return;
        };

        if let Some(new_cursor) =
            self.cursor
                .move_down(&self.status_lines, &self.mode, self.flags.show_files)
        {
            self.cursor = new_cursor;
            self.details.mark_dirty();
        }
    }

    fn handle_jump_previous(&mut self) {
        let Mode::Jump(_) = &*self.mode else {
            return;
        };

        if let Some(new_cursor) =
            self.cursor
                .move_up(&self.status_lines, &self.mode, self.flags.show_files)
        {
            self.cursor = new_cursor;
            self.details.mark_dirty();
        }
    }
}
