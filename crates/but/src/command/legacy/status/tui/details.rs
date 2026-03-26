use std::iter::{once, repeat_n};

use bstr::ByteSlice;
use but_core::{TreeChange, TreeStatus, UnifiedPatch, unified_diff::DiffHunk};
use but_ctx::{Context, OnDemand};
use gix::actor::Signature;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Widget},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    CliId,
    command::legacy::status::tui::{
        CommandMessage, CommitMessage, DebugType, FilesMessage, Message, MoveMessage,
        RewordMessage, RubMessage,
    },
};

use super::BranchMessage;

// colors from delta
const MINUS_BG: Color = Color::Rgb(0x3f, 0x00, 0x01);
const MINUS_EMPH_BG: Color = Color::Rgb(0x90, 0x10, 0x11);
const PLUS_BG: Color = Color::Rgb(0x00, 0x28, 0x00);
const PLUS_EMPH_BG: Color = Color::Rgb(0x00, 0x60, 0x00);

const MONOKAI_THEME: &[u8] =
    include_bytes!("../../../../../assets/syntax-highlighting-themes/Monokai Extended.tmTheme");

const _TODOS: () = {
    // - show diffs for all kinds of cli ids
    // - show and hide details
    // - show short ids for hunks
    // - escape closes things in the right order
    // - general clean up
    //
    // FUTURE
    // - can we cache DiffWidget?
    // - can we render the diff incrementally or on a separate thread?
    let todo_ = ();
};

#[derive(Debug)]
pub(super) struct Details {
    is_dirty: bool,
    scroll_top: usize,
    widget: Option<DetailsAndDiffWidget>,
    syntax_set: DebugType<OnDemand<SyntaxSet>>,
    dark_theme: DebugType<OnDemand<Theme>>,
}

impl Default for Details {
    fn default() -> Self {
        Self {
            is_dirty: Default::default(),
            widget: Default::default(),
            scroll_top: Default::default(),
            syntax_set: OnDemand::new(|| Ok(SyntaxSet::load_defaults_newlines())).into(),
            dark_theme: OnDemand::new(|| {
                Ok(ThemeSet::load_from_reader(&mut std::io::Cursor::new(MONOKAI_THEME)).unwrap())
            })
            .into(),
        }
    }
}

impl Details {
    pub(super) fn needs_update_after_message(&self, msg: &Message) -> bool {
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
                DetailsMessage::ScrollUp(_) | DetailsMessage::ScrollDown(_) => false,
            },
        }
    }

    pub(super) fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.is_dirty
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
        self.is_dirty = false;
        self.scroll_top = 0;

        self.widget = self.update_widget(ctx, selection)?;

        Ok(())
    }

    fn update_widget(
        &mut self,
        ctx: &mut Context,
        selection: Option<&CliId>,
    ) -> anyhow::Result<Option<DetailsAndDiffWidget>> {
        let Some(selection) = selection else {
            return Ok(None);
        };

        Ok(Some(match selection {
            CliId::Commit { commit_id, .. } => DetailsAndDiffWidget::from_commit(
                ctx,
                *commit_id,
                &*self.syntax_set.get()?,
                &*self.dark_theme.get()?,
            )?,
            CliId::Uncommitted(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Unassigned { .. }
            | CliId::Stack { .. } => {
                return Ok(None);
            }
        }))
    }

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let layout = Layout::horizontal([Constraint::Length(1), Constraint::Min(1)]).split(area);

        let block = Block::new()
            .borders(Borders::LEFT)
            .border_style(Style::default().dim());
        frame.render_widget(block, layout[0]);

        if let Some(diff) = &self.widget {
            diff.render(self.scroll_top, layout[1], frame);
        } else {
            frame.render_widget("Unable to load details", layout[1]);
        }
    }
}

#[derive(Debug)]
struct DetailsAndDiffWidget {
    header_items: Vec<ListItem<'static>>,
    message: String,
    diff_line_items: Vec<ListItem<'static>>,
}

impl DetailsAndDiffWidget {
    fn from_commit(
        ctx: &mut Context,
        commit_id: gix::ObjectId,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> anyhow::Result<Self> {
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

        let mut diff_line_items = Vec::new();
        render_diff_line_items(
            ctx,
            &commit_details.diff_with_first_parent,
            syntax_set,
            theme,
            &mut diff_line_items,
        );

        Ok(DetailsAndDiffWidget {
            header_items,
            message,
            diff_line_items,
        })
    }

    fn total_rows(&self, width: u16) -> usize {
        self.items_for_width(width).count()
    }

    fn render(&self, scroll_top: usize, area: Rect, buf: &mut Frame) {
        let items = self.items_for_width(area.width).skip(scroll_top);

        List::new(items).render(area, buf.buffer_mut());
    }

    fn items_for_width(&self, width: u16) -> impl Iterator<Item = ListItem<'static>> {
        let width = usize::from(width).max(1);

        self.header_items
            .clone()
            .into_iter()
            .chain(once(ListItem::new("")))
            .chain(
                textwrap::wrap(&self.message, textwrap::Options::new(width))
                    .into_iter()
                    .map(|line| ListItem::new(line.into_owned())),
            )
            .chain(once(ListItem::new("")))
            .chain(self.diff_line_items.clone())
    }
}

fn render_diff_line_items(
    ctx: &mut Context,
    tree_change: &[TreeChange],
    syntax_set: &SyntaxSet,
    theme: &Theme,
    out: &mut Vec<ListItem<'static>>,
) {
    for change in tree_change {
        let status = change_status(&change.status);
        let path = change.path.to_string();
        let path_line = Line::from_iter([Span::raw(" "), status, Span::raw(": "), Span::raw(path)]);
        out.extend(bordered_line_top_right_bottom(path_line).map(ListItem::new));
        out.push(ListItem::from(""));

        if let Some(patch) = but_api::diff::tree_change_diffs(ctx, change.clone().into())
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
                    render_unified_patch(
                        change,
                        syntax_set,
                        theme,
                        hunks,
                        is_result_of_binary_to_text_conversion,
                        out,
                    );
                }
                UnifiedPatch::Binary => {
                    out.push(ListItem::new("Binary file - no diff available"));
                }
                UnifiedPatch::TooLarge { size_in_bytes } => {
                    out.push(ListItem::new(format!(
                        "File too large ({size_in_bytes} bytes) - no diff available"
                    )));
                }
            }

            out.push(ListItem::new(""));
        }
    }
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

fn render_unified_patch(
    change: &TreeChange,
    syntax_set: &SyntaxSet,
    theme: &Theme,
    hunks: Vec<DiffHunk>,
    is_result_of_binary_to_text_conversion: bool,
    out: &mut Vec<ListItem<'static>>,
) {
    let syntax = {
        let path = change.path.to_path_lossy();
        path.extension()
            .and_then(|ext| syntax_set.find_syntax_by_extension(ext.to_str()?))
            .or_else(|| {
                path.file_name()
                    .and_then(|file_name| syntax_set.find_syntax_by_extension(file_name.to_str()?))
            })
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    };

    let mut highlight_lines = HighlightLines::new(syntax, theme);

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
            out.push(ListItem::new(
                "(diff generated from binary-to-text conversion)",
            ));
        }

        if let Some(headers) = diff.lines().next() {
            out.extend([
                ListItem::new(Span::raw(headers.to_str_lossy().to_string()).dim()),
                ListItem::new(Line::from_iter(repeat_n("─", headers.to_str_lossy().width())).dim()),
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

        let mut old_line_num = old_start;
        let mut new_line_num = new_start;

        for line in diff.lines().skip(1) {
            let item = if let Some(rest) = line.strip_prefix(b"+") {
                let code = rest.to_str_lossy().to_string();
                let item = ListItem::new(Line::from_iter(
                    [
                        Span::raw(" ".repeat(old_width as _)),
                        Span::raw(" ┊ ").dim(),
                        Span::raw(" ".repeat((new_width - num_digits(new_line_num)) as _)),
                        Span::raw(new_line_num.to_string()).fg(PLUS_EMPH_BG),
                        Span::raw(" │ ").dim(),
                    ]
                    .into_iter()
                    .chain(syntax_highlight(
                        &code,
                        Some(PLUS_BG),
                        &mut highlight_lines,
                        syntax_set,
                    )),
                ));
                new_line_num += 1;
                item
            } else if let Some(rest) = line.strip_prefix(b"-") {
                let code = rest.to_str_lossy().to_string();

                let item = ListItem::new(Line::from_iter(
                    [
                        Span::raw(" ".repeat((old_width - num_digits(old_line_num)) as _)),
                        Span::raw(old_line_num.to_string()).fg(MINUS_EMPH_BG),
                        Span::raw(" ┊ ").dim(),
                        Span::raw(" ".repeat(new_width as _)),
                        Span::raw(" │ ").dim(),
                    ]
                    .into_iter()
                    .chain(syntax_highlight(
                        &code,
                        Some(MINUS_BG),
                        &mut highlight_lines,
                        syntax_set,
                    )),
                ));
                old_line_num += 1;
                item
            } else {
                let line = line.strip_prefix(b" ").unwrap_or(line);

                let code = line.to_str_lossy().to_string();

                let item = ListItem::new(Line::from_iter(
                    [
                        Span::raw(" ".repeat((old_width - num_digits(old_line_num)) as _)),
                        Span::raw(old_line_num.to_string()).dark_gray(),
                        Span::raw(" ┊ ").dim(),
                        Span::raw(" ".repeat((new_width - num_digits(new_line_num)) as _)),
                        Span::raw(new_line_num.to_string()).dark_gray(),
                        Span::raw(" │ ").dim(),
                    ]
                    .into_iter()
                    .chain(syntax_highlight(
                        &code,
                        None,
                        &mut highlight_lines,
                        syntax_set,
                    )),
                ));
                old_line_num += 1;
                new_line_num += 1;
                item
            };
            out.push(item);
        }

        if hunk_iter.peek().is_some() {
            out.push(ListItem::new(""));
        }
    }
}

fn num_digits(n: u32) -> u32 {
    n.ilog10() + 1
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

#[derive(Debug, Clone)]
pub(super) enum DetailsMessage {
    ScrollUp(usize),
    ScrollDown(usize),
}
