use std::{
    cell::{Cell, RefCell},
    fmt::Display,
    sync::{
        Arc,
        atomic::AtomicUsize,
        mpsc::{Sender, TryRecvError},
    },
    time::Instant,
};

use anyhow::Context as _;
use bstr::{BString, ByteSlice as _};
use but_ctx::{Context, OnDemand};
use gix::ObjectId;
use itertools::{Itertools as _, Position};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Stylize as _},
    text::{Line, Span},
};
use syntect::{easy::HighlightLines, highlighting, parsing::SyntaxSet};

use crate::{
    CliId,
    command::legacy::status::tui::{
        Message, count_allocations,
        details::DetailsMessage,
        details2::strings::{SharedStrings, Strings},
        highlight::{self, Highlights},
    },
    theme::Theme,
    utils::DebugAsType,
};

mod rendering;
mod strings;

const CHANNEL_SIZE: usize = 1024;

#[derive(Debug)]
pub struct Details2 {
    theme: &'static Theme,
    selection: Option<CliId>,
    lines: Vec<DetailsLine>,
    line_reader: ChannelLineReader,
    syntax_set: DebugAsType<OnDemand<SyntaxSet>>,
    syntax_theme: DebugAsType<OnDemand<highlighting::Theme>>,
    strings: Strings,
    selected_section: Cell<SelectedSection>,
    sections: Vec<Section>,
    scroll: ScrollState,
    layout_cache: RefCell<LayoutCache>,
    cache: Cache,
    out_of_band_messages_tx: Sender<Message>,
    pub highlights: Highlights<SectionId>,
}

#[derive(Debug, Default)]
enum ChannelLineReader {
    #[default]
    NotStarted,
    Started {
        rx: std::sync::mpsc::Receiver<DetailsLine>,
        start: Instant,
        cache_key: Option<CacheKey>,
    },
    Finished,
}

impl Details2 {
    pub fn new(theme: &'static Theme, out_of_band_messages_tx: Sender<Message>) -> Self {
        Self {
            theme,
            selection: Default::default(),
            lines: Default::default(),
            sections: Default::default(),
            syntax_set: OnDemand::new(|| Ok(SyntaxSet::load_defaults_newlines())).into(),
            syntax_theme: OnDemand::new(|| theme.load_syntax_highlighting_theme()).into(),
            strings: Default::default(),
            selected_section: Cell::default(),
            line_reader: Default::default(),
            scroll: Default::default(),
            layout_cache: Default::default(),
            cache: Default::default(),
            out_of_band_messages_tx,
            highlights: Default::default(),
        }
    }

    pub fn is_finished_rendering(&self) -> bool {
        match &self.line_reader {
            ChannelLineReader::NotStarted | ChannelLineReader::Started { .. } => false,
            ChannelLineReader::Finished => true,
        }
    }

    pub fn is_polling_thread(&self) -> bool {
        match &self.line_reader {
            ChannelLineReader::NotStarted | ChannelLineReader::Finished => false,
            ChannelLineReader::Started { .. } => true,
        }
    }

    pub fn num_threads(&self) -> usize {
        NUM_THREADS.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn cache_size(&self) -> usize {
        self.cache.num_lines
    }

    pub fn update(
        &mut self,
        ctx: &mut Context,
        new_selection: Option<&CliId>,
        is_visible: bool,
    ) -> anyhow::Result<bool> {
        if !is_visible {
            self.clear_lines();
            self.line_reader = Default::default();
            self.reset_scroll();
            return Ok(false);
        }

        let (selection, selection_did_change) = match (self.selection.as_ref(), new_selection) {
            (None, None) => {
                // no selection
                self.line_reader = Default::default();

                return Ok(false);
            }
            (None, Some(new)) => {
                // selected something
                self.selection = Some(new.clone());
                self.line_reader = Default::default();

                (new, true)
            }
            (Some(_), None) => {
                // deselected
                self.selection = None;
                self.line_reader = Default::default();
                self.clear_lines();
                self.reset_scroll();

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
                    self.line_reader = Default::default();
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
                    move |ctx, theme, id_gen, line_writer| {
                        rendering::render_commit(commit, ctx, theme, id_gen, line_writer)
                    },
                )
            }
            CliId::Branch { name, .. } => {
                let name = name.to_owned();
                self.poll_render_thread(
                    ctx,
                    None,
                    selection_did_change,
                    move |ctx, theme, id_gen, line_writer| {
                        rendering::render_branch(name, ctx, theme, id_gen, line_writer)
                    },
                )
            }
            CliId::Uncommitted { .. } => self.poll_render_thread(
                ctx,
                None,
                selection_did_change,
                move |ctx, theme, id_gen, line_writer| {
                    rendering::render_uncommitted(ctx, theme, id_gen, line_writer)
                },
            ),
            CliId::UncommittedHunkOrFile(uncommitted) => {
                let uncommitted = uncommitted.clone();
                self.poll_render_thread(
                    ctx,
                    None,
                    selection_did_change,
                    move |ctx, theme, id_gen, line_writer| {
                        rendering::render_uncommitted_hunk(
                            uncommitted,
                            ctx,
                            theme,
                            id_gen,
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
                    move |ctx, theme, id_gen, line_writer| {
                        rendering::render_committed_file(
                            commit,
                            path,
                            id,
                            ctx,
                            theme,
                            id_gen,
                            line_writer,
                        )
                    },
                )
            }
            CliId::Stack { .. } => {
                self.clear_lines();
                self.reset_scroll();
                push_line(
                    &mut self.lines,
                    &mut self.sections,
                    DetailsLine::Text {
                        id: None,
                        line: Line::from("(stack assignments are not supported)")
                            .style(self.theme.hint),
                        skip_when_copying_hunk: false,
                    },
                );
                Ok(true)
            }
            CliId::PathPrefix { .. } => {
                self.clear_lines();
                self.reset_scroll();
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
                &mut dyn LineWriter,
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

        let num_threads_guard = NumThreadsGuard::new();

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

                // spawning a new thread immediately here without a pool is fine since, if the
                // selection changes the previous will thread will end when it tries to send on
                // the, now disconnected, channel
                std::thread::spawn(move || {
                    let mut ctx = ctx.into_thread_local();
                    let mut id_gen = IdGen::new(strings);

                    count_allocations("details fetch diff", || {
                        if let Err(err) = f(&mut ctx, theme, &mut id_gen, &mut line_writer)
                            .context("failed rendering commit diff")
                            && err.downcast_ref::<SendErrorCode>().is_none()
                        {
                            tracing::error!("{err:#}");
                            _ = error_tx.send(Message::ShowError(Arc::new(err)));
                        }
                    });

                    drop(num_threads_guard);
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
                        Ok(line) => {
                            push_line(&mut self.lines, &mut self.sections, line);
                        }
                        Err(err) => match err {
                            TryRecvError::Empty => break Ok(false),
                            TryRecvError::Disconnected => {
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

                                self.line_reader = ChannelLineReader::Finished;

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
            ChannelLineReader::Finished => Ok(false),
        }
    }

    pub fn render(&self, _help_shown: bool, tui_has_focus: bool, area: Rect, frame: &mut Frame) {
        let syntax_set = self.syntax_set.get().unwrap();
        let syntax_theme = self.syntax_theme.get().unwrap();

        let section_selected_bg = self.theme.discrete_selection_highlight.bg.unwrap();

        let mut layout_cache = self.layout_cache.borrow_mut();
        layout_cache.update(area.width, &self.lines);

        let total_display_lines = layout_cache.total_display_lines();
        let viewport_height = area.height as usize;
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
                section_selected_bg,
                &syntax_set,
                &syntax_theme,
                frame,
            );

            if !rendered.filled_viewport {
                line_index += 1;
                line_offset = 0;
            } else {
                break;
            }
        }
    }

    #[allow(clippy::ptr_arg)]
    pub fn try_handle_message(
        &mut self,
        msg: DetailsMessage,
        _messages: &mut Vec<Message>,
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
                self.selected_section
                    .set(match self.selected_section.get() {
                        SelectedSection::None => SelectedSection::Selected(0),
                        SelectedSection::Selected(n) | SelectedSection::Deselected(n) => {
                            SelectedSection::Selected(n)
                        }
                    });
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
        }

        Ok(())
    }

    #[expect(clippy::too_many_arguments)]
    fn render_details_line(
        &self,
        line: &DetailsLine,
        skip_display_lines: usize,
        areas: &mut impl Iterator<Item = Rect>,
        width: u16,
        tui_has_focus: bool,
        section_selected_bg: Color,
        syntax_set: &SyntaxSet,
        syntax_theme: &highlighting::Theme,
        frame: &mut Frame,
    ) -> RenderedLine {
        match line {
            DetailsLine::Text {
                line,
                id,
                skip_when_copying_hunk: _,
            } => {
                if skip_display_lines == 0 {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };

                    if let Some(id) = id
                        && self.section_is_highlighted(*id)
                    {
                        frame.render_widget(line.clone().style(highlight::style()), line_area);
                    } else if let Some(id) = id
                        && self.section_is_selected(*id, tui_has_focus)
                    {
                        frame.render_widget(line.clone().bg(section_selected_bg), line_area);
                    } else {
                        frame.render_widget(line, line_area);
                    }
                }
            }
            DetailsLine::TextToWrap { text, id } => {
                for line in wrapped_text_lines(text, width).skip(skip_display_lines) {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };

                    if self.section_is_highlighted(*id) {
                        frame.render_widget(Line::from(line).style(highlight::style()), line_area);
                    } else if self.section_is_selected(*id, tui_has_focus) {
                        frame.render_widget(Line::from(line).bg(section_selected_bg), line_area);
                    } else {
                        frame.render_widget(&*line, line_area);
                    }
                }
            }
            DetailsLine::Code(line) => {
                if skip_display_lines == 0 {
                    let Some(line_area) = areas.next() else {
                        return RenderedLine::viewport_filled();
                    };

                    let id = line.id;

                    let mut strings = self.strings.lock();
                    line.ensure_highlighted(syntax_set, syntax_theme, self.theme, &mut strings);

                    let highlighted_line = line.highlighted_line.borrow();
                    let highlighted_line = highlighted_line
                        .as_ref()
                        .expect("ensure_highlighted was just called");

                    if self.section_is_highlighted(id) {
                        frame.render_widget(
                            highlighted_line.clone().style(highlight::style()),
                            line_area,
                        );
                    } else if self.section_is_selected(id, tui_has_focus) {
                        frame.render_widget(
                            highlighted_line.clone().bg(section_selected_bg),
                            line_area,
                        );
                    } else {
                        frame.render_widget(highlighted_line, line_area);
                    }
                }
            }
            DetailsLine::SectionSeparator => {
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
        self.clear_lines();
        self.reset_scroll();
        self.line_reader = ChannelLineReader::Finished;
        for line in cached_lines {
            push_line(&mut self.lines, &mut self.sections, line);
        }
    }

    fn clear_lines(&mut self) {
        self.lines.clear();
        self.sections.clear();
        self.selected_section.set(SelectedSection::None);
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

    fn section_is_selected(&self, id: SectionId, tui_has_focus: bool) -> bool {
        if !tui_has_focus {
            return false;
        }
        match self.selected_section.get() {
            SelectedSection::Selected(n) => {
                self.sections.get(n).is_some_and(|section| section.id == id)
            }
            SelectedSection::None | SelectedSection::Deselected(_) => false,
        }
    }

    fn section_is_highlighted(&self, id: SectionId) -> bool {
        self.highlights.contains(&id)
    }

    fn copy_current_hunk(&mut self) -> anyhow::Result<()> {
        let section = match self.selected_section.get() {
            SelectedSection::Selected(i) => &self.sections[i],
            SelectedSection::None | SelectedSection::Deselected(_) => return Ok(()),
        };

        let lines = &self.lines[section.first_line..=section.last_line];
        let hunk_text = format_lines_in_section(lines);

        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(hunk_text))
            .context("failed to copy to system clipboard")?;

        self.highlights.insert(section.id);

        Ok(())
    }
}

trait LineWriter {
    fn push(&mut self, line: DetailsLine) -> anyhow::Result<()>;

    fn push_selectable_text(&mut self, id: SectionId, line: Line<'static>) -> anyhow::Result<()> {
        self.push(DetailsLine::Text {
            id: Some(id),
            line,
            skip_when_copying_hunk: false,
        })
    }

    fn push_hunk_header(&mut self, id: SectionId, line: Line<'static>) -> anyhow::Result<()> {
        self.push(DetailsLine::Text {
            id: Some(id),
            line,
            skip_when_copying_hunk: true,
        })
    }

    #[expect(dead_code)]
    fn push_non_selectable_text(&mut self, line: Line<'static>) -> anyhow::Result<()> {
        self.push(DetailsLine::Text {
            id: None,
            line,
            skip_when_copying_hunk: false,
        })
    }

    fn push_empty_line(&mut self, id: SectionId) -> anyhow::Result<()> {
        self.push_selectable_text(id, " ".into())
    }

    fn push_section_separator(&mut self) -> anyhow::Result<()> {
        self.push(DetailsLine::SectionSeparator)
    }

    fn push_text_to_wrap(&mut self, id: SectionId, text: String) -> anyhow::Result<()> {
        self.push(DetailsLine::TextToWrap { id, text })
    }

    fn push_code(
        &mut self,
        id: SectionId,
        line_numbers: CodeLineNumbers,
        line_start_end: (usize, usize),
        diff: Arc<BString>,
        path: Arc<BString>,
    ) -> anyhow::Result<()> {
        self.push(DetailsLine::Code(DetailsCodeLine {
            id,
            highlighted_line: RefCell::new(None),
            line_numbers,
            line_start_end,
            diff,
            path,
        }))
    }
}

struct ChannelLineWriter {
    tx: std::sync::mpsc::SyncSender<DetailsLine>,
}

impl LineWriter for ChannelLineWriter {
    fn push(&mut self, line: DetailsLine) -> anyhow::Result<()> {
        let result = self.tx.send(line);
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
struct IdGen<'a> {
    pub strings: Strings,
    scope: &'static str,
    _marker: std::marker::PhantomData<&'a mut ()>,
}

impl IdGen<'_> {
    fn new(strings: Strings) -> Self {
        IdGen {
            strings,
            scope: "details",
            _marker: std::marker::PhantomData,
        }
    }

    fn new_id(&mut self, id: impl Display) -> SectionId {
        SectionId(self.strings.get(format!("{}/{}", self.scope, id)))
    }

    fn scoped(&mut self, scope: impl Display) -> IdGen<'_> {
        let scope = self.strings.get(format!("{}/{}", self.scope, scope));
        IdGen {
            strings: self.strings.clone(),
            scope,
            _marker: std::marker::PhantomData,
        }
    }
}

/// Each line in the details view is considered to be part of a "section". A section is the group
/// of lines that can be selected together such as a hunk.
///
/// `SectionId` is used to track which lines belong to the same section. `Details` tracks the
/// currently selected `SectionId` and when it renders a line with a matching it it'll highlight
/// it.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct SectionId(&'static str);

#[derive(Debug)]
struct Section {
    id: SectionId,
    first_line: usize,
    last_line: usize,
}

fn push_line(lines: &mut Vec<DetailsLine>, sections: &mut Vec<Section>, line: DetailsLine) {
    let line_index = lines.len();
    extend_section_list(sections, line_index, &line);
    lines.push(line);
}

fn extend_section_list(sections: &mut Vec<Section>, line_index: usize, line: &DetailsLine) {
    let id = match line {
        DetailsLine::Text { id, .. } => {
            if let Some(id) = id {
                *id
            } else {
                return;
            }
        }
        DetailsLine::Code(line) => line.id,
        DetailsLine::TextToWrap { id, .. } => *id,
        DetailsLine::SectionSeparator => return,
    };

    if let Some(last) = sections.last_mut()
        && last.id == id
    {
        last.last_line = line_index;
        return;
    }

    sections.push(Section {
        id,
        first_line: line_index,
        last_line: line_index,
    });
}

#[derive(Debug, Copy, Clone)]
struct CodeLineNumbers {
    old_width: u32,
    new_width: u32,
    kind: CodeLineKind,
}

#[derive(Debug, Copy, Clone)]
enum CodeLineKind {
    Addition { new_line: u32 },
    Deletion { old_line: u32 },
    Context { old_line: u32, new_line: u32 },
}

impl CodeLineKind {
    fn bg(self, theme: &'static Theme) -> Option<Color> {
        match self {
            CodeLineKind::Addition { .. } => theme.addition_rich.bg,
            CodeLineKind::Deletion { .. } => theme.deletion_rich.bg,
            CodeLineKind::Context { .. } => None,
        }
    }
}

impl CodeLineNumbers {
    fn addition(old_width: u32, new_width: u32, new_line: u32) -> Self {
        Self {
            old_width,
            new_width,
            kind: CodeLineKind::Addition { new_line },
        }
    }

    fn deletion(old_width: u32, new_width: u32, old_line: u32) -> Self {
        Self {
            old_width,
            new_width,
            kind: CodeLineKind::Deletion { old_line },
        }
    }

    fn context(old_width: u32, new_width: u32, old_line: u32, new_line: u32) -> Self {
        Self {
            old_width,
            new_width,
            kind: CodeLineKind::Context { old_line, new_line },
        }
    }

    fn spans(
        self,
        strings: &mut strings::SharedStrings,
        theme: &'static Theme,
    ) -> [Span<'static>; 6] {
        match self.kind {
            CodeLineKind::Addition { new_line } => [
                Span::raw(strings.get_spaces(self.old_width as _)),
                Span::styled(" ┊ ", theme.border),
                Span::raw(strings.get_spaces((self.new_width - num_digits(new_line)) as _)),
                Span::raw(strings.get_u32(new_line)).style(theme.addition),
                Span::styled(" │ ", theme.border),
                Span::raw("+").style(theme.addition_rich),
            ],
            CodeLineKind::Deletion { old_line } => [
                Span::raw(strings.get_spaces((self.old_width - num_digits(old_line)) as _)),
                Span::raw(strings.get_u32(old_line)).style(theme.deletion),
                Span::styled(" ┊ ", theme.border),
                Span::raw(strings.get_spaces(self.new_width as _)),
                Span::styled(" │ ", theme.border),
                Span::raw("-").style(theme.deletion_rich),
            ],
            CodeLineKind::Context { old_line, new_line } => [
                Span::raw(strings.get_spaces((self.old_width - num_digits(old_line)) as _)),
                Span::styled(strings.get_u32(old_line), theme.hint),
                Span::styled(" ┊ ", theme.border),
                Span::raw(strings.get_spaces((self.new_width - num_digits(new_line)) as _)),
                Span::styled(strings.get_u32(new_line), theme.hint),
                Span::styled(" │  ", theme.border),
            ],
        }
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum DetailsLine {
    Text {
        /// None if this line cannot be selected
        id: Option<SectionId>,
        line: Line<'static>,
        skip_when_copying_hunk: bool,
    },
    TextToWrap {
        id: SectionId,
        text: String,
    },
    Code(DetailsCodeLine),
    SectionSeparator,
}

#[derive(Debug, Clone)]
struct DetailsCodeLine {
    id: SectionId,
    line_numbers: CodeLineNumbers,
    // indexes into `diff` where the line starts and ends, including any line terminators
    line_start_end: (usize, usize),
    // the whole diff this line is part of
    //
    // we share the diff and store indexes to get the line to avoid allocating each line
    diff: Arc<BString>,
    path: Arc<BString>,
    // HACK: only when drawing this line to the screen do we syntax highlight it and cache the
    // result directly here. We dont have a mutable reference in `Details2::render` so have to
    // cheat with a `RefCell`.
    highlighted_line: RefCell<Option<Line<'static>>>,
}

impl DetailsCodeLine {
    fn ensure_highlighted(
        &self,
        syntax_set: &SyntaxSet,
        syntax_theme: &highlighting::Theme,
        theme: &'static Theme,
        strings: &mut SharedStrings,
    ) {
        let Self {
            highlighted_line,
            line_numbers,
            path,
            line_start_end: _,
            diff: _,
            id: _,
        } = self;

        if highlighted_line.borrow().is_some() {
            return;
        }

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

        // TODO: creating a new `HighlightLines` per line isn't ideal. Instead we should have reuse
        // two `HighlightLines` per hunk. One for added+context lines and one for deleted lines.
        // That should highlight correctly across lines and reuse internal buffers better.
        //
        // Remember to advance both `HighlightLines` on context lines.
        let mut highlight_lines = HighlightLines::new(syntax, syntax_theme);

        self.with_line_from_diff(|line| {
            let bg = line_numbers.kind.bg(theme);
            let line_numbers = line_numbers.spans(strings, theme);
            *highlighted_line.borrow_mut() =
                Some(Line::from_iter(line_numbers.into_iter().chain(
                    syntax_highlight(line, bg, &mut highlight_lines, syntax_set),
                )));
        });
    }

    fn with_line_from_diff<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&str) -> T,
    {
        let (start, end) = self.line_start_end;
        let line = self.diff[start..end].to_str_lossy();
        let line = line.strip_suffix('\n').unwrap_or(&line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        f(line)
    }
}

fn syntax_highlight(
    code: &str,
    bg: Option<Color>,
    highlight_lines: &mut HighlightLines<'_>,
    syntax_set: &SyntaxSet,
) -> Vec<Span<'static>> {
    let Ok(ranges) = highlight_lines.highlight_line(code, syntax_set) else {
        return Vec::from([Span::raw(code.to_owned())]);
    };

    ranges
        .iter()
        .map(|(style, text)| {
            let color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            Span::raw(text.to_string()).fg(color)
        })
        .map(move |span| {
            if let Some(background) = bg {
                span.bg(background)
            } else {
                span
            }
        })
        .collect::<Vec<_>>()
}

fn num_digits(n: u32) -> u32 {
    if n == 0 { 1 } else { n.ilog10() + 1 }
}

/// Counter for tracking how many threads we're currently running.
static NUM_THREADS: AtomicUsize = AtomicUsize::new(0);

struct NumThreadsGuard;

impl NumThreadsGuard {
    fn new() -> Self {
        NUM_THREADS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self
    }
}

impl Drop for NumThreadsGuard {
    fn drop(&mut self) {
        NUM_THREADS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
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
enum ScrollDirection {
    Up,
    Down,
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

fn available_lines_in_area(area: Rect) -> impl Iterator<Item = Rect> {
    (0..area.height).map(move |i| {
        let y = area.y + i;
        Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        }
    })
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
                id: _,
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
            DetailsLine::SectionSeparator => {}
        }
    }

    if let Some(path) = path {
        format!("{path}\n\n{text}")
    } else {
        text
    }
}
