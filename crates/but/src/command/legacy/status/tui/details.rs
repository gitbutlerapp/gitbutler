use std::{
    collections::BTreeMap,
    iter::{once, repeat_n},
    sync::LazyLock,
    time::Instant,
};

use bstr::{BStr, ByteSlice};
use but_core::{
    UnifiedPatch,
    ui::{TreeChange, TreeStatus},
    unified_diff::DiffHunk,
};
use but_ctx::{Context, OnDemand};
use but_hunk_assignment::HunkAssignment;
use gitbutler_stack::StackId;
use gix::actor::Signature;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
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
use tracing::Level;
use unicode_width::UnicodeWidthStr;

use crate::{
    CliId,
    command::legacy::status::tui::{
        CommandMessage, CommitMessage, DebugAsType, FilesMessage, Message, MoveMessage,
        RewordMessage, RubMessage,
    },
    id::UncommittedCliId,
};

use super::BranchMessage;

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
    include_bytes!("../../../../../assets/syntax-highlighting-themes/Monokai Extended.tmTheme");

#[derive(Debug, Default, Copy, Clone)]
pub(super) enum DetailsVisibility {
    #[default]
    Hidden,
    VisibleVertical,
}

#[derive(Debug, Clone)]
pub(super) enum DetailsMessage {
    ScrollUp(usize),
    ScrollDown(usize),
    ToggleVisibility,
}

#[derive(Debug)]
pub(super) struct Details {
    is_dirty: bool,
    scroll_top: usize,
    widget: Option<DetailsAndDiffWidget>,
    renderer: IncrementalDiffRenderer,
    syntax_set: DebugAsType<OnDemand<SyntaxSet>>,
    dark_theme: DebugAsType<OnDemand<Theme>>,
    visibility: DetailsVisibility,
}

impl Details {
    pub(super) fn new_hidden() -> Self {
        Self {
            is_dirty: false,
            widget: Default::default(),
            renderer: Default::default(),
            scroll_top: Default::default(),
            visibility: Default::default(),
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
            visibility: DetailsVisibility::VisibleVertical,
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
            DetailsVisibility::VisibleVertical => true,
        }
    }

    pub(super) fn needs_update(&self) -> bool {
        self.is_visible() && self.is_dirty()
    }

    pub(super) fn needs_update_after_message(&self, msg: &Message) -> bool {
        match self.visibility {
            DetailsVisibility::Hidden => return false,
            DetailsVisibility::VisibleVertical => {}
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
                self.scroll_top = self.scroll_top.saturating_sub(n);
            }
            DetailsMessage::ScrollDown(n) => {
                self.scroll_top = self.scroll_top.saturating_add(n);
            }
            DetailsMessage::ToggleVisibility => {
                self.visibility = match self.visibility {
                    DetailsVisibility::Hidden => DetailsVisibility::VisibleVertical,
                    DetailsVisibility::VisibleVertical => DetailsVisibility::Hidden,
                };

                match self.visibility {
                    DetailsVisibility::Hidden => {
                        self.scroll_top = 0;
                    }
                    DetailsVisibility::VisibleVertical => {
                        self.mark_dirty();
                    }
                }
            }
        }

        self.clamp_scroll_top(viewport);

        Ok(())
    }

    fn clamp_scroll_top(&mut self, viewport: Rect) {
        let max_scroll_top = self
            .widget
            .as_ref()
            .map(|diff| {
                diff.total_rows(viewport.width)
                    .saturating_sub(viewport.height as usize)
            })
            .unwrap_or(0);

        self.scroll_top = self.scroll_top.min(max_scroll_top);
    }

    pub(super) fn update(
        &mut self,
        ctx: &mut Context,
        selection: Option<&CliId>,
    ) -> anyhow::Result<()> {
        if let Some(widget) = &mut self.widget {
            let syntax_set = self.syntax_set.get()?;
            let theme = self.dark_theme.get()?;
            match self
                .renderer
                .render_next_chunk(&syntax_set, &theme, widget.diff_line_items_mut())
            {
                RenderNextChunkResult::Done => {
                    self.is_dirty = false;
                    tracing::trace!("rendered diff in {:?}", self.renderer.created_at.elapsed());
                }
                RenderNextChunkResult::Meta | RenderNextChunkResult::Diff => {
                    tracing::trace!("render_next_chunk");
                }
            }
        } else {
            let Some(selection) = selection else {
                self.is_dirty = false;
                return Ok(());
            };

            self.is_dirty = true;
            self.scroll_top = 0;
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
                CliId::Uncommitted(uncommitted) => from_uncommitted(
                    uncommitted,
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                )?,
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
                CliId::Unassigned { .. } => from_unassigned(
                    ctx,
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                )?,
                CliId::Stack { stack_id, .. } => from_stack(
                    ctx,
                    *stack_id,
                    &*self.syntax_set.get()?,
                    &mut self.renderer,
                    previous_diff_line_items,
                )?,
            });
        }

        Ok(())
    }

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let layout = Layout::horizontal([Constraint::Length(1), Constraint::Min(1)]).split(area);

        let block = Block::new()
            .borders(Borders::LEFT)
            .border_style(Style::default().dim());
        frame.render_widget(block, layout[0]);

        if let Some(diff) = &self.widget {
            diff.render(self.scroll_top, layout[1], frame);
        }
    }
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
            | DetailsAndDiffWidget::FromDiffLines { diff_line_items } => diff_line_items,
        }
    }

    #[tracing::instrument(level = Level::TRACE, skip_all)]
    fn total_rows(&self, width: u16) -> usize {
        self.items_for_width(width).count()
    }

    fn render(&self, scroll_top: usize, area: Rect, buf: &mut Frame) {
        let items = self.items_for_width(area.width).skip(scroll_top);
        List::new(items).render(area, buf.buffer_mut());
    }

    fn items_for_width(&self, width: u16) -> impl Iterator<Item = ListItem<'static>> {
        let width = usize::from(width).max(1);

        match self {
            DetailsAndDiffWidget::FromCommit {
                header_items,
                message,
                diff_line_items,
            } => {
                let iter = header_items
                    .clone()
                    .into_iter()
                    .chain([ListItem::new("")])
                    .chain(
                        textwrap::wrap(message, textwrap::Options::new(width))
                            .into_iter()
                            .map(|line| ListItem::new(line.into_owned())),
                    )
                    .chain([ListItem::new("")])
                    .chain(diff_line_items.clone());
                itertools::Either::Left(iter)
            }
            DetailsAndDiffWidget::FromDiffLines { diff_line_items } => {
                itertools::Either::Right(diff_line_items.clone().into_iter())
            }
        }
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

    build_tree_changes(
        ctx,
        &tree_changes,
        syntax_set,
        &mut renderer.partially_rendered_diff,
    );

    Ok(DetailsAndDiffWidget::FromCommit {
        header_items,
        message,
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_uncommitted(
    uncommitted: &UncommittedCliId,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    // the path is the same for all hunks so only show that once
    let first_hunk = uncommitted.hunk_assignments.first();
    build_hunk_path_header(
        first_hunk.path_bytes.as_ref(),
        Some(ShortIdOrTreeStatus::ShortId(&uncommitted.id)),
        &mut renderer.partially_rendered_diff,
    );

    let mut hunk_assignments_iter = uncommitted.hunk_assignments.iter().peekable();
    while let Some(hunk_assignment) = hunk_assignments_iter.next() {
        build_hunk_assignment(
            hunk_assignment,
            syntax_set,
            &mut renderer.partially_rendered_diff,
        );

        if hunk_assignments_iter.peek().is_some() {
            renderer
                .partially_rendered_diff
                .push(PartiallyRenderedDiff::SingleLine(ListItem::new("")));
        }
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
    let mut hunk_assignments_iter = hunk_assignments.into_iter().peekable();
    while let Some((id, hunk_assignment)) = hunk_assignments_iter.next() {
        build_hunk_path_header(
            hunk_assignment.path_bytes.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(id)),
            &mut renderer.partially_rendered_diff,
        );

        build_hunk_assignment(
            hunk_assignment,
            syntax_set,
            &mut renderer.partially_rendered_diff,
        );

        if hunk_assignments_iter.peek().is_some() {
            renderer
                .partially_rendered_diff
                .push(PartiallyRenderedDiff::SingleLine(ListItem::new("")));
        }
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

    build_tree_changes(
        ctx,
        &tree_changes,
        syntax_set,
        &mut renderer.partially_rendered_diff,
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
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let tree_changes = but_api::branch::branch_diff(ctx, name)?;

    build_tree_changes(
        ctx,
        &tree_changes.changes,
        syntax_set,
        &mut renderer.partially_rendered_diff,
    );

    Ok(DetailsAndDiffWidget::FromDiffLines {
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_unassigned(
    ctx: &mut Context,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        Some(changes),
        context_lines,
    )?;
    let hunk_assignments = assignments
        .iter()
        .filter(|assignment| assignment.stack_id.is_none());

    group_and_build_hunk_assignments(
        hunk_assignments,
        syntax_set,
        &mut renderer.partially_rendered_diff,
    );

    Ok(DetailsAndDiffWidget::FromDiffLines {
        diff_line_items: diff_line_items.unwrap_or_default(),
    })
}

fn from_stack(
    ctx: &mut Context,
    stack: StackId,
    syntax_set: &SyntaxSet,
    renderer: &mut IncrementalDiffRenderer,
    diff_line_items: Option<Vec<ListItem<'static>>>,
) -> anyhow::Result<DetailsAndDiffWidget> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        Some(changes),
        context_lines,
    )?;
    let hunk_assignments = assignments
        .iter()
        .filter(|assignment| assignment.stack_id.is_some_and(|s| s == stack));

    group_and_build_hunk_assignments(
        hunk_assignments,
        syntax_set,
        &mut renderer.partially_rendered_diff,
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
    partially_rendered_diff: Vec<PartiallyRenderedDiff>,
    state: IncrementalDiffRendererState,
    /// How many diff lines to process on each poll.
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
        idx: usize,
    },
    Diff {
        idx: usize,
        diff_idx: usize,
        old_line_num: u32,
        new_line_num: u32,
    },
}

/// A diff thats been partially rendered.
///
/// Used with `IncrementalDiffRenderer` which can incrementally render the final diff.
#[derive(Debug)]
enum PartiallyRenderedDiff {
    Header(Vec<ListItem<'static>>),
    SingleLine(ListItem<'static>),
    DiffLines {
        old_width: u32,
        new_width: u32,
        old_start: u32,
        new_start: u32,
        syntax: Box<SyntaxReference>,
        diff: Vec<Box<[u8]>>,
    },
}

/// The result of rendering the one diff chunk.
enum RenderNextChunkResult {
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
            partially_rendered_diff: Default::default(),
            state: IncrementalDiffRendererState::Top { idx: 0 },
            chunk_size: 500,
            created_at: Instant::now(),
        }
    }
}

impl IncrementalDiffRenderer {
    /// Clear any internal state so the allocations can be reused.
    fn clear(&mut self) {
        let Self {
            partially_rendered_diff,
            state,
            chunk_size,
            created_at,
        } = self;

        partially_rendered_diff.clear();

        let default = Self::default();
        *state = default.state;
        *chunk_size = default.chunk_size;
        *created_at = default.created_at;
    }

    fn render_next_chunk(
        &mut self,
        syntax_set: &SyntaxSet,
        theme: &Theme,
        out: &mut Vec<ListItem<'static>>,
    ) -> RenderNextChunkResult {
        loop {
            match self.state {
                IncrementalDiffRendererState::Top { idx } => {
                    if idx >= self.partially_rendered_diff.len() {
                        break RenderNextChunkResult::Done;
                    }
                }
                IncrementalDiffRendererState::Diff { .. } => {}
            }

            match &mut self.state {
                IncrementalDiffRendererState::Top { idx } => {
                    match &mut self.partially_rendered_diff[*idx] {
                        PartiallyRenderedDiff::Header(list_items) => {
                            out.append(list_items);
                            self.state = IncrementalDiffRendererState::Top { idx: (*idx) + 1 };
                            break RenderNextChunkResult::Meta;
                        }
                        PartiallyRenderedDiff::SingleLine(list_item) => {
                            out.push(std::mem::replace(
                                list_item,
                                ListItem::from(Text::default()),
                            ));
                            self.state = IncrementalDiffRendererState::Top { idx: (*idx) + 1 };
                            break RenderNextChunkResult::Meta;
                        }
                        PartiallyRenderedDiff::DiffLines {
                            old_start,
                            new_start,
                            ..
                        } => {
                            self.state = IncrementalDiffRendererState::Diff {
                                idx: *idx,
                                // the first line is the `@@ -1,6 +1,8 @@` header, skip that
                                diff_idx: 1,
                                old_line_num: *old_start,
                                new_line_num: *new_start,
                            };
                        }
                    }
                }
                IncrementalDiffRendererState::Diff {
                    idx,
                    diff_idx,
                    old_line_num,
                    new_line_num,
                } => {
                    let PartiallyRenderedDiff::DiffLines {
                        old_width,
                        new_width,
                        syntax,
                        diff,
                        old_start: _,
                        new_start: _,
                    } = &mut self.partially_rendered_diff[*idx]
                    else {
                        unreachable!();
                    };

                    if *diff_idx >= diff.len() {
                        self.state = IncrementalDiffRendererState::Top { idx: (*idx) + 1 };
                        continue;
                    }

                    let mut highlight_lines = HighlightLines::new(syntax, theme);

                    for line in diff.iter().skip(*diff_idx).take(self.chunk_size) {
                        *diff_idx += 1;

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
                                    Some(*PLUS_BG),
                                    &mut highlight_lines,
                                    syntax_set,
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
                                    Span::raw(old_line_num.to_string()).fg(*MINUS_EMPH_BG),
                                    Span::raw(" ┊ ").dim(),
                                    Span::raw(" ".repeat(*new_width as _)),
                                    Span::raw(" │ ").dim(),
                                    Span::raw("-").bg(*MINUS_BG),
                                ]
                                .into_iter()
                                .chain(syntax_highlight(
                                    &code,
                                    Some(*MINUS_BG),
                                    &mut highlight_lines,
                                    syntax_set,
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
                                    None,
                                    &mut highlight_lines,
                                    syntax_set,
                                )),
                            ));
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

fn group_and_build_hunk_assignments<'a, I>(
    hunk_assignments: I,
    syntax_set: &SyntaxSet,
    out: &mut Vec<PartiallyRenderedDiff>,
) where
    I: IntoIterator<Item = &'a HunkAssignment>,
{
    // group hunks for the same file
    let mut path_to_hunk_assignments = BTreeMap::<_, Vec<_>>::new();
    for hunk_assignment in hunk_assignments {
        path_to_hunk_assignments
            .entry(&hunk_assignment.path_bytes)
            .or_default()
            .push(hunk_assignment);
    }

    let mut path_to_hunk_assignments_iter = path_to_hunk_assignments.into_iter().peekable();
    while let Some((path, hunk_assignments)) = path_to_hunk_assignments_iter.next() {
        build_hunk_path_header(path.as_ref(), None, out);

        let mut hunk_assignments_iter = hunk_assignments.into_iter().peekable();
        while let Some(hunk_assignment) = hunk_assignments_iter.next() {
            build_hunk_assignment(hunk_assignment, syntax_set, out);

            if hunk_assignments_iter.peek().is_some() {
                out.push(PartiallyRenderedDiff::SingleLine(ListItem::new("")));
            }
        }

        if path_to_hunk_assignments_iter.peek().is_some() {
            out.push(PartiallyRenderedDiff::SingleLine(ListItem::new("")));
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
            let hunks = Vec::from([DiffHunk {
                old_start: hunk_header.old_start,
                old_lines: hunk_header.old_lines,
                new_start: hunk_header.new_start,
                new_lines: hunk_header.new_lines,
                diff,
            }]);

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
    out: &mut Vec<PartiallyRenderedDiff>,
) {
    for tree_change in tree_changes {
        let mut header = Vec::new();
        render_hunk_path_header(
            tree_change.path.as_ref(),
            Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
            &mut header,
        );
        out.push(PartiallyRenderedDiff::Header(header));

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
                        out,
                    );
                }
                UnifiedPatch::Binary => {
                    out.push(PartiallyRenderedDiff::SingleLine(ListItem::new(
                        "Binary file - no diff available",
                    )));
                }
                UnifiedPatch::TooLarge { size_in_bytes } => {
                    out.push(PartiallyRenderedDiff::SingleLine(ListItem::new(format!(
                        "File too large ({size_in_bytes} bytes) - no diff available"
                    ))));
                }
            }

            out.push(PartiallyRenderedDiff::SingleLine(ListItem::new("")));
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
    hunks: Vec<DiffHunk>,
    is_result_of_binary_to_text_conversion: bool,
    syntax_set: &SyntaxSet,
    out: &mut Vec<PartiallyRenderedDiff>,
) {
    let mut hunk_iter = hunks.into_iter().peekable();
    while let Some(hunk) = hunk_iter.next() {
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
            old_width,
            new_width,
            old_start,
            new_start,
            syntax: Box::new(syntax.to_owned()),
            diff: diff_lines,
        });

        if hunk_iter.peek().is_some() {
            out.push(PartiallyRenderedDiff::SingleLine(ListItem::new("")));
        }
    }
}

fn num_digits(n: u32) -> u32 {
    if n == 0 { 1 } else { n.ilog10() + 1 }
}

fn syntax_highlight(
    code: &str,
    bg: Option<Color>,
    highlight_lines: &mut HighlightLines<'_>,
    syntax_set: &SyntaxSet,
) -> impl Iterator<Item = Span<'static>> {
    let Ok(ranges) = highlight_lines.highlight_line(code, syntax_set) else {
        return itertools::Either::Left(std::iter::empty());
    };

    let spans = ranges.into_iter().map(move |(style, text)| {
        let color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
        let span = Span::raw(text.to_owned()).fg(color);
        if let Some(background) = bg {
            span.bg(background)
        } else {
            span
        }
    });

    itertools::Either::Right(spans)
}
