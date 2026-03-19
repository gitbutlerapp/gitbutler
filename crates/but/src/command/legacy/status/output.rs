use std::sync::Arc;

use gitbutler_stack::StackId;
use ratatui::text::Span;

use crate::{
    CliId,
    command::legacy::status::{CommitClassification, render_oneshot},
    utils::WriteWithUtils,
};

pub(super) enum StatusOutput<'a> {
    /// Immediately print the outputs as it's being generated.
    ///
    /// This is used when running the status command in one-shot mode.
    Immediate { out: &'a mut dyn WriteWithUtils },
    /// Buffer the output so it can be rendered in the TUI.
    Buffer {
        lines: &'a mut Vec<StatusOutputLine>,
    },
}

impl StatusOutput<'_> {
    fn push_line(
        &mut self,
        connector: Option<Vec<Span<'static>>>,
        content: StatusOutputContent,
        data: StatusOutputLineData,
    ) -> anyhow::Result<()> {
        let output_line = StatusOutputLine {
            connector,
            content,
            data,
        };

        match self {
            StatusOutput::Immediate { out } => {
                render_oneshot::render_oneshot(output_line, *out)?;
            }
            StatusOutput::Buffer { lines } => {
                lines.push(output_line);
            }
        }

        Ok(())
    }

    pub(super) fn update_notice(&mut self, line: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            None,
            StatusOutputContent::Plain(line),
            StatusOutputLineData::UpdateNotice,
        )
    }

    pub(super) fn connector(&mut self, connector: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(<_>::default()),
            StatusOutputLineData::Connector,
        )
    }

    pub(super) fn staged_changes(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::StagedChanges {
                cli_id: Arc::new(id),
            },
        )
    }

    pub(super) fn staged_file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::StagedFile {
                cli_id: Arc::new(id),
            },
        )
    }

    pub(super) fn unstaged_changes(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::UnstagedChanges {
                cli_id: Arc::new(id),
            },
        )
    }

    pub(super) fn unstaged_file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::UnstagedFile {
                cli_id: Arc::new(id),
            },
        )
    }

    pub(super) fn branch(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::Branch {
                cli_id: Arc::new(id),
            },
        )
    }

    pub(super) fn file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::File {
                cli_id: Arc::new(id),
            },
        )
    }

    pub(super) fn commit(
        &mut self,
        connector: Vec<Span<'static>>,
        line: CommitLineContent,
        id: CliId,
        stack_id: Option<StackId>,
        classification: CommitClassification,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Commit(line),
            StatusOutputLineData::Commit {
                cli_id: Arc::new(id),
                stack_id,
                classification,
            },
        )
    }

    pub(super) fn commit_message(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::CommitMessage,
        )
    }

    pub(super) fn empty_commit_message(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::EmptyCommitMessage,
        )
    }

    pub(super) fn warning(&mut self, line: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            None,
            StatusOutputContent::Plain(line),
            StatusOutputLineData::Warning,
        )
    }

    pub(super) fn hint(&mut self, line: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            None,
            StatusOutputContent::Plain(line),
            StatusOutputLineData::Hint,
        )
    }

    pub(super) fn no_assignments_unstaged(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::NoAssignmentsUnstaged,
        )
    }

    pub(super) fn merge_base(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::MergeBase,
        )
    }

    pub(super) fn upstream_changes(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(line),
            StatusOutputLineData::UpstreamChanges,
        )
    }
}

/// The non-connector content rendered for one status line.
#[derive(Debug, Clone)]
pub(super) enum StatusOutputContent {
    /// Generic status content represented as one flat list of spans.
    Plain(Vec<Span<'static>>),
    /// Structured content for commit rows where SHA and message are split.
    Commit(CommitLineContent),
}

/// Structured content for a commit row in status output.
#[derive(Debug, Default, Clone)]
pub(super) struct CommitLineContent {
    pub(super) sha: Vec<Span<'static>>,
    pub(super) author: Vec<Span<'static>>,
    pub(super) message: Vec<Span<'static>>,
    pub(super) suffix: Vec<Span<'static>>,
}

#[derive(Debug, Clone)]
pub(super) struct StatusOutputLine {
    /// The span holding the connector, if any, for this line. Includes padding and indicators that
    /// might be shown along side the connector.
    ///
    /// Example:
    ///
    /// ╭┄zz [unstaged changes]                                         | Some("╭┄")
    /// ┊   ur M flake.nix                                              | Some("┊   ")
    /// ┊                                                               | Some("┊ ")
    /// ┊╭┄dp [dp-branch-4]                                             | Some("┊╭┄")
    /// ┊●   3dd0f00 (no commit message) (no changes)                   | Some("┊●   ")
    /// ├╯                                                              | Some("├╯ ")
    /// ┊                                                               | Some("┊ ")
    /// ┊● 7cd07f6 (upstream) ⏫ 1 new commits (checked 34 seconds ago) | Some("┊● ")
    /// ├╯ 8678259 [origin/main] 2026-03-11 nix                         | Some("├╯ ")
    pub(super) connector: Option<Vec<Span<'static>>>,
    /// The content of the line such as the commit, branch, or file.
    pub(super) content: StatusOutputContent,
    /// The backing data associated with this line.
    ///
    /// This tells the TUI what data the actual line is showing. Used for performing operations on
    /// the line.
    pub(super) data: StatusOutputLineData,
}

impl StatusOutputLine {
    pub(super) fn is_selectable(&self) -> bool {
        match &self.data {
            StatusOutputLineData::Commit { classification, .. } => match classification {
                CommitClassification::LocalOnly
                | CommitClassification::Pushed
                | CommitClassification::Modified => true,
                CommitClassification::Upstream | CommitClassification::Integrated => false,
            },
            StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnstagedChanges { .. }
            | StatusOutputLineData::UnstagedFile { .. }
            | StatusOutputLineData::Branch { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::File { .. } => true,
            StatusOutputLineData::Connector
            | StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::EmptyCommitMessage => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum StatusOutputLineData {
    UpdateNotice,
    Connector,
    StagedChanges {
        cli_id: Arc<CliId>,
    },
    StagedFile {
        cli_id: Arc<CliId>,
    },
    UnstagedChanges {
        cli_id: Arc<CliId>,
    },
    UnstagedFile {
        cli_id: Arc<CliId>,
    },
    Branch {
        cli_id: Arc<CliId>,
    },
    Commit {
        cli_id: Arc<CliId>,
        stack_id: Option<StackId>,
        classification: CommitClassification,
    },
    CommitMessage,
    EmptyCommitMessage,
    File {
        cli_id: Arc<CliId>,
    },
    MergeBase,
    UpstreamChanges,
    Warning,
    Hint,
    NoAssignmentsUnstaged,
}

impl StatusOutputLineData {
    pub(super) fn cli_id(&self) -> Option<&Arc<CliId>> {
        match self {
            StatusOutputLineData::UnstagedChanges { cli_id }
            | StatusOutputLineData::UnstagedFile { cli_id }
            | StatusOutputLineData::Branch { cli_id }
            | StatusOutputLineData::StagedChanges { cli_id }
            | StatusOutputLineData::StagedFile { cli_id }
            | StatusOutputLineData::Commit { cli_id, .. }
            | StatusOutputLineData::File { cli_id } => Some(cli_id),
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::EmptyCommitMessage
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged => None,
        }
    }
}
