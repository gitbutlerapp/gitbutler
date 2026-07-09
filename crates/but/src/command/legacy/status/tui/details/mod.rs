use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    iter::{empty, once, repeat_n},
    sync::Arc,
};

use anyhow::{Context as _, bail};
use bstr::{BStr, BString, ByteSlice};
use but_core::{
    HunkHeader, UnifiedPatch,
    diff::LineStats,
    ui::{TreeChange, TreeStatus},
    unified_diff::DiffHunk,
};
use but_ctx::{Context, OnDemand};
use but_hunk_assignment::HunkAssignment;
use gix::actor::Signature;
use itertools::{Either, Itertools, Position};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Stylize},
    text::{Line, Span},
    widgets::{List, ListItem, Widget},
};
use syntect::{
    easy::HighlightLines,
    highlighting,
    parsing::{SyntaxReference, SyntaxSet},
};
use unicode_width::UnicodeWidthStr;
use uuid::Uuid;

use crate::{
    CliId, IdMap,
    command::legacy::status::tui::{
        CommandMessage, CommitMessage, DebugAsType, DetailsLayoutMessage, FilesMessage, Message,
        MessageOnDrop, MoveMessage,
        app::{CommittedHunk, RewordMessage, RubMessage},
        details::details_cursor::DetailsCursor,
        highlight,
        message_on_drop::message_on_drop,
    },
    id::{UncommittedHunk, UncommittedHunkOrFile},
    theme::Theme,
};

use super::{HelpMessage, JumpMessage, StackMessage, app::RubSource};

mod details_cursor;

#[derive(Debug, Clone)]
pub enum DetailsMessage {
    Deselect,
    SelectFirstSection,
    CopyCurrentHunk,
    SelectNextSection,
    SelectPrevSection,
    ScrollUp(usize),
    ScrollDown(usize),
    GotoTop,
    GotoBottom,
    StartRub,
    Unlock,
}

#[derive(Debug)]
pub struct Details {
    is_dirty: bool,
    cursor: DetailsCursor,
    scroll_top: usize,
    widget: Option<DetailsAndDiffWidget>,
    sections: DiffSections,
    syntax_set: DebugAsType<OnDemand<SyntaxSet>>,
    syntax_theme: DebugAsType<OnDemand<highlighting::Theme>>,
    /// `RefCell` because visible lines are highlighted (and cached) during `render`, which only
    /// has `&self`.
    line_highlight_cache: RefCell<HighlightCache>,
    is_locked: bool,
    copied_hunk_highlight: highlight::Highlights<SectionId>,
    theme: &'static Theme,
}

impl Details {
    pub fn new(theme: &'static Theme) -> Self {
        Self {
            is_dirty: false,
            is_locked: false,
            widget: Default::default(),
            sections: Default::default(),
            cursor: Default::default(),
            scroll_top: 0,
            line_highlight_cache: Default::default(),
            copied_hunk_highlight: Default::default(),
            syntax_set: OnDemand::new(|| Ok(SyntaxSet::load_defaults_newlines())).into(),
            syntax_theme: OnDemand::new(|| theme.load_syntax_highlighting_theme()).into(),
            theme,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.widget = None;
        self.is_dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn update_highlight(&mut self) -> bool {
        self.copied_hunk_highlight.update()
    }

    fn lock(&mut self, messages: &mut Vec<Message>) -> MessageOnDrop {
        self.is_locked = true;
        message_on_drop(Message::Details(DetailsMessage::Unlock), messages)
    }

    pub fn unlock(&mut self) {
        if !self.is_locked {
            return;
        }
        self.is_locked = false;
        self.mark_dirty();
    }

    pub fn needs_update(&self, is_visible: bool) -> bool {
        is_visible && self.is_dirty()
    }

    pub fn reset_scroll(&mut self) {
        self.cursor = DetailsCursor::default();
        self.scroll_top = 0;
    }

    pub fn needs_update_after_message(&self, is_visible: bool, msg: &Message) -> bool {
        if self.is_locked {
            return false;
        }

        if !is_visible {
            return false;
        }

        match msg {
            Message::JustRender
            | Message::CopySelection
            | Message::CopySelectionPicker
            | Message::Quit
            | Message::ConfirmAndQuit
            | Message::DetailsLayout(DetailsLayoutMessage::Focus { .. })
            | Message::Discard
            | Message::DropToBeDiscarded
            | Message::Debug(_)
            | Message::ShowError(_)
            | Message::ShowToast { .. }
            | Message::Confirm(_)
            | Message::FuzzyPicker(_)
            | Message::GrowDetails
            | Message::ShrinkDetails
            | Message::PickAndGotoBranch
            | Message::ToggleHelp
            | Message::Mark
            | Message::ClearNormalModeMarks
            | Message::SetHasFocus(_)
            | Message::RegisterOutOfBandMessage(_)
            | Message::WithOneFrameDelay(_)
            | Message::Back
            | Message::UnfocusDetails
            | Message::DetailsLayout(DetailsLayoutMessage::ToggleFullScreen)
            | Message::DetailsLayout(DetailsLayoutMessage::ToggleVisibility)
            | Message::DetailsLayout(DetailsLayoutMessage::Dismiss)
            | Message::Undo
            | Message::Redo
            | Message::EnterNormalModeAfterConfirmingOperation => false,

            Message::MoveCursorUp(_)
            | Message::MoveCursorDown(_)
            | Message::SelectBranch(_)
            | Message::MoveCursorPreviousSection
            | Message::MoveCursorNextSection
            | Message::SelectUncommitted
            | Message::SelectMergeBase
            | Message::Reload(..)
            | Message::NewBranch => true,

            Message::Commit(commit_message) => match commit_message {
                CommitMessage::Confirm
                | CommitMessage::CommitToNewBranch
                | CommitMessage::CreateEmpty => true,
                CommitMessage::Start
                | CommitMessage::ToggleMessageComposer(..)
                | CommitMessage::ToggleInsertSide => false,
            },
            Message::Rub(rub_message) => match rub_message {
                RubMessage::Start
                | RubMessage::StartReverse
                | RubMessage::UseTargetMessage
                | RubMessage::UseSourceMessage
                | RubMessage::StartWithSource { .. } => false,
                RubMessage::Confirm => true,
            },
            Message::Reword(reword_message) => match reword_message {
                RewordMessage::OpenEditor
                | RewordMessage::WithEditor
                | RewordMessage::InlineConfirm => true,
                RewordMessage::InlineStart | RewordMessage::InlineInput(_) => false,
            },
            Message::Command(command_message) => match command_message {
                CommandMessage::Start(_) | CommandMessage::Input(_) => false,
                CommandMessage::Confirm => true,
            },
            Message::Files(files_message) => match files_message {
                FilesMessage::ToggleGlobalFilesList | FilesMessage::ToggleFilesForCommit => true,
            },
            Message::Move(move_message) => match move_message {
                MoveMessage::Start | MoveMessage::ToggleInsertSide => false,
                MoveMessage::Confirm => true,
            },
            Message::Details(details_message) => match details_message {
                DetailsMessage::Unlock // `unlock` sets the dirty flag if necessary
                | DetailsMessage::Deselect
                | DetailsMessage::CopyCurrentHunk
                | DetailsMessage::SelectFirstSection
                | DetailsMessage::SelectNextSection
                | DetailsMessage::SelectPrevSection
                | DetailsMessage::GotoTop
                | DetailsMessage::GotoBottom
                | DetailsMessage::StartRub
                | DetailsMessage::ScrollUp(_)
                | DetailsMessage::ScrollDown(_) => false,
            },
            Message::Help(help_message) => match help_message {
                HelpMessage::Close | HelpMessage::ScrollUp(_) | HelpMessage::ScrollDown(_) => false,
            },
            Message::Stack(stack_message) => match stack_message {
                StackMessage::Enter => {
                    // entering stack mode might move the cursor which will require an update
                    true
                }
                StackMessage::Unapply
                | StackMessage::ShowApplyPicker
                | StackMessage::MoveStart
                | StackMessage::MoveConfirm => false,
            },
            Message::Jump(jump_message) => match jump_message {
                // the handler functions themselves mark the details as dirty if necessary
                JumpMessage::Enter
                | JumpMessage::Input(..)
                | JumpMessage::Confirm
                | JumpMessage::Previous
                | JumpMessage::Next => false,
            },

            Message::AndThen { .. } => true,
        }
    }

    pub fn try_handle_message(
        &mut self,
        msg: DetailsMessage,
        viewport: Rect,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match msg {
            DetailsMessage::ScrollUp(n) => {
                self.scroll_top = self.scroll_top.saturating_sub(n);
                self.clamp_scroll_top(viewport);
                self.select_visible_section_if_selection_is_hidden(viewport);
            }
            DetailsMessage::ScrollDown(n) => {
                self.scroll_top = self.scroll_top.saturating_add(n);
                self.clamp_scroll_top(viewport);
                self.select_visible_section_if_selection_is_hidden(viewport);
            }
            DetailsMessage::SelectNextSection => {
                self.cursor
                    .move_selection_by(&self.sections.sections, |i| i.saturating_add(1));

                self.ensure_selection_visible(viewport);
            }
            DetailsMessage::SelectPrevSection => {
                self.cursor
                    .move_selection_by(&self.sections.sections, |i| i.saturating_sub(1));

                self.ensure_selection_visible(viewport);
            }
            DetailsMessage::GotoTop => {
                self.cursor
                    .move_selection_by(&self.sections.sections, |_| 0);
                self.scroll_top = 0;
            }
            DetailsMessage::GotoBottom => {
                self.cursor
                    .move_selection_by(&self.sections.sections, |_| usize::MAX);
                self.ensure_selection_visible(viewport);
            }
            DetailsMessage::Deselect => {
                self.cursor.deselect();
            }
            DetailsMessage::SelectFirstSection => {
                if let Some(section) = self.sections.sections.first() {
                    self.cursor.select_section(section.id.clone());
                    self.ensure_selection_visible(viewport);
                }
            }
            DetailsMessage::CopyCurrentHunk => {
                self.copy_current_hunk()?;
            }
            DetailsMessage::StartRub => {
                let Some(selection) = self.cursor.selection() else {
                    return Ok(());
                };
                let source = match selection {
                    SectionId::ShortId(cli_id) => RubSource::CliId(Arc::clone(cli_id)),
                    SectionId::Opaque(_) => return Ok(()),
                    SectionId::CommittedHunk { id: _, hunk } => {
                        RubSource::CommittedHunk(hunk.clone())
                    }
                };

                let unlock = self.lock(messages);

                messages.extend([Message::Rub(RubMessage::StartWithSource {
                    source,
                    unlock_details: Some(unlock),
                })]);
            }
            DetailsMessage::Unlock => {
                self.unlock();
            }
        }

        self.clamp_scroll_top(viewport);

        Ok(())
    }

    pub fn ensure_selection_visible(&mut self, viewport: Rect) {
        let Some(selection) = self.cursor.selection() else {
            return;
        };

        let Some(widget) = self.widget.as_ref() else {
            return;
        };

        let content_width = details_content_width(viewport);
        let content_height = details_content_height(viewport);

        let rows_before_diff = widget.rows_before_diff(content_width);
        let Some((section_start, section_end)) = self.sections.section_row_range(selection) else {
            return;
        };
        let row_start = rows_before_diff.saturating_add(section_start);
        let row_end = rows_before_diff.saturating_add(section_end);

        let row_height = row_end.saturating_sub(row_start);
        let viewport_start = self.scroll_top;
        let viewport_end = viewport_start.saturating_add(content_height);

        if row_height <= content_height {
            if row_start < viewport_start {
                self.scroll_top = row_start;
            } else if row_end > viewport_end {
                self.scroll_top = row_end.saturating_sub(content_height);
            }
        } else {
            self.scroll_top = row_start;
        }
    }

    pub fn selection(&self) -> Option<&SectionId> {
        self.cursor.selection()
    }

    fn select_visible_section_if_selection_is_hidden(&mut self, viewport: Rect) {
        let Some(selection) = self.cursor.selection() else {
            return;
        };

        let Some(widget) = self.widget.as_ref() else {
            return;
        };

        let content_width = details_content_width(viewport);
        let content_height = details_content_height(viewport);
        let viewport_start = self.scroll_top;
        let viewport_end = viewport_start.saturating_add(content_height);
        let rows_before_diff = widget.rows_before_diff(content_width);
        let Some((section_start, section_end)) = self.sections.section_row_range(selection) else {
            return;
        };
        let selection_start = rows_before_diff.saturating_add(section_start);
        let selection_end = rows_before_diff.saturating_add(section_end);

        if selection_start < viewport_end && selection_end > viewport_start {
            return;
        }

        let select_last_visible = selection_start >= viewport_end;
        let diff_start = viewport_start.saturating_sub(rows_before_diff);
        let diff_end = viewport_end
            .saturating_sub(rows_before_diff)
            .min(self.sections.total_rows);
        if diff_start >= diff_end {
            return;
        }

        let Some(section) =
            self.sections
                .visible_section(diff_start, diff_end, select_last_visible)
        else {
            return;
        };

        self.cursor.select_section(section);
    }

    fn copy_current_hunk(&mut self) -> anyhow::Result<()> {
        let Some(selection) = self.cursor.selection().cloned() else {
            return Ok(());
        };
        let Some(hunk) = self.hunk_text(&selection) else {
            return Ok(());
        };

        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(hunk))
            .context("failed to copy to system clipboard")?;

        self.copied_hunk_highlight.insert(selection);

        Ok(())
    }

    fn hunk_text(&self, selection: &SectionId) -> Option<String> {
        let section = self
            .sections
            .sections
            .iter()
            .find(|section| &section.id == selection)?;

        let SectionContent::DiffLines { path, diff, .. } = section
            .content
            .iter()
            .find(|content| matches!(content, SectionContent::DiffLines { .. }))?
        else {
            return None;
        };

        let mut hunk = path.to_str_lossy().into_owned();
        hunk.push_str("\n\n");
        for line in diff.lines() {
            hunk.push_str(&line.to_str_lossy());
            hunk.push('\n');
        }
        Some(hunk)
    }

    fn clamp_scroll_top(&mut self, viewport: Rect) {
        let content_width = details_content_width(viewport);
        let content_height = details_content_height(viewport);

        let max_scroll_top = self
            .widget
            .as_ref()
            .map(|widget| {
                widget
                    .rows_before_diff(content_width)
                    .saturating_add(self.sections.total_rows)
                    .saturating_sub(content_height)
            })
            .unwrap_or(0);

        self.scroll_top = self.scroll_top.min(max_scroll_top);
    }

    /// Builds the details widget and its diff sections for `selection` if they haven't been built
    /// yet.
    ///
    /// This only gathers the raw diff data and builds a row index; the actual lines are
    /// materialized lazily in [`Details::render`], so even huge diffs stay cheap to build and
    /// render.
    ///
    /// Returns `true` if a diff is available for rendering afterwards.
    pub fn update(&mut self, ctx: &mut Context, selection: Option<&CliId>) -> anyhow::Result<bool> {
        if self.widget.is_some() {
            self.is_dirty = false;
            return Ok(true);
        }

        let Some(selection) = selection else {
            self.is_dirty = false;
            return Ok(false);
        };

        self.cursor = DetailsCursor::default();
        self.scroll_top = 0;
        self.sections.clear();

        // load the syntax highlighting theme up front so `render` never has to fall back to
        // plain text
        self.syntax_theme.get()?;

        self.widget = match selection {
            CliId::Commit { commit_id, .. } => Some(from_commit(
                ctx,
                *commit_id,
                &*self.syntax_set.get()?,
                &mut self.sections,
                self.theme,
            )?),
            CliId::UncommittedHunkOrFile(uncommitted) => {
                let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                let id_map = IdMap::legacy_new_from_context(ctx, Some(wt_changes.assignments))?;
                let uncommitted_hunks =
                    filter_uncommitted_hunks(ctx, &id_map, |hunk_assignment| {
                        uncommitted_hunk_matches_selection(hunk_assignment, uncommitted)
                    })?;
                Some(from_uncommitted_hunks(
                    uncommitted_hunks,
                    &*self.syntax_set.get()?,
                    &mut self.sections,
                    self.theme,
                )?)
            }
            // the tui never shows path prefix ids, those only come from users
            // so ignore them for now
            CliId::PathPrefix { .. } => {
                tracing::error!("tui diff doesn't yet support path prefix cli ids");
                None
            }
            CliId::CommittedFile {
                commit_id, path, ..
            } => Some(from_committed_file(
                ctx,
                *commit_id,
                path.as_ref(),
                &*self.syntax_set.get()?,
                &mut self.sections,
                self.theme,
            )?),
            CliId::Branch { name, .. } => Some(from_branch(
                ctx,
                name.to_owned(),
                &*self.syntax_set.get()?,
                &mut self.sections,
                self.theme,
            )?),
            CliId::Uncommitted { .. } => {
                let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                let id_map = IdMap::legacy_new_from_context(ctx, Some(wt_changes.assignments))?;
                let uncommitted_hunks =
                    filter_uncommitted_hunks(ctx, &id_map, |hunk_assignment| {
                        hunk_assignment.stack_id.is_none()
                    })?;
                Some(from_uncommitted_hunks(
                    uncommitted_hunks,
                    &*self.syntax_set.get()?,
                    &mut self.sections,
                    self.theme,
                )?)
            }
            CliId::Stack { stack_id, .. } => {
                let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                let id_map = IdMap::legacy_new_from_context(ctx, Some(wt_changes.assignments))?;
                let uncommitted_hunks =
                    filter_uncommitted_hunks(ctx, &id_map, |hunk_assignment| {
                        hunk_assignment.stack_id.is_some_and(|id| id == *stack_id)
                    })?;
                Some(from_uncommitted_hunks(
                    uncommitted_hunks,
                    &*self.syntax_set.get()?,
                    &mut self.sections,
                    self.theme,
                )?)
            }
        };

        self.sections.finalize();
        self.is_dirty = false;

        Ok(self.widget.is_some())
    }

    pub fn render(&self, help_shown: bool, has_focus: bool, area: Rect, frame: &mut Frame) {
        let Some(widget) = &self.widget else {
            return;
        };

        let syntax_set = self.syntax_set.get().ok();
        let syntax_theme = self.syntax_theme.get().ok();
        let mut cache = self.line_highlight_cache.borrow_mut();
        let mut highlight_context = HighlightContext {
            syntax_set: syntax_set.as_deref(),
            syntax_theme: syntax_theme.as_deref(),
            cache: &mut cache,
        };

        widget.render(
            &self.sections,
            &self.cursor,
            self.scroll_top,
            area,
            frame,
            help_shown,
            has_focus,
            self.is_dirty,
            &self.copied_hunk_highlight,
            &mut highlight_context,
            self.theme,
        );
    }
}

fn details_content_width(viewport: Rect) -> u16 {
    viewport.width.max(1)
}

fn details_content_height(viewport: Rect) -> usize {
    viewport.height.max(1) as usize
}

/// Returns true if `hunk_assignment` is part of the selected uncommitted entity.
fn uncommitted_hunk_matches_selection(
    hunk_assignment: &HunkAssignment,
    uncommitted: &UncommittedHunkOrFile,
) -> bool {
    let selected_hunk = uncommitted.hunk_assignments.first();

    if uncommitted.is_entire_file {
        hunk_assignment.path_bytes == selected_hunk.path_bytes
            && hunk_assignment.stack_id == selected_hunk.stack_id
    } else {
        hunk_assignment == selected_hunk && hunk_assignment.stack_id == selected_hunk.stack_id
    }
}

fn filter_uncommitted_hunks<'a, F>(
    ctx: &'a mut Context,
    id_map: &'a IdMap,
    mut filter: F,
) -> anyhow::Result<Vec<(&'a str, Arc<CliId>, &'a UncommittedHunk)>>
where
    F: FnMut(&HunkAssignment) -> bool,
{
    let mut uncommitted_hunks = id_map
        .uncommitted_hunks
        .iter()
        .filter(move |(_, hunk)| filter(&hunk.hunk_assignment))
        .map(|(raw_id, hunk)| {
            let mut cli_ids = id_map.parse_using_context(raw_id, ctx)?;
            if cli_ids.len() == 1 {
                Ok((&**raw_id, Arc::new(cli_ids.remove(0)), hunk))
            } else if cli_ids.is_empty() {
                bail!("'{raw_id}' no found")
            } else {
                bail!(
                    "'{raw_id}' resolved to more than one hunk ({})",
                    cli_ids.len()
                )
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    uncommitted_hunks.sort_by(|(id_a, _, hunk_a), (id_b, _, hunk_b)| {
        (
            &hunk_a.hunk_assignment.path_bytes,
            hunk_a
                .hunk_assignment
                .hunk_header
                .as_ref()
                .map(|header| header.old_start),
            id_a,
        )
            .cmp(&(
                &hunk_b.hunk_assignment.path_bytes,
                hunk_b
                    .hunk_assignment
                    .hunk_header
                    .as_ref()
                    .map(|header| header.old_start),
                id_b,
            ))
    });

    Ok(uncommitted_hunks)
}

#[derive(Debug)]
enum DetailsAndDiffWidget {
    FromCommit {
        header_items: Vec<ListItem<'static>>,
        message: String,
        line_stats: Vec<ListItem<'static>>,
    },
    FromDiffLines {
        line_stats: Vec<ListItem<'static>>,
    },
}

impl DetailsAndDiffWidget {
    /// The number of rows rendered above the diff sections.
    fn rows_before_diff(&self, width: u16) -> usize {
        match self {
            DetailsAndDiffWidget::FromCommit {
                header_items,
                message,
                line_stats,
            } => {
                header_items.len()
                    + 1 // +1 to match the empty line added in `render`
                    + commit_message_row_count(message, width)
                    + 1 // +1 to match the empty line added in `render`
                    + line_stats.len()
                    // +1 to match the empty line added in `render`
                    + (if line_stats.is_empty() { 0 } else { 1 })
            }
            DetailsAndDiffWidget::FromDiffLines { line_stats } => {
                line_stats.len()
                // +1 to match the empty line added in `render`
                + (if line_stats.is_empty() { 0 } else { 1 })
            }
        }
    }

    fn render(
        &self,
        sections: &DiffSections,
        cursor: &DetailsCursor,
        scroll_top: usize,
        area: Rect,
        buf: &mut Frame,
        help_shown: bool,
        has_focus: bool,
        is_dirty: bool,
        copied_hunk_highlight: &highlight::Highlights<SectionId>,
        highlight_context: &mut HighlightContext<'_>,
        theme: &'static Theme,
    ) {
        enum ListItemOrString<'a> {
            ListItem(&'a ListItem<'a>),
            Str(Cow<'a, str>),
        }

        let empty_list_item = ListItem::new("");
        let no_commit_message = ListItem::new("(no commit message)").style(theme.hint);

        let wrapped_message_iter = match self {
            DetailsAndDiffWidget::FromCommit { message, .. } => {
                if message.is_empty() {
                    let item = ListItemOrString::ListItem(&no_commit_message);
                    Some(Either::Left(once(item)))
                } else {
                    Some(Either::Right(
                        wrapped_commit_message_lines(message, area.width)
                            .map(ListItemOrString::Str),
                    ))
                }
            }
            DetailsAndDiffWidget::FromDiffLines { .. } => None,
        }
        .into_iter()
        .flatten();

        let rendered_line_stats = match self {
            DetailsAndDiffWidget::FromCommit { line_stats, .. }
            | DetailsAndDiffWidget::FromDiffLines { line_stats } => {
                if line_stats.is_empty() {
                    Either::Left(empty())
                } else {
                    let iter = empty()
                        .chain(line_stats.iter().map(ListItemOrString::ListItem))
                        .chain([ListItemOrString::ListItem(&empty_list_item)]);
                    Either::Right(iter)
                }
            }
        };

        let rows_above_diff = match self {
            DetailsAndDiffWidget::FromCommit {
                header_items,
                // message rendered in `wrapped_message_iter`
                message: _,
                // `line_stats` rendered in `rendered_line_stats`
                line_stats: _,
            } => {
                let iter = empty()
                    .chain(header_items.iter().map(ListItemOrString::ListItem))
                    .chain([ListItemOrString::ListItem(&empty_list_item)])
                    .chain(wrapped_message_iter)
                    .chain([ListItemOrString::ListItem(&empty_list_item)])
                    .chain(rendered_line_stats);
                Either::Left(iter)
            }
            DetailsAndDiffWidget::FromDiffLines {
                // `line_stats` rendered in `rendered_line_stats`
                line_stats: _,
            } => Either::Right(rendered_line_stats),
        };

        let height = area.height as usize;

        let mut items = rows_above_diff
            .skip(scroll_top)
            .take(height)
            .map(|item| match item {
                ListItemOrString::ListItem(list_item) => list_item.to_owned(),
                ListItemOrString::Str(cow) => ListItem::new(cow),
            })
            .collect::<Vec<_>>();

        // Only the diff rows that are actually visible get materialized (and syntax
        // highlighted); everything above scrolled past or below the viewport stays raw bytes.
        let remaining_height = height.saturating_sub(items.len());
        let first_diff_row = scroll_top.saturating_sub(self.rows_before_diff(area.width));
        render_visible_diff_rows(
            sections,
            first_diff_row,
            remaining_height,
            highlight_context,
            theme,
            |section_id, item| {
                let item = match section_id {
                    Some(section_id) => {
                        if copied_hunk_highlight.contains(section_id) {
                            item.style(highlight::style())
                        } else if !help_shown
                            && has_focus
                            && cursor
                                .selection()
                                .is_some_and(|selection| selection == section_id)
                        {
                            item.style(theme.discrete_selection_highlight)
                        } else {
                            item
                        }
                    }
                    None => item,
                };
                items.push(item);
            },
        );

        if !items.is_empty() {
            List::new(items).render(area, buf.buffer_mut());
        } else if !is_dirty {
            Span::styled("No changes", theme.hint).render(area, buf.buffer_mut());
        }
    }
}

fn commit_message_row_count(message: &str, width: u16) -> usize {
    if message.is_empty() {
        1
    } else {
        wrapped_commit_message_lines(message, width).count()
    }
}

fn wrapped_commit_message_lines(message: &str, width: u16) -> impl Iterator<Item = Cow<'_, str>> {
    textwrap::wrap(message, textwrap::Options::new(width as usize))
        .into_iter()
        .with_position()
        .filter_map(|(pos, line)| match pos {
            Position::First | Position::Middle | Position::Only => Some(line),
            Position::Last => (!line.is_empty()).then_some(line),
        })
}

fn render_line_stats(line_stats: LineStats) -> Vec<ListItem<'static>> {
    let LineStats {
        lines_added,
        lines_removed,
        files_changed,
    } = line_stats;

    let line = Line::from_iter([
        if files_changed == 1 {
            Span::raw(format!("{files_changed} file changed"))
        } else {
            Span::raw(format!("{files_changed} files changed"))
        },
        Span::raw(", "),
        Span::raw(format!("+{lines_added}")).green(),
        Span::raw(" "),
        Span::raw(format!("-{lines_removed}")).red(),
    ]);

    Vec::from([ListItem::from(line)])
}

fn from_commit(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
    syntax_set: &SyntaxSet,
    sections: &mut DiffSections,
    theme: &'static Theme,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let commit_details =
        but_api::diff::commit_details(ctx, commit_id, but_api::diff::ComputeLineStats::No)?;

    let header_items = Vec::from([
        ListItem::new(Line::from_iter([
            Span::raw(format!("{:<11}", "Commit ID:")),
            Span::styled(commit_id.to_hex().to_string(), theme.commit_id),
        ])),
        ListItem::new(Line::from_iter(
            once(Span::raw(format!("{:<11}", "Author:")))
                .chain(render_signature(&commit_details.commit.author, theme)),
        )),
        ListItem::new(Line::from_iter(
            once(Span::raw(format!("{:<11}", "Committer:")))
                .chain(render_signature(&commit_details.commit.committer, theme)),
        )),
    ]);

    let message = commit_details.commit.message.to_string();

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();

    let mut line_stats = LineStats::default();

    build_tree_changes(
        ctx,
        &tree_changes,
        Some(commit_id),
        syntax_set,
        sections,
        &mut line_stats,
        theme,
    );

    Ok(DetailsAndDiffWidget::FromCommit {
        header_items,
        message,
        line_stats: render_line_stats(line_stats),
    })
}

fn from_uncommitted_hunks(
    uncommitted_hunks: Vec<(&str, Arc<CliId>, &UncommittedHunk)>,
    syntax_set: &SyntaxSet,
    sections: &mut DiffSections,
    theme: &'static Theme,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let mut line_stats = LineStats::default();
    let mut unique_paths = HashSet::new();

    for (raw_id, cli_id, UncommittedHunk { hunk_assignment }) in uncommitted_hunks {
        unique_paths.insert(&hunk_assignment.path_bytes);

        line_stats.lines_added += hunk_assignment
            .line_nums_added
            .as_ref()
            .map_or(0, |lines| lines.len() as u64);
        line_stats.lines_removed += hunk_assignment
            .line_nums_removed
            .as_ref()
            .map_or(0, |lines| lines.len() as u64);

        let section = sections.new_section_mut(SectionId::ShortId(cli_id));

        build_hunk_path_header(
            hunk_assignment.path_bytes.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(raw_id)),
            &mut section.content,
            theme,
        );

        build_hunk_assignment(hunk_assignment, syntax_set, theme, &mut section.content);
    }

    line_stats.files_changed = unique_paths.len() as u64;

    Ok(DetailsAndDiffWidget::FromDiffLines {
        line_stats: render_line_stats(line_stats),
    })
}

fn from_committed_file(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
    path: &BStr,
    syntax_set: &SyntaxSet,
    sections: &mut DiffSections,
    theme: &'static Theme,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let commit_details =
        but_api::diff::commit_details(ctx, commit_id, but_api::diff::ComputeLineStats::No)?;

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .filter(|change| change.path == path)
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();

    let mut line_stats = LineStats::default();

    build_tree_changes(
        ctx,
        &tree_changes,
        Some(commit_id),
        syntax_set,
        sections,
        &mut line_stats,
        theme,
    );

    Ok(DetailsAndDiffWidget::FromDiffLines {
        line_stats: render_line_stats(line_stats),
    })
}

fn from_branch(
    ctx: &mut Context,
    name: String,
    syntax_set: &SyntaxSet,
    sections: &mut DiffSections,
    theme: &'static Theme,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let tree_changes = but_api::branch::branch_diff(ctx, name)?;

    let mut line_stats = LineStats::default();

    build_tree_changes(
        ctx,
        &tree_changes.changes,
        None,
        syntax_set,
        sections,
        &mut line_stats,
        theme,
    );

    Ok(DetailsAndDiffWidget::FromDiffLines {
        line_stats: render_line_stats(line_stats),
    })
}

/// How often line-number checkpoints are recorded in [`SectionContent::DiffLines`].
///
/// Computing the line numbers at an arbitrary diff line means replaying at most this many lines
/// from the nearest checkpoint, instead of scanning from the start of the hunk.
const LINE_NUMBER_CHECKPOINT_INTERVAL: usize = 512;

/// The diff sections, together with a row index that maps between diff-area rows and sections.
///
/// Rendering diff lines is expensive, mostly because of syntax highlighting. So instead of
/// rendering the entire diff up front, only the raw bytes and this row index are stored, and the
/// rows visible in the viewport are materialized on every frame. That keeps opening the detail
/// view and scrolling fast, and memory usage low, even for diffs with millions of lines.
#[derive(Debug, Default)]
struct DiffSections {
    sections: Vec<DiffSection>,
    /// The total number of diff rows, including the separator rows between sections.
    total_rows: usize,
}

impl DiffSections {
    fn new_section_mut(&mut self, id: SectionId) -> &mut DiffSection {
        self.sections.push(DiffSection {
            id,
            start_row: 0,
            row_count: 0,
            content: Default::default(),
        });
        self.sections.last_mut().unwrap()
    }

    /// Clear all sections so the allocations can be reused.
    fn clear(&mut self) {
        self.sections.clear();
        self.total_rows = 0;
    }

    /// Compute the row index. Must be called once all sections have been built.
    fn finalize(&mut self) {
        let mut row = 0;
        for (idx, section) in self.sections.iter_mut().enumerate() {
            if idx > 0 {
                // the separator row between sections
                row += 1;
            }
            section.start_row = row;
            section.row_count = section.content.iter().map(SectionContent::row_count).sum();
            row += section.row_count;
        }
        self.total_rows = row;
    }

    /// Returns the start and end (exclusive) rows of a section, in diff-area row coordinates.
    fn section_row_range(&self, id: &SectionId) -> Option<(usize, usize)> {
        let section = self.sections.iter().find(|section| &section.id == id)?;
        Some((
            section.start_row,
            section.start_row.saturating_add(section.row_count),
        ))
    }

    /// Returns the first (or last) section with rows visible in `start..end`.
    fn visible_section(&self, start: usize, end: usize, last: bool) -> Option<SectionId> {
        if last {
            let idx = self
                .sections
                .partition_point(|section| section.start_row < end);
            self.sections[..idx]
                .iter()
                .rev()
                .find(|section| {
                    section.row_count > 0 && section.start_row + section.row_count > start
                })
                .map(|section| section.id.clone())
        } else {
            let idx = self
                .sections
                .partition_point(|section| section.start_row + section.row_count <= start);
            let section = self.sections[idx..]
                .iter()
                .find(|section| section.row_count > 0)?;
            (section.start_row < end).then(|| section.id.clone())
        }
    }
}

#[derive(Debug)]
pub struct DiffSection {
    id: SectionId,
    /// The first row of this section, in diff-area row coordinates.
    start_row: usize,
    /// The number of rows this section occupies.
    row_count: usize,
    content: Vec<SectionContent>,
}

/// An id only used by the TUI to identify this section. Doesn't have any meaning in the
/// rest of the system.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TuiId(Uuid);

impl TuiId {
    fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SectionId {
    ShortId(Arc<CliId>),
    CommittedHunk { id: TuiId, hunk: CommittedHunk },
    Opaque(TuiId),
}

/// The content of a section. Diff lines are kept as raw bytes and only turned into the
/// `ListItem`s that ratatui renders once they become visible.
#[derive(Debug)]
enum SectionContent {
    /// A header for a file like
    ///
    /// ────────────────╮
    /// added: a/b/c.rs │
    /// ────────────────╯
    FileHeader(Vec<ListItem<'static>>),
    /// A hunk header line like `@@ -1,6 +1,8 @@`
    HunkHeader([ListItem<'static>; 2]),
    /// A line saying the diff is unavailable, perhaps because of binary files.
    DiffUnavailable(Cow<'static, str>),
    /// The actual lines of the diff
    DiffLines {
        path: BString,
        old_width: u32,
        new_width: u32,
        syntax: Box<SyntaxReference>,
        /// The raw hunk text, including the `@@` header line.
        diff: BString,
        /// The byte offset at which each diff body line starts (excludes the `@@` header line).
        line_offsets: Vec<u32>,
        /// The (old, new) line numbers at every [`LINE_NUMBER_CHECKPOINT_INTERVAL`]-th body line.
        line_number_checkpoints: Vec<(u32, u32)>,
    },
}

impl SectionContent {
    fn row_count(&self) -> usize {
        match self {
            SectionContent::FileHeader(items) => items.len(),
            SectionContent::HunkHeader(items) => items.len(),
            SectionContent::DiffUnavailable(_) => 1,
            SectionContent::DiffLines { line_offsets, .. } => line_offsets.len(),
        }
    }
}

/// Everything needed to syntax highlight diff lines during `render`.
struct HighlightContext<'a> {
    syntax_set: Option<&'a SyntaxSet>,
    syntax_theme: Option<&'a highlighting::Theme>,
    cache: &'a mut HighlightCache,
}

impl<'a> HighlightContext<'a> {
    fn new_highlighter(
        &self,
        syntax: &SyntaxReference,
    ) -> Option<(HighlightLines<'a>, &'a SyntaxSet)> {
        Some((
            HighlightLines::new(syntax, self.syntax_theme?),
            self.syntax_set?,
        ))
    }
}

/// Materializes the diff rows `first_row..first_row + max_rows` (in diff-area row coordinates),
/// emitting each row together with the section it belongs to. Separator rows belong to no
/// section.
fn render_visible_diff_rows(
    sections: &DiffSections,
    first_row: usize,
    max_rows: usize,
    highlight_context: &mut HighlightContext<'_>,
    theme: &'static Theme,
    mut emit: impl FnMut(Option<&SectionId>, ListItem<'static>),
) {
    let mut row = first_row;
    let end_row = first_row.saturating_add(max_rows).min(sections.total_rows);
    let mut idx = sections
        .sections
        .partition_point(|section| section.start_row + section.row_count <= row);

    while row < end_row {
        let Some(section) = sections.sections.get(idx) else {
            break;
        };

        if row < section.start_row {
            // the separator row between sections
            emit(None, ListItem::new(""));
            row += 1;
            continue;
        }

        let local_start = row - section.start_row;
        let count = (end_row - row).min(section.row_count - local_start);
        render_section_rows(
            section,
            local_start,
            count,
            highlight_context,
            theme,
            &mut emit,
        );
        row += count;
        if local_start + count >= section.row_count {
            idx += 1;
        }
    }
}

/// Materializes the rows `local_start..local_start + count` of a single section.
fn render_section_rows(
    section: &DiffSection,
    local_start: usize,
    count: usize,
    highlight_context: &mut HighlightContext<'_>,
    theme: &'static Theme,
    emit: &mut impl FnMut(Option<&SectionId>, ListItem<'static>),
) {
    let mut offset = local_start;
    let mut remaining = count;

    for content in &section.content {
        if remaining == 0 {
            break;
        }

        let rows = content.row_count();
        if offset >= rows {
            offset -= rows;
            continue;
        }

        let take = (rows - offset).min(remaining);
        match content {
            SectionContent::FileHeader(items) => {
                for item in &items[offset..offset + take] {
                    emit(Some(&section.id), item.clone());
                }
            }
            SectionContent::HunkHeader(items) => {
                for item in &items[offset..offset + take] {
                    emit(Some(&section.id), item.clone());
                }
            }
            SectionContent::DiffUnavailable(message) => {
                emit(Some(&section.id), ListItem::new(message.clone()));
            }
            SectionContent::DiffLines {
                path,
                old_width,
                new_width,
                syntax,
                diff,
                line_offsets,
                line_number_checkpoints,
            } => {
                let (mut old_line_num, mut new_line_num) =
                    line_numbers_at(diff, line_offsets, line_number_checkpoints, offset);

                let mut highlighter = highlight_context.new_highlighter(syntax);

                for line_idx in offset..offset + take {
                    let line = diff_body_line(diff, line_offsets, line_idx);
                    let item = render_diff_line_item(
                        line,
                        path.as_ref(),
                        *old_width,
                        *new_width,
                        &mut old_line_num,
                        &mut new_line_num,
                        &mut highlighter,
                        highlight_context.cache,
                        theme,
                    );
                    emit(Some(&section.id), item);
                }
            }
        }

        remaining -= take;
        offset = 0;
    }
}

/// Returns the bytes of diff body line `idx`, without its trailing line break.
fn diff_body_line<'a>(diff: &'a [u8], line_offsets: &[u32], idx: usize) -> &'a [u8] {
    let start = line_offsets[idx] as usize;
    let end = line_offsets
        .get(idx + 1)
        .map_or(diff.len(), |&offset| offset as usize);
    let mut line = &diff[start..end];
    if line.last() == Some(&b'\n') {
        line = &line[..line.len() - 1];
    }
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }
    line
}

/// Returns the (old, new) line numbers at diff body line `idx` by replaying the lines since the
/// nearest checkpoint.
fn line_numbers_at(
    diff: &[u8],
    line_offsets: &[u32],
    line_number_checkpoints: &[(u32, u32)],
    idx: usize,
) -> (u32, u32) {
    let checkpoint_idx = idx / LINE_NUMBER_CHECKPOINT_INTERVAL;
    let (mut old_line_num, mut new_line_num) = line_number_checkpoints[checkpoint_idx];
    for offset in &line_offsets[checkpoint_idx * LINE_NUMBER_CHECKPOINT_INTERVAL..idx] {
        match diff.get(*offset as usize) {
            Some(b'+') => new_line_num += 1,
            Some(b'-') => old_line_num += 1,
            _ => {
                old_line_num += 1;
                new_line_num += 1;
            }
        }
    }
    (old_line_num, new_line_num)
}

fn render_diff_line_item(
    line: &[u8],
    path: &BStr,
    old_width: u32,
    new_width: u32,
    old_line_num: &mut u32,
    new_line_num: &mut u32,
    highlighter: &mut Option<(HighlightLines<'_>, &SyntaxSet)>,
    cache: &mut HighlightCache,
    theme: &'static Theme,
) -> ListItem<'static> {
    if let Some(rest) = line.strip_prefix(b"+") {
        let code = rest.to_str_lossy().to_string();
        let item = ListItem::new(Line::from_iter(
            [
                Span::raw(" ".repeat(old_width as _)),
                Span::styled(" ┊ ", theme.border),
                Span::raw(" ".repeat((new_width - num_digits(*new_line_num)) as _)),
                Span::raw(new_line_num.to_string()).style(theme.addition),
                Span::styled(" │ ", theme.border),
                Span::raw("+").style(theme.addition_rich),
            ]
            .into_iter()
            .chain(syntax_highlight(
                &code,
                path,
                theme.addition_rich.bg,
                highlighter,
                cache,
            )),
        ));
        *new_line_num += 1;
        item
    } else if let Some(rest) = line.strip_prefix(b"-") {
        let code = rest.to_str_lossy().to_string();
        let item = ListItem::new(Line::from_iter(
            [
                Span::raw(" ".repeat((old_width - num_digits(*old_line_num)) as _)),
                Span::raw(old_line_num.to_string()).style(theme.deletion),
                Span::styled(" ┊ ", theme.border),
                Span::raw(" ".repeat(new_width as _)),
                Span::styled(" │ ", theme.border),
                Span::raw("-").style(theme.deletion_rich),
            ]
            .into_iter()
            .chain(syntax_highlight(
                &code,
                path,
                theme.deletion_rich.bg,
                highlighter,
                cache,
            )),
        ));
        *old_line_num += 1;
        item
    } else {
        let line = line.strip_prefix(b" ").unwrap_or(line);
        let code = line.to_str_lossy().to_string();
        let item = ListItem::new(Line::from_iter(
            [
                Span::raw(" ".repeat((old_width - num_digits(*old_line_num)) as _)),
                Span::styled(old_line_num.to_string(), theme.hint),
                Span::styled(" ┊ ", theme.border),
                Span::raw(" ".repeat((new_width - num_digits(*new_line_num)) as _)),
                Span::styled(new_line_num.to_string(), theme.hint),
                Span::styled(" │  ", theme.border),
            ]
            .into_iter()
            .chain(syntax_highlight(&code, path, None, highlighter, cache)),
        ));
        *old_line_num += 1;
        *new_line_num += 1;
        item
    }
}

fn build_hunk_assignment(
    hunk_assignment: &HunkAssignment,
    syntax_set: &SyntaxSet,
    theme: &'static Theme,
    out: &mut Vec<SectionContent>,
) {
    if let Some(hunk_header) = hunk_assignment.hunk_header {
        if let Some(diff) = hunk_assignment.diff.clone() {
            let hunk = DiffHunk {
                old_start: hunk_header.old_start,
                old_lines: hunk_header.old_lines,
                new_start: hunk_header.new_start,
                new_lines: hunk_header.new_lines,
                diff,
            };

            let is_result_of_binary_to_text_conversion = false;

            build_unified_patch(
                hunk_assignment.path_bytes.as_ref(),
                hunk,
                is_result_of_binary_to_text_conversion,
                syntax_set,
                theme,
                out,
            );
        } else {
            out.push(SectionContent::DiffUnavailable("No diff available".into()));
        }
    } else {
        out.push(SectionContent::DiffUnavailable(
            "No diff available - file is either empty, binary, or too large".into(),
        ));
    }
}

fn build_tree_changes(
    ctx: &mut Context,
    tree_changes: &[TreeChange],
    commit_id: Option<gix::ObjectId>,
    syntax_set: &SyntaxSet,
    sections: &mut DiffSections,
    line_stats: &mut LineStats,
    theme: &'static Theme,
) {
    let mut unique_paths = HashSet::new();

    for tree_change in tree_changes {
        unique_paths.insert(&tree_change.path_bytes);

        if let Some(patch) = but_api::diff::tree_change_diffs(ctx, tree_change.clone())
            .ok()
            .flatten()
        {
            match patch {
                UnifiedPatch::Patch {
                    hunks,
                    is_result_of_binary_to_text_conversion,
                    lines_added,
                    lines_removed,
                } => {
                    line_stats.lines_added += u64::from(lines_added);
                    line_stats.lines_removed += u64::from(lines_removed);

                    let mut first_hunk = true;
                    for diff_hunk in hunks {
                        let section_id = if let Some(commit_id) = commit_id {
                            SectionId::CommittedHunk {
                                id: TuiId::new(),
                                hunk: CommittedHunk {
                                    header: HunkHeader::from(&diff_hunk),
                                    path: Arc::from(tree_change.path_bytes.clone()),
                                    commit_id,
                                },
                            }
                        } else {
                            SectionId::Opaque(TuiId::new())
                        };
                        let section = sections.new_section_mut(section_id);

                        if std::mem::take(&mut first_hunk) {
                            let mut header = Vec::new();
                            render_hunk_path_header(
                                tree_change.path.as_ref(),
                                Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                                &mut header,
                                theme,
                            );
                            section.content.push(SectionContent::FileHeader(header));
                        }

                        build_unified_patch(
                            tree_change.path.as_ref(),
                            diff_hunk,
                            is_result_of_binary_to_text_conversion,
                            syntax_set,
                            theme,
                            &mut section.content,
                        );
                    }
                }
                UnifiedPatch::Binary => {
                    let section = sections.new_section_mut(SectionId::Opaque(TuiId::new()));

                    let mut header = Vec::new();
                    render_hunk_path_header(
                        tree_change.path.as_ref(),
                        Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                        &mut header,
                        theme,
                    );
                    section.content.push(SectionContent::FileHeader(header));

                    section.content.push(SectionContent::DiffUnavailable(
                        "Binary file - no diff available".into(),
                    ));
                }
                UnifiedPatch::TooLarge { size_in_bytes } => {
                    let section = sections.new_section_mut(SectionId::Opaque(TuiId::new()));

                    let mut header = Vec::new();
                    render_hunk_path_header(
                        tree_change.path.as_ref(),
                        Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                        &mut header,
                        theme,
                    );
                    section.content.push(SectionContent::FileHeader(header));

                    section.content.push(SectionContent::DiffUnavailable(
                        format!("File too large ({size_in_bytes} bytes) - no diff available")
                            .into(),
                    ));
                }
            }
        }
    }

    line_stats.files_changed = unique_paths.len() as _;
}

enum ShortIdOrTreeStatus<'a> {
    ShortId(&'a str),
    TreeStatus(&'a TreeStatus),
}

fn render_hunk_path_header(
    path: &BStr,
    status: Option<ShortIdOrTreeStatus<'_>>,
    out: &mut Vec<ListItem<'static>>,
    theme: &'static Theme,
) {
    let status = status.map(|id_or_status| match id_or_status {
        ShortIdOrTreeStatus::ShortId(id) => Span::styled(id.to_owned(), theme.cli_id),
        ShortIdOrTreeStatus::TreeStatus(status) => change_status(status, theme),
    });
    let path = path.to_string();
    let path_line = Line::from_iter(
        [Span::raw(" ")]
            .into_iter()
            .chain(
                status
                    .into_iter()
                    .flat_map(|status| [status, Span::raw(" ")]),
            )
            .chain([Span::raw(path)]),
    );
    out.extend(bordered_line_top_right_bottom(path_line, theme).map(ListItem::new));
    out.push(ListItem::from(""));
}

fn build_hunk_path_header(
    path: &BStr,
    status: Option<ShortIdOrTreeStatus<'_>>,
    out: &mut Vec<SectionContent>,
    theme: &'static Theme,
) {
    let status = status.map(|id_or_status| match id_or_status {
        ShortIdOrTreeStatus::ShortId(id) => Span::raw(id.to_owned()).blue(),
        ShortIdOrTreeStatus::TreeStatus(status) => change_status(status, theme),
    });
    let path = path.to_string();
    let path_line = Line::from_iter(
        [Span::raw(" ")]
            .into_iter()
            .chain(
                status
                    .into_iter()
                    .flat_map(|status| [status, Span::raw(" ")]),
            )
            .chain([Span::raw(path)]),
    );
    out.push(SectionContent::FileHeader(
        bordered_line_top_right_bottom(path_line, theme)
            .map(ListItem::new)
            .chain([ListItem::from("")])
            .collect(),
    ));
}

fn change_status(status: &TreeStatus, theme: &'static Theme) -> Span<'static> {
    match status {
        TreeStatus::Addition { .. } => Span::styled("added", theme.addition),
        TreeStatus::Deletion { .. } => Span::styled("deleted", theme.deletion),
        TreeStatus::Modification { .. } => Span::styled("modified", theme.modification),
        TreeStatus::Rename { .. } => Span::styled("renamed", theme.renaming),
    }
}

fn bordered_line_top_right_bottom(
    mut text: Line<'static>,
    theme: &'static Theme,
) -> impl Iterator<Item = Line<'static>> {
    let width_including_padding = text.width() + 1;

    text.spans
        .extend([Span::raw(" "), Span::styled("│", theme.border)]);

    [
        Line::from_iter(repeat_n("─", width_including_padding).chain(once("╮")))
            .style(theme.border),
        text,
        Line::from_iter(repeat_n("─", width_including_padding).chain(once("╯")))
            .style(theme.border),
    ]
    .into_iter()
}

fn render_signature(
    sig: &Signature,
    theme: &'static Theme,
) -> impl IntoIterator<Item = Span<'static>> {
    [
        Span::styled(sig.name.to_string(), theme.user),
        Span::raw(" <"),
        Span::styled(sig.email.to_string(), theme.user),
        Span::raw(">"),
        Span::raw(" ("),
        Span::styled(
            sig.time.format_or_unix(gix::date::time::format::DEFAULT),
            theme.time,
        ),
        Span::raw(")"),
    ]
    .into_iter()
}

fn build_unified_patch(
    path: &BStr,
    hunk: DiffHunk,
    is_result_of_binary_to_text_conversion: bool,
    syntax_set: &SyntaxSet,
    theme: &'static Theme,
    content: &mut Vec<SectionContent>,
) {
    let DiffHunk {
        old_start,
        new_start,
        diff,
        old_lines: _,
        new_lines: _,
    } = hunk;

    if is_result_of_binary_to_text_conversion {
        content.push(SectionContent::DiffUnavailable(
            "(diff generated from binary-to-text conversion)".into(),
        ));
    }

    if let Some(headers) = diff.lines().next() {
        content.extend([SectionContent::HunkHeader([
            ListItem::new(Span::styled(headers.to_str_lossy().to_string(), theme.hint)),
            ListItem::new(
                Line::from_iter(repeat_n("─", headers.to_str_lossy().width())).style(theme.border),
            ),
        ])]);
    }

    if u32::try_from(diff.len()).is_err() {
        // keeps `line_offsets` at `u32`, which halves its size and matters for diffs with
        // millions of lines
        content.push(SectionContent::DiffUnavailable(
            format!("Hunk too large ({} bytes) - no diff available", diff.len()).into(),
        ));
        return;
    }

    // A single pass over the raw diff records where each body line starts, line-number
    // checkpoints, and the final line numbers which determine the gutter widths. The lines
    // themselves are only materialized once they become visible.
    let body_start = diff
        .find_byte(b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(diff.len());
    let mut line_offsets = Vec::new();
    let mut line_number_checkpoints = Vec::new();
    let mut old_line = old_start;
    let mut new_line = new_start;
    let mut offset = body_start;
    while offset < diff.len() {
        if line_offsets.len() % LINE_NUMBER_CHECKPOINT_INTERVAL == 0 {
            line_number_checkpoints.push((old_line, new_line));
        }
        line_offsets.push(offset as u32);

        match diff[offset] {
            b'+' => new_line += 1,
            b'-' => old_line += 1,
            _ => {
                old_line += 1;
                new_line += 1;
            }
        }

        offset = diff[offset..]
            .find_byte(b'\n')
            .map(|idx| offset + idx + 1)
            .unwrap_or(diff.len());
    }
    let (old_width, new_width) = (num_digits(old_line), num_digits(new_line));

    let syntax = {
        let path = path.to_path_lossy();
        path.extension()
            .and_then(|ext| syntax_set.find_syntax_by_extension(ext.to_str()?))
            .or_else(|| {
                path.file_name()
                    .and_then(|file_name| syntax_set.find_syntax_by_extension(file_name.to_str()?))
            })
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    };

    content.push(SectionContent::DiffLines {
        path: path.to_owned(),
        old_width,
        new_width,
        syntax: Box::new(syntax.to_owned()),
        diff,
        line_offsets,
        line_number_checkpoints,
    });
}

fn num_digits(n: u32) -> u32 {
    if n == 0 { 1 } else { n.ilog10() + 1 }
}

/// How many highlighted lines to keep in [`HighlightCache`] before it is emptied.
///
/// Scrolling through a huge diff would otherwise accumulate unbounded memory.
const MAX_CACHED_HIGHLIGHT_LINES: usize = 100_000;

/// The majority of time in diff rendering is spent syntax highlighting. So we cache highlighted
/// lines by file path and raw line content.
///
/// Large files that take noticable time to highlight are also likely to contain many duplicate
/// lines, such as json files. Regular code files don't contain that many duplicate lines but
/// they're also unlikely to be big so they're fast to highlight.
#[derive(Debug, Default)]
struct HighlightCache {
    lines: HashMap<BString, HashMap<Box<str>, Vec<Span<'static>>>>,
    //             ^^^^^^^          ^^^^^^^^  ^^^^^^^^^^^^^^^^^^
    //             file path        raw line  highlighted line
    len: usize,
}

impl HighlightCache {
    fn get(&self, path: &BStr, code: &str) -> Option<&Vec<Span<'static>>> {
        self.lines.get(path)?.get(code)
    }

    fn insert(&mut self, path: &BStr, code: &str, spans: Vec<Span<'static>>) {
        if self.len >= MAX_CACHED_HIGHLIGHT_LINES {
            self.lines.clear();
            self.len = 0;
        }
        self.len += 1;
        if let Some(lines) = self.lines.get_mut(path) {
            lines.insert(Box::from(code), spans);
        } else {
            self.lines
                .insert(path.to_owned(), HashMap::from([(Box::from(code), spans)]));
        }
    }
}

fn syntax_highlight(
    code: &str,
    path: &BStr,
    bg: Option<Color>,
    highlighter: &mut Option<(HighlightLines<'_>, &SyntaxSet)>,
    cache: &mut HighlightCache,
) -> impl Iterator<Item = Span<'static>> {
    let spans = if let Some(cached_spans) = cache.get(path, code) {
        Some(cached_spans.clone())
    } else if let Some((highlight_lines, syntax_set)) = highlighter.as_mut() {
        highlight_lines
            .highlight_line(code, syntax_set)
            .ok()
            .map(|ranges| {
                let spans = ranges
                    .iter()
                    .map(|(style, text)| {
                        let color =
                            Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                        Span::raw(text.to_string()).fg(color)
                    })
                    .collect::<Vec<_>>();
                cache.insert(path, code, spans.clone());
                spans
            })
    } else {
        None
    };

    match spans {
        Some(spans) => Either::Left(spans.into_iter().map(move |span| {
            if let Some(background) = bg {
                span.bg(background)
            } else {
                span
            }
        })),
        None => Either::Right(once(Span::raw(code.to_owned()))),
    }
}

#[cfg(test)]
mod tests {
    use super::uncommitted_hunk_matches_selection;
    use bstr::BString;
    use but_core::{HunkHeader, ref_metadata::StackId};
    use but_hunk_assignment::HunkAssignment;
    use nonempty::NonEmpty;

    use crate::id::UncommittedHunkOrFile;

    fn hunk_assignment(path: &str, stack_id: Option<StackId>, old_start: u32) -> HunkAssignment {
        HunkAssignment {
            id: None,
            hunk_header: Some(HunkHeader {
                old_start,
                old_lines: 1,
                new_start: old_start,
                new_lines: 1,
            }),
            path: path.to_owned(),
            path_bytes: BString::from(path),
            stack_id,
            branch_ref_bytes: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        }
    }

    #[test]
    fn entire_file_selection_only_matches_same_path_and_stack() {
        let stack_a = StackId::from_number_for_testing(1);
        let stack_b = StackId::from_number_for_testing(2);
        let selected_hunk = hunk_assignment("file.txt", Some(stack_a), 1);
        let id = UncommittedHunkOrFile {
            id: "aa".to_owned(),
            hunk_assignments: NonEmpty::new(selected_hunk.clone()),
            is_entire_file: true,
        };

        assert!(uncommitted_hunk_matches_selection(
            &hunk_assignment("file.txt", Some(stack_a), 10),
            &id
        ));
        assert!(!uncommitted_hunk_matches_selection(
            &hunk_assignment("file.txt", None, 10),
            &id
        ));
        assert!(!uncommitted_hunk_matches_selection(
            &hunk_assignment("file.txt", Some(stack_b), 10),
            &id
        ));
        assert!(!uncommitted_hunk_matches_selection(
            &hunk_assignment("other.txt", Some(stack_a), 10),
            &id
        ));
    }

    #[test]
    fn single_hunk_selection_only_matches_that_hunk() {
        let stack_a = StackId::from_number_for_testing(1);
        let selected_hunk = hunk_assignment("file.txt", Some(stack_a), 1);
        let id = UncommittedHunkOrFile {
            id: "ab".to_owned(),
            hunk_assignments: NonEmpty::new(selected_hunk.clone()),
            is_entire_file: false,
        };

        assert!(uncommitted_hunk_matches_selection(&selected_hunk, &id));
        assert!(!uncommitted_hunk_matches_selection(
            &hunk_assignment("file.txt", Some(stack_a), 2),
            &id
        ));
        assert!(!uncommitted_hunk_matches_selection(
            &hunk_assignment("file.txt", None, 1),
            &id
        ));
    }
}
