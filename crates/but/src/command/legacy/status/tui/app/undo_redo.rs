use but_ctx::Context;
use ratatui::prelude::{Line, Span};

use crate::command::legacy::{
    status::tui::{Message, ReloadCause, app::App, toast::ToastKind},
    undo_redo::{self, Direction, Operation, UndoRedoOutcome},
};

impl App {
    pub fn handle_undo(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        self.restore_to_target_snapshot(Operation::Undo, ctx, messages)
    }

    pub fn handle_redo(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        self.restore_to_target_snapshot(Operation::Redo, ctx, messages)
    }

    fn restore_to_target_snapshot(
        &mut self,
        operation: Operation,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let outcome = {
            let mut guard = ctx.exclusive_worktree_access();
            undo_redo::run(ctx, guard.write_permission(), operation)?
        };

        let UndoRedoOutcome::Restored {
            direction,
            snapshot_id,
            target_operation,
            target_time,
        } = outcome
        else {
            return Ok(());
        };

        let time = target_time.format_or_unix(gix::date::time::format::DEFAULT);
        let commit = snapshot_id.to_hex_with_len(7);
        let text = Line::from_iter([
            Span::raw(match direction {
                Direction::Undo => "Undid ",
                Direction::Redo => "Redid ",
            }),
            Span::raw(commit.to_string()).style(self.theme.cli_id),
            Span::raw(" "),
            Span::raw(time).style(self.theme.time),
            Span::raw(" "),
            Span::raw(target_operation.title()).style(self.theme.attention),
        ]);

        messages.extend([
            Message::Reload(None, ReloadCause::Mutation),
            Message::ShowToast {
                kind: ToastKind::Info,
                text: text.into(),
            },
        ]);

        Ok(())
    }
}
