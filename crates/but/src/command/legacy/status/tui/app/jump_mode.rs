use bstr::ByteSlice;
use crossterm::event::Event;
use ratatui::prelude::*;
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    CliId,
    command::legacy::status::{
        FilesStatusFlag, StatusOutputLine,
        tui::{
            App, Backstack, Message, Mode, NormalMode,
            cursor::{self, Cursor},
            render::ModeRender,
        },
    },
};

#[derive(Debug, Clone)]
pub struct JumpMode {
    pub textarea: Box<TextArea<'static>>,
    pub return_mode: Box<Mode>,
    pub return_backstack: Backstack,
}

impl ModeRender for JumpMode {
    fn render_hot_bar_content(&self, _app: &App, area: Rect, frame: &mut Frame) {
        let jump_layout =
            Layout::horizontal([Constraint::Length(2), Constraint::Min(1)]).split(area);

        frame.render_widget("/ ", jump_layout[0]);
        frame.render_widget(&*self.textarea, jump_layout[1]);
    }
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

#[derive(Debug)]
pub enum JumpMessage {
    Enter,
    Input(Event),
    Previous,
    Next,
    Confirm,
}

fn find_line_by_jump_id<'a>(
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
        && jump_id_has_prefix(id, query)
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
    if !cursor::is_selectable_in_mode(line, return_mode.as_ref(), show_files_flag) {
        return false;
    }
    if query.is_empty() {
        true
    } else {
        jump_id_has_prefix(id, query)
    }
}

fn jump_id_has_prefix(id: &CliId, query: &str) -> bool {
    match id {
        CliId::UncommittedHunkOrFile(hunk) => hunk.id.starts_with(query),
        CliId::Commit {
            commit_id,
            id,
            change_id,
        } => {
            if let Some(change_id) = change_id {
                change_id
                    .as_bytes()
                    .to_str()
                    .unwrap_or(id)
                    .starts_with(query)
            } else {
                let mut buf = gix::hash::Kind::hex_buf();
                commit_id.hex_to_buf(&mut buf).starts_with(query)
            }
        }
        CliId::PathPrefix { id, .. }
        | CliId::CommittedFile { id, .. }
        | CliId::Branch { id, .. }
        | CliId::Uncommitted { id }
        | CliId::Stack { id, .. } => id.starts_with(query),
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
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        else {
            return;
        };

        mode.textarea.input(ev);

        if let Some(line) = find_line_by_jump_id(
            mode.query(),
            &self.status_lines,
            &mode.return_mode,
            self.flags.show_files,
        ) && let Some(data) = line.data.cli_id()
            && let Some(new_cursor) = cursor::Cursor::restore(data, &self.status_lines)
        {
            self.cursor = new_cursor;

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

        let new_cursor =
            find_jump_match(self.cursor, &self.status_lines, mode, self.flags.show_files);

        if let Some(new_cursor) = new_cursor {
            self.cursor = new_cursor;

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
        }
    }
}

pub fn find_jump_match(
    cursor: Cursor,
    lines: &[StatusOutputLine],
    mode: &JumpMode,
    show_files: FilesStatusFlag,
) -> Option<Cursor> {
    cursor
        .selected_line(lines)
        .filter(|line| prefix_match(mode.query(), line, &mode.return_mode, show_files))
        .map(|_| cursor)
        .or_else(|| {
            lines
                .iter()
                .find(|line| prefix_match(mode.query(), line, &mode.return_mode, show_files))
                .and_then(|line| line.data.cli_id())
                .and_then(|data| cursor::Cursor::restore(data, lines))
        })
}
