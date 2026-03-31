use std::{
    borrow::Cow,
    collections::HashMap,
    iter::{empty, once, repeat_n},
    sync::LazyLock,
    time::Instant,
};

use bstr::{BStr, BString, ByteSlice};
use but_core::{
    UnifiedPatch,
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
    palette::Hsl,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Widget},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    CliId, IdMap,
    command::legacy::status::tui::{
        CommandMessage, CommitMessage, DebugAsType, FilesMessage, Message, MoveMessage,
        RewordMessage, RubMessage, details::details_cursor::DetailsCursor,
    },
    id::{UncommittedCliId, UncommittedHunk},
};

use super::BranchMessage;

mod details_cursor;

// we don't currently compute word level diffs so MINUS_EMPH_BG and PLUS_EMPH_BG aren't used (in
// the diff lines themselves). Without that MINUS_BG and PLUS_BG are a little too hard to see, so
// this adjustment is applied to make them more clear.
const LIGHTNESS_ADJUSTMENT: f32 = 0.05;

// colors from delta with slight adjustment
static MINUS_BG: LazyLock<Color> =
    LazyLock::new(|| Color::from_hsl(Hsl::new(-0.952, 1.0, 0.123 + LIGHTNESS_ADJUSTMENT)));
static PLUS_BG: LazyLock<Color> =
    LazyLock::new(|| Color::from_hsl(Hsl::new(120.0, 1.0, 0.078 + LIGHTNESS_ADJUSTMENT)));

static MINUS_EMPH_BG: LazyLock<Color> =
    LazyLock::new(|| Color::from_hsl(Hsl::new(-0.468, 0.8, 0.313)));
static PLUS_EMPH_BG: LazyLock<Color> =
    LazyLock::new(|| Color::from_hsl(Hsl::new(120.0, 1.0, 0.188)));

const MONOKAI_THEME: &[u8] =
    include_bytes!("../../../../../../assets/syntax-highlighting-themes/Monokai Extended.tmTheme");

#[derive(Debug, Default, Copy, Clone)]
pub(super) enum DetailsVisibility {
    #[default]
    Hidden,
    VisibleVertical {
        focused: bool,
    },
}

#[derive(Debug, Clone)]
pub(super) enum DetailsMessage {
    ScrollUp(usize),
    ScrollDown(usize),
    ToggleVisibility,
    ToggleFocus,
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
    widget: Option<DetailsAndDiffWidget>,
    renderer: IncrementalDiffRenderer,
    syntax_set: DebugAsType<OnDemand<SyntaxSet>>,
    dark_theme: DebugAsType<OnDemand<Theme>>,
    visibility: DetailsVisibility,
    line_highlight_cache: LineHighlightCache,
}

impl Details {
    pub(super) fn new_hidden() -> Self {
        Self {
            is_dirty: false,
            widget: Default::default(),
            renderer: Default::default(),
            cursor: Default::default(),
            visibility: Default::default(),
            line_highlight_cache: Default::default(),
            syntax_set: OnDemand::new(|| Ok(SyntaxSet::load_defaults_newlines())).into(),
            dark_theme: OnDemand::new(|| {
                Ok(ThemeSet::load_from_reader(&mut std::io::Cursor::new(MONOKAI_THEME)).unwrap())
            })
            .into(),
        }
    }

    pub(super) fn new_visible() -> Self {
        Self {
            is_dirty: true,
            visibility: DetailsVisibility::VisibleVertical { focused: false },
            ..Self::new_hidden()
        }
    }

    pub(super) fn mark_dirty(&mut self) {
        self.widget = None;
        self.is_dirty = true;
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub(super) fn visibility(&self) -> DetailsVisibility {
        self.visibility
    }

    pub(super) fn is_visible(&self) -> bool {
        match self.visibility {
            DetailsVisibility::Hidden => false,
            DetailsVisibility::VisibleVertical { .. } => true,
        }
    }

    pub(super) fn is_focused(&self) -> bool {
        match self.visibility {
            DetailsVisibility::VisibleVertical { focused } => focused,
            DetailsVisibility::Hidden => false,
        }
    }

    pub(super) fn needs_update(&self) -> bool {
        self.is_visible() && self.is_dirty()
    }

    pub(super) fn needs_update_after_message(&self, msg: &Message) -> bool {
        match self.visibility {
            DetailsVisibility::Hidden => return false,
            DetailsVisibility::VisibleVertical { .. } => {}
        }

        match msg {
            Message::JustRender
            | Message::CopySelection
            | Message::Quit
            | Message::ShowError(_)
            | Message::ShowToast { .. }
            | Message::Confirm(_)
            | Message::EnterNormalMode => false,

            Message::MoveCursorUp
            | Message::MoveCursorDown
            | Message::MoveCursorPreviousSection
            | Message::MoveCursorNextSection
            | Message::Reload(_)
            | Message::RunAfterConfirmation(_) => true,

            Message::Commit(commit_message) => match commit_message {
                CommitMessage::Confirm { .. } | CommitMessage::CreateEmpty => true,
                CommitMessage::Start | CommitMessage::SetInsertSide(_) => false,
            },
            Message::Rub(rub_message) => match rub_message {
                RubMessage::Start { .. } => false,
                RubMessage::Confirm => true,
            },
            Message::Reword(reword_message) => match reword_message {
                RewordMessage::WithEditor | RewordMessage::InlineConfirm => true,
                RewordMessage::InlineStart | RewordMessage::InlineInput(_) => false,
            },
            Message::Command(command_message) => match command_message {
                CommandMessage::Start | CommandMessage::Input(_) => false,
                CommandMessage::Confirm => true,
            },
            Message::Files(files_message) => match files_message {
                FilesMessage::ToggleGlobalFilesList | FilesMessage::ToggleFilesForCommit => true,
            },
            Message::Move(move_message) => match move_message {
                MoveMessage::Start | MoveMessage::SetInsertSide(_) => false,
                MoveMessage::Confirm => true,
            },
            Message::Branch(branch_message) => match branch_message {
                BranchMessage::Start => false,
                BranchMessage::New => true,
            },
            Message::Details(details_message) => match details_message {
                DetailsMessage::ScrollUp(_)
                | DetailsMessage::ScrollDown(_)
                | DetailsMessage::ToggleFocus
                | DetailsMessage::ToggleVisibility => false,
            },
        }
    }

    pub(super) fn try_handle_message(
        &mut self,
        msg: DetailsMessage,
        viewport: Rect,
    ) -> anyhow::Result<()> {
        match msg {
            DetailsMessage::ScrollUp(n) => {
                self.cursor = self.cursor.scroll_up(n);
            }
            DetailsMessage::ScrollDown(n) => {
                self.cursor = self.cursor.scroll_down(n);
            }
            DetailsMessage::ToggleVisibility => {
                self.visibility = match self.visibility {
                    DetailsVisibility::Hidden => {
                        DetailsVisibility::VisibleVertical { focused: false }
                    }
                    DetailsVisibility::VisibleVertical { .. } => DetailsVisibility::Hidden,
                };

                match self.visibility {
                    DetailsVisibility::Hidden => {
                        self.cursor = DetailsCursor::default();
                    }
                    DetailsVisibility::VisibleVertical { .. } => {
                        self.mark_dirty();
                    }
                }
            }
            DetailsMessage::ToggleFocus => match &mut self.visibility {
                DetailsVisibility::Hidden => {}
                DetailsVisibility::VisibleVertical { focused } => {
                    *focused = !*focused;
                }
            },
        }

        self.clamp_scroll_top(viewport);

        Ok(())
    }

    fn clamp_scroll_top(&mut self, viewport: Rect) {
        self.cursor = self.cursor.clamp(
            viewport,
            self.widget.as_ref(),
            self.renderer.pending_section_separator_count(),
        );
    }

    pub(super) fn update(
        &mut self,
        ctx: &mut Context,
        selection: Option<&CliId>,
    ) -> anyhow::Result<Option<RenderNextChunkResult>> {
        if let Some(widget) = &mut self.widget {
            let syntax_set = self.syntax_set.get()?;
            let theme = self.dark_theme.get()?;
            let result = self.renderer.render_next_chunk(
                &syntax_set,
                &theme,
                &mut self.line_highlight_cache,
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
            self.renderer.clear();

            // reuse the allocation of the previous `DetailsAndDiffWidget`
            let previous_diff_line_items = self.widget.as_mut().map(|widget| {
                let buf = widget.diff_line_items_mut();
                buf.clear();
                std::mem::take(buf)
            });

            self.widget = Some(match selection {
                CliId::Commit { commit_id, .. } => from_commit(
                    ctx,
                    *commit_id,
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                )?,
                CliId::Uncommitted(uncommitted) => {
                    let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                    let id_map = IdMap::new_from_context(ctx, Some(wt_changes.assignments))?;
                    let uncommitted_hunks = filter_uncommitted_hunks(&id_map, |hunk_assignment| {
                        uncommitted_hunk_matches_selection(hunk_assignment, uncommitted)
                    })?;
                    from_uncommitted_hunks(
                        uncommitted_hunks,
                        &*self.syntax_set.get()?,
                        &mut self.renderer,
                        previous_diff_line_items,
                    )?
                }
                CliId::PathPrefix {
                    hunk_assignments, ..
                } => from_path_prefix(
                    hunk_assignments,
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                )?,
                CliId::CommittedFile {
                    commit_id, path, ..
                } => from_committed_file(
                    ctx,
                    *commit_id,
                    path.as_ref(),
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                )?,
                CliId::Branch { name, .. } => from_branch(
                    ctx,
                    name.to_owned(),
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                )?,
                CliId::Unassigned { .. } => {
                    let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                    let id_map = IdMap::new_from_context(ctx, Some(wt_changes.assignments))?;
                    let uncommitted_hunks = filter_uncommitted_hunks(&id_map, |hunk_assignment| {
                        hunk_assignment.stack_id.is_none()
                    })?;
                    from_uncommitted_hunks(
                        uncommitted_hunks,
                        &*self.syntax_set.get()?,
                        &mut self.renderer,
                        previous_diff_line_items,
                    )?
                }
                CliId::Stack { stack_id, .. } => {
                    let wt_changes = but_api::diff::changes_in_worktree(ctx)?;
                    let id_map = IdMap::new_from_context(ctx, Some(wt_changes.assignments))?;
                    let uncommitted_hunks = filter_uncommitted_hunks(&id_map, |hunk_assignment| {
                        hunk_assignment.stack_id.is_some_and(|id| id == *stack_id)
                    })?;
                    from_uncommitted_hunks(
                        uncommitted_hunks,
                        &*self.syntax_set.get()?,
                        &mut self.renderer,
                        previous_diff_line_items,
                    )?
                }
            });

            #[cfg(test)]
            {
                // the incremental rendering makes tests harder to write, so lets just render the
                // whole diff in test mode
                let widget = self.widget.as_mut().unwrap();
                let syntax_set = self.syntax_set.get().unwrap();
                let theme = self.dark_theme.get().unwrap();
                loop {
                    match self.renderer.render_next_chunk(
                        &syntax_set,
                        &theme,
                        &mut self.line_highlight_cache,
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

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let outer_block = Block::bordered()
            .borders(Borders::LEFT)
            .border_style(Style::default().dim());
        let inner_area = outer_block.inner(area);
        frame.render_widget(outer_block, area);

        if let Some(diff) = &self.widget {
            diff.render(self.cursor, inner_area, frame);
        }
    }
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

fn filter_uncommitted_hunks<F>(
    id_map: &IdMap,
    mut filter: F,
) -> anyhow::Result<Vec<(&String, &UncommittedHunk)>>
where
    F: FnMut(&HunkAssignment) -> bool,
{
    let mut uncommitted_hunks = id_map
        .uncommitted_hunks
        .iter()
        .filter(move |(_, hunk)| filter(&hunk.hunk_assignment))
        .collect::<Vec<_>>();

    uncommitted_hunks.sort_by(|(id_a, hunk_a), (id_b, hunk_b)| {
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
        diff_line_items: Vec<ListItem<'static>>,
    },
    FromDiffLines {
        diff_line_items: Vec<ListItem<'static>>,
    },
}

impl DetailsAndDiffWidget {
    fn diff_line_items_mut(&mut self) -> &mut Vec<ListItem<'static>> {
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

    fn render(&self, cursor: DetailsCursor, area: Rect, buf: &mut Frame) {
        enum ListItemOrString<'a> {
            ListItem(&'a ListItem<'a>),
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

        let items = match self {
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
                    .chain(diff_line_items.iter().map(ListItemOrString::ListItem));
                Either::Left(iter)
            }
            DetailsAndDiffWidget::FromDiffLines {
                diff_line_items, ..
            } => Either::Right(diff_line_items.iter().map(ListItemOrString::ListItem)),
        }
        // ensure we `skip` and `take` before allocating anything
        .skip(cursor.scroll_top())
        .take(area.height as usize)
        .map(|item| match item {
            ListItemOrString::ListItem(list_item) => list_item.to_owned(),
            ListItemOrString::Str(cow) => ListItem::new(cow),
        });

        List::new(items).render(area, buf.buffer_mut());
    }
}

fn from_commit(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let commit_details =
        but_api::diff::commit_details(ctx, commit_id, but_api::diff::ComputeLineStats::No)?;

    let header_items = Vec::from([
        ListItem::new(Line::from_iter([
            Span::raw(format!("{:<11}", "Commit ID:")),
            Span::raw(commit_id.to_hex().to_string()).blue(),
        ])),
        ListItem::new(Line::from_iter(
            once(Span::raw(format!("{:<11}", "Author:")))
                .chain(render_signature(&commit_details.commit.author)),
        )),
        ListItem::new(Line::from_iter(
            once(Span::raw(format!("{:<11}", "Committer:")))
                .chain(render_signature(&commit_details.commit.committer)),
        )),
    ]);

    let message = commit_details.commit.message.to_string();

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();

    build_tree_changes(ctx, &tree_changes, syntax_set, renderer);

    Ok(DetailsAndDiffWidget::FromCommit {
        header_items,
        message,
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_uncommitted_hunks(
    uncommitted_hunks: Vec<(&String, &UncommittedHunk)>,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    for (id, UncommittedHunk { hunk_assignment }) in uncommitted_hunks {
        let section = renderer.new_section_mut(SectionId::ShortId(id.to_owned()));

        build_hunk_path_header(
            hunk_assignment.path_bytes.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(id)),
            &mut section.diffs,
        );

        build_hunk_assignment(hunk_assignment, syntax_set, &mut section.diffs);
    }

    Ok(DetailsAndDiffWidget::FromDiffLines {
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_path_prefix<'a>(
    hunk_assignments: impl IntoIterator<Item = &'a (String, HunkAssignment)>,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    for (id, hunk_assignment) in hunk_assignments {
        let section = renderer.new_section_mut(SectionId::ShortId(id.to_owned()));

        build_hunk_path_header(
            hunk_assignment.path_bytes.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(id)),
            &mut section.diffs,
        );

        build_hunk_assignment(hunk_assignment, syntax_set, &mut section.diffs);
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
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let commit_details =
        but_api::diff::commit_details(ctx, commit_id, but_api::diff::ComputeLineStats::No)?;

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .filter(|change| change.path == path)
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();

    build_tree_changes(ctx, &tree_changes, syntax_set, renderer);

    Ok(DetailsAndDiffWidget::FromDiffLines {
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_branch(
    ctx: &mut Context,
    name: String,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let tree_changes = but_api::branch::branch_diff(ctx, name)?;

    build_tree_changes(ctx, &tree_changes.changes, syntax_set, renderer);

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

#[derive(Debug, Clone)]
enum SectionId {
    ShortId(String),
    TreeChange(uuid::Uuid),
}

#[derive(Debug)]
struct PartiallyRenderedDiffSection {
    id: SectionId,
    diffs: Vec<PartiallyRenderedDiff>,
}

/// A diff thats been partially rendered.
///
/// Used with `IncrementalDiffRenderer` which can incrementally render the final diff.
#[derive(Debug)]
enum PartiallyRenderedDiff {
    Header(Vec<ListItem<'static>>),
    SingleLine(ListItem<'static>),
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
            diffs: Default::default(),
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
        theme: &Theme,
        cache: &mut LineHighlightCache,
        out: &mut Vec<ListItem<'static>>,
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

                    let section_len = self.sections[section_idx].diffs.len();
                    if diff_idx >= section_len {
                        self.state = IncrementalDiffRendererState::Top {
                            section_idx: section_idx + 1,
                            diff_idx: 0,
                        };

                        // render separator if there is one more section
                        if section_idx + 1 < self.sections.len() {
                            out.push(ListItem::new(""));
                        }

                        continue;
                    }
                }
                IncrementalDiffRendererState::Diff { .. } => {}
            }

            // TODO: This color will eventually be used for the section background highlight for
            // selected sections.
            let bg = Color::Reset;

            match &mut self.state {
                IncrementalDiffRendererState::Top {
                    section_idx,
                    diff_idx,
                } => {
                    let PartiallyRenderedDiffSection { diffs, id: _ } =
                        &mut self.sections[*section_idx];
                    match &mut diffs[*diff_idx] {
                        PartiallyRenderedDiff::Header(list_items) => {
                            // out.append(list_items);
                            out.extend(
                                std::mem::take(list_items)
                                    .into_iter()
                                    .map(|item| item.bg(bg)),
                            );

                            self.state = IncrementalDiffRendererState::Top {
                                section_idx: *section_idx,
                                diff_idx: (*diff_idx) + 1,
                            };
                            break RenderNextChunkResult::Meta;
                        }
                        PartiallyRenderedDiff::SingleLine(list_item) => {
                            out.push(
                                std::mem::replace(list_item, ListItem::from(Text::default()))
                                    .bg(bg),
                            );
                            self.state = IncrementalDiffRendererState::Top {
                                section_idx: *section_idx,
                                diff_idx: (*diff_idx) + 1,
                            };
                            break RenderNextChunkResult::Meta;
                        }
                        PartiallyRenderedDiff::DiffLines {
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
                    let PartiallyRenderedDiffSection { diffs, id: _ } =
                        &mut self.sections[*section_idx];
                    let PartiallyRenderedDiff::DiffLines {
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

                    let mut highlight_lines = HighlightLines::new(syntax.as_ref(), theme);

                    for line in diff.iter().skip(*line_idx).take(self.chunk_size) {
                        *line_idx += 1;

                        let item = if let Some(rest) = line.strip_prefix(b"+") {
                            let code = rest.to_str_lossy().to_string();
                            let item = ListItem::new(Line::from_iter(
                                [
                                    Span::raw(" ".repeat(*old_width as _)),
                                    Span::raw(" ┊ ").dim(),
                                    Span::raw(
                                        " ".repeat((*new_width - num_digits(*new_line_num)) as _),
                                    ),
                                    Span::raw(new_line_num.to_string()).fg(*PLUS_EMPH_BG),
                                    Span::raw(" │ ").dim(),
                                    Span::raw("+").bg(*PLUS_BG),
                                ]
                                .into_iter()
                                .chain(syntax_highlight(
                                    &code,
                                    path.as_ref(),
                                    Some(*PLUS_BG),
                                    &mut highlight_lines,
                                    syntax_set,
                                    cache,
                                )),
                            ))
                            .bg(bg);
                            *new_line_num += 1;
                            item
                        } else if let Some(rest) = line.strip_prefix(b"-") {
                            let code = rest.to_str_lossy().to_string();
                            let item = ListItem::new(Line::from_iter(
                                [
                                    Span::raw(
                                        " ".repeat((*old_width - num_digits(*old_line_num)) as _),
                                    ),
                                    Span::raw(old_line_num.to_string()).fg(*MINUS_EMPH_BG),
                                    Span::raw(" ┊ ").dim(),
                                    Span::raw(" ".repeat(*new_width as _)),
                                    Span::raw(" │ ").dim(),
                                    Span::raw("-").bg(*MINUS_BG),
                                ]
                                .into_iter()
                                .chain(syntax_highlight(
                                    &code,
                                    path.as_ref(),
                                    Some(*MINUS_BG),
                                    &mut highlight_lines,
                                    syntax_set,
                                    cache,
                                )),
                            ))
                            .bg(bg);
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
                                    Span::raw(old_line_num.to_string()).dark_gray(),
                                    Span::raw(" ┊ ").dim(),
                                    Span::raw(
                                        " ".repeat((*new_width - num_digits(*new_line_num)) as _),
                                    ),
                                    Span::raw(new_line_num.to_string()).dark_gray(),
                                    Span::raw(" │  ").dim(),
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
                            ))
                            .bg(bg);
                            *old_line_num += 1;
                            *new_line_num += 1;
                            item
                        };
                        out.push(item);
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
    out: &mut Vec<PartiallyRenderedDiff>,
) {
    if let Some(hunk_header) = hunk_assignment.hunk_header {
        if let Some(diff) = hunk_assignment.diff.clone() {
            let hunks = [DiffHunk {
                old_start: hunk_header.old_start,
                old_lines: hunk_header.old_lines,
                new_start: hunk_header.new_start,
                new_lines: hunk_header.new_lines,
                diff,
            }];

            let is_result_of_binary_to_text_conversion = false;

            build_unified_patch(
                hunk_assignment.path_bytes.as_ref(),
                hunks,
                is_result_of_binary_to_text_conversion,
                syntax_set,
                out,
            );
        } else {
            out.push(PartiallyRenderedDiff::SingleLine(ListItem::new(
                "No diff available",
            )));
        }
    } else {
        out.push(PartiallyRenderedDiff::SingleLine(ListItem::new(
            "File is too large or binary - no diff available",
        )));
    }
}

fn build_tree_changes(
    ctx: &mut Context,
    tree_changes: &[TreeChange],
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
) {
    for tree_change in tree_changes {
        let section = renderer.new_section_mut(SectionId::TreeChange(uuid::Uuid::new_v4()));

        let mut header = Vec::new();
        render_hunk_path_header(
            tree_change.path.as_ref(),
            Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
            &mut header,
        );
        section.diffs.push(PartiallyRenderedDiff::Header(header));

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
                    build_unified_patch(
                        tree_change.path.as_ref(),
                        hunks,
                        is_result_of_binary_to_text_conversion,
                        syntax_set,
                        &mut section.diffs,
                    );
                }
                UnifiedPatch::Binary => {
                    section
                        .diffs
                        .push(PartiallyRenderedDiff::SingleLine(ListItem::new(
                            "Binary file - no diff available",
                        )));
                }
                UnifiedPatch::TooLarge { size_in_bytes } => {
                    section
                        .diffs
                        .push(PartiallyRenderedDiff::SingleLine(ListItem::new(format!(
                            "File too large ({size_in_bytes} bytes) - no diff available"
                        ))));
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
) {
    let status = status.map(|id_or_status| match id_or_status {
        ShortIdOrTreeStatus::ShortId(id) => Span::raw(id.to_owned()).blue(),
        ShortIdOrTreeStatus::TreeStatus(status) => change_status(status),
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
    out.extend(bordered_line_top_right_bottom(path_line).map(ListItem::new));
    out.push(ListItem::from(""));
}

fn build_hunk_path_header(
    path: &BStr,
    status: Option<ShortIdOrTreeStatus<'_>>,
    out: &mut Vec<PartiallyRenderedDiff>,
) {
    let status = status.map(|id_or_status| match id_or_status {
        ShortIdOrTreeStatus::ShortId(id) => Span::raw(id.to_owned()).blue(),
        ShortIdOrTreeStatus::TreeStatus(status) => change_status(status),
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
    out.push(PartiallyRenderedDiff::Header(
        bordered_line_top_right_bottom(path_line)
            .map(ListItem::new)
            .chain([ListItem::from("")])
            .collect(),
    ));
}

fn change_status(status: &TreeStatus) -> Span<'static> {
    match status {
        TreeStatus::Addition { .. } => Span::raw("added").green(),
        TreeStatus::Deletion { .. } => Span::raw("deleted").red(),
        TreeStatus::Modification { .. } => Span::raw("modified").magenta(),
        TreeStatus::Rename { .. } => Span::raw("renamed").blue(),
    }
}

fn bordered_line_top_right_bottom(mut text: Line<'static>) -> impl Iterator<Item = Line<'static>> {
    let width_including_padding = text.width() + 1;

    text.spans.extend([Span::raw(" "), Span::raw("│").dim()]);

    [
        Line::from_iter(repeat_n("─", width_including_padding).chain(once("╮"))).dim(),
        text,
        Line::from_iter(repeat_n("─", width_including_padding).chain(once("╯"))).dim(),
    ]
    .into_iter()
}

fn render_signature(sig: &Signature) -> impl IntoIterator<Item = Span<'static>> {
    [
        Span::raw(sig.name.to_string()).yellow(),
        Span::raw(" <"),
        Span::raw(sig.email.to_string()).yellow(),
        Span::raw(">"),
        Span::raw(" ("),
        Span::raw(sig.time.format_or_unix(gix::date::time::format::DEFAULT)).green(),
        Span::raw(")"),
    ]
    .into_iter()
}

fn build_unified_patch(
    path: &BStr,
    hunks: impl IntoIterator<Item = DiffHunk>,
    is_result_of_binary_to_text_conversion: bool,
    syntax_set: &SyntaxSet,
    out: &mut Vec<PartiallyRenderedDiff>,
) {
    for hunk in hunks {
        let DiffHunk {
            old_start,
            new_start,
            diff,
            old_lines: _,
            new_lines: _,
        } = hunk;

        if is_result_of_binary_to_text_conversion {
            out.push(PartiallyRenderedDiff::SingleLine(ListItem::new(
                "(diff generated from binary-to-text conversion)",
            )));
        }

        if let Some(headers) = diff.lines().next() {
            out.extend([
                PartiallyRenderedDiff::SingleLine(ListItem::new(
                    Span::raw(headers.to_str_lossy().to_string()).dim(),
                )),
                PartiallyRenderedDiff::SingleLine(ListItem::new(
                    Line::from_iter(repeat_n("─", headers.to_str_lossy().width())).dim(),
                )),
            ]);
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
                    path.file_name().and_then(|file_name| {
                        syntax_set.find_syntax_by_extension(file_name.to_str()?)
                    })
                })
                .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
        };

        out.push(PartiallyRenderedDiff::DiffLines {
            path: path.to_owned(),
            old_width,
            new_width,
            old_start,
            new_start,
            syntax: Box::new(syntax.to_owned()),
            diff: diff_lines,
        });
    }
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
