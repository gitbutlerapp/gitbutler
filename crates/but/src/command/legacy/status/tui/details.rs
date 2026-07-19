use std::{
    cell::{Cell, RefCell},
    fmt::Display,
    sync::{
        Arc,
        mpsc::{Sender, TryRecvError},
    },
    time::{Duration, Instant},
};

use anyhow::Context as _;
use but_ctx::{Context, OnDemand};
use gix::ObjectId;
use itertools::{Itertools as _, Position};
use nonempty::NonEmpty;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Stylize as _},
    text::{Line, Span},
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use syntect::{easy::HighlightLines, highlighting, parsing::SyntaxSet};

use crate::{
    CliId,
    command::legacy::status::tui::{
        Message, ReloadCause, SelectAfterReload,
        app::{
            Modal,
            mark::{
                MarkStore, MarkableRef, Marks, MarksRef, synthetic_parent_hunk, toggle_markables,
            },
        },
        backstack::Backstack,
        confirm::Confirm,
        copy_selection_picker::Clipboard,
        count_allocations,
        details::worker::Worker,
        highlight::{self, Highlights},
        message_on_drop::message_on_drop,
        mode::ModeDiscriminant,
        render::{RenderSingleLineSpans, available_lines_in_area},
    },
    theme::{Rgb, Theme},
    utils::{
        DebugAsType,
        diff_rendering::{
            self, CodeLineKind, DetailsLine, DiffLineWriter, IdGen, SectionId, load_syntax_set,
        },
        diff_specs::DiffSpecBuilder,
        string_interning::Strings,
    },
};

mod worker;

const CHANNEL_SIZE: usize = 1024;

#[derive(Debug)]
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
    Discard,
    DropToBeDiscarded,
    Mark,
}

#[derive(Debug)]
pub struct Details {
    theme: &'static Theme,
    selection: Option<CliId>,
    lines: Vec<DetailsLine>,
    line_reader: ChannelLineReader,
    syntax_set: DebugAsType<OnDemand<SyntaxSet>>,
    syntax_theme: DebugAsType<OnDemand<highlighting::Theme>>,
    strings: Strings,
    selected_section: Cell<SelectedSection>,
    pending_section_selection: Cell<PendingSectionSelection>,
    sections: Vec<Section>,
    scroll: ScrollState,
    layout_cache: RefCell<LayoutCache>,
    cache: Cache,
    out_of_band_messages_tx: Sender<Message>,
    highlights: Highlights<SectionId>,
    worker: Worker,
    to_be_discarded: Vec<SectionId>,
    clipboard: Clipboard,
}

#[derive(Debug, Default)]
enum ChannelLineReader {
    #[default]
    NotStarted,
    Started {
        rx: std::sync::mpsc::Receiver<RenderThreadMessage>,
        start: Instant,
        cache_key: Option<CacheKey>,
    },
    Finished,
    Failed,
}

#[derive(Debug)]
enum RenderThreadMessage {
    Line(DetailsLine),
    Finished,
    Failed,
}

impl Details {
    pub fn new(
        theme: &'static Theme,
        out_of_band_messages_tx: Sender<Message>,
        clipboard: Clipboard,
    ) -> Self {
        Self {
            theme,
            selection: Default::default(),
            lines: Default::default(),
            sections: Default::default(),
            syntax_set: OnDemand::new(|| Ok(load_syntax_set())).into(),
            syntax_theme: OnDemand::new(|| theme.load_syntax_highlighting_theme()).into(),
            strings: Default::default(),
            selected_section: Default::default(),
            pending_section_selection: Default::default(),
            line_reader: Default::default(),
            scroll: Default::default(),
            layout_cache: Default::default(),
            cache: Default::default(),
            out_of_band_messages_tx,
            highlights: Default::default(),
            worker: Worker::new(),
            to_be_discarded: Default::default(),
            clipboard,
        }
    }

    pub fn is_polling_thread(&self) -> bool {
        match &self.line_reader {
            ChannelLineReader::NotStarted
            | ChannelLineReader::Finished
            | ChannelLineReader::Failed => false,
            ChannelLineReader::Started { .. } => true,
        }
    }

    pub fn started_polling_thread_at(&self) -> Option<Instant> {
        match &self.line_reader {
            ChannelLineReader::Started { start, .. } => Some(*start),
            ChannelLineReader::NotStarted
            | ChannelLineReader::Finished
            | ChannelLineReader::Failed => None,
        }
    }

    pub fn worker_is_busy(&self) -> bool {
        self.worker.is_busy()
    }

    pub fn cache_size(&self) -> usize {
        self.cache.num_lines
    }

    pub fn selected_section_cli_id(&self) -> Option<&Arc<CliId>> {
        let index = self.selected_section.get().index()?;
        self.sections.get(index)?.cli_id.as_ref()
    }

    pub fn on_hidden(&mut self) {
        self.reset_line_reader();
    }

    pub fn selection(&self) -> Option<&CliId> {
        self.selection.as_ref()
    }

    pub fn clear_selection_for_reload(&mut self, select_first_section_when_available: bool) {
        self.selection = None;
        self.reset_line_reader();
        self.clear_lines();
        self.reset_scroll();
        if select_first_section_when_available {
            self.pending_section_selection
                .set(PendingSectionSelection::First);
        }
    }

    pub fn update_highlights(&mut self) -> bool {
        self.highlights.update()
    }

    pub fn update(
        &mut self,
        ctx: &mut Context,
        new_selection: Option<&CliId>,
        is_visible: bool,
    ) -> anyhow::Result<bool> {
        if !is_visible {
            self.clear_lines();
            self.reset_line_reader();
            self.reset_scroll();
            self.clear_pending_first_section_selection();
            return Ok(false);
        }

        let (selection, selection_did_change) = match (self.selection.as_ref(), new_selection) {
            (None, None) => {
                // no selection
                self.reset_line_reader();
                self.clear_pending_first_section_selection();

                return Ok(false);
            }
            (None, Some(new)) => {
                // selected something
                self.selection = Some(new.clone());
                self.reset_line_reader();

                (new, true)
            }
            (Some(_), None) => {
                // deselected
                self.selection = None;
                self.reset_line_reader();
                self.clear_lines();
                self.reset_scroll();
                self.clear_pending_first_section_selection();

                return Ok(true);
            }
            (Some(old), Some(new)) => {
                if old == new {
                    // selection didn't change
                    // we might have to poll the channel so dont return
                    (old, false)
                } else {
                    // selected something new
                    self.selection = Some(new.clone());
                    self.reset_line_reader();
                    (new, true)
                }
            }
        };

        match selection {
            CliId::Commit {
                commit_id: commit, ..
            } => {
                let commit = *commit;
                self.poll_render_thread(
                    ctx,
                    Some(CacheKey::Commit(commit)),
                    selection_did_change,
                    move |ctx, theme, id_gen, line_writer, options| {
                        diff_rendering::render_commit(
                            commit,
                            ctx,
                            theme,
                            id_gen,
                            options,
                            line_writer,
                        )
                    },
                )
            }
            CliId::Branch { name, .. } => {
                let name = name.to_owned();
                self.poll_render_thread(
                    ctx,
                    None,
                    selection_did_change,
                    move |ctx, theme, id_gen, line_writer, options| {
                        diff_rendering::render_branch(
                            name,
                            ctx,
                            theme,
                            id_gen,
                            options,
                            line_writer,
                        )
                    },
                )
            }
            CliId::Uncommitted { .. } => self.poll_render_thread(
                ctx,
                None,
                selection_did_change,
                move |ctx, theme, id_gen, line_writer, options| {
                    diff_rendering::render_uncommitted(ctx, theme, id_gen, options, line_writer)
                },
            ),
            CliId::UncommittedHunkOrFile(uncommitted) => {
                let uncommitted = uncommitted.clone();
                self.poll_render_thread(
                    ctx,
                    None,
                    selection_did_change,
                    move |ctx, theme, id_gen, line_writer, options| {
                        diff_rendering::render_uncommitted_hunk(
                            uncommitted,
                            ctx,
                            theme,
                            id_gen,
                            options,
                            line_writer,
                        )
                    },
                )
            }
            CliId::CommittedFile {
                commit_id,
                path,
                id,
            } => {
                let commit = *commit_id;
                let path = path.clone();
                let id = id.clone();
                self.poll_render_thread(
                    ctx,
                    None,
                    selection_did_change,
                    move |ctx, theme, id_gen, line_writer, options| {
                        diff_rendering::render_committed_file(
                            commit,
                            path,
                            id,
                            ctx,
                            theme,
                            id_gen,
                            options,
                            line_writer,
                        )
                    },
                )
            }
            CliId::Stack { .. } => {
                self.reset_line_reader();
                self.clear_lines();
                self.reset_scroll();
                let section_added = push_line(
                    &mut self.lines,
                    &mut self.sections,
                    DetailsLine::Text {
                        id: None,
                        cli_id: None,
                        line: Line::from("(stack assignments are not supported)")
                            .style(self.theme.hint),
                        skip_when_copying_hunk: false,
                    },
                );
                if section_added {
                    self.select_first_section_if_pending();
                } else {
                    self.clear_pending_first_section_selection();
                }
                Ok(true)
            }
            CliId::PathPrefix { .. } => {
                self.reset_line_reader();
                self.clear_lines();
                self.reset_scroll();
                self.clear_pending_first_section_selection();
                Ok(true)
            }
        }
    }

    fn poll_render_thread<F>(
        &mut self,
        ctx: &Context,
        cache_key: Option<CacheKey>,
        selection_did_change: bool,
        f: F,
    ) -> anyhow::Result<bool>
    where
        F: FnOnce(
                &mut Context,
                &'static Theme,
                &mut IdGen<'_>,
                &mut dyn DiffLineWriter,
                diff_rendering::Options,
            ) -> anyhow::Result<()>
            + Clone
            + Send
            + 'static,
    {
        if cfg!(test) {
            // we don't bother with threads during tests since that makes since non-deterministic,
            // so just keep polling until we've received all lines
            loop {
                self.poll_render_thread_inner(ctx, cache_key, selection_did_change, f.clone())?;
                match &self.line_reader {
                    ChannelLineReader::NotStarted | ChannelLineReader::Started { .. } => {}
                    ChannelLineReader::Finished | ChannelLineReader::Failed => break Ok(true),
                }
            }
        } else {
            self.poll_render_thread_inner(ctx, cache_key, selection_did_change, f)
        }
    }

    fn poll_render_thread_inner<F>(
        &mut self,
        ctx: &Context,
        cache_key: Option<CacheKey>,
        selection_did_change: bool,
        f: F,
    ) -> anyhow::Result<bool>
    where
        F: FnOnce(
                &mut Context,
                &'static Theme,
                &mut IdGen<'_>,
                &mut dyn DiffLineWriter,
                diff_rendering::Options,
            ) -> anyhow::Result<()>
            + Send
            + 'static,
    {
        if let Some(cache_key) = cache_key
            && let Some(cached_lines) = self.cache.get(cache_key)
        {
            if selection_did_change || self.lines.is_empty() {
                self.restore_cached_lines(cached_lines.clone());
                return Ok(true);
            } else {
                return Ok(false);
            }
        }

        match &mut self.line_reader {
            ChannelLineReader::NotStarted => {
                tracing::debug!("spawning thread");

                self.clear_lines();
                self.reset_scroll();

                let (tx, rx) = std::sync::mpsc::sync_channel(CHANNEL_SIZE);
                self.line_reader = ChannelLineReader::Started {
                    rx,
                    start: Instant::now(),
                    cache_key,
                };
                let mut line_writer = ChannelLineWriter { tx };
                let strings = self.strings.clone();
                let theme = self.theme;
                let ctx = ctx.to_sync();
                let error_tx = self.out_of_band_messages_tx.clone();

                self.worker.replace_next_job(move || {
                    let mut ctx = ctx.into_thread_local();
                    let mut id_gen = IdGen::new(strings);

                    count_allocations("details fetch diff", || {
                        match f(
                            &mut ctx,
                            theme,
                            &mut id_gen,
                            &mut line_writer,
                            diff_rendering::Options::default(),
                        )
                        .context("failed rendering commit diff")
                        {
                            Ok(()) => {
                                _ = line_writer.tx.send(RenderThreadMessage::Finished);
                            }
                            Err(err) if err.downcast_ref::<SendErrorCode>().is_none() => {
                                tracing::error!("{err:#}");
                                _ = error_tx.send(Message::ShowError(err));
                                _ = line_writer.tx.send(RenderThreadMessage::Failed);
                            }
                            Err(_) => {}
                        }
                    });
                });

                Ok(true)
            }
            ChannelLineReader::Started {
                rx,
                start,
                cache_key,
            } => {
                let mut n = CHANNEL_SIZE;
                loop {
                    match rx.try_recv() {
                        Ok(RenderThreadMessage::Line(line)) => {
                            let section_added =
                                push_line(&mut self.lines, &mut self.sections, line);
                            if section_added {
                                apply_pending_section_selection(
                                    &self.pending_section_selection,
                                    &self.selected_section,
                                    &self.scroll,
                                    self.sections.len(),
                                );
                            }
                        }
                        Ok(RenderThreadMessage::Finished) => {
                            let num_strings = self.strings.len();
                            tracing::debug!(
                                "finished reading from channel in {:?} ({} lines, {} strings)",
                                start.elapsed(),
                                self.lines.len(),
                                num_strings,
                            );

                            if let Some(cache_key) = *cache_key {
                                self.cache.insert(cache_key, self.lines.clone());
                            }

                            if self.sections.is_empty() {
                                self.pending_section_selection
                                    .set(PendingSectionSelection::None);
                            }

                            self.line_reader = ChannelLineReader::Finished;

                            break Ok(true);
                        }
                        Ok(RenderThreadMessage::Failed) => {
                            // The thread sent the user-facing error separately. Don't cache partial
                            // output, otherwise reopening the same commit would restore the failed
                            // render instead of trying again.
                            self.clear_lines();
                            self.line_reader = ChannelLineReader::Failed;
                            self.reset_scroll();
                            self.pending_section_selection
                                .set(PendingSectionSelection::None);

                            tracing::debug!("diff render thread failed");

                            break Ok(true);
                        }
                        Err(err) => match err {
                            TryRecvError::Empty => break Ok(false),
                            TryRecvError::Disconnected => {
                                tracing::debug!(
                                    "diff render thread disconnected before completion"
                                );
                                self.line_reader = ChannelLineReader::Failed;
                                self.pending_section_selection
                                    .set(PendingSectionSelection::None);
                                break Ok(true);
                            }
                        },
                    }

                    n -= 1;
                    if n == 0 {
                        break Ok(true);
                    }
                }
            }
            ChannelLineReader::Finished | ChannelLineReader::Failed => Ok(false),
        }
    }

    pub fn render(
        &self,
        _help_shown: bool,
        tui_has_focus: bool,
        marks: MarksRef<'_>,
        area: Rect,
        frame: &mut Frame,
    ) {
        if !cfg!(test)
            && self.lines.is_empty()
            && let ChannelLineReader::Started { start, .. } = &self.line_reader
            && start.elapsed() > Duration::from_millis(500)
        {
            frame.render_widget(
                format!("Loading diff ({:.1}s)", start.elapsed().as_secs_f32()).dim(),
                area,
            );
            return;
        }

        let syntax_set = self.syntax_set.get().unwrap();
        let syntax_theme = self.syntax_theme.get().unwrap();
        let mut highlight_lines = None;

        let mut layout_cache = self.layout_cache.borrow_mut();
        layout_cache.update(area.width, &self.lines);

        let viewport_height = area.height as usize;
        let total_display_lines = layout_cache.total_display_lines();
        let show_scrollbar = total_display_lines > viewport_height;
        let max_scroll_top = total_display_lines.saturating_sub(viewport_height);

        match self.scroll.take_pending() {
            Some(ScrollIntent::Bottom) => self.scroll.set_top(max_scroll_top),
            Some(ScrollIntent::Section { index }) => {
                if let Some(section) = self.sections.get(index) {
                    let (section_start, section_end) =
                        section_display_range(section, &layout_cache);
                    self.scroll.set_top(scroll_top_for_section(
                        section_start,
                        section_end,
                        viewport_height,
                        self.scroll.top(),
                        max_scroll_top,
                    ));
                }
            }
            None if self.scroll.top() > max_scroll_top => self.scroll.set_top(max_scroll_top),
            None => {}
        }

        let scroll_top = self.scroll.top();
        self.update_selected_section_for_visible_range(scroll_top, viewport_height, &layout_cache);
        let selected_section_visible_range = self.selected_section_visible_range(
            scroll_top,
            viewport_height,
            &layout_cache,
            tui_has_focus,
        );

        let Some((mut line_index, mut line_offset)) = layout_cache.line_at_display_row(scroll_top)
        else {
            return;
        };
        drop(layout_cache);

        let mut areas = available_lines_in_area(area);
        while let Some(line) = self.lines.get(line_index) {
            let rendered = self.render_details_line(
                line,
                line_offset,
                &mut areas,
                area.width,
                tui_has_focus,
                &syntax_set,
                &syntax_theme,
                &mut highlight_lines,
                marks,
                frame,
            );

            if !rendered.filled_viewport {
                line_index += 1;
                line_offset = 0;
            } else {
                break;
            }
        }

        if let Some((section_id, cli_id, range)) = selected_section_visible_range {
            self.render_selected_section_marker(section_id, cli_id, area, marks, range, frame);
        }

        // Draw the scrollbar over the content instead of reserving a column for it.
        // Reserving a column changes the text width, so the layout cache would need
        // to recompute wrapped line heights when the scrollbar appears, which is
        // expensive for large diffs.
        if show_scrollbar && max_scroll_top > 0 {
            self.render_scrollbar(area, scroll_top, max_scroll_top, frame);
        }
    }

    fn render_selected_section_marker(
        &self,
        id: SectionId,
        cli_id: Option<Arc<CliId>>,
        area: Rect,
        marks: MarksRef<'_>,
        visible_range: std::ops::Range<usize>,
        frame: &mut Frame,
    ) {
        let Some(x) = area.x.checked_sub(1) else {
            return;
        };
        for row in visible_range {
            let color = if self.section_is_highlighted(id) {
                highlight::style().bg.unwrap()
            } else if let Some(cli_id) = cli_id.as_ref()
                && marks.contains_cli_id(cli_id)
            {
                self.theme.tui_mark.bg.unwrap()
            } else {
                ModeDiscriminant::Details.bg(self.theme)
            };
            frame.render_widget(
                Span::raw("▌").fg(color),
                Rect {
                    x,
                    y: area.y + row as u16,
                    width: 1,
                    height: 1,
                },
            );
        }
    }

    fn render_scrollbar(
        &self,
        area: Rect,
        scroll_top: usize,
        max_scroll_top: usize,
        frame: &mut Frame,
    ) {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .thumb_symbol("█")
            .track_symbol(None)
            .style(self.theme.border);
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll_top.saturating_add(1)).position(scroll_top);
        let scrollbar_area = Rect {
            x: area.right().saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }

    pub fn try_handle_message(
        &mut self,
        msg: DetailsMessage,
        messages: &mut Vec<Message>,
        marks: Option<&mut Marks>,
        backstack: &mut Backstack,
    ) -> anyhow::Result<()> {
        match msg {
            DetailsMessage::ScrollUp(n) => self.scroll.up(n),
            DetailsMessage::ScrollDown(n) => self.scroll.down(n),
            DetailsMessage::SelectNextSection => {
                let selected_section = self.selected_section.get();
                if let Some(n) = selected_section.index()
                    && self.sections.get(n + 1).is_some()
                {
                    let index = n + 1;
                    self.selected_section
                        .set(selected_section.with_index(index));
                    self.scroll.to_section(index, ScrollDirection::Down);
                }
            }
            DetailsMessage::SelectPrevSection => {
                let selected_section = self.selected_section.get();
                if let Some(n) = selected_section.index()
                    && let Some(index) = n.checked_sub(1)
                    && self.sections.get(index).is_some()
                {
                    self.selected_section
                        .set(selected_section.with_index(index));
                    self.scroll.to_section(index, ScrollDirection::Up);
                }
            }
            DetailsMessage::Deselect => {
                self.selected_section
                    .set(match self.selected_section.get() {
                        SelectedSection::None => SelectedSection::None,
                        SelectedSection::Selected(n) | SelectedSection::Deselected(n) => {
                            SelectedSection::Deselected(n)
                        }
                    });
            }
            DetailsMessage::SelectFirstSection => {
                if self.sections.is_empty() {
                    self.pending_section_selection
                        .set(PendingSectionSelection::First);
                } else {
                    self.clear_pending_first_section_selection();
                    self.selected_section
                        .set(match self.selected_section.get() {
                            SelectedSection::None => SelectedSection::Selected(0),
                            SelectedSection::Selected(n) | SelectedSection::Deselected(n) => {
                                SelectedSection::Selected(n)
                            }
                        });
                }
            }
            DetailsMessage::GotoTop => {
                self.scroll.goto_top();
                self.selected_section.set(if self.sections.is_empty() {
                    SelectedSection::None
                } else {
                    SelectedSection::Selected(0)
                });
            }
            DetailsMessage::GotoBottom => {
                self.scroll.goto_bottom();
                self.selected_section.set(
                    self.sections
                        .len()
                        .checked_sub(1)
                        .map_or(SelectedSection::None, SelectedSection::Selected),
                );
            }
            DetailsMessage::CopyCurrentHunk => {
                self.copy_current_hunk()?;
            }
            DetailsMessage::Discard => {
                let Some(marks) = marks else {
                    return Ok(());
                };
                self.handle_discard(messages, marks.as_ref());
            }
            DetailsMessage::DropToBeDiscarded => {
                self.to_be_discarded.clear();
            }
            DetailsMessage::Mark => {
                let Some(marks) = marks else {
                    return Ok(());
                };
                self.handle_mark(messages, marks, backstack)?;
            }
        }

        Ok(())
    }

    #[expect(clippy::too_many_arguments)]
    fn render_details_line<'a>(
        &self,
        line: &DetailsLine,
        skip_display_lines: usize,
        areas: &mut impl Iterator<Item = Rect>,
        width: u16,
        _tui_has_focus: bool,
        syntax_set: &'a SyntaxSet,
        syntax_theme: &'a syntect::highlighting::Theme,
        highlight_lines: &mut Option<HighlightLines<'a>>,
        marks: MarksRef<'_>,
        frame: &mut Frame,
    ) -> RenderedLine {
        match line {
            DetailsLine::Text {
                line,
                id,
                skip_when_copying_hunk,
                cli_id: _,
            } => {
                *highlight_lines = None;

                if skip_display_lines == 0 {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };

                    if let Some(id) = id {
                        if !*skip_when_copying_hunk && self.section_is_to_be_discarded(*id) {
                            let crossed_out_line = line
                                .spans
                                .iter()
                                .cloned()
                                .map(|span| span.crossed_out())
                                .collect::<Line<'_>>()
                                .style(line.style);
                            frame.render_widget(crossed_out_line, line_area);
                        } else {
                            frame.render_widget(line, line_area);
                        }
                    } else {
                        frame.render_widget(line, line_area);
                    }
                }
            }
            DetailsLine::HunkHeader {
                width,
                line,
                cli_id,
                id: _,
            } => {
                *highlight_lines = None;

                let is_marked = cli_id.as_ref().is_some_and(|id| marks.contains_cli_id(id));

                let width = if is_marked { *width + 3 } else { *width };

                if skip_display_lines == 0 {
                    let Some(mut out) = areas
                        .next()
                        .map(|area| RenderSingleLineSpans::new(frame, area))
                    else {
                        return RenderedLine::viewport_filled();
                    };
                    for _ in 0..width {
                        out.render(Span::raw("─").style(self.theme.border));
                    }
                    out.render(Span::raw("╮").style(self.theme.border));
                }

                if skip_display_lines <= 1 {
                    let Some(mut out) = areas
                        .next()
                        .map(|area| RenderSingleLineSpans::new(frame, area))
                    else {
                        return RenderedLine::viewport_filled();
                    };
                    if is_marked {
                        out.extend([
                            Span::raw(" "),
                            self.theme.sym().mark.span(),
                            Span::raw(" ").style(self.theme.tui_mark),
                        ]);
                    }
                    for span in line {
                        out.render_ref(span);
                    }
                }

                if skip_display_lines <= 2 {
                    let Some(mut out) = areas
                        .next()
                        .map(|area| RenderSingleLineSpans::new(frame, area))
                    else {
                        return RenderedLine::viewport_filled();
                    };
                    for _ in 0..width {
                        out.render(Span::raw("─").style(self.theme.border));
                    }
                    out.render(Span::raw("╯").style(self.theme.border));
                }

                if skip_display_lines <= 3 {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };
                    frame.render_widget(" ", line_area);
                }
            }
            DetailsLine::TextToWrap { text, .. } => {
                *highlight_lines = None;

                for line in wrapped_text_lines(text, width).skip(skip_display_lines) {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };

                    frame.render_widget(&*line, line_area);
                }
            }
            DetailsLine::Code(line) => {
                if skip_display_lines == 0 {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };

                    if highlight_lines.is_none() {
                        let syntax = line.syntax(syntax_set);
                        *highlight_lines = Some(HighlightLines::new(syntax, syntax_theme));
                    }

                    let id = line.id;
                    let mut strings = self.strings.lock();

                    line.ensure_highlighted(
                        syntax_set,
                        highlight_lines.as_mut().unwrap(),
                        self.theme,
                        &mut strings,
                    );

                    let syntax_highlighted_line = line.syntax_highlighted_line.borrow();
                    let syntax_highlighted_line = syntax_highlighted_line
                        .as_ref()
                        .expect("line should have been highlighted by now");

                    if self.section_is_to_be_discarded(id) {
                        let crossed_out_line = syntax_highlighted_line
                            .spans
                            .iter()
                            .cloned()
                            .map(|span| span.crossed_out())
                            .collect::<Line<'_>>()
                            .style(syntax_highlighted_line.style);
                        frame.render_widget(crossed_out_line, line_area);
                    } else {
                        frame.render_widget(syntax_highlighted_line, line_area);
                    }

                    if line
                        .cli_id
                        .as_ref()
                        .is_some_and(|id| marks.contains_cli_id(id))
                    {
                        if let Some(color) = line.line_numbers.kind.bg(self.theme) {
                            if let Color::Rgb(r, g, b) = color {
                                let base = Rgb(r, g, b);
                                let mix = Rgb(150, 150, 150);
                                let weight = 0.3;
                                frame
                                    .buffer_mut()
                                    .set_style(line_area, base.lerp(mix, weight).into_bg_style());
                            }
                        } else {
                            frame
                                .buffer_mut()
                                .set_style(line_area, self.theme.tui_details_context_lines_marked);
                        }
                    }
                }
            }
            DetailsLine::SectionSeparator => {
                *highlight_lines = None;

                if skip_display_lines == 0 {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };

                    frame.render_widget("", line_area);
                }
            }
        }

        RenderedLine::line_finished()
    }

    fn reset_scroll(&self) {
        self.scroll.reset();
        *self.layout_cache.borrow_mut() = LayoutCache::default();
    }

    fn restore_cached_lines(&mut self, cached_lines: Vec<DetailsLine>) {
        self.worker.clear_next_job();
        self.clear_lines();
        self.reset_scroll();
        self.line_reader = ChannelLineReader::Finished;
        for line in cached_lines {
            let section_added = push_line(&mut self.lines, &mut self.sections, line);
            if section_added {
                self.select_first_section_if_pending();
            }
        }
        if self.sections.is_empty() {
            self.clear_pending_first_section_selection();
        }
    }

    fn clear_lines(&mut self) {
        self.lines.clear();
        self.sections.clear();
        self.selected_section.set(SelectedSection::None);
    }

    fn reset_line_reader(&mut self) {
        self.line_reader = Default::default();
        self.worker.clear_next_job();
    }

    pub(super) fn select_section_when_available(&self, index: usize, direction: ScrollDirection) {
        self.pending_section_selection
            .set(PendingSectionSelection::Section { index, direction });
    }

    fn select_first_section_if_pending(&self) {
        apply_pending_section_selection(
            &self.pending_section_selection,
            &self.selected_section,
            &self.scroll,
            self.sections.len(),
        );
    }

    fn clear_pending_first_section_selection(&self) {
        self.pending_section_selection
            .set(PendingSectionSelection::None);
    }

    fn update_selected_section_for_visible_range(
        &self,
        visible_start: usize,
        viewport_height: usize,
        layout_cache: &LayoutCache,
    ) {
        let SelectedSection::Selected(selected_index) = self.selected_section.get() else {
            return;
        };
        let visible_end = visible_start.saturating_add(viewport_height);
        if visible_start >= visible_end {
            return;
        }

        if self.sections.get(selected_index).is_some_and(|section| {
            section_intersects_visible_range(section, visible_start, visible_end, layout_cache)
        }) {
            return;
        }

        let Some(direction) = self.scroll.direction() else {
            return;
        };
        let new_index = match direction {
            ScrollDirection::Down => {
                self.topmost_visible_section_index(visible_start, visible_end, layout_cache)
            }
            ScrollDirection::Up => {
                self.bottommost_visible_section_index(visible_start, visible_end, layout_cache)
            }
        };

        if let Some(new_index) = new_index {
            self.selected_section
                .set(SelectedSection::Selected(new_index));
        }
    }

    fn topmost_visible_section_index(
        &self,
        visible_start: usize,
        visible_end: usize,
        layout_cache: &LayoutCache,
    ) -> Option<usize> {
        let index = self.sections.partition_point(|section| {
            section_display_range(section, layout_cache).1 <= visible_start
        });
        self.sections.get(index).and_then(|section| {
            section_intersects_visible_range(section, visible_start, visible_end, layout_cache)
                .then_some(index)
        })
    }

    fn bottommost_visible_section_index(
        &self,
        visible_start: usize,
        visible_end: usize,
        layout_cache: &LayoutCache,
    ) -> Option<usize> {
        let index = self.sections.partition_point(|section| {
            section_display_range(section, layout_cache).0 < visible_end
        });
        let index = index.checked_sub(1)?;
        self.sections.get(index).and_then(|section| {
            section_intersects_visible_range(section, visible_start, visible_end, layout_cache)
                .then_some(index)
        })
    }

    fn selected_section_visible_range(
        &self,
        scroll_top: usize,
        viewport_height: usize,
        layout_cache: &LayoutCache,
        tui_has_focus: bool,
    ) -> Option<(SectionId, Option<Arc<CliId>>, std::ops::Range<usize>)> {
        if !tui_has_focus {
            return None;
        }
        let SelectedSection::Selected(index) = self.selected_section.get() else {
            return None;
        };
        let section = self.sections.get(index)?;
        let visible_end = scroll_top.saturating_add(viewport_height);
        let (section_start, section_end) = section_display_range(section, layout_cache);
        let start = section_start.max(scroll_top);
        let end = section_end.min(visible_end);
        let range = (start < end).then_some(start - scroll_top..end - scroll_top)?;
        Some((section.id, section.cli_id.as_ref().map(Arc::clone), range))
    }

    fn section_is_highlighted(&self, id: SectionId) -> bool {
        self.highlights.contains(&id)
    }

    fn section_is_to_be_discarded(&self, id: SectionId) -> bool {
        self.to_be_discarded.contains(&id)
    }

    fn copy_current_hunk(&mut self) -> anyhow::Result<()> {
        let section = match self.selected_section.get() {
            SelectedSection::Selected(i) => &self.sections[i],
            SelectedSection::None | SelectedSection::Deselected(_) => return Ok(()),
        };

        let lines = &self.lines[section.first_line..=section.last_line];
        let hunk_text = format_lines_in_section(lines);

        self.clipboard.set_text(hunk_text)?;

        self.highlights.insert(section.id);

        Ok(())
    }

    fn selected_uncommitted(&self) -> bool {
        let Some(status_selection) = &self.selection else {
            return false;
        };
        match status_selection {
            CliId::UncommittedHunkOrFile(..) | CliId::Uncommitted { .. } => true,
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Commit { .. }
            | CliId::Stack { .. } => false,
        }
    }

    fn handle_discard(&mut self, messages: &mut Vec<Message>, marks: MarksRef<'_>) {
        if marks.is_empty() {
            self.handle_discard_selection(messages);
        } else {
            self.handle_discard_marks(messages, marks);
        }
    }

    fn handle_discard_selection(&mut self, messages: &mut Vec<Message>) {
        if !self.selected_uncommitted() {
            return;
        }
        let SelectedSection::Selected(selected_section_idx) = self.selected_section.get() else {
            return;
        };
        let section = &self.sections[selected_section_idx];
        let Some(section_cli_id) = section.cli_id.as_ref().map(Arc::clone) else {
            return;
        };
        let select_after_discard = if self.sections.get(selected_section_idx + 1).is_some() {
            PendingSectionSelection::Section {
                index: selected_section_idx,
                direction: ScrollDirection::Down,
            }
        } else {
            PendingSectionSelection::Section {
                index: selected_section_idx.saturating_sub(1),
                direction: ScrollDirection::Up,
            }
        };

        self.to_be_discarded = Vec::from([section.id]);

        let drop_to_be_discarded = message_on_drop(
            Message::Details(DetailsMessage::DropToBeDiscarded),
            messages,
        );

        let (formatted_cli_id, formatted_path) =
            if let CliId::UncommittedHunkOrFile(hunk) = &*section_cli_id {
                (
                    Span::raw(section_cli_id.to_short_string()).style(self.theme.cli_id),
                    Span::raw(hunk.hunk_assignments.head.path.clone()),
                )
            } else {
                return;
            };

        let confirm = Confirm::new(
            NonEmpty::new(Line::from_iter([
                Span::raw("Discard hunk "),
                formatted_cli_id,
                Span::raw(" "),
                formatted_path,
                Span::raw("?"),
            ])),
            self.theme,
            move |ctx, messages| {
                let changes = {
                    let context_lines = ctx.settings.context_lines;
                    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
                    let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
                    builder.push_changes_from_id(&section_cli_id)?;
                    builder.into_diff_specs()
                };

                if changes.is_empty() {
                    return Ok(());
                }

                but_api::legacy::workspace::discard_worktree_changes(ctx, changes)?;

                let PendingSectionSelection::Section { index, direction } = select_after_discard
                else {
                    unreachable!("discard selection is always a specific details section")
                };
                messages.push(Message::Reload(
                    Some(SelectAfterReload::UncommittedDetailsSection { index, direction }),
                    ReloadCause::Mutation,
                ));

                drop(drop_to_be_discarded);

                Ok(())
            },
        );

        messages.push(Message::ShowModal(Modal::Confirm { confirm }));
    }

    fn handle_discard_marks(&mut self, messages: &mut Vec<Message>, marks: MarksRef<'_>) {
        match marks {
            MarksRef::Hunks { .. } => {}
            MarksRef::Empty | MarksRef::Commits { .. } | MarksRef::CommittedFiles { .. } => return,
        }

        self.to_be_discarded = self
            .sections
            .iter()
            .filter_map(|section| {
                let cli_id = section.cli_id.as_deref()?;
                if marks.contains_cli_id(cli_id) {
                    Some(section.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Keep the current section if it remains; otherwise select the first remaining section
        // below it, falling back to the first remaining section above it. Translate that section's
        // index to its position after all marked sections have been removed.
        let select_after_discard = self
            .selected_section
            .get()
            .index()
            .and_then(|selected_index| {
                let selected_section = self.sections.get(selected_index)?;
                let target_index = if !section_is_marked(selected_section, marks) {
                    selected_index
                } else {
                    self.sections
                        .iter()
                        .enumerate()
                        .skip(selected_index + 1)
                        .find_map(|(index, section)| {
                            (!section_is_marked(section, marks)).then_some(index)
                        })
                        .or_else(|| {
                            self.sections
                                .iter()
                                .enumerate()
                                .take(selected_index)
                                .rev()
                                .find_map(|(index, section)| {
                                    (!section_is_marked(section, marks)).then_some(index)
                                })
                        })?
                };
                let index = self.sections[..target_index]
                    .iter()
                    .filter(|section| !section_is_marked(section, marks))
                    .count();
                let direction = if target_index > selected_index {
                    ScrollDirection::Down
                } else {
                    ScrollDirection::Up
                };
                Some(PendingSectionSelection::Section { index, direction })
            });

        let drop_to_be_discarded = message_on_drop(
            Message::Details(DetailsMessage::DropToBeDiscarded),
            messages,
        );

        let confirm = {
            let marks = marks.to_owned();

            Confirm::new(
                NonEmpty::new(Span::raw("Discard?").into()),
                self.theme,
                move |ctx, messages| {
                    let changes = {
                        let context_lines = ctx.settings.context_lines;
                        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
                        let mut builder = DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
                        for mark in marks.iter() {
                            match mark {
                                MarkableRef::Uncommitted(hunk) => {
                                    builder.push_changes_from_uncommitted(hunk)?;
                                }
                                MarkableRef::Commit(c) => {
                                    builder.push_changes_from_commit(c.commit_id, c.id)?;
                                }
                                MarkableRef::CommittedFile(c) => {
                                    builder
                                        .push_changes_from_committed_file(c.commit_id, c.path)?;
                                }
                            }
                        }
                        match marks {
                            Marks::Hunks(..) => {
                                builder.reconcile_worktree_diff_specs()?;
                            }
                            Marks::Empty | Marks::Commits(..) | Marks::CommittedFiles(..) => {}
                        }
                        builder.into_diff_specs()
                    };

                    if changes.is_empty() {
                        return Ok(());
                    }

                    but_api::legacy::workspace::discard_worktree_changes(ctx, changes)?;

                    let select_after_reload = select_after_discard.map(|selection| {
                        let PendingSectionSelection::Section { index, direction } = selection
                        else {
                            unreachable!("discard marks can only select a specific details section")
                        };
                        SelectAfterReload::UncommittedDetailsSection { index, direction }
                    });
                    messages.extend([
                        Message::Reload(select_after_reload, ReloadCause::Mutation),
                        Message::ClearMarks,
                    ]);

                    drop(drop_to_be_discarded);

                    Ok(())
                },
            )
        };

        messages.push(Message::ShowModal(Modal::Confirm { confirm }));
    }

    fn handle_mark(
        &mut self,
        messages: &mut Vec<Message>,
        marks: &mut Marks,
        backstack: &mut Backstack,
    ) -> anyhow::Result<()> {
        // we need all the sections to toggle parent marks so dont allow marking until we've
        // finished streaming the diff
        if self.is_polling_thread() {
            return Ok(());
        }

        if !self.selected_uncommitted() {
            return Ok(());
        }
        let SelectedSection::Selected(selected_section_idx) = self.selected_section.get() else {
            return Ok(());
        };
        let section = &self.sections[selected_section_idx];
        let Some(section_cli_id) = section.cli_id.as_ref().map(Arc::clone) else {
            return Ok(());
        };
        let Some(markable @ MarkableRef::Uncommitted(hunk)) =
            MarkableRef::try_from_cli_id(section_cli_id.as_ref())
        else {
            return Ok(());
        };

        toggle_markables(marks, [markable.to_owned()])?;

        let sections_for_file_cli_ids = self
            .sections
            .iter()
            .filter_map(|section| {
                let cli_id = section.cli_id.as_deref()?;
                let CliId::UncommittedHunkOrFile(section_hunk) = cli_id else {
                    return None;
                };
                // the detail view can contain hunks from multiple files, for example if viewing
                // the uncommitted area, so only check hunks from the marked file
                if section_hunk.hunk_assignments.head.path_bytes
                    != hunk.hunk_assignments.head.path_bytes
                {
                    return None;
                }
                Some(section_hunk)
            })
            .collect::<Vec<_>>();
        if let Some(sections_for_file_cli_ids) = NonEmpty::from_vec(sections_for_file_cli_ids) {
            let all_sections_marked = sections_for_file_cli_ids
                .iter()
                .all(|hunk| marks.contains_mark(*hunk));
            let parent_hunk = synthetic_parent_hunk(
                &sections_for_file_cli_ids.head.id,
                0,
                sections_for_file_cli_ids.flat_map(|hunk| hunk.hunk_assignments.clone()),
            );
            if all_sections_marked {
                marks.insert_mark(parent_hunk)?;
            } else {
                marks.remove_mark(&parent_hunk);
            }
        }

        if marks.is_empty() {
            backstack.remove_mark();
        } else {
            backstack.push_mark();
        }

        messages.push(Message::Details(DetailsMessage::SelectNextSection));

        Ok(())
    }
}

fn apply_pending_section_selection(
    pending_section_selection: &Cell<PendingSectionSelection>,
    selected_section: &Cell<SelectedSection>,
    scroll: &ScrollState,
    sections_len: usize,
) {
    match pending_section_selection.get() {
        PendingSectionSelection::None => {}
        PendingSectionSelection::First => {
            if sections_len == 0 {
                return;
            }
            selected_section.set(SelectedSection::Selected(0));
            scroll.goto_top();
            pending_section_selection.set(PendingSectionSelection::None);
        }
        PendingSectionSelection::Section { index, direction } => {
            if index >= sections_len {
                return;
            }
            selected_section.set(SelectedSection::Selected(index));
            scroll.to_section(index, direction);
            pending_section_selection.set(PendingSectionSelection::None);
        }
    }
}

struct ChannelLineWriter {
    tx: std::sync::mpsc::SyncSender<RenderThreadMessage>,
}

impl DiffLineWriter for ChannelLineWriter {
    fn write(&mut self, line: DetailsLine) -> anyhow::Result<()> {
        let result = self.tx.send(RenderThreadMessage::Line(line));
        if result.is_ok() {
            Ok(())
        } else {
            Err(anyhow::Error::new(SendErrorCode))
        }
    }
}

/// Error code used to identify errors cause the receiving half of channel having been dropped.
///
/// This is expected and will happen if we start rendering the diff of one item but then change our
/// selection.
#[derive(Debug)]
struct SendErrorCode;

impl Display for SendErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("send failed, receiver disconnected")
    }
}

impl std::error::Error for SendErrorCode {}

#[derive(Debug)]
struct Section {
    id: SectionId,
    cli_id: Option<Arc<CliId>>,
    first_line: usize,
    last_line: usize,
}

/// Returns whether a new section was added.
#[must_use]
fn push_line(lines: &mut Vec<DetailsLine>, sections: &mut Vec<Section>, line: DetailsLine) -> bool {
    let line_index = lines.len();
    let section_added = extend_section_list(sections, line_index, &line);
    lines.push(line);
    section_added
}

fn extend_section_list(sections: &mut Vec<Section>, line_index: usize, line: &DetailsLine) -> bool {
    let (id, cli_id) = match line {
        DetailsLine::Text { id, cli_id, .. } => {
            if let Some(id) = id {
                (*id, cli_id.clone())
            } else {
                return false;
            }
        }
        DetailsLine::HunkHeader { id, cli_id, .. } => (*id, cli_id.clone()),
        DetailsLine::Code(line) => (line.id, line.cli_id.clone()),
        DetailsLine::TextToWrap { id, .. } => (*id, None),
        DetailsLine::SectionSeparator => return false,
    };

    if let Some(last) = sections.last_mut()
        && last.id == id
    {
        last.last_line = line_index;
        return false;
    }

    sections.push(Section {
        id,
        cli_id,
        first_line: line_index,
        last_line: line_index,
    });

    true
}

#[derive(Debug, Default)]
struct ScrollState {
    top: Cell<usize>,
    pending: Cell<Option<ScrollIntent>>,
    direction: Cell<Option<ScrollDirection>>,
}

impl ScrollState {
    fn top(&self) -> usize {
        self.top.get()
    }

    fn set_top(&self, top: usize) {
        self.top.set(top);
    }

    fn up(&self, n: usize) {
        self.top.set(self.top.get().saturating_sub(n));
        self.pending.set(None);
        self.direction.set(Some(ScrollDirection::Up));
    }

    fn down(&self, n: usize) {
        self.top.set(self.top.get().saturating_add(n));
        self.pending.set(None);
        self.direction.set(Some(ScrollDirection::Down));
    }

    fn goto_top(&self) {
        self.top.set(0);
        self.pending.set(None);
        self.direction.set(Some(ScrollDirection::Up));
    }

    fn goto_bottom(&self) {
        self.pending.set(Some(ScrollIntent::Bottom));
        self.direction.set(Some(ScrollDirection::Down));
    }

    fn to_section(&self, index: usize, direction: ScrollDirection) {
        self.pending.set(Some(ScrollIntent::Section { index }));
        self.direction.set(Some(direction));
    }

    fn direction(&self) -> Option<ScrollDirection> {
        self.direction.get()
    }

    fn take_pending(&self) -> Option<ScrollIntent> {
        let pending = self.pending.get();
        self.pending.set(None);
        pending
    }

    fn reset(&self) {
        self.top.set(0);
        self.pending.set(None);
        self.direction.set(None);
    }
}

#[derive(Debug, Copy, Clone)]
enum ScrollIntent {
    Bottom,
    Section { index: usize },
}

#[derive(Debug, Copy, Clone)]
pub(super) enum ScrollDirection {
    Up,
    Down,
}

#[derive(Debug, Copy, Clone, Default)]
enum PendingSectionSelection {
    #[default]
    None,
    First,
    Section {
        index: usize,
        direction: ScrollDirection,
    },
}

#[derive(Debug, Default)]
struct LayoutCache {
    width: u16,
    line_count: usize,
    heights: Vec<usize>,
    prefix_sum: Vec<usize>,
}

impl LayoutCache {
    fn update(&mut self, width: u16, lines: &[DetailsLine]) {
        count_allocations("update_cache", || {
            if self.width != width || self.line_count > lines.len() {
                self.rebuild(width, lines);
                return;
            }

            if self.line_count == lines.len() {
                return;
            }

            if self.prefix_sum.is_empty() {
                self.prefix_sum.push(0);
            }

            for line in &lines[self.line_count..] {
                let height = display_height(line, width);
                self.heights.push(height);
                let next = self.prefix_sum.last().copied().unwrap_or_default() + height;
                self.prefix_sum.push(next);
            }
            self.line_count = lines.len();
        });
    }

    fn rebuild(&mut self, width: u16, lines: &[DetailsLine]) {
        self.width = width;
        self.line_count = 0;
        self.heights.clear();
        self.prefix_sum.clear();
        self.update(width, lines);
    }

    fn total_display_lines(&self) -> usize {
        self.prefix_sum.last().copied().unwrap_or_default()
    }

    fn line_at_display_row(&self, row: usize) -> Option<(usize, usize)> {
        if self.line_count == 0 || row >= self.total_display_lines() {
            return None;
        }

        let line_index = self.prefix_sum.partition_point(|start| *start <= row) - 1;
        Some((line_index, row - self.prefix_sum[line_index]))
    }

    fn display_row_for_line(&self, line_index: usize) -> usize {
        self.prefix_sum[line_index]
    }

    fn display_row_after_line(&self, line_index: usize) -> usize {
        self.prefix_sum[line_index + 1]
    }
}

fn scroll_top_for_section(
    section_start: usize,
    section_end: usize,
    viewport_height: usize,
    current_top: usize,
    max_scroll_top: usize,
) -> usize {
    let current_top = current_top.min(max_scroll_top);
    if viewport_height == 0 {
        return current_top;
    }

    let visible_end = current_top.saturating_add(viewport_height);
    let section_height = section_end.saturating_sub(section_start);
    if section_height > viewport_height || section_start < current_top {
        return section_start.min(max_scroll_top);
    }

    if section_end > visible_end {
        return section_end
            .saturating_sub(viewport_height)
            .min(max_scroll_top);
    }

    current_top
}

fn section_intersects_visible_range(
    section: &Section,
    visible_start: usize,
    visible_end: usize,
    layout_cache: &LayoutCache,
) -> bool {
    let (section_start, section_end) = section_display_range(section, layout_cache);
    section_start < visible_end && section_end > visible_start
}

fn section_display_range(section: &Section, layout_cache: &LayoutCache) -> (usize, usize) {
    (
        layout_cache.display_row_for_line(section.first_line),
        layout_cache.display_row_after_line(section.last_line),
    )
}

fn display_height(line: &DetailsLine, width: u16) -> usize {
    match line {
        DetailsLine::Text { .. } | DetailsLine::Code(_) | DetailsLine::SectionSeparator => 1,
        DetailsLine::TextToWrap { text, .. } => wrapped_text_lines(text, width).count(),
        DetailsLine::HunkHeader { .. } => {
            // 2 for the top and bottom boxes, 1 for the path itself, and 1 for the bottom padding:
            // ─────────╮
            //  or file │
            // ─────────╯
            //
            4
        }
    }
}

fn wrapped_text_lines(text: &str, width: u16) -> impl Iterator<Item = std::borrow::Cow<'_, str>> {
    textwrap::wrap(text, textwrap::Options::new(usize::from(width.max(1))))
        .into_iter()
        .with_position()
        .filter_map(|(pos, line)| match pos {
            Position::First | Position::Middle | Position::Only => Some(line),
            Position::Last => (!line.is_empty()).then_some(line),
        })
        .map(|line| if line.is_empty() { " ".into() } else { line })
}

struct RenderedLine {
    filled_viewport: bool,
}

impl RenderedLine {
    fn viewport_filled() -> Self {
        Self {
            filled_viewport: true,
        }
    }

    fn line_finished() -> Self {
        Self {
            filled_viewport: false,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
enum SelectedSection {
    #[default]
    None,
    Selected(usize),
    Deselected(usize),
}

impl SelectedSection {
    fn index(self) -> Option<usize> {
        match self {
            SelectedSection::None => None,
            SelectedSection::Selected(n) | SelectedSection::Deselected(n) => Some(n),
        }
    }

    fn with_index(self, index: usize) -> Self {
        match self {
            SelectedSection::None | SelectedSection::Selected(_) => {
                SelectedSection::Selected(index)
            }
            SelectedSection::Deselected(_) => SelectedSection::Deselected(index),
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
enum CacheKey {
    Commit(ObjectId),
}

/// The diffs for commits don't change (since that'd change the sha) so we can cache them.
#[derive(Debug, Default)]
struct Cache {
    // using a Vec is fine since the number of cache entries can't exceed commits in the workspace,
    // which is naturally bounded
    //
    // sorted by length of `Vec<DetailsLine>` (longest last)
    entries: Vec<(CacheKey, Vec<DetailsLine>)>,
    num_lines: usize,
}

impl Cache {
    const MAX_CACHE_LINES: usize = 500_000;

    fn insert(&mut self, key: CacheKey, lines: Vec<DetailsLine>) {
        if self.get(key).is_some() {
            return;
        }

        self.num_lines += lines.len();
        self.entries.push((key, lines));
        self.entries.sort_unstable_by_key(|(_, a)| a.len());
        self.bound_size();
    }

    fn get(&self, key: CacheKey) -> Option<&Vec<DetailsLine>> {
        self.entries
            .iter()
            .find_map(|(k, v)| (key == *k).then_some(v))
    }

    fn bound_size(&mut self) {
        while self.num_lines > Self::MAX_CACHE_LINES {
            let Some((_key, lines)) = self.entries.pop() else {
                break;
            };
            self.num_lines -= lines.len();
        }
    }
}

fn format_lines_in_section(lines: &[DetailsLine]) -> String {
    let mut text = String::new();
    let mut path = None;

    for line in lines {
        match line {
            DetailsLine::Text {
                line,
                skip_when_copying_hunk,
                ..
            } => {
                if *skip_when_copying_hunk {
                    continue;
                }
                for span in &line.spans {
                    text.push_str(&span.content);
                }
                text.push('\n');
            }
            DetailsLine::TextToWrap {
                text: line_text, ..
            } => {
                text.push_str(line_text);
                text.push('\n');
            }
            DetailsLine::Code(code_line) => {
                path = Some(Arc::clone(&code_line.path));
                code_line.with_line_from_diff(|line_text| {
                    match code_line.line_numbers.kind {
                        CodeLineKind::Addition { .. } => text.push('+'),
                        CodeLineKind::Deletion { .. } => text.push('-'),
                        CodeLineKind::Context { .. } => text.push(' '),
                    }
                    text.push_str(line_text);
                });
                text.push('\n');
            }
            DetailsLine::SectionSeparator | DetailsLine::HunkHeader { .. } => {}
        }
    }

    if let Some(path) = path {
        format!("{path}\n\n{text}")
    } else {
        text
    }
}

fn section_is_marked(section: &Section, marks: MarksRef<'_>) -> bool {
    let Some(cli_id) = section.cli_id.as_deref() else {
        return false;
    };
    marks.contains_cli_id(cli_id)
}
