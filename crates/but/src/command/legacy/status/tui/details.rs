use std::iter::{once, repeat_n};

use bstr::ByteSlice;
use but_core::{UnifiedPatch, unified_diff::DiffHunk};
use but_ctx::Context;
use gix::actor::Signature;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    CliId,
    command::legacy::status::tui::{
        CommandMessage, CommitMessage, FilesMessage, Message, MoveMessage, RewordMessage,
        RubMessage,
    },
};

use super::BranchMessage;

// colors from delta
#[expect(dead_code)]
const MINUS_BG: Color = Color::Rgb(0x3f, 0x00, 0x01);
const MINUS_EMPH_BG: Color = Color::Rgb(0x90, 0x10, 0x11);
#[expect(dead_code)]
const PLUS_BG: Color = Color::Rgb(0x00, 0x28, 0x00);
const PLUS_EMPH_BG: Color = Color::Rgb(0x00, 0x60, 0x00);

#[derive(Default, Debug)]
pub(super) struct Details {
    is_dirty: bool,
    updates: u32,
    diff_widget: Option<DiffWidget>,
}

impl Details {
    pub(super) fn needs_update_after_message(&mut self, msg: &Message) -> bool {
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
        }
    }

    pub(super) fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub(super) fn update(
        &mut self,
        ctx: &mut Context,
        selection: Option<&CliId>,
    ) -> anyhow::Result<()> {
        self.is_dirty = false;
        self.updates += 1;

        let Some(selection) = selection else {
            self.diff_widget = None;
            return Ok(());
        };

        let commit_id = match selection {
            CliId::Commit { commit_id, .. } => *commit_id,
            CliId::Uncommitted(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Unassigned { .. }
            | CliId::Stack { .. } => {
                self.diff_widget = None;
                return Ok(());
            }
        };

        let commit_details =
            but_api::diff::commit_details(ctx, commit_id, but_api::diff::ComputeLineStats::No)?;

        let mut diff_line_items = Vec::new();
        for change in &commit_details.diff_with_first_parent {
            let status = match &change.status {
                but_core::TreeStatus::Addition { .. } => Span::raw("added").green(),
                but_core::TreeStatus::Deletion { .. } => Span::raw("deleted").red(),
                but_core::TreeStatus::Modification { .. } => Span::raw("modified").magenta(),
                but_core::TreeStatus::Rename { .. } => Span::raw("renamed").blue(),
            };

            let path = change.path.to_string();
            let width = path.width() + status.width() + 2;
            let padding = 2;
            diff_line_items.extend([
                ListItem::new(Line::from_iter(
                    repeat_n("─", width + padding).chain(once("╮")),
                ))
                .dim(),
                ListItem::new(Line::from_iter([
                    Span::raw(" "),
                    status,
                    Span::raw(": "),
                    Span::raw(path),
                    Span::raw(" "),
                    Span::raw("│").dim(),
                ])),
                ListItem::new(Line::from_iter(
                    repeat_n("─", width + padding).chain(once("╯")),
                ))
                .dim(),
                ListItem::new(""),
            ]);

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
                                diff_line_items.push(ListItem::new(
                                    "(diff generated from binary-to-text conversion)",
                                ));
                            }

                            if let Some(headers) = diff.lines().next() {
                                diff_line_items.push(ListItem::new(
                                    Span::raw(headers.to_str_lossy().to_string()).dim(),
                                ));
                                diff_line_items.push(ListItem::new(
                                    Line::from_iter(repeat_n("─", headers.to_str_lossy().width()))
                                        .dim(),
                                ))
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
                                    let item = ListItem::new(Line::from_iter([
                                        Span::raw(" ".repeat(old_width as _)),
                                        Span::raw(" ┊ ").dim(),
                                        Span::raw(
                                            " ".repeat((new_width - num_digits(new_line_num)) as _),
                                        ),
                                        Span::raw(new_line_num.to_string()).fg(PLUS_EMPH_BG),
                                        Span::raw(" │ ").dim(),
                                        Span::raw(rest.to_str_lossy().to_string()).bg(PLUS_EMPH_BG),
                                    ]));
                                    new_line_num += 1;
                                    item
                                } else if let Some(rest) = line.strip_prefix(b"-") {
                                    let item = ListItem::new(Line::from_iter([
                                        Span::raw(
                                            " ".repeat((old_width - num_digits(old_line_num)) as _),
                                        ),
                                        Span::raw(old_line_num.to_string()).fg(MINUS_EMPH_BG),
                                        Span::raw(" ┊ ").dim(),
                                        Span::raw(" ".repeat(new_width as _)),
                                        Span::raw(" │ ").dim(),
                                        Span::raw(rest.to_str_lossy().to_string())
                                            .bg(MINUS_EMPH_BG),
                                    ]));
                                    old_line_num += 1;
                                    item
                                } else {
                                    let line = line.strip_prefix(b" ").unwrap_or(line);
                                    let item = ListItem::new(Line::from_iter([
                                        Span::raw(
                                            " ".repeat((old_width - num_digits(old_line_num)) as _),
                                        ),
                                        Span::raw(old_line_num.to_string()).dark_gray(),
                                        Span::raw(" ┊ ").dim(),
                                        Span::raw(
                                            " ".repeat((new_width - num_digits(new_line_num)) as _),
                                        ),
                                        Span::raw(new_line_num.to_string()).dark_gray(),
                                        Span::raw(" │ ").dim(),
                                        Span::raw(line.to_str_lossy().to_string()),
                                    ]));
                                    old_line_num += 1;
                                    new_line_num += 1;
                                    item
                                };
                                diff_line_items.push(item);
                            }

                            if hunk_iter.peek().is_some() {
                                diff_line_items.push(ListItem::new(""));
                            }
                        }
                    }
                    UnifiedPatch::Binary => {
                        diff_line_items.push(ListItem::new("Binary file - no diff available"));
                    }
                    UnifiedPatch::TooLarge { size_in_bytes } => {
                        diff_line_items.push(ListItem::new(format!(
                            "File too large ({size_in_bytes} bytes) - no diff available"
                        )));
                    }
                }

                diff_line_items.push(ListItem::new(""));
            }
        }

        let mut header_items = Vec::new();

        header_items.extend([
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

        self.diff_widget = Some(DiffWidget {
            header_items,
            message,
            diff_line_items,
        });

        Ok(())
    }

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let layout = Layout::horizontal([Constraint::Length(1), Constraint::Min(1)]).split(area);

        let block = Block::new()
            .borders(Borders::LEFT)
            .border_style(Style::default().dim());
        frame.render_widget(block, layout[0]);

        if let Some(diff) = &self.diff_widget {
            frame.render_widget(diff, layout[1]);
        } else {
            let paragraph = Paragraph::new(self.updates.to_string());
            frame.render_widget(paragraph, layout[1]);
        }
    }
}

#[derive(Debug)]
struct DiffWidget {
    header_items: Vec<ListItem<'static>>,
    message: String,
    diff_line_items: Vec<ListItem<'static>>,
}

impl Widget for &DiffWidget {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let mut items = self.header_items.clone();

        items.push(ListItem::new(""));

        let message_lines = textwrap::wrap(&self.message, textwrap::Options::new(area.width as _));
        items.extend(message_lines.into_iter().map(ListItem::new));

        items.push(ListItem::new(""));

        items.extend(self.diff_line_items.clone());

        List::new(items).render(area, buf);
    }
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

fn num_digits(n: u32) -> u32 {
    n.ilog10() + 1
}
