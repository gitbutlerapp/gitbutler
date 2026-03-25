use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

use crate::command::legacy::status::tui::{
    CommandMessage, CommitMessage, FilesMessage, Message, MoveMessage, RewordMessage, RubMessage,
};

use super::BranchMessage;

#[derive(Default, Debug)]
pub(super) struct Details {
    is_dirty: bool,
    updates: u32,
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

    pub(super) fn update(&mut self) {
        self.is_dirty = false;

        self.updates += 1;
    }

    pub(super) fn render(&self, area: Rect, frame: &mut Frame) {
        let widget = Paragraph::new(self.updates.to_string()).block(
            Block::new()
                .borders(Borders::LEFT)
                .border_style(Style::default().dark_gray()),
        );

        frame.render_widget(widget, area);
    }
}
