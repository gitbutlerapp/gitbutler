use std::sync::Arc;

use bstr::BString;
use but_core::{HunkHeader, ref_metadata::StackId};
use but_workspace::commit::squash_commits::MessageCombinationStrategy;
use gix::refs::FullName;
use ratatui::style::Color;
use ratatui_textarea::TextArea;

use crate::{
    CliId,
    command::legacy::status::tui::{Markable, Marks, MessageOnDrop},
    id::{ShortId, UncommittedCliId},
    theme::Theme,
};

#[derive(Debug, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::EnumIter, Hash))]
#[strum_discriminants(name(ModeDiscriminant))]
pub(super) enum Mode {
    Normal(NormalMode),
    Rub(RubMode),
    InlineReword(InlineRewordMode),
    Command(CommandMode),
    Commit(CommitMode),
    Move(MoveMode),
    Details(DetailsMode),
    Stack(StackMode),
    MoveStack(MoveStackMode),
    PickChanges(PickUncommittedMode),
}

impl Default for Mode {
    fn default() -> Self {
        Self::Normal(Default::default())
    }
}

impl Mode {
    pub(super) fn bg(&self, theme: &'static Theme) -> Color {
        ModeDiscriminant::from(self).bg(theme)
    }

    #[expect(dead_code)]
    pub(super) fn fg(&self, theme: &'static Theme) -> Color {
        ModeDiscriminant::from(self).fg(theme)
    }

    pub(super) fn marks(&self) -> Option<&Marks> {
        match self {
            Mode::Normal(normal_mode) => Some(&normal_mode.marks),
            Mode::Rub(rub_mode) => match &rub_mode.source {
                RubSource::Marks(marks) => Some(marks),
                RubSource::CliId(..) | RubSource::CommittedHunk(..) => None,
            },
            Mode::Commit(commit_mode) => match &*commit_mode.source {
                CommitSource::Marks(marks) => Some(marks),
                CommitSource::Unassigned(..)
                | CommitSource::Uncommitted(..)
                | CommitSource::Stack(..) => None,
            },
            Mode::PickChanges(pick_uncommitted_mode) => Some(&pick_uncommitted_mode.marks),
            Mode::Details(details_mode) => Some(details_mode.return_mode.marks()),
            Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Move(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..) => None,
        }
    }
}

impl ModeDiscriminant {
    pub(super) fn bg(self, theme: &'static Theme) -> Color {
        match self {
            Self::Normal => theme.tui_mode_normal.bg.unwrap_or(Color::DarkGray),
            Self::Commit | Self::PickChanges => theme.tui_mode_commit.bg.unwrap_or(Color::Green),
            Self::Rub => theme.tui_mode_rub.bg.unwrap_or(Color::Blue),
            Self::InlineReword | Self::Stack => {
                theme.tui_mode_inline_reword.bg.unwrap_or(Color::Magenta)
            }
            Self::Command => theme.tui_mode_command.bg.unwrap_or(Color::Yellow),
            Self::Move | Self::MoveStack => theme.tui_mode_move.bg.unwrap_or(Color::Cyan),
            Self::Details => theme
                .tui_mode_details
                .bg
                .unwrap_or(Color::Rgb(255, 165, 0) /* orange */),
        }
    }

    pub(super) fn fg(self, theme: &'static Theme) -> Color {
        match self {
            Self::Normal => theme.tui_mode_normal.fg.unwrap_or(Color::White),
            Self::Commit | Self::PickChanges => theme.tui_mode_commit.fg.unwrap_or(Color::Black),
            Self::Rub => theme.tui_mode_rub.fg.unwrap_or(Color::Black),
            Self::InlineReword | Self::Stack => {
                theme.tui_mode_inline_reword.fg.unwrap_or(Color::Black)
            }
            Self::Command => theme.tui_mode_command.fg.unwrap_or(Color::Black),
            Self::Move | Self::MoveStack => theme.tui_mode_move.fg.unwrap_or(Color::Black),
            Self::Details => theme.tui_mode_details.fg.unwrap_or(Color::Black),
        }
    }

    pub(super) fn hotbar_string(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Rub => "rub",
            Self::InlineReword => "reword",
            Self::Command => "command",
            Self::Commit => "commit",
            Self::PickChanges => "pick changes",
            Self::Move => "move",
            Self::Details => "details",
            Self::Stack => "stack",
            Self::MoveStack => "move stack",
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct NormalMode {
    pub(super) marks: Marks,
}

#[derive(Debug)]
pub(super) struct RubMode {
    pub(super) source: RubSource,
    pub(super) available_targets: Vec<Arc<CliId>>,
    pub(super) how_to_combine_messages: MessageCombinationStrategy,
    pub(super) _unlock_details: Option<MessageOnDrop>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RubSource {
    Marks(Marks),
    CliId(Arc<CliId>),
    CommittedHunk(CommittedHunk),
}

impl RubSource {
    pub(super) fn contains(&self, other: &CliId) -> bool {
        match self {
            RubSource::Marks(marks) => {
                Markable::try_from_cli_id(other).is_some_and(|markable| marks.contains(&markable))
            }
            RubSource::CliId(source) => &**source == other,
            RubSource::CommittedHunk { .. } => false,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct CommittedHunk {
    pub(super) commit_id: gix::ObjectId,
    pub(super) header: HunkHeader,
    pub(super) path: Arc<BString>,
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
    pub(super) kind: CommandModeKind,
}

#[derive(Debug, Copy, Clone)]
pub(super) enum CommandModeKind {
    But,
    Shell,
}

#[derive(Debug)]
pub(super) struct CommitMode {
    pub(super) source: Arc<CommitSource>,
    /// If set, then the commit must be made on this stack
    ///
    /// Used when committing changes staged to a specific stack
    pub(super) scope_to_stack: Option<StackId>,
    /// How to compose the commit message.
    pub(super) message_composer: CommitMessageComposer,
}

#[derive(Debug, Copy, Clone, Default)]
pub(super) enum CommitMessageComposer {
    /// Open an editor to compose the commit message.
    #[default]
    Editor,
    /// Use an inline editor to compose the commit message.
    Inline,
    /// Create the commit with an empty message.
    Empty,
}

#[derive(Debug)]
pub(super) struct MoveMode {
    pub(super) source: Arc<MoveSource>,
}

/// A subset of [`CliId`] that supports being committed
#[derive(Debug)]
#[expect(clippy::large_enum_variant)]
pub(super) enum CommitSource {
    Marks(Marks),
    Unassigned(UnassignedCommitSource),
    Uncommitted(UncommittedCliId),
    Stack(StackCommitSource),
}

#[derive(Debug)]
pub(super) struct UnassignedCommitSource {
    pub(super) id: ShortId,
}

#[derive(Debug)]
pub(super) struct StackCommitSource {
    pub(super) stack_id: StackId,
}

impl CommitSource {
    pub fn try_new(id: CliId) -> Option<Self> {
        match id {
            CliId::Unassigned { id } => Some(Self::Unassigned(UnassignedCommitSource { id })),
            CliId::Uncommitted(uncommitted_cli_id) => Some(Self::Uncommitted(uncommitted_cli_id)),
            CliId::Stack { stack_id, .. } => Some(Self::Stack(StackCommitSource { stack_id })),
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Commit { .. } => None,
        }
    }

    pub(super) fn contains(&self, other: &CliId) -> bool {
        match self {
            CommitSource::Marks(marks) => {
                Markable::try_from_cli_id(other).is_some_and(|markable| marks.contains(&markable))
            }
            CommitSource::Unassigned(UnassignedCommitSource { id: lhs_id }) => {
                if let CliId::Unassigned { id: rhs_id } = other {
                    lhs_id == rhs_id
                } else {
                    false
                }
            }
            CommitSource::Uncommitted(lhs) => {
                if let CliId::Uncommitted(rhs) = other {
                    lhs == rhs
                } else {
                    false
                }
            }
            CommitSource::Stack(StackCommitSource {
                stack_id: stack_id_lhs,
            }) => {
                if let CliId::Stack {
                    stack_id: stack_id_rhs,
                    ..
                } = other
                {
                    stack_id_lhs == stack_id_rhs
                } else {
                    false
                }
            }
        }
    }
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

impl MoveSource {
    pub(super) fn is_commit(&self) -> bool {
        matches!(self, Self::Commit { .. })
    }
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

#[derive(Debug)]
pub(super) struct DetailsMode {
    pub(super) full_screen: bool,
    pub(super) return_mode: DetailsReturnMode,
}

#[derive(Debug)]
pub(super) enum DetailsReturnMode {
    Normal(NormalMode),
    PickChanges(PickUncommittedMode),
}

impl DetailsReturnMode {
    fn marks(&self) -> &Marks {
        match self {
            DetailsReturnMode::Normal(normal_mode) => &normal_mode.marks,
            DetailsReturnMode::PickChanges(pick_uncommitted_mode) => &pick_uncommitted_mode.marks,
        }
    }
}

#[derive(Debug)]
pub(super) struct StackMode {
    pub(super) stack_heads: Vec<FullName>,
}

#[derive(Debug, Default)]
pub(super) struct PickUncommittedMode {
    pub(super) marks: Marks,
}

#[derive(Debug)]
pub(super) struct MoveStackMode {
    pub(super) source: ReorderStackSource,
}

#[derive(Debug)]
pub(super) struct ReorderStackSource {
    pub(super) stack: StackId,
    pub(super) branch: String,
}

impl ReorderStackSource {
    pub(super) fn matches(&self, id: &CliId) -> bool {
        match id {
            CliId::Branch { name, stack_id, .. } => {
                stack_id.is_some_and(|stack| self.stack == stack) && self.branch == *name
            }
            CliId::Stack { .. }
            | CliId::Uncommitted(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Commit { .. }
            | CliId::Unassigned { .. } => false,
        }
    }
}
