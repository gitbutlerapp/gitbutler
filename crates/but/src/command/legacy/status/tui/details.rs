use std::iter::once;

use bstr::BString;
use but_ctx::Context;
use gix::actor::Signature;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget, Wrap},
};

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

        let message = commit_details.commit.message.clone();

        self.diff_widget = Some(DiffWidget {
            commit_id,
            message,
            author: commit_details.commit.author.clone(),
            committer: commit_details.commit.committer.clone(),
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
            let widget = Paragraph::new(self.updates.to_string());
            frame.render_widget(widget, layout[1]);
        }
    }
}

#[derive(Debug, Clone)]
struct DiffWidget {
    commit_id: gix::ObjectId,
    author: Signature,
    committer: Signature,
    message: BString,
}

impl Widget for &DiffWidget {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let header = self.build_header_widget();
        let message = self.build_message_widget();

        let layout = Layout::vertical([
            Constraint::Length((header.len() + 1) as _),
            Constraint::Length(message.line_count(area.width) as _),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        header.render(layout[0], buf);
        message.render(layout[1], buf);
        Clear.render(layout[2], buf);
        self.render_diff(layout[3], buf);
    }
}

impl DiffWidget {
    fn build_header_widget(&self) -> List<'static> {
        let items = [
            ListItem::new(Line::from_iter([
                Span::raw(format!("{:<11}", "Commit ID:")),
                Span::raw(self.commit_id.to_hex().to_string()).blue(),
            ])),
            ListItem::new(Line::from_iter(
                once(Span::raw(format!("{:<11}", "Author:"))).chain(render_signature(&self.author)),
            )),
            ListItem::new(Line::from_iter(
                once(Span::raw(format!("{:<11}", "Committer:")))
                    .chain(render_signature(&self.committer)),
            )),
        ];
        List::new(items)
    }

    fn build_message_widget(&self) -> Paragraph<'static> {
        if self.message.is_empty() {
            Paragraph::new("(no commit message)").dim()
        } else {
            let message = self.message.to_string();
            Paragraph::new(message).wrap(Wrap { trim: true })
        }
    }

    fn render_diff(&self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        List::new([
            "diff", "diff", "diff", "diff", "diff", "diff", "diff", "diff", "diff", "diff", "diff",
            "diff", "diff", "diff",
        ])
        .render(area, buf);
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
