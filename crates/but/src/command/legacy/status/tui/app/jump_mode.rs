use std::collections::BTreeSet;

use bstr::ByteSlice;
use crossterm::event::Event;
use ratatui::prelude::*;
use ratatui_textarea::{CursorMove, TextArea};

use crate::{
    CliId,
    command::legacy::status::{
        FilesStatusFlag, StatusOutputLine,
        output::StatusOutputContent,
        tui::{
            App, Backstack, Message, Mode, NormalMode,
            cursor::{self, Cursor},
            render::ModeRender,
        },
    },
    id::CommitId,
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
    /// Rows that will be selected immediately after typing their next hinted character.
    pub fn immediate_jump_targets(
        &self,
        lines: &[StatusOutputLine],
        show_files: FilesStatusFlag,
    ) -> Vec<bool> {
        let query = self.query();
        let mut next_chars = BTreeSet::new();
        for line in lines {
            if !prefix_match(query, line, &self.return_mode, show_files) {
                continue;
            }
            let mut hex_buf = gix::hash::Kind::hex_buf();
            let Some(id) = jump_id(line, &mut hex_buf) else {
                continue;
            };
            if let Some(next_char) = id.strip_prefix(query).and_then(|rest| rest.chars().next()) {
                next_chars.insert(next_char);
            }
        }

        // Resolve once per distinct key, not once per row. Include offscreen rows and use
        // the input resolver so exact IDs win over longer IDs with the same prefix.
        let mut targets = Vec::with_capacity(lines.len());
        targets.resize(lines.len(), false);
        let mut next_query = query.to_owned();
        for next_char in next_chars {
            next_query.push(next_char);
            if let Some(target) =
                find_line_by_jump_id(&next_query, lines, &self.return_mode, show_files)
                && let Some(index) = lines.iter().position(|line| std::ptr::eq(line, target))
            {
                targets[index] = true;
            }
            next_query.truncate(query.len());
        }
        targets
    }

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
        .filter(|line| prefix_match(query, line, return_mode, show_files_flag))
        .peekable();

    let needle = matches.next()?;
    if matches.peek().is_none() {
        return Some(needle);
    }
    // A worktree's `wt` is a strict prefix of its own area `wt:@`, so typing it can never
    // become unique; an ID typed out in full wins over the IDs extending it.
    std::iter::once(needle).chain(matches).find(|line| {
        line.data
            .cli_id()
            .is_some_and(|id| id.short_string() == query)
    })
}

pub fn prefix_match(
    query: &str,
    line: &StatusOutputLine,
    return_mode: &Mode,
    show_files_flag: FilesStatusFlag,
) -> bool {
    if !cursor::is_selectable_in_mode(line, return_mode.as_ref(), show_files_flag) {
        return false;
    }
    let mut buf = gix::hash::Kind::hex_buf();
    jump_id(line, &mut buf).is_some_and(|id| id.starts_with(query))
}

fn jump_id<'a>(line: &'a StatusOutputLine, hex_buf: &'a mut [u8]) -> Option<&'a str> {
    if let StatusOutputContent::MergeBase(merge_base) = &line.content {
        return Some(merge_base.commit_id.hex_to_buf(hex_buf));
    }

    let id = line.data.cli_id()?;
    match &**id {
        CliId::UncommittedHunkOrFile(hunk) => Some(&hunk.id),
        CliId::Commit {
            commit: CommitId {
                commit_id,
                change_id,
            },
            id,
        } => {
            if let Some(change_id) = change_id {
                Some(change_id.as_bytes().to_str().unwrap_or(id))
            } else {
                Some(commit_id.hex_to_buf(hex_buf))
            }
        }
        CliId::PathPrefix { id, .. }
        | CliId::CommittedFile { id, .. }
        | CliId::Uncommitted { id }
        | CliId::Worktree { id, .. }
        | CliId::WorktreeUncommitted { id, .. }
        | CliId::Stack { id, .. } => Some(id),
        CliId::Branch(branch) => Some(&branch.id),
        CliId::AnonymousSegment(segment) => Some(&segment.id),
        CliId::CommittedHunk(..) => None,
    }
}

fn cursor_for_jump_line(line: &StatusOutputLine, lines: &[StatusOutputLine]) -> Option<Cursor> {
    if matches!(line.content, StatusOutputContent::MergeBase(..)) {
        Cursor::select_merge_base(lines)
    } else {
        line.data.cli_id().and_then(|id| Cursor::restore(id, lines))
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
        let previous_mode = match &*self.mode {
            Mode::Details(..) => return,
            mode @ (Mode::Normal(..)
            | Mode::Squash(..)
            | Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Commit(..)
            | Mode::Move(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..)
            | Mode::PickChanges(..)
            | Mode::CherryPick(..)
            | Mode::Branch(..)
            | Mode::Worktree(..)
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
        ) && let Some(new_cursor) = cursor_for_jump_line(line, &self.status_lines)
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
                .and_then(|line| cursor_for_jump_line(line, lines))
        })
}
