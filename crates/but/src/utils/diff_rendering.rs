use std::{
    cell::RefCell,
    collections::HashSet,
    fmt::Display,
    iter::{once, repeat_n},
    sync::Arc,
};

use anyhow::bail;
use bstr::{BStr, BString, ByteSlice as _};
use but_core::{
    UnifiedPatch,
    diff::LineStats,
    ui::{TreeChange, TreeStatus},
    unified_diff::DiffHunk,
};
use but_ctx::Context;
use gix::{ObjectId, actor::Signature};
use itertools::{Itertools, Position};
use nonempty::NonEmpty;
use ratatui::{
    style::{Color, Stylize as _},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines,
    parsing::{SyntaxReference, SyntaxSet},
};
use unicode_width::UnicodeWidthStr as _;

use crate::{
    CliId, IdMap,
    id::{IdAndHunk, UncommittedHunk, UncommittedHunkOrFile},
    theme::Theme,
    utils::string_interning::{SharedStrings, Strings},
};

/// Each line in the diff is considered to be part of a "section". A section is the group of lines
/// that can be selected together such as a hunk.
///
/// `SectionId` is used to track which lines belong to the same section. `Details` tracks the
/// currently selected `SectionId` and when it renders a line with a matching it it'll highlight
/// it.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct SectionId(pub &'static str);

#[derive(Debug)]
pub struct IdGen<'a> {
    pub strings: Strings,
    scope: &'static str,
    _marker: std::marker::PhantomData<&'a mut ()>,
}

impl IdGen<'_> {
    pub fn new(strings: Strings) -> Self {
        IdGen {
            strings,
            scope: "details",
            _marker: std::marker::PhantomData,
        }
    }

    pub fn new_id(&mut self, id: impl Display) -> SectionId {
        SectionId(self.strings.get(format!("{}/{}", self.scope, id)))
    }

    pub fn scoped(&mut self, scope: impl Display) -> IdGen<'_> {
        let scope = self.strings.get(format!("{}/{}", self.scope, scope));
        IdGen {
            strings: self.strings.clone(),
            scope,
            _marker: std::marker::PhantomData,
        }
    }
}

pub trait DiffLineWriter {
    fn write(&mut self, line: DetailsLine) -> anyhow::Result<()>;

    fn write_selectable_text(
        &mut self,
        id: SectionId,
        cli_id: Option<Arc<CliId>>,
        line: Line<'static>,
    ) -> anyhow::Result<()> {
        self.write(DetailsLine::Text {
            id: Some(id),
            cli_id,
            line,
            skip_when_copying_hunk: false,
        })
    }

    fn write_hunk_header(
        &mut self,
        id: SectionId,
        cli_id: Option<Arc<CliId>>,
        line: Line<'static>,
    ) -> anyhow::Result<()> {
        self.write(DetailsLine::Text {
            id: Some(id),
            cli_id,
            line,
            skip_when_copying_hunk: true,
        })
    }

    #[expect(dead_code)]
    fn write_non_selectable_text(&mut self, line: Line<'static>) -> anyhow::Result<()> {
        self.write(DetailsLine::Text {
            id: None,
            cli_id: None,
            line,
            skip_when_copying_hunk: false,
        })
    }

    fn write_empty_line(
        &mut self,
        id: SectionId,
        cli_id: Option<Arc<CliId>>,
    ) -> anyhow::Result<()> {
        self.write_selectable_text(id, cli_id, " ".into())
    }

    fn write_section_separator(&mut self) -> anyhow::Result<()> {
        self.write(DetailsLine::SectionSeparator)
    }

    fn write_text_to_wrap(&mut self, id: SectionId, text: String) -> anyhow::Result<()> {
        self.write(DetailsLine::TextToWrap { id, text })
    }

    fn write_code(
        &mut self,
        id: SectionId,
        cli_id: Option<Arc<CliId>>,
        line_numbers: CodeLineNumbers,
        line_start_end: (usize, usize),
        diff: Arc<BString>,
        path: Arc<BString>,
    ) -> anyhow::Result<()> {
        self.write(DetailsLine::Code(DetailsCodeLine {
            id,
            cli_id,
            syntax_highlighted_line: RefCell::new(None),
            line_numbers,
            line_start_end,
            diff,
            path,
        }))
    }
}

pub struct WithSyntaxHighlighting<'a, T> {
    inner: T,
    strings: Strings,
    syntax_set: &'a SyntaxSet,
    syntax_theme: &'a syntect::highlighting::Theme,
    highlight_lines: Option<HighlightLines<'a>>,
    theme: &'static Theme,
}

impl<'a, T> WithSyntaxHighlighting<'a, T> {
    pub fn new(
        inner: T,
        strings: Strings,
        syntax_set: &'a SyntaxSet,
        syntax_theme: &'a syntect::highlighting::Theme,
    ) -> Self {
        Self {
            inner,
            strings,
            syntax_set,
            syntax_theme,
            highlight_lines: None,
            theme: crate::theme::get(),
        }
    }
}

impl<T> DiffLineWriter for WithSyntaxHighlighting<'_, T>
where
    T: DiffLineWriter,
{
    fn write(&mut self, line: DetailsLine) -> anyhow::Result<()> {
        match line {
            DetailsLine::Code(code_line) => {
                if self.highlight_lines.is_none() {
                    let syntax = code_line.syntax(self.syntax_set);
                    self.highlight_lines = Some(HighlightLines::new(syntax, self.syntax_theme));
                }

                let highlight_lines = self.highlight_lines.as_mut().unwrap();

                let mut strings = self.strings.lock();

                code_line.ensure_highlighted(
                    self.syntax_set,
                    highlight_lines,
                    self.theme,
                    &mut strings,
                );

                self.inner.write(DetailsLine::Code(code_line))?;
            }
            other => {
                self.highlight_lines = None;
                self.inner.write(other)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum DetailsLine {
    Text {
        /// None if this line cannot be selected
        id: Option<SectionId>,
        cli_id: Option<Arc<CliId>>,
        line: Line<'static>,
        /// Exclude this UI-only text from the clipboard when copying the current hunk
        skip_when_copying_hunk: bool,
    },
    TextToWrap {
        id: SectionId,
        text: String,
    },
    Code(DetailsCodeLine),
    SectionSeparator,
    HunkHeader {
        id: SectionId,
        cli_id: Option<Arc<CliId>>,
        width: usize,
        line: Vec<Span<'static>>,
    },
}

#[derive(Debug, Clone)]
pub struct DetailsCodeLine {
    pub id: SectionId,
    pub cli_id: Option<Arc<CliId>>,
    pub line_numbers: CodeLineNumbers,
    // indexes into `diff` where the line starts and ends, including any line terminators
    pub line_start_end: (usize, usize),
    // the whole diff this line is part of
    //
    // we share the diff and store indexes to get the line to avoid allocating each line
    pub diff: Arc<BString>,
    pub path: Arc<BString>,
    // HACK: only when drawing this line to the screen do we syntax highlight it and cache the
    // result directly here. We dont have a mutable reference in `Details::render` so have to
    // cheat with a `RefCell`.
    pub syntax_highlighted_line: RefCell<Option<Line<'static>>>,
}

impl DetailsCodeLine {
    pub fn ensure_highlighted(
        &self,
        syntax_set: &SyntaxSet,
        highlight_lines: &mut HighlightLines<'_>,
        theme: &'static Theme,
        strings: &mut SharedStrings,
    ) {
        if self.syntax_highlighted_line.borrow().is_some() {
            return;
        }

        self.with_line_from_diff(|line| {
            let bg = self.line_numbers.kind.bg(theme);
            let line_numbers = self.line_numbers.spans(strings, theme);
            let mut line = Line::from_iter(line_numbers.into_iter().chain(syntax_highlight(
                line,
                highlight_lines,
                syntax_set,
            )));
            if let Some(bg) = bg {
                line = line.bg(bg);
            }
            *self.syntax_highlighted_line.borrow_mut() = Some(line);
        });
    }

    pub fn syntax<'a>(&self, syntax_set: &'a SyntaxSet) -> &'a SyntaxReference {
        let path = self.path.to_path_lossy();
        path.extension()
            .and_then(|ext| syntax_set.find_syntax_by_extension(ext.to_str()?))
            .or_else(|| {
                path.file_name()
                    .and_then(|file_name| syntax_set.find_syntax_by_extension(file_name.to_str()?))
            })
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    }

    pub fn with_line_from_diff<F, T>(&self, f: F) -> T
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
    highlight_lines: &mut HighlightLines<'_>,
    syntax_set: &SyntaxSet,
) -> Vec<Span<'static>> {
    let mut visual_column = 0;
    match highlight_lines.highlight_line(code, syntax_set) {
        Ok(ranges) => ranges
            .iter()
            .map(|(style, text)| {
                let color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                let span = Span::raw(text.to_string()).fg(color);
                expand_tabs_for_display(span, &mut visual_column)
            })
            .collect(),
        Err(_) => Vec::from([expand_tabs_for_display(
            Span::raw(code.to_owned()),
            &mut visual_column,
        )]),
    }
}

fn expand_tabs_for_display(mut span: Span<'static>, visual_column: &mut usize) -> Span<'static> {
    const TAB_WIDTH: usize = 4;

    if !span.content.contains('\t') {
        *visual_column += span.content.width();
        return span;
    }

    let expanded = {
        let content = span.content.as_ref();
        let mut expanded = String::with_capacity(content.len());
        let mut segment_start = 0;

        for (index, character) in content.char_indices() {
            if character != '\t' {
                continue;
            }

            let segment = &content[segment_start..index];
            expanded.push_str(segment);
            *visual_column += segment.width();

            let spaces = TAB_WIDTH - *visual_column % TAB_WIDTH;
            expanded.extend(repeat_n(' ', spaces));
            *visual_column += spaces;
            segment_start = index + character.len_utf8();
        }

        let remaining = &content[segment_start..];
        expanded.push_str(remaining);
        *visual_column += remaining.width();
        expanded
    };

    span.content = expanded.into();
    span
}

pub fn num_digits(n: u32) -> u32 {
    if n == 0 { 1 } else { n.ilog10() + 1 }
}

#[derive(Debug, Copy, Clone)]
pub struct CodeLineNumbers {
    pub old_width: u32,
    pub new_width: u32,
    pub kind: CodeLineKind,
}

#[derive(Debug, Copy, Clone)]
pub enum CodeLineKind {
    Addition { new_line: u32 },
    Deletion { old_line: u32 },
    Context { old_line: u32, new_line: u32 },
}

impl CodeLineKind {
    pub fn bg(self, theme: &'static Theme) -> Option<Color> {
        match self {
            CodeLineKind::Addition { .. } => theme.addition_rich.bg,
            CodeLineKind::Deletion { .. } => theme.deletion_rich.bg,
            CodeLineKind::Context { .. } => None,
        }
    }
}

impl CodeLineNumbers {
    pub fn addition(old_width: u32, new_width: u32, new_line: u32) -> Self {
        Self {
            old_width,
            new_width,
            kind: CodeLineKind::Addition { new_line },
        }
    }

    pub fn deletion(old_width: u32, new_width: u32, old_line: u32) -> Self {
        Self {
            old_width,
            new_width,
            kind: CodeLineKind::Deletion { old_line },
        }
    }

    pub fn context(old_width: u32, new_width: u32, old_line: u32, new_line: u32) -> Self {
        Self {
            old_width,
            new_width,
            kind: CodeLineKind::Context { old_line, new_line },
        }
    }

    pub fn spans(self, strings: &mut SharedStrings, theme: &'static Theme) -> [Span<'static>; 6] {
        match self.kind {
            CodeLineKind::Addition { new_line } => [
                Span::raw(strings.get_spaces(self.old_width as _)),
                Span::styled(" ┊ ", theme.border),
                Span::raw(strings.get_spaces((self.new_width - num_digits(new_line)) as _)),
                Span::raw(strings.get_u32(new_line)).style(theme.addition),
                Span::styled(" │ ", theme.border),
                Span::raw("+"),
            ],
            CodeLineKind::Deletion { old_line } => [
                Span::raw(strings.get_spaces((self.old_width - num_digits(old_line)) as _)),
                Span::raw(strings.get_u32(old_line)).style(theme.deletion),
                Span::styled(" ┊ ", theme.border),
                Span::raw(strings.get_spaces(self.new_width as _)),
                Span::styled(" │ ", theme.border),
                Span::raw("-"),
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

#[derive(Debug, Copy, Clone, Default)]
pub struct Options {
    pub skip_commit_header: bool,
    pub skip_line_stats: bool,
}

pub fn render_commit(
    commit: ObjectId,
    change_id: Option<but_core::ChangeId>,
    ctx: &Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    options: Options,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("commits");
    let mut id_gen = id_gen.scoped(commit);

    let commit_details =
        but_api::diff::commit_details(ctx, commit, but_api::diff::ComputeLineStats::No)?;

    let header_id = id_gen.new_id("header");

    if !options.skip_commit_header {
        out.write_selectable_text(
            header_id,
            None,
            Line::from_iter([
                Span::raw(format!("{:<11}", "Commit ID:")),
                Span::styled(commit.to_hex().to_string(), theme.commit_id),
            ]),
        )?;
        if let Some(change_id) = change_id {
            out.write_selectable_text(
                header_id,
                None,
                Line::from_iter([
                    Span::raw(format!("{:<11}", "Change ID:")),
                    Span::styled(change_id.to_string(), theme.change_id),
                ]),
            )?;
        }
        out.write_selectable_text(
            header_id,
            None,
            Line::from_iter(
                once(Span::raw(format!("{:<11}", "Author:")))
                    .chain(render_signature(&commit_details.commit.author, theme)),
            ),
        )?;
        out.write_selectable_text(
            header_id,
            None,
            Line::from_iter(
                once(Span::raw(format!("{:<11}", "Committer:")))
                    .chain(render_signature(&commit_details.commit.committer, theme)),
            ),
        )?;

        out.write_empty_line(header_id, None)?;

        let message = commit_details.commit.message.to_string();
        if message.is_empty() {
            out.write_selectable_text(
                header_id,
                None,
                Line::from("(no commit message)").style(theme.hint),
            )?;
        } else {
            out.write_text_to_wrap(header_id, message)?;
        }
        out.write_empty_line(header_id, None)?;
    }

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();

    let tree_changes = tree_changes_with_patches(ctx, tree_changes);

    if !options.skip_line_stats {
        let mut line_stats = LineStats::default();
        compute_line_stats_from_tree_changes(&tree_changes, &mut line_stats);
        out.write_selectable_text(header_id, None, render_line_stats(line_stats))?;
        out.write_section_separator()?;
    }

    render_tree_changes(tree_changes, theme, &mut id_gen, out)?;

    Ok(())
}

pub fn render_branch(
    name: String,
    ctx: &Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    options: Options,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("branches");
    let mut id_gen = id_gen.scoped(&name);

    let tree_changes = but_api::branch::branch_diff(ctx, name.clone())?;
    let tree_changes = tree_changes_with_patches(ctx, tree_changes.changes);

    if !options.skip_line_stats {
        let mut line_stats = LineStats::default();
        compute_line_stats_from_tree_changes(&tree_changes, &mut line_stats);
        out.write_selectable_text(
            id_gen.new_id("line_stats"),
            None,
            render_line_stats(line_stats),
        )?;
        out.write_section_separator()?;
    }

    render_tree_changes(tree_changes, theme, &mut id_gen, out)?;

    Ok(())
}

pub fn render_uncommitted(
    ctx: &Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    options: Options,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("uncommitted");

    let id_map = IdMap::legacy_new_from_context(ctx)?;
    let uncommitted_hunks = filter_uncommitted_hunks(ctx, &id_map, |_| true)?;

    if !options.skip_line_stats {
        let line_stats = render_line_stats(compute_line_stats_from_uncommitted_hunks(
            uncommitted_hunks.iter().map(|(_, _, hunk)| &hunk.hunk),
        ));
        out.write_selectable_text(id_gen.new_id("line_stats"), None, line_stats)?;
        out.write_section_separator()?;
    }

    for (pos, (raw_id, cli_id, UncommittedHunk { hunk })) in
        uncommitted_hunks.into_iter().with_position()
    {
        let id = id_gen.new_id(raw_id);

        render_hunk_path_header(
            id,
            Some(Arc::clone(&cli_id)),
            hunk.path.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(raw_id)),
            out,
            theme,
        )?;

        render_hunk(id, Some(Arc::clone(&cli_id)), hunk, theme, out)?;

        if pos.needs_padding_below() {
            out.write_section_separator()?;
        }
    }

    Ok(())
}

pub fn render_uncommitted_hunk(
    hunk: UncommittedHunkOrFile,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    options: Options,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("hunk");
    let mut id_gen = id_gen.scoped(&hunk.id);
    render_id_and_hunks(
        hunk.hunks.into_iter().collect(),
        theme,
        &mut id_gen,
        options,
        out,
    )
}

pub fn render_path_prefix(
    id: &str,
    hunks: NonEmpty<IdAndHunk>,
    _ctx: &mut Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    options: Options,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("path_prefix");
    let mut id_gen = id_gen.scoped(id);
    render_id_and_hunks(
        hunks.into_iter().collect(),
        theme,
        &mut id_gen,
        options,
        out,
    )
}

fn render_id_and_hunks(
    mut hunks: Vec<IdAndHunk>,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    options: Options,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    hunks.sort_by(|a, b| {
        (
            &a.hunk.path,
            a.hunk.hunk_header.as_ref().map(|header| header.old_start),
            &a.id,
        )
            .cmp(&(
                &b.hunk.path,
                b.hunk.hunk_header.as_ref().map(|header| header.old_start),
                &b.id,
            ))
    });

    if !options.skip_line_stats {
        let line_stats = render_line_stats(compute_line_stats_from_uncommitted_hunks(
            hunks.iter().map(|hunk| &hunk.hunk),
        ));
        out.write_selectable_text(id_gen.new_id("line_stats"), None, line_stats)?;
        out.write_section_separator()?;
    }

    for (pos, id_and_hunk) in hunks.into_iter().with_position() {
        let raw_id = &id_and_hunk.id;
        let id = id_gen.new_id(raw_id);
        let cli_id = Arc::new(CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
            id: raw_id.clone(),
            hunks: NonEmpty::new(id_and_hunk.clone()),
            is_entire_file: false,
        }));

        render_hunk_path_header(
            id,
            Some(Arc::clone(&cli_id)),
            id_and_hunk.hunk.path.as_ref(),
            Some(ShortIdOrTreeStatus::ShortId(raw_id)),
            out,
            theme,
        )?;

        render_hunk(id, Some(Arc::clone(&cli_id)), &id_and_hunk.hunk, theme, out)?;

        if pos.needs_padding_below() {
            out.write_section_separator()?;
        }
    }

    Ok(())
}

pub fn render_committed_file(
    commit: ObjectId,
    path: BString,
    ctx: &Context,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    options: Options,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("committed_file");
    let mut id_gen = id_gen.scoped(commit);

    let commit_details =
        but_api::diff::commit_details(ctx, commit, but_api::diff::ComputeLineStats::No)?;

    let tree_changes = commit_details
        .diff_with_first_parent
        .iter()
        .filter(|change| change.path == path)
        .map(|change| TreeChange::from(change.clone()))
        .collect::<Vec<_>>();
    let tree_changes = tree_changes_with_patches(ctx, tree_changes);

    if !options.skip_line_stats {
        let mut line_stats = LineStats::default();
        compute_line_stats_from_tree_changes(&tree_changes, &mut line_stats);
        out.write_selectable_text(
            id_gen.new_id("line_stats"),
            None,
            render_line_stats(line_stats),
        )?;
        out.write_section_separator()?;
    }

    render_tree_changes(tree_changes, theme, &mut id_gen, out)?;

    Ok(())
}

fn tree_changes_with_patches(
    ctx: &Context,
    tree_changes: Vec<TreeChange>,
) -> Vec<(TreeChange, UnifiedPatch)> {
    tree_changes
        .into_iter()
        .filter_map(|tree_change| {
            let patch = but_api::diff::tree_change_diffs(ctx, tree_change.clone())
                .ok()
                .flatten()?;
            Some((tree_change, patch))
        })
        .collect::<Vec<_>>()
}

fn render_hunk(
    id: SectionId,
    cli_id: Option<Arc<CliId>>,
    hunk: &but_core::SingleHunk,
    theme: &'static Theme,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    if let Some(hunk_header) = hunk.hunk_header {
        if let Some(diff) = hunk.diff.clone() {
            let diff_hunk = DiffHunk {
                old_start: hunk_header.old_start,
                old_lines: hunk_header.old_lines,
                new_start: hunk_header.new_start,
                new_lines: hunk_header.new_lines,
                diff,
            };

            let is_result_of_binary_to_text_conversion = false;

            let path = Arc::new(hunk.path.clone());

            render_unified_patch(
                id,
                cli_id.as_ref().map(Arc::clone),
                &path,
                diff_hunk,
                is_result_of_binary_to_text_conversion,
                theme,
                out,
            )?;
        } else {
            out.write_selectable_text(
                id,
                cli_id.as_ref().map(Arc::clone),
                "No diff available".into(),
            )?;
        }
    } else {
        out.write_selectable_text(
            id,
            cli_id.as_ref().map(Arc::clone),
            "No diff available - file is either empty, binary, or too large".into(),
        )?;
    }

    Ok(())
}

fn compute_line_stats_from_tree_changes(
    tree_changes: &[(TreeChange, UnifiedPatch)],
    line_stats: &mut LineStats,
) {
    for (_tree_change, patch) in tree_changes {
        line_stats.files_changed += 1;
        match patch {
            UnifiedPatch::Patch {
                hunks: _,
                is_result_of_binary_to_text_conversion: _,
                lines_added,
                lines_removed,
            } => {
                line_stats.lines_added += (*lines_added) as u64;
                line_stats.lines_removed += (*lines_removed) as u64;
            }
            UnifiedPatch::Binary | UnifiedPatch::TooLarge { .. } => {}
        }
    }
}

fn render_tree_changes(
    tree_changes: Vec<(TreeChange, UnifiedPatch)>,
    theme: &'static Theme,
    id_gen: &mut IdGen<'_>,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let mut id_gen = id_gen.scoped("tree_changes");

    for (tree_change_pos, (i, (tree_change, patch))) in
        tree_changes.into_iter().enumerate().with_position()
    {
        let mut id_gen = id_gen.scoped(i);
        let path = Arc::new(tree_change.path_bytes.clone());
        match patch {
            UnifiedPatch::Patch {
                hunks,
                is_result_of_binary_to_text_conversion,
                lines_added: _,
                lines_removed: _,
            } => {
                let mut first_hunk = true;
                let mut id_gen = id_gen.scoped("hunks");
                for (hunk_pos, (j, hunk)) in hunks.into_iter().enumerate().with_position() {
                    let hunk_id = id_gen.new_id(j);

                    if std::mem::take(&mut first_hunk) {
                        render_hunk_path_header(
                            hunk_id,
                            None,
                            tree_change.path.as_ref(),
                            Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                            out,
                            theme,
                        )?;
                    }

                    render_unified_patch(
                        hunk_id,
                        None,
                        &path,
                        hunk,
                        is_result_of_binary_to_text_conversion,
                        theme,
                        out,
                    )?;

                    if tree_change_pos.needs_padding_below() || hunk_pos.needs_padding_below() {
                        out.write_section_separator()?;
                    }
                }
            }
            UnifiedPatch::Binary => {
                let patch_id = id_gen.new_id("binary");

                render_hunk_path_header(
                    patch_id,
                    None,
                    tree_change.path.as_ref(),
                    Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                    out,
                    theme,
                )?;

                out.write_selectable_text(
                    patch_id,
                    None,
                    "Binary file - no diff available".into(),
                )?;

                if tree_change_pos.needs_padding_below() {
                    out.write_section_separator()?;
                }
            }
            UnifiedPatch::TooLarge { size_in_bytes } => {
                let patch_id = id_gen.new_id("too_large");

                render_hunk_path_header(
                    patch_id,
                    None,
                    tree_change.path.as_ref(),
                    Some(ShortIdOrTreeStatus::TreeStatus(&tree_change.status)),
                    out,
                    theme,
                )?;

                out.write_selectable_text(
                    patch_id,
                    None,
                    format!("File too large ({size_in_bytes} bytes) - no diff available").into(),
                )?;

                if tree_change_pos.needs_padding_below() {
                    out.write_section_separator()?;
                }
            }
        }
    }

    Ok(())
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

enum ShortIdOrTreeStatus<'a> {
    ShortId(&'a str),
    TreeStatus(&'a TreeStatus),
}

fn render_hunk_path_header(
    id: SectionId,
    cli_id: Option<Arc<CliId>>,
    path: &BStr,
    status: Option<ShortIdOrTreeStatus<'_>>,
    out: &mut dyn DiffLineWriter,
    theme: &'static Theme,
) -> anyhow::Result<()> {
    let status = status.map(|id_or_status| match id_or_status {
        ShortIdOrTreeStatus::ShortId(id) => Span::raw(id.to_owned()).blue(),
        ShortIdOrTreeStatus::TreeStatus(status) => change_status(status, theme),
    });
    let path = path.to_string();
    let path_line = Vec::from_iter(
        [Span::raw(" ")]
            .into_iter()
            .chain(
                status
                    .into_iter()
                    .flat_map(|status| [status, Span::raw(" ")]),
            )
            .chain([
                Span::raw(path),
                Span::raw(" "),
                Span::styled("│", theme.border),
            ]),
    );

    let width_including_padding = 1 + path_line.iter().map(|span| span.width()).sum::<usize>() - 2;

    out.write(DetailsLine::HunkHeader {
        id,
        cli_id,
        width: width_including_padding,
        line: path_line,
    })?;

    Ok(())
}

fn change_status(status: &TreeStatus, theme: &'static Theme) -> Span<'static> {
    match status {
        TreeStatus::Addition { .. } => Span::styled("added", theme.addition),
        TreeStatus::Deletion { .. } => Span::styled("deleted", theme.deletion),
        TreeStatus::Modification { .. } => Span::styled("modified", theme.modification),
        TreeStatus::Rename { .. } => Span::styled("renamed", theme.renaming),
    }
}

fn render_unified_patch(
    id: SectionId,
    cli_id: Option<Arc<CliId>>,
    path: &Arc<BString>,
    hunk: DiffHunk,
    is_result_of_binary_to_text_conversion: bool,
    theme: &'static Theme,
    out: &mut dyn DiffLineWriter,
) -> anyhow::Result<()> {
    let DiffHunk {
        old_start,
        new_start,
        diff,
        old_lines: _,
        new_lines: _,
    } = hunk;

    if is_result_of_binary_to_text_conversion {
        out.write_selectable_text(
            id,
            cli_id.as_ref().map(Arc::clone),
            "(diff generated from binary-to-text conversion)".into(),
        )?;
    }

    if let Some(headers) = diff.lines().next() {
        out.write_selectable_text(
            id,
            cli_id.as_ref().map(Arc::clone),
            Span::styled(headers.to_str_lossy().to_string(), theme.hint).into(),
        )?;

        out.write_hunk_header(
            id,
            cli_id.as_ref().map(Arc::clone),
            Line::from_iter(repeat_n("─", headers.to_str_lossy().width())).style(theme.border),
        )?;
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

    let mut old_line_num = old_start;
    let mut new_line_num = new_start;

    let diff = Arc::new(diff);

    let mut first_line = true;
    let mut i = 0;
    for line in diff.lines_with_terminator() {
        if std::mem::take(&mut first_line) {
            i += line.len();
            continue;
        }

        let (line_numbers, line_start_end) = if let Some(rest) = line.strip_prefix(b"+") {
            let start = i + 1;
            let end = start + rest.len();
            let line_numbers = CodeLineNumbers::addition(old_width, new_width, new_line_num);
            new_line_num += 1;
            (line_numbers, (start, end))
        } else if let Some(rest) = line.strip_prefix(b"-") {
            let start = i + 1;
            let end = start + rest.len();
            let line_numbers = CodeLineNumbers::deletion(old_width, new_width, old_line_num);
            old_line_num += 1;
            (line_numbers, (start, end))
        } else {
            let (start, end) = if let Some(rest) = line.strip_prefix(b" ") {
                let start = i + 1;
                (start, start + rest.len())
            } else {
                (i, i + line.len())
            };
            let line_numbers =
                CodeLineNumbers::context(old_width, new_width, old_line_num, new_line_num);
            old_line_num += 1;
            new_line_num += 1;
            (line_numbers, (start, end))
        };

        out.write_code(
            id,
            cli_id.as_ref().map(Arc::clone),
            line_numbers,
            line_start_end,
            Arc::clone(&diff),
            Arc::clone(path),
        )?;

        i += line.len();
    }

    Ok(())
}

fn compute_line_stats_from_uncommitted_hunks<'a>(
    uncommitted_hunks: impl IntoIterator<Item = &'a but_core::SingleHunk>,
) -> LineStats {
    let mut line_stats = LineStats::default();
    let mut unique_paths = HashSet::new();
    for hunk in uncommitted_hunks {
        unique_paths.insert(&hunk.path);
        if let Some((added, removed)) = hunk.line_nums() {
            line_stats.lines_added += added.len() as u64;
            line_stats.lines_removed += removed.len() as u64;
        }
    }
    line_stats.files_changed = unique_paths.len() as u64;
    line_stats
}

fn filter_uncommitted_hunks<'a, F>(
    ctx: &'a Context,
    id_map: &'a IdMap,
    mut filter: F,
) -> anyhow::Result<Vec<(&'a str, Arc<CliId>, &'a UncommittedHunk)>>
where
    F: FnMut(&but_core::SingleHunk) -> bool,
{
    let mut uncommitted_hunks = id_map
        .uncommitted_hunks
        .iter()
        .filter(move |(_, hunk)| filter(&hunk.hunk))
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
            &hunk_a.hunk.path,
            hunk_a
                .hunk
                .hunk_header
                .as_ref()
                .map(|header| header.old_start),
            id_a,
        )
            .cmp(&(
                &hunk_b.hunk.path,
                hunk_b
                    .hunk
                    .hunk_header
                    .as_ref()
                    .map(|header| header.old_start),
                id_b,
            ))
    });

    Ok(uncommitted_hunks)
}

trait PositionExt {
    fn needs_padding_below(self) -> bool;
}

impl PositionExt for Position {
    fn needs_padding_below(self) -> bool {
        match self {
            Position::First | Position::Middle => true,
            Position::Last | Position::Only => false,
        }
    }
}

fn render_line_stats(line_stats: LineStats) -> Line<'static> {
    let LineStats {
        lines_added,
        lines_removed,
        files_changed,
    } = line_stats;

    Line::from_iter([
        if files_changed == 1 {
            Span::raw(format!(
                "{} file changed",
                format_with_dot_thousands(files_changed)
            ))
        } else {
            Span::raw(format!(
                "{} files changed",
                format_with_dot_thousands(files_changed)
            ))
        },
        Span::raw(", "),
        Span::raw(format!("+{}", format_with_dot_thousands(lines_added))).green(),
        Span::raw(" "),
        Span::raw(format!("-{}", format_with_dot_thousands(lines_removed))).red(),
    ])
}

fn format_with_dot_thousands(value: u64) -> String {
    let value = value.to_string();
    let mut formatted = String::with_capacity(value.len() + value.len() / 3);

    for (index, char) in value.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push('.');
        }
        formatted.push(char);
    }

    formatted.chars().rev().collect()
}

pub fn load_syntax_set() -> SyntaxSet {
    SyntaxSet::load_defaults_nonewlines()
}

#[cfg(test)]
mod tests {
    use bstr::BString;
    use but_core::HunkHeader;
    use nonempty::NonEmpty;
    use ratatui::text::Span;

    use super::{
        DetailsLine, DiffLineWriter, IdGen, Options, compute_line_stats_from_uncommitted_hunks,
        expand_tabs_for_display, format_with_dot_thousands, render_uncommitted_hunk,
    };
    use crate::{
        CliId,
        id::{IdAndHunk, UncommittedHunkOrFile},
        utils::string_interning::Strings,
    };

    #[derive(Default)]
    struct CollectingWriter {
        lines: Vec<DetailsLine>,
    }

    impl DiffLineWriter for CollectingWriter {
        fn write(&mut self, line: DetailsLine) -> anyhow::Result<()> {
            self.lines.push(line);
            Ok(())
        }
    }

    fn id_and_hunk(id: &str, path: &str) -> IdAndHunk {
        IdAndHunk {
            id: id.to_owned(),
            hunk: but_core::SingleHunk {
                hunk_header: None,
                path: BString::from(path),
                diff: None,
            },
        }
    }

    #[test]
    fn renders_the_hunks_stored_in_the_selection() -> anyhow::Result<()> {
        let selection = UncommittedHunkOrFile {
            id: "fi".to_owned(),
            hunks: NonEmpty {
                head: id_and_hunk("fi:b", "file.txt"),
                tail: vec![id_and_hunk("fi:a", "file.txt")],
            },
            is_entire_file: true,
        };
        let mut id_gen = IdGen::new(Strings::default());
        let mut writer = CollectingWriter::default();

        render_uncommitted_hunk(
            selection,
            crate::theme::get(),
            &mut id_gen,
            Options {
                skip_commit_header: true,
                skip_line_stats: true,
            },
            &mut writer,
        )?;

        let rendered_ids = writer
            .lines
            .iter()
            .filter_map(|line| match line {
                DetailsLine::HunkHeader {
                    cli_id: Some(cli_id),
                    ..
                } => match &**cli_id {
                    CliId::UncommittedHunkOrFile(hunk) => Some(hunk.id.as_str()),
                    CliId::PathPrefix { .. }
                    | CliId::CommittedFile { .. }
                    | CliId::Branch(..)
                    | CliId::Commit { .. }
                    | CliId::Uncommitted { .. }
                    | CliId::Stack { .. } => None,
                },
                DetailsLine::Text { .. }
                | DetailsLine::TextToWrap { .. }
                | DetailsLine::Code(..)
                | DetailsLine::SectionSeparator
                | DetailsLine::HunkHeader { cli_id: None, .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_ids,
            ["fi:a", "fi:b"],
            "the renderer uses every supplied hunk and preserves deterministic ordering"
        );
        Ok(())
    }

    #[test]
    fn computes_stats_from_the_supplied_hunks() {
        let mut first = id_and_hunk("fi:a", "first.txt").hunk;
        first.hunk_header = Some(HunkHeader {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 2,
        });
        first.diff = Some(BString::from("@@ -1,1 +1,2 @@\n-x\n+a\n+b\n"));
        let mut second = id_and_hunk("se:a", "second.txt").hunk;
        second.hunk_header = Some(HunkHeader {
            old_start: 1,
            old_lines: 0,
            new_start: 1,
            new_lines: 1,
        });
        second.diff = Some(BString::from("@@ -1,0 +1,1 @@\n+a\n"));

        let stats = compute_line_stats_from_uncommitted_hunks([&first, &second]);

        assert_eq!(stats.files_changed, 2, "both supplied paths are counted");
        assert_eq!(stats.lines_added, 3, "all supplied additions are counted");
        assert_eq!(stats.lines_removed, 1, "all supplied removals are counted");
    }

    #[test]
    fn expands_tabs_to_four_column_stops_across_spans() {
        let mut visual_column = 0;
        let contents = [
            Span::raw("\troot "),
            Span::raw("a\tb"),
            Span::raw("\t\tdeep"),
        ]
        .into_iter()
        .map(|span| expand_tabs_for_display(span, &mut visual_column))
        .map(|span| span.content.into_owned())
        .collect::<Vec<_>>();

        assert_eq!(
            contents,
            ["    root ", "a  b", "       deep"],
            "tabs advance to the next four-column stop across syntax spans"
        );
    }

    #[test]
    fn formats_numbers_with_dot_thousands_separators() {
        assert_eq!(format_with_dot_thousands(0), "0");
        assert_eq!(format_with_dot_thousands(39), "39");
        assert_eq!(format_with_dot_thousands(999), "999");
        assert_eq!(format_with_dot_thousands(1_000), "1.000");
        assert_eq!(format_with_dot_thousands(4_489), "4.489");
        assert_eq!(format_with_dot_thousands(9_391), "9.391");
        assert_eq!(format_with_dot_thousands(1_234_567), "1.234.567");
    }
}
