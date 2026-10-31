use std::sync::LazyLock;

use crossterm::event::Event;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use ratatui::prelude::*;
use ratatui_textarea::{CursorMove, TextArea};

use crate::command::legacy::status::{
    FilesStatusFlag, StatusOutputLine,
    output::{
        BranchLineContent, CommitLineContent, FileLineContent, MergeBaseLineContent,
        StatusOutputContent, UncommittedLineContent,
    },
    tui::{
        App, Backstack, Message, Mode, NormalMode,
        app::MoveCursorDiration,
        cursor::{self, Cursor},
        render::ModeRender,
    },
};

#[derive(Debug, Clone)]
pub struct JumpMode {
    pub textarea: Box<TextArea<'static>>,
    pub return_mode: Box<Mode>,
    pub return_backstack: Backstack,
    /// Where cancelling puts the cursor back.
    pub return_cursor: Cursor,
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
    /// Where the query fuzzy matches `line`, or `None` if the line can't be jumped to.
    ///
    /// An empty query matches every line that can be jumped to.
    pub fn match_line(
        &self,
        line: &StatusOutputLine,
        show_files: FilesStatusFlag,
    ) -> Option<JumpMatch> {
        if !cursor::is_selectable_in_mode(line, Mode::as_ref(&self.return_mode), show_files) {
            return None;
        }
        let (id, text) = jump_parts(&line.content)?;
        let query = self.query();
        let id_match = fuzzy_match(id, query);
        let text_match = fuzzy_match(text, query);
        // The ID wins ties so typing an ID highlights it rather than similar text.
        let (part, char_indices) = match (id_match, text_match) {
            (Some((id_score, _)), Some((text_score, char_indices))) if text_score > id_score => {
                (JumpPart::Text, char_indices)
            }
            (Some((_, char_indices)), _) => (JumpPart::Id, char_indices),
            (None, Some((_, char_indices))) => (JumpPart::Text, char_indices),
            (None, None) => return None,
        };
        Some(JumpMatch { part, char_indices })
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

/// Where a jump query matched within a status line.
#[derive(Debug)]
pub struct JumpMatch {
    part: JumpPart,
    char_indices: Vec<usize>,
}

impl JumpMatch {
    /// The matched character indices into the text of `part`, empty if the query matched the
    /// line's other part.
    pub fn char_indices(&self, part: JumpPart) -> &[usize] {
        if self.part == part {
            &self.char_indices
        } else {
            &[]
        }
    }
}

/// The parts of a status line that a jump query is matched against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpPart {
    /// The short ID, such as `g0` for a branch.
    Id,
    /// The text describing the line, such as a branch name, commit message or file path.
    Text,
}

/// The spans showing the [`JumpPart::Id`] and [`JumpPart::Text`] of a line, or `None` if the
/// line has nothing to jump to.
fn jump_parts(content: &StatusOutputContent) -> Option<(&[Span<'static>], &[Span<'static>])> {
    match content {
        StatusOutputContent::Plain(..) => None,
        StatusOutputContent::MergeBase(MergeBaseLineContent { id, suffix: _ }) => Some((id, &[])),
        StatusOutputContent::Commit(CommitLineContent {
            change_id,
            sha,
            author: _,
            message,
            suffix: _,
        }) => {
            // The SHA is only shown when the commit has no change ID.
            let id = if change_id.is_empty() { sha } else { change_id };
            Some((id, message))
        }
        StatusOutputContent::Branch(BranchLineContent {
            id,
            decoration_start: _,
            branch_name,
            decoration_end: _,
            suffix: _,
        }) => Some((id, branch_name)),
        StatusOutputContent::File(FileLineContent {
            id,
            status: _,
            path,
        }) => Some((id, path)),
        StatusOutputContent::Uncommitted(UncommittedLineContent {
            id,
            decoration_start: _,
            label,
            decoration_end: _,
            suffix: _,
        }) => Some((id, label)),
    }
}

fn fuzzy_match(content: &[Span<'static>], query: &str) -> Option<(i64, Vec<usize>)> {
    static MATCHER: LazyLock<SkimMatcherV2> = LazyLock::new(SkimMatcherV2::default);
    MATCHER.fuzzy_indices(&spans_text(content), query)
}

fn spans_text(content: &[Span<'static>]) -> String {
    content.iter().map(|span| span.content.as_ref()).collect()
}

#[derive(Debug)]
pub enum JumpMessage {
    Enter,
    Input(Event),
    Previous,
    Next,
    Confirm,
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
        let return_cursor = self.mode.update(&mut self.backstack, |backstack, mode| {
            let previous_mode = std::mem::replace(mode, Mode::Normal(NormalMode::default()));
            let Mode::Jump(jump_mode) = previous_mode else {
                *mode = previous_mode;
                return None;
            };

            *mode = *jump_mode.return_mode;
            *backstack = jump_mode.return_backstack;

            Some(jump_mode.return_cursor)
        });
        let Some(return_cursor) = return_cursor else {
            return false;
        };

        self.cursor = return_cursor;
        // If a reload removed the starting line, its fallback may not be selectable here.
        self.ensure_cursor_is_on_selectable_line(MoveCursorDiration::Down);
        true
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
                    return_cursor: self.cursor,
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

        // Clearing the query leaves the cursor where it is rather than moving it to the top.
        if !mode.textarea.input(ev) || mode.query().is_empty() {
            return;
        }

        if let Some(new_cursor) = find_jump_match(&self.status_lines, mode, self.flags.show_files) {
            self.cursor = new_cursor;
        }
    }

    fn handle_jump_confirm(&mut self, _messages: &mut Vec<Message>) {
        let Mode::Jump(mode) = &*self.mode else {
            return;
        };

        // Stay in jump mode when there is no match under the cursor to confirm.
        if !self
            .cursor
            .selected_line(&self.status_lines)
            .is_some_and(|line| mode.match_line(line, self.flags.show_files).is_some())
        {
            return;
        }

        let return_mode = mode.return_mode.clone();
        let return_backstack = mode.return_backstack.clone();

        self.mode.update(&mut self.backstack, |backstack, mode| {
            *mode = *return_mode;
            *backstack = return_backstack;
        });
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

fn find_jump_match(
    lines: &[StatusOutputLine],
    mode: &JumpMode,
    show_files: FilesStatusFlag,
) -> Option<Cursor> {
    let query = mode.query();
    lines
        .iter()
        .filter(|line| mode.match_line(line, show_files).is_some())
        // Typing an ID lands on it: an exact ID beats an ID starting with the query, which beats
        // any other match. Otherwise the first match wins.
        .min_by_key(|line| {
            let id = jump_parts(&line.content)
                .map(|(id, _)| spans_text(id))
                .unwrap_or_default();
            let id = id.trim();
            (id != query, !id.starts_with(query))
        })
        .and_then(|line| cursor_for_jump_line(line, lines))
}
