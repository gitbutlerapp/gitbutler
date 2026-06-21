use std::{
    borrow::Cow,
    collections::HashMap,
    iter::{empty, once, repeat_n},
    sync::Arc,
    time::Instant,
};

use anyhow::{Context as _, bail};
use bstr::{BStr, BString, ByteSlice};
use but_core::{
    HunkHeader, UnifiedPatch,
    ui::{TreeChange, TreeStatus},
    unified_diff::DiffHunk,
};
use but_ctx::{Context, OnDemand};
use but_hunk_assignment::HunkAssignment;
use gix::actor::Signature;
use itertools::Either;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Stylize},
    text::{Line, Span, Text},
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
        MessageOnDrop, MoveMessage, RewordMessage, RubMessage,
        details::details_cursor::DetailsCursor, highlight, message_on_drop::message_on_drop,
        mode::CommittedHunk,
    },
    id::{UncommittedCliId, UncommittedHunk},
    theme::Theme,
};

use super::{HelpMessage, RubSource, StackMessage};

mod details_cursor;

#[derive(Debug, Clone)]
pub(super) enum DetailsMessage {
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

// The majority of time in diff rendering is spent syntax highlighting. So we cache highlighted
// lines.
//
// Large files that take noticable time to highlight are also likely to contain many duplicate
// lines, such as json files. Regular code files don't contain that many duplicate lines but
// they're also unlikely to be big so they're fast to highlight.
type LineHighlightCache = HashMap<BString, HashMap<Box<str>, Vec<Span<'static>>>>;
//                                ^^^^^^^          ^^^^^^^^  ^^^^^^^^^^^^^^^^^^
//                                file path        raw line  highlighted line

#[derive(Debug)]
pub(super) struct Details {
    is_dirty: bool,
    cursor: DetailsCursor,
    scroll_top: usize,
    widget: Option<DetailsAndDiffWidget>,
    renderer: IncrementalDiffRenderer,
    syntax_set: DebugAsType<OnDemand<SyntaxSet>>,
    syntax_theme: DebugAsType<OnDemand<highlighting::Theme>>,
    line_highlight_cache: LineHighlightCache,
    is_locked: bool,
    copied_hunk_highlight: highlight::Highlights<SectionId>,
    theme: &'static Theme,
}

impl Details {
    pub(super) fn new(theme: &'static Theme) -> Self {
        Self {
            is_dirty: false,
            is_locked: false,
            widget: Default::default(),
            renderer: Default::default(),
            cursor: Default::default(),
            scroll_top: 0,
            line_highlight_cache: Default::default(),
            copied_hunk_highlight: Default::default(),
            syntax_set: OnDemand::new(|| Ok(SyntaxSet::load_defaults_newlines())).into(),
            syntax_theme: OnDemand::new(|| theme.load_syntax_highlighting_theme()).into(),
            theme,
        }
    }

    pub(super) fn mark_dirty(&mut self) {
        self.widget = None;
        self.is_dirty = true;
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub(super) fn update_highlight(&mut self) -> bool {
        self.copied_hunk_highlight.update()
    }

    fn lock(&mut self, messages: &mut Vec<Message>) -> MessageOnDrop {
        self.is_locked = true;
        message_on_drop(Message::Details(DetailsMessage::Unlock), messages)
    }

    pub(super) fn unlock(&mut self) {
        if !self.is_locked {
            return;
        }
        self.is_locked = false;
        self.mark_dirty();
    }

    pub(super) fn needs_update(&self, is_visible: bool) -> bool {
        is_visible && self.is_dirty()
    }

    pub(super) fn reset_scroll(&mut self) {
        self.cursor = DetailsCursor::default();
        self.scroll_top = 0;
    }

    pub(super) fn needs_update_after_message(&self, is_visible: bool, msg: &Message) -> bool {
        if self.is_locked {
            return false;
        }

        if !is_visible {
            return false;
        }

        match msg {
            Message::JustRender
            | Message::CopySelection
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
            | Message::SelectUnassigned
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

            Message::AndThen { .. } => true,
        }
    }

    pub(super) fn try_handle_message(
        &mut self,
        msg: DetailsMessage,
        viewport: Rect,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        match msg {
            DetailsMessage::ScrollUp(n) => {
                self.scroll_top = self.scroll_top.saturating_sub(n);
            }
            DetailsMessage::ScrollDown(n) => {
                self.scroll_top = self.scroll_top.saturating_add(n);
            }
            DetailsMessage::SelectNextSection => {
                self.cursor
                    .move_selection_by(&self.renderer.sections, |i| i.saturating_add(1));

                self.ensure_selection_visible(viewport);
            }
            DetailsMessage::SelectPrevSection => {
                self.cursor
                    .move_selection_by(&self.renderer.sections, |i| i.saturating_sub(1));

                self.ensure_selection_visible(viewport);
            }
            DetailsMessage::GotoTop => {
                self.cursor
                    .move_selection_by(&self.renderer.sections, |_| 0);
                self.scroll_top = 0;
            }
            DetailsMessage::GotoBottom => {
                self.cursor
                    .move_selection_by(&self.renderer.sections, |_| usize::MAX);
                self.ensure_selection_visible(viewport);
            }
            DetailsMessage::Deselect => {
                self.cursor.deselect();
            }
            DetailsMessage::SelectFirstSection => {
                if let Some(section) = self.renderer.sections.first() {
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

    pub(super) fn ensure_selection_visible(&mut self, viewport: Rect) {
        let Some(selection) = self.cursor.selection() else {
            return;
        };

        let Some(widget) = self.widget.as_ref() else {
            return;
        };

        let content_width = details_content_width(viewport);
        let content_height = details_content_height(viewport);

        let Some((row_start, row_end)) = widget.section_row_range(selection, content_width) else {
            return;
        };

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

    pub(super) fn selection(&self) -> Option<&SectionId> {
        self.cursor.selection()
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
            .renderer
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
        for line in diff {
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
            .map(|diff| {
                diff.total_rows(content_width)
                    .saturating_add(self.renderer.pending_section_separator_count())
                    .saturating_sub(content_height)
            })
            .unwrap_or(0);

        self.scroll_top = self.scroll_top.min(max_scroll_top);
    }

    pub(super) fn update(
        &mut self,
        ctx: &mut Context,
        selection: Option<&CliId>,
    ) -> anyhow::Result<Option<RenderNextChunkResult>> {
        if let Some(widget) = &mut self.widget {
            let syntax_set = self.syntax_set.get()?;
            let theme = self.syntax_theme.get()?;
            let result = self.renderer.render_next_chunk(
                &syntax_set,
                &theme,
                &mut self.line_highlight_cache,
                self.theme,
                widget.diff_line_items_mut(),
            );
            match result {
                RenderNextChunkResult::Done => {
                    self.is_dirty = false;
                }
                RenderNextChunkResult::Meta | RenderNextChunkResult::Diff => {}
            }
            Ok(Some(result))
        } else {
            let Some(selection) = selection else {
                self.is_dirty = false;
                return Ok(None);
            };

            self.is_dirty = true;
            self.cursor = DetailsCursor::default();
            self.scroll_top = 0;
            self.renderer.clear();

            // reuse the allocation of the previous `DetailsAndDiffWidget`
            let previous_diff_line_items = self.widget.as_mut().map(|widget| {
                let buf = widget.diff_line_items_mut();
                buf.clear();
                std::mem::take(buf)
            });

            self.widget = match selection {
                CliId::Commit { commit_id, .. } => Some(from_commit(
                    ctx,
                    *commit_id,
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                    self.theme,
                )?),
                CliId::Uncommitted(uncommitted) => {
                    let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                    let id_map = IdMap::legacy_new_from_context(ctx, Some(wt_changes.assignments))?;
                    let uncommitted_hunks =
                        filter_uncommitted_hunks(ctx, &id_map, |hunk_assignment| {
                            uncommitted_hunk_matches_selection(hunk_assignment, uncommitted)
                        })?;
                    Some(from_uncommitted_hunks(
                        uncommitted_hunks,
                        &*self.syntax_set.get()?,
                        &mut self.renderer,
                        previous_diff_line_items,
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
                    &mut self.renderer,
                    previous_diff_line_items,
                    self.theme,
                )?),
                CliId::Branch { name, .. } => Some(from_branch(
                    ctx,
                    name.to_owned(),
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                    self.theme,
                )?),
                CliId::Unassigned { .. } => {
                    let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                    let id_map = IdMap::legacy_new_from_context(ctx, Some(wt_changes.assignments))?;
                    let uncommitted_hunks =
                        filter_uncommitted_hunks(ctx, &id_map, |hunk_assignment| {
                            hunk_assignment.stack_id.is_none()
                        })?;
                    Some(from_uncommitted_hunks(
                        uncommitted_hunks,
                        &*self.syntax_set.get()?,
                        &mut self.renderer,
                        previous_diff_line_items,
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
                        &mut self.renderer,
                        previous_diff_line_items,
                        self.theme,
                    )?)
                }
            };

            #[cfg(test)]
            {
                // the incremental rendering makes tests harder to write, so lets just render the
                // whole diff in test mode
                let widget = self.widget.as_mut().unwrap();
                let syntax_set = self.syntax_set.get().unwrap();
                let theme = self.syntax_theme.get().unwrap();
                loop {
                    match self.renderer.render_next_chunk(
                        &syntax_set,
                        &theme,
                        &mut self.line_highlight_cache,
                        self.theme,
                        widget.diff_line_items_mut(),
                    ) {
                        RenderNextChunkResult::Done => {
                            break;
                        }
                        RenderNextChunkResult::Meta | RenderNextChunkResult::Diff => {}
                    }
                }
            }

            Ok(None)
        }
    }

    pub(super) fn render(&self, help_shown: bool, has_focus: bool, area: Rect, frame: &mut Frame) {
        if let Some(diff) = &self.widget {
            diff.render(
                &self.cursor,
                self.scroll_top,
                area,
                frame,
                help_shown,
                has_focus,
                self.is_dirty,
                &self.copied_hunk_highlight,
                self.theme,
            );
        }
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
    uncommitted: &UncommittedCliId,
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
enum RenderedDiffLine {
    Separator,
    DiffLine {
        section_id: SectionId,
        item: ListItem<'static>,
    },
}

#[derive(Debug)]
enum DetailsAndDiffWidget {
    FromCommit {
        header_items: Vec<ListItem<'static>>,
        message: String,
        diff_line_items: Vec<RenderedDiffLine>,
    },
    FromDiffLines {
        diff_line_items: Vec<RenderedDiffLine>,
    },
}

impl DetailsAndDiffWidget {
    fn diff_line_items_mut(&mut self) -> &mut Vec<RenderedDiffLine> {
        match self {
            DetailsAndDiffWidget::FromCommit {
                diff_line_items, ..
            }
            | DetailsAndDiffWidget::FromDiffLines {
                diff_line_items, ..
            } => diff_line_items,
        }
    }

    fn total_rows(&self, width: u16) -> usize {
        match self {
            DetailsAndDiffWidget::FromCommit {
                header_items,
                message,
                diff_line_items,
                ..
            } => {
                header_items.len()
                    + 1 // +1 to match the empty line added in `render`
                    + textwrap::wrap(message, textwrap::Options::new(width as usize)).len()
                    + 1 // +1 to match the empty line added in `render`
                    + diff_line_items.len()
            }
            DetailsAndDiffWidget::FromDiffLines {
                diff_line_items, ..
            } => diff_line_items.len(),
        }
    }

    /// Returns the start and end (exclusive) row index for a rendered section.
    ///
    /// Row indexes are absolute in the same coordinate space as `Details::scroll_top`.
    fn section_row_range(&self, section: &SectionId, width: u16) -> Option<(usize, usize)> {
        let (rows_before_diff, diff_line_items) = match self {
            DetailsAndDiffWidget::FromCommit {
                header_items,
                message,
                diff_line_items,
            } => {
                let rows_before_diff = header_items.len()
                    + 1 // +1 to match the empty line added in `render`
                    + textwrap::wrap(message, textwrap::Options::new(width as usize)).len()
                    + 1; // +1 to match the empty line added in `render`
                (rows_before_diff, diff_line_items.as_slice())
            }
            DetailsAndDiffWidget::FromDiffLines { diff_line_items } => {
                (0, diff_line_items.as_slice())
            }
        };

        let first = diff_line_items
            .iter()
            .position(|line| matches!(line, RenderedDiffLine::DiffLine { section_id, .. } if section_id.eq(section)))?;

        let last = diff_line_items
            .iter()
            .rposition(|line| matches!(line, RenderedDiffLine::DiffLine { section_id, .. } if section_id.eq(section)))?;

        Some((
            rows_before_diff.saturating_add(first),
            rows_before_diff.saturating_add(last).saturating_add(1),
        ))
    }

    fn render(
        &self,
        cursor: &DetailsCursor,
        scroll_top: usize,
        area: Rect,
        buf: &mut Frame,
        help_shown: bool,
        has_focus: bool,
        is_dirty: bool,
        copied_hunk_highlight: &highlight::Highlights<SectionId>,
        theme: &'static Theme,
    ) {
        enum ListItemOrString<'a> {
            ListItem(&'a ListItem<'a>),
            ListItemInSection(&'a SectionId, &'a ListItem<'a>),
            Str(Cow<'a, str>),
        }

        let empty_list_item = ListItem::new("");

        let wrapped_message_iter = match self {
            DetailsAndDiffWidget::FromCommit { message, .. } => Some(
                textwrap::wrap(message, textwrap::Options::new(area.width as usize))
                    .into_iter()
                    .map(ListItemOrString::Str),
            ),
            DetailsAndDiffWidget::FromDiffLines { .. } => None,
        }
        .into_iter()
        .flatten();

        let mut items = match self {
            DetailsAndDiffWidget::FromCommit {
                header_items,
                diff_line_items,
                ..
            } => {
                let iter = empty()
                    .chain(header_items.iter().map(ListItemOrString::ListItem))
                    .chain([ListItemOrString::ListItem(&empty_list_item)])
                    .chain(wrapped_message_iter)
                    .chain([ListItemOrString::ListItem(&empty_list_item)])
                    .chain(diff_line_items.iter().map(|item| match item {
                        RenderedDiffLine::Separator => ListItemOrString::ListItem(&empty_list_item),
                        RenderedDiffLine::DiffLine { section_id, item } => {
                            ListItemOrString::ListItemInSection(section_id, item)
                        }
                    }));
                Either::Left(iter)
            }
            DetailsAndDiffWidget::FromDiffLines {
                diff_line_items, ..
            } => Either::Right(diff_line_items.iter().map(|item| match item {
                RenderedDiffLine::Separator => ListItemOrString::ListItem(&empty_list_item),
                RenderedDiffLine::DiffLine { section_id, item } => {
                    ListItemOrString::ListItemInSection(section_id, item)
                }
            })),
        }
        // ensure we `skip` and `take` before allocating anything
        .skip(scroll_top)
        .take(area.height as usize)
        .map(|item| match item {
            ListItemOrString::ListItem(list_item) => list_item.to_owned(),
            ListItemOrString::ListItemInSection(section_id, list_item) => {
                if copied_hunk_highlight.contains(section_id) {
                    list_item.to_owned().style(highlight::style())
                } else if !help_shown
                    && has_focus
                    && cursor
                        .selection()
                        .is_some_and(|selection| selection == section_id)
                {
                    list_item
                        .to_owned()
                        .style(theme.discrete_selection_highlight)
                } else {
                    list_item.to_owned()
                }
            }
            ListItemOrString::Str(cow) => ListItem::new(cow),
        })
        .peekable();

        if items.peek().is_some() {
            List::new(items).render(area, buf.buffer_mut());
        } else if !is_dirty {
            Span::styled("No changes", theme.hint).render(area, buf.buffer_mut());
        }
    }
}

fn from_commit(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<RenderedDiffLine>>,
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

    build_tree_changes(
        ctx,
        &tree_changes,
        Some(commit_id),
        syntax_set,
        renderer,
        theme,
    );

    Ok(DetailsAndDiffWidget::FromCommit {
        header_items,
        message,
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_uncommitted_hunks(
    uncommitted_hunks: Vec<(&str, Arc<CliId>, &UncommittedHunk)>,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<RenderedDiffLine>>,
    theme: &'static Theme,
) -> anyhow::Result<DetailsAndDiffWidget> {
    for (raw_id, cli_id, UncommittedHunk { hunk_assignment }) in uncommitted_hunks {
        let section = renderer.new_section_mut(SectionId::ShortId(cli_id));

        build_hunk_path_header(
            hunk_assignment.path_bytes.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(raw_id)),
            &mut section.content,
            theme,
        );

        build_hunk_assignment(hunk_assignment, syntax_set, theme, &mut section.content);
    }

    Ok(DetailsAndDiffWidget::FromDiffLines {
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_committed_file(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
    path: &BStr,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<RenderedDiffLine>>,
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

    build_tree_changes(
        ctx,
        &tree_changes,
        Some(commit_id),
        syntax_set,
        renderer,
        theme,
    );

    Ok(DetailsAndDiffWidget::FromDiffLines {
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_branch(
    ctx: &mut Context,
    name: String,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<RenderedDiffLine>>,
    theme: &'static Theme,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let tree_changes = but_api::branch::branch_diff(ctx, name)?;

    build_tree_changes(
        ctx,
        &tree_changes.changes,
        None,
        syntax_set,
        renderer,
        theme,
    );

    Ok(DetailsAndDiffWidget::FromDiffLines {
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

/// Rendering large diffs is expensive. `IncrementalDiffRenderer` enables rendering diffs
/// incrementally, in chunks. So instead of rendering the entire diff when activating the detail
/// view we instead render `chunk_size` items per frame. That way the TUI remains response even
/// while we're rendering a large diff.
#[derive(Debug)]
struct IncrementalDiffRenderer {
    sections: Vec<PartiallyRenderedDiffSection>,
    state: IncrementalDiffRendererState,
    /// How many diff lines to process on each update.
    ///
    /// Start with a small initial chunk size so the first diff render is quick if the
    /// initial chunk size is too large there is a noticable delay between opening the
    /// details view and the diff appearing. However if the chunk size remains small the
    /// diff takes a while to render. So make it small initially and double it as we render
    /// more.
    chunk_size: usize,
    created_at: Instant,
}

#[derive(Debug)]
enum IncrementalDiffRendererState {
    Top {
        section_idx: usize,
        diff_idx: usize,
    },
    Diff {
        section_idx: usize,
        diff_idx: usize,
        line_idx: usize,
        old_line_num: u32,
        new_line_num: u32,
    },
}

/// An id only used by the TUI to identify this section. Doesn't have any meaning in the
/// rest of the system.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) struct TuiId(Uuid);

impl TuiId {
    fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum SectionId {
    ShortId(Arc<CliId>),
    CommittedHunk { id: TuiId, hunk: CommittedHunk },
    Opaque(TuiId),
}

#[derive(Debug)]
struct PartiallyRenderedDiffSection {
    id: SectionId,
    content: Vec<SectionContent>,
}

/// The content of a section. This has not been fully rendered yet.
/// `IncrementalDiffRenderer::render_next_chunk` does that and turns the content into
/// `DiffLineItem` which represents the diff thats actually rendered by ratatui.
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
        old_start: u32,
        new_start: u32,
        syntax: Box<SyntaxReference>,
        diff: Vec<Box<[u8]>>,
    },
}

/// The result of rendering the one diff chunk.
pub(super) enum RenderNextChunkResult {
    /// We're done. All chunks have been rendered.
    Done,
    /// Some meta data, such as the commit messages was rendered.
    Meta,
    /// Some diff lines were rendered.
    Diff,
}

impl Default for IncrementalDiffRenderer {
    fn default() -> Self {
        Self {
            sections: Default::default(),
            state: IncrementalDiffRendererState::Top {
                section_idx: 0,
                diff_idx: 0,
            },
            chunk_size: 500,
            created_at: Instant::now(),
        }
    }
}

impl IncrementalDiffRenderer {
    fn section_separator_count(&self) -> usize {
        self.sections.len().saturating_sub(1)
    }

    fn rendered_section_separator_count(&self) -> usize {
        let current_section_idx = match self.state {
            IncrementalDiffRendererState::Top { section_idx, .. }
            | IncrementalDiffRendererState::Diff { section_idx, .. } => section_idx,
        };

        current_section_idx.min(self.section_separator_count())
    }

    fn pending_section_separator_count(&self) -> usize {
        self.section_separator_count()
            .saturating_sub(self.rendered_section_separator_count())
    }

    fn new_section_mut(&mut self, id: SectionId) -> &mut PartiallyRenderedDiffSection {
        let section = PartiallyRenderedDiffSection {
            id,
            content: Default::default(),
        };
        self.sections.push(section);
        self.sections.last_mut().unwrap()
    }

    /// Clear any internal state so the allocations can be reused.
    fn clear(&mut self) {
        let Self {
            sections,
            state,
            chunk_size,
            created_at,
        } = self;

        sections.clear();

        let default = Self::default();
        *state = default.state;
        *chunk_size = default.chunk_size;
        *created_at = default.created_at;
    }

    fn render_next_chunk(
        &mut self,
        syntax_set: &SyntaxSet,
        syntax_theme: &highlighting::Theme,
        cache: &mut LineHighlightCache,
        theme: &'static Theme,
        out: &mut Vec<RenderedDiffLine>,
    ) -> RenderNextChunkResult {
        loop {
            match self.state {
                IncrementalDiffRendererState::Top {
                    section_idx,
                    diff_idx,
                } => {
                    if section_idx >= self.sections.len() {
                        break RenderNextChunkResult::Done;
                    }

                    let section_len = self.sections[section_idx].content.len();
                    if diff_idx >= section_len {
                        self.state = IncrementalDiffRendererState::Top {
                            section_idx: section_idx + 1,
                            diff_idx: 0,
                        };

                        // render separator if there is one more section
                        if section_idx + 1 < self.sections.len() {
                            out.push(RenderedDiffLine::Separator);
                        }

                        continue;
                    }
                }
                IncrementalDiffRendererState::Diff { .. } => {}
            }

            match &mut self.state {
                IncrementalDiffRendererState::Top {
                    section_idx,
                    diff_idx,
                } => {
                    let PartiallyRenderedDiffSection { content: diffs, id } =
                        &mut self.sections[*section_idx];
                    match &mut diffs[*diff_idx] {
                        SectionContent::FileHeader(list_items) => {
                            out.extend(std::mem::take(list_items).into_iter().map(|item| {
                                RenderedDiffLine::DiffLine {
                                    section_id: id.clone(),
                                    item,
                                }
                            }));

                            self.state = IncrementalDiffRendererState::Top {
                                section_idx: *section_idx,
                                diff_idx: (*diff_idx) + 1,
                            };
                            break RenderNextChunkResult::Meta;
                        }
                        SectionContent::HunkHeader(list_items) => {
                            for item in std::mem::replace(
                                list_items,
                                [
                                    ListItem::from(Text::default()),
                                    ListItem::from(Text::default()),
                                ],
                            ) {
                                out.push(RenderedDiffLine::DiffLine {
                                    item,
                                    section_id: id.clone(),
                                });
                            }
                            self.state = IncrementalDiffRendererState::Top {
                                section_idx: *section_idx,
                                diff_idx: (*diff_idx) + 1,
                            };
                            break RenderNextChunkResult::Meta;
                        }
                        SectionContent::DiffUnavailable(message) => {
                            out.push(RenderedDiffLine::DiffLine {
                                item: ListItem::from(std::mem::take(message)),
                                section_id: id.clone(),
                            });
                            self.state = IncrementalDiffRendererState::Top {
                                section_idx: *section_idx,
                                diff_idx: (*diff_idx) + 1,
                            };
                            break RenderNextChunkResult::Meta;
                        }
                        SectionContent::DiffLines {
                            old_start,
                            new_start,
                            ..
                        } => {
                            self.state = IncrementalDiffRendererState::Diff {
                                section_idx: *section_idx,
                                diff_idx: *diff_idx,
                                // the first line is the `@@ -1,6 +1,8 @@` header, skip that
                                line_idx: 1,
                                old_line_num: *old_start,
                                new_line_num: *new_start,
                            };
                        }
                    }
                }
                IncrementalDiffRendererState::Diff {
                    section_idx,
                    diff_idx,
                    line_idx,
                    old_line_num,
                    new_line_num,
                } => {
                    let PartiallyRenderedDiffSection { content: diffs, id } =
                        &mut self.sections[*section_idx];
                    let SectionContent::DiffLines {
                        path,
                        old_width,
                        new_width,
                        syntax,
                        diff,
                        old_start: _,
                        new_start: _,
                    } = &mut diffs[*diff_idx]
                    else {
                        unreachable!();
                    };

                    if *line_idx >= diff.len() {
                        self.state = IncrementalDiffRendererState::Top {
                            section_idx: *section_idx,
                            diff_idx: (*diff_idx) + 1,
                        };
                        continue;
                    }

                    let mut highlight_lines = HighlightLines::new(syntax.as_ref(), syntax_theme);

                    for line in diff.iter().skip(*line_idx).take(self.chunk_size) {
                        *line_idx += 1;

                        let item = if let Some(rest) = line.strip_prefix(b"+") {
                            let code = rest.to_str_lossy().to_string();
                            let item = ListItem::new(Line::from_iter(
                                [
                                    Span::raw(" ".repeat(*old_width as _)),
                                    Span::styled(" ┊ ", theme.border),
                                    Span::raw(
                                        " ".repeat((*new_width - num_digits(*new_line_num)) as _),
                                    ),
                                    Span::raw(new_line_num.to_string()).style(theme.addition),
                                    Span::styled(" │ ", theme.border),
                                    Span::raw("+").style(theme.addition_rich),
                                ]
                                .into_iter()
                                .chain(syntax_highlight(
                                    &code,
                                    path.as_ref(),
                                    theme.addition_rich.bg,
                                    &mut highlight_lines,
                                    syntax_set,
                                    cache,
                                )),
                            ));
                            *new_line_num += 1;
                            item
                        } else if let Some(rest) = line.strip_prefix(b"-") {
                            let code = rest.to_str_lossy().to_string();
                            let item = ListItem::new(Line::from_iter(
                                [
                                    Span::raw(
                                        " ".repeat((*old_width - num_digits(*old_line_num)) as _),
                                    ),
                                    Span::raw(old_line_num.to_string()).style(theme.deletion),
                                    Span::styled(" ┊ ", theme.border),
                                    Span::raw(" ".repeat(*new_width as _)),
                                    Span::styled(" │ ", theme.border),
                                    Span::raw("-").style(theme.deletion_rich),
                                ]
                                .into_iter()
                                .chain(syntax_highlight(
                                    &code,
                                    path.as_ref(),
                                    theme.deletion_rich.bg,
                                    &mut highlight_lines,
                                    syntax_set,
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
                                    Span::raw(
                                        " ".repeat((*old_width - num_digits(*old_line_num)) as _),
                                    ),
                                    Span::styled(old_line_num.to_string(), theme.hint),
                                    Span::styled(" ┊ ", theme.border),
                                    Span::raw(
                                        " ".repeat((*new_width - num_digits(*new_line_num)) as _),
                                    ),
                                    Span::styled(new_line_num.to_string(), theme.hint),
                                    Span::styled(" │  ", theme.border),
                                ]
                                .into_iter()
                                .chain(syntax_highlight(
                                    &code,
                                    path.as_ref(),
                                    None,
                                    &mut highlight_lines,
                                    syntax_set,
                                    cache,
                                )),
                            ));
                            *old_line_num += 1;
                            *new_line_num += 1;
                            item
                        };
                        out.push(RenderedDiffLine::DiffLine {
                            section_id: id.clone(),
                            item,
                        });
                    }

                    self.chunk_size = std::cmp::min(self.chunk_size.saturating_mul(2), 10_000);

                    break RenderNextChunkResult::Diff;
                }
            }
        }
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
    renderer: &mut IncrementalDiffRenderer,
    theme: &'static Theme,
) {
    for tree_change in tree_changes {
        if let Some(patch) = but_api::diff::tree_change_diffs(ctx, tree_change.clone())
            .ok()
            .flatten()
        {
            match patch {
                UnifiedPatch::Patch {
                    hunks,
                    is_result_of_binary_to_text_conversion,
                    lines_added: _,
                    lines_removed: _,
                } => {
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
                        let section = renderer.new_section_mut(section_id);

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
                    let section = renderer.new_section_mut(SectionId::Opaque(TuiId::new()));

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
                    let section = renderer.new_section_mut(SectionId::Opaque(TuiId::new()));

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
                    .flat_map(|status| [status, Span::raw(": ")]),
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
                    .flat_map(|status| [status, Span::raw(": ")]),
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

    let (old_width, new_width) = {
        let mut old_line = old_start;
        let mut new_line = new_start;
        for line in diff.lines().skip(1) {
            if line.starts_with(b"+") {
                new_line += 1;
            } else if line.starts_with(b"-") {
                old_line += 1;
            } else {
                old_line += 1;
                new_line += 1;
            }
        }
        (num_digits(old_line), num_digits(new_line))
    };

    let diff_lines = diff.lines().map(Box::<[u8]>::from).collect::<Vec<_>>();

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
        old_start,
        new_start,
        syntax: Box::new(syntax.to_owned()),
        diff: diff_lines,
    });
}

fn num_digits(n: u32) -> u32 {
    if n == 0 { 1 } else { n.ilog10() + 1 }
}

fn syntax_highlight(
    code: &str,
    path: &BStr,
    bg: Option<Color>,
    highlight_lines: &mut HighlightLines<'_>,
    syntax_set: &SyntaxSet,
    cache: &mut LineHighlightCache,
) -> impl Iterator<Item = Span<'static>> {
    loop {
        if let Some(cached_spans) = cache.get(path).and_then(|cache| cache.get(code)) {
            return Either::Left(cached_spans.clone().into_iter().map(move |span| {
                if let Some(background) = bg {
                    span.bg(background)
                } else {
                    span
                }
            }));
        }

        let Ok(ranges) = highlight_lines.highlight_line(code, syntax_set) else {
            return Either::Right(once(Span::raw(code.to_owned())));
        };

        if let Some(lines) = cache.get_mut(path) {
            lines.insert(
                Box::from(code),
                ranges
                    .iter()
                    .map(|(style, text)| {
                        let color =
                            Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                        Span::raw(text.to_string()).fg(color)
                    })
                    .collect(),
            );
        } else {
            cache.insert(path.to_owned(), Default::default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::uncommitted_hunk_matches_selection;
    use bstr::BString;
    use but_core::{HunkHeader, ref_metadata::StackId};
    use but_hunk_assignment::HunkAssignment;
    use nonempty::NonEmpty;

    use crate::id::UncommittedCliId;

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
        let id = UncommittedCliId {
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
        let id = UncommittedCliId {
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
