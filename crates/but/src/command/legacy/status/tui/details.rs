use std::iter::{once, repeat_n};

use bstr::BString;
use but_core::UnifiedPatch;
use but_ctx::Context;
use gix::actor::Signature;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Widget, Wrap},
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
            // let Some(patch) = but_api::diff::tree_change_diffs(ctx, change.clone().into())? else {
            //     continue;
            // };

            // match patch {
            //     UnifiedPatch::Binary => todo!(),
            //     UnifiedPatch::TooLarge { size_in_bytes } => todo!(),
            //     UnifiedPatch::Patch {
            //         hunks,
            //         is_result_of_binary_to_text_conversion,
            //         lines_added,
            //         lines_removed,
            //     } => {
            //         let s = hunks
            //             .iter()
            //             .map(|hunk| format!("{}\n", hunk.diff))
            //             .collect::<String>();
            //         panic!("{s}");
            //     }
            // }

            let status = match &change.status {
                but_core::TreeStatus::Addition { .. } => Span::raw("added").green(),
                but_core::TreeStatus::Deletion { .. } => Span::raw("deleted").red(),
                but_core::TreeStatus::Modification { .. } => Span::raw("modified").magenta(),
                but_core::TreeStatus::Rename { .. } => Span::raw("renamed").blue(),
            };

            let path = change.path.to_string();
            let width = path.width() + status.width() + 2;
            let padding = 2;
            diff_line_items.push(
                ListItem::new(Line::from_iter(
                    repeat_n("─", width + padding).chain(once("╮")),
                ))
                .dim(),
            );
            diff_line_items.push(ListItem::new(Line::from_iter([
                Span::raw(" "),
                status,
                Span::raw(": "),
                Span::raw(path),
                Span::raw(" "),
                Span::raw("│").dim(),
            ])));
            diff_line_items.push(
                ListItem::new(Line::from_iter(
                    repeat_n("─", width + padding).chain(once("╯")),
                ))
                .dim(),
            );
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

// #[derive(Debug)]
// struct DiffEntry {
//     path: BString,
// }

// #[derive(Debug)]
// struct DiffWidget {
//     commit_id: gix::ObjectId,
//     author: Signature,
//     committer: Signature,
//     message: BString,
//     diff_entries: Vec<DiffEntry>,
// }

// impl Widget for &DiffWidget {
//     fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
//         let header = self.build_header_widget();
//         let message = self.build_message_widget();

//         let layout = Layout::vertical([
//             Constraint::Length((header.len() + 1) as _),
//             Constraint::Length(message.line_count(area.width) as _),
//             Constraint::Length(1),
//             Constraint::Min(1),
//         ])
//         .split(area);

//         header.render(layout[0], buf);
//         message.render(layout[1], buf);
//         Clear.render(layout[2], buf);
//         self.render_diff(layout[3], buf);
//     }
// }

// impl DiffWidget {
//     fn build_header_widget(&self) -> List<'static> {
//         let items = [
//             ListItem::new(Line::from_iter([
//                 Span::raw(format!("{:<11}", "Commit ID:")),
//                 Span::raw(self.commit_id.to_hex().to_string()).blue(),
//             ])),
//             ListItem::new(Line::from_iter(
//                 once(Span::raw(format!("{:<11}", "Author:"))).chain(render_signature(&self.author)),
//             )),
//             ListItem::new(Line::from_iter(
//                 once(Span::raw(format!("{:<11}", "Committer:")))
//                     .chain(render_signature(&self.committer)),
//             )),
//         ];
//         List::new(items)
//     }

//     fn build_message_widget(&self) -> Paragraph<'static> {
//         if self.message.is_empty() {
//             Paragraph::new("(no commit message)").dim()
//         } else {
//             let message = self.message.to_string();
//             Paragraph::new(message).wrap(Wrap { trim: true })
//         }
//     }

//     fn render_diff(&self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
//         let layouts = Layout::vertical(self.diff_entries.iter().flat_map(|_| {
//             [
//                 // for header
//                 Constraint::Length(3),
//                 // padding between header and diff
//                 Constraint::Length(1),
//                 // for actual diff
//                 Constraint::Min(1),
//                 // padding between this entry and the next
//                 Constraint::Length(1),
//             ]
//         }))
//         .split(area);

//         for (idx, diff_entry) in self.diff_entries.iter().enumerate() {
//             let header_layout = layouts[idx];
//             let _padding = layouts[idx + 1];
//             let diff_layout = layouts[idx + 2];
//             let _padding = layouts[idx + 3];

//             let header = Paragraph::new(diff_entry.path.to_string()).block(
//                 Block::new()
//                     .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
//                     .border_type(BorderType::Rounded)
//                     .border_style(Style::default().dim()),
//             );
//             header.render(header_layout, buf);

//             List::new([
//                 "diff",
//                 "diff",
//                 "diff",
//                 "diff",
//                 "diff",
//                 "diff",
//                 "diff",
//                 "diff",
//                 "diff",
//             ]).render(diff_layout, buf);
//         }
//     }
// }

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
