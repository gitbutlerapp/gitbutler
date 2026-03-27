use std::sync::Arc;

use but_rebase::graph_rebase::mutate::InsertSide;
use gitbutler_stack::StackId;
use ratatui_textarea::TextArea;

use crate::{
    CliId,
    id::{ShortId, UncommittedCliId},
};

#[derive(Debug, Default, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::EnumIter, Hash))]
#[strum_discriminants(name(ModeDiscriminant))]
pub(super) enum Mode {
    #[default]
    Normal,
    Rub(RubMode),
    RubButApi(RubMode),
    InlineReword(InlineRewordMode),
    Command(CommandMode),
    Commit(CommitMode),
    Move(MoveMode),
    Branch,
}

#[derive(Debug)]
pub(super) struct RubMode {
    pub(super) source: Arc<CliId>,
    pub(super) available_targets: Vec<Arc<CliId>>,
}

#[derive(Debug)]
pub(super) enum InlineRewordMode {
    Commit {
        commit_id: gix::ObjectId,
        textarea: Box<TextArea<'static>>,
    },
    Branch {
        name: String,
        stack_id: StackId,
        textarea: Box<TextArea<'static>>,
    },
}

impl InlineRewordMode {
    pub(super) fn textarea(&self) -> &TextArea<'static> {
        match self {
            InlineRewordMode::Commit { textarea, .. }
            | InlineRewordMode::Branch { textarea, .. } => textarea,
        }
    }

    pub(super) fn textarea_mut(&mut self) -> &mut TextArea<'static> {
        match self {
            InlineRewordMode::Commit { textarea, .. }
            | InlineRewordMode::Branch { textarea, .. } => textarea,
        }
    }
}

#[derive(Debug)]
pub(super) struct CommandMode {
    pub(super) textarea: Box<TextArea<'static>>,
}

#[derive(Debug)]
pub(super) struct CommitMode {
    pub(super) source: Arc<CommitSource>,
    /// If set, then the commit must be made on this stack
    ///
    /// Used when committing changes staged to a specific stack
    pub(super) scope_to_stack: Option<StackId>,
    /// The side to insert the new commit on, relative to the target commit.
    ///
    /// Note this is only respected when inserting at a commit. If inserting at a branch we'll
    /// always use [`InsertSide::Below`].
    pub(super) insert_side: InsertSide,
}

/// A subset of [`CliId`] that supports being committed
#[derive(Debug)]
pub(super) enum CommitSource {
    Unassigned(UnassignedCommitSource),
    Uncommitted(Box<UncommittedCliId>),
    Stack(StackCommitSource),
}

#[derive(Debug)]
pub(super) struct UnassignedCommitSource {
    pub(super) id: ShortId,
}

#[derive(Debug)]
pub(super) struct StackCommitSource {
    pub(super) id: ShortId,
    pub(super) stack_id: StackId,
}

impl TryFrom<CliId> for CommitSource {
    type Error = anyhow::Error;

    fn try_from(id: CliId) -> Result<Self, Self::Error> {
        match id {
            CliId::Unassigned { id } => Ok(Self::Unassigned(UnassignedCommitSource { id })),
            CliId::Uncommitted(uncommitted_cli_id) => {
                Ok(Self::Uncommitted(Box::new(uncommitted_cli_id)))
            }
            CliId::Stack { id, stack_id } => Ok(Self::Stack(StackCommitSource { id, stack_id })),
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Commit { .. } => anyhow::bail!("cannot commit: {id:?}"),
        }
    }
}

impl PartialEq<CliId> for CommitSource {
    fn eq(&self, other: &CliId) -> bool {
        match self {
            CommitSource::Unassigned(UnassignedCommitSource { id: lhs_id }) => {
                if let CliId::Unassigned { id: rhs_id } = other {
                    lhs_id == rhs_id
                } else {
                    false
                }
            }
            CommitSource::Uncommitted(lhs) => {
                if let CliId::Uncommitted(rhs) = other {
                    &**lhs == rhs
                } else {
                    false
                }
            }
            CommitSource::Stack(StackCommitSource {
                id: id_lhs,
                stack_id: stack_id_lhs,
            }) => {
                if let CliId::Stack {
                    id: id_rhs,
                    stack_id: stack_id_rhs,
                } = other
                {
                    id_lhs == id_rhs && stack_id_lhs == stack_id_rhs
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct MoveMode {
    pub(super) source: Arc<MoveSource>,
    pub(super) insert_side: InsertSide,
}

/// A subset of [`CliId`] that supports being moved
#[derive(Debug)]
pub(super) enum MoveSource {
    Commit {
        commit_id: gix::ObjectId,
        id: ShortId,
    },
    Branch {
        name: String,
        id: ShortId,
        stack_id: Option<StackId>,
    },
}

impl TryFrom<CliId> for MoveSource {
    type Error = anyhow::Error;

    fn try_from(id: CliId) -> Result<Self, Self::Error> {
        match id {
            CliId::Branch { name, id, stack_id } => Ok(Self::Branch { name, id, stack_id }),
            CliId::Commit { commit_id, id } => Ok(Self::Commit { commit_id, id }),
            CliId::Uncommitted(uncommitted_cli_id) => {
                anyhow::bail!("cannot move: {:?}", uncommitted_cli_id.id)
            }
            CliId::PathPrefix { id, .. }
            | CliId::CommittedFile { id, .. }
            | CliId::Unassigned { id }
            | CliId::Stack { id, .. } => {
                anyhow::bail!("cannot move: {id:?}")
            }
        }
    }
}

impl PartialEq<CliId> for MoveSource {
    fn eq(&self, other: &CliId) -> bool {
        match self {
            MoveSource::Commit {
                commit_id: commit_id_lhs,
                id: id_lhs,
            } => {
                if let CliId::Commit {
                    commit_id: commit_id_rhs,
                    id: id_rhs,
                } = other
                {
                    commit_id_lhs == commit_id_rhs && id_lhs == id_rhs
                } else {
                    false
                }
            }
            MoveSource::Branch {
                name: name_lhs,
                id: id_lhs,
                stack_id: stack_id_lhs,
            } => {
                if let CliId::Branch {
                    name: name_rhs,
                    id: id_rhs,
                    stack_id: stack_id_rhs,
                } = other
                {
                    name_lhs == name_rhs && id_lhs == id_rhs && stack_id_lhs == stack_id_rhs
                } else {
                    false
                }
            }
        }
    }
}
