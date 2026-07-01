use std::borrow::Cow;

use but_api::legacy::oplog::RestoreKind;
use but_ctx::Context;
use ratatui::prelude::{Line, Span};

use crate::command::legacy::status::tui::{
    Message, ReloadCause, app::App, operations, toast::ToastKind,
};

impl App {
    pub fn handle_undo(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        self.restore_to_target_snapshot(UndoOrRedo::Undo, ctx, messages)
    }

    pub fn handle_redo(
        &mut self,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        self.restore_to_target_snapshot(UndoOrRedo::Redo, ctx, messages)
    }

    fn restore_to_target_snapshot(
        &mut self,
        kind: UndoOrRedo,
        ctx: &mut Context,
        messages: &mut Vec<Message>,
    ) -> anyhow::Result<()> {
        let target_snapshot = match kind {
            UndoOrRedo::Undo => operations::get_undo_target_snapshot_legacy(ctx)?,
            UndoOrRedo::Redo => operations::get_redo_target_snapshot_legacy(ctx)?,
        };
        let Some(target_snapshot) = target_snapshot else {
            return Ok(());
        };

        let text = {
            let restore_from = if let Ok(Some(snapshot)) =
                operations::peel_restore_snapshot_legacy(ctx, target_snapshot.commit_id)
                && snapshot.commit_id != target_snapshot.commit_id
                && snapshot.details.is_some()
            {
                Cow::Owned(snapshot)
            } else {
                Cow::Borrowed(&target_snapshot)
            };

            let time = restore_from
                .created_at
                .format_or_unix(gix::date::time::format::DEFAULT);

            let commit = restore_from.commit_id.to_hex_with_len(7);

            Line::from_iter(
                [
                    Span::raw(match kind {
                        UndoOrRedo::Undo => "Undid ",
                        UndoOrRedo::Redo => "Redid ",
                    }),
                    Span::raw(commit.to_string()).style(self.theme.cli_id),
                ]
                .into_iter()
                .chain([Span::raw(" "), Span::raw(time).style(self.theme.time)])
                .chain(restore_from.details.iter().flat_map(|details| {
                    [
                        Span::raw(" "),
                        Span::raw(details.operation.title()).style(self.theme.attention),
                    ]
                })),
            )
        };

        let commit = target_snapshot.commit_id;

        operations::restore_snapshot_with_kind_legacy(
            ctx,
            match kind {
                UndoOrRedo::Undo => RestoreKind::RestoreFromSnapshotViaUndo,
                UndoOrRedo::Redo => RestoreKind::RestoreFromSnapshotViaRedo,
            },
            commit,
        )?;
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

#[derive(Copy, Clone)]
enum UndoOrRedo {
    Undo,
    Redo,
}
