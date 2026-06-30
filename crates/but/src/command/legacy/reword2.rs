use bstr::BString;
use but_api::diff::ComputeLineStats;
use but_core::{RefMetadata, diff::CommitDetails};
use but_error::Code;
use but_transaction::Transaction;
use gix::prelude::ObjectIdExt as _;

use crate::command::legacy::{ShowDiffInEditor, reword::get_commit_message_from_editor};

#[derive(Debug, Clone)]
pub enum RewordCommitOperation {
    NoMessage,
    Message(String),
    UseEditor,
}

impl RewordCommitOperation {
    /// Check if this operation will open an editor.
    ///
    /// Used by the TUI to suspend itself.
    pub fn will_open_editor(&self) -> bool {
        match self {
            RewordCommitOperation::UseEditor => true,
            RewordCommitOperation::NoMessage | RewordCommitOperation::Message(_) => false,
        }
    }

    pub fn resolve(no_message: bool, message: Option<Vec<String>>) -> Self {
        match (no_message, message) {
            (true, None) => Self::NoMessage,
            (false, None) => Self::UseEditor,
            (false, Some(message)) => Self::Message(message.join("\n\n")),
            (true, Some(_)) => {
                unreachable!("--no-message and --message are mutually exclusive")
            }
        }
    }

    pub fn execute(
        self,
        new_commit: gix::ObjectId,
        tx: &mut Transaction<'_, '_, impl RefMetadata>,
    ) -> anyhow::Result<gix::ObjectId> {
        let message = match self {
            RewordCommitOperation::NoMessage => String::new(),
            RewordCommitOperation::Message(message) => message,
            RewordCommitOperation::UseEditor => {
                let repo = tx.repo();
                let commit_details = CommitDetails::from_commit_id(
                    new_commit.attach(repo),
                    ComputeLineStats::No.into(),
                )?;

                let current_message = commit_details.commit.inner.message.to_string();

                match get_commit_message_from_editor(
                    tx.repo(),
                    tx.context_lines(),
                    commit_details,
                    current_message,
                    "",
                    ShowDiffInEditor::Unspecified,
                ) {
                    Ok(message) => message.unwrap_or_default(),
                    Err(err) => {
                        return Err(
                            if let Some(Code::EditorExitedWithNonZeroStatus) =
                                err.downcast_ref::<but_error::Code>()
                            {
                                anyhow::anyhow!("Editor exited with non-zero status")
                            } else {
                                err
                            },
                        );
                    }
                }
            }
        };

        let reworded_commit = tx.reword_commit(new_commit, BString::from(message).as_ref())?;

        Ok(reworded_commit)
    }
}
