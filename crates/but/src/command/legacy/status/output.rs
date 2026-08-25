use std::sync::Arc;

use but_core::ref_metadata::StackId;
use ratatui::text::Span;

use crate::{
    CliId,
    command::legacy::status::{CommitClassification, render_oneshot},
    utils::WriteWithUtils,
};

pub enum StatusOutput<'a> {
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

    pub fn update_notice(&mut self, line: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            None,
            StatusOutputContent::Plain(line),
            StatusOutputLineData::UpdateNotice,
        )
    }

    pub fn connector(&mut self, connector: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(<_>::default()),
            StatusOutputLineData::Connector,
        )
    }

    pub fn between_stacks(&mut self, connector: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Plain(<_>::default()),
            StatusOutputLineData::BetweenStacks,
        )
    }

    pub fn staged_changes(
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

    pub fn staged_file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: FileLineContent,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::File(line),
            StatusOutputLineData::StagedFile {
                cli_id: Arc::new(id),
            },
        )
    }

    pub fn uncommitted_changes(
        &mut self,
        connector: Vec<Span<'static>>,
        line: UncommittedLineContent,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Uncommitted(line),
            StatusOutputLineData::UncommittedChanges {
                cli_id: Arc::new(id),
            },
        )
    }

    pub fn uncommitted_changes_in_worktree(
        &mut self,
        connector: Vec<Span<'static>>,
        line: UncommittedLineContent,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Uncommitted(line),
            StatusOutputLineData::WorktreeUncommittedChanges {
                cli_id: Arc::new(id),
            },
        )
    }

    pub fn uncommitted_file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: FileLineContent,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::File(line),
            StatusOutputLineData::UncommittedFile {
                cli_id: Arc::new(id),
            },
        )
    }

    pub fn branch(
        &mut self,
        connector: Vec<Span<'static>>,
        line: BranchLineContent,
        id: CliId,
        is_merged_upstream: bool,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::Branch(line),
            StatusOutputLineData::Branch {
                cli_id: Arc::new(id),
                is_merged_upstream,
            },
        )
    }

    pub fn file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: FileLineContent,
        id: CliId,
    ) -> anyhow::Result<()> {
        self.push_line(
            Some(connector),
            StatusOutputContent::File(line),
            StatusOutputLineData::File {
                cli_id: Arc::new(id),
            },
        )
    }

    pub fn commit(
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

    pub fn commit_message(
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

    pub fn empty_commit_message(
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

    pub fn warning(&mut self, line: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            None,
            StatusOutputContent::Plain(line),
            StatusOutputLineData::Warning,
        )
    }

    pub fn hint(&mut self, line: Vec<Span<'static>>) -> anyhow::Result<()> {
        self.push_line(
            None,
            StatusOutputContent::Plain(line),
            StatusOutputLineData::Hint,
        )
    }

    pub fn no_assignments_unstaged(
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

    pub fn merge_base(
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

    pub fn upstream_changes(
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
pub enum StatusOutputContent {
    Plain(Vec<Span<'static>>),
    Commit(CommitLineContent),
    Branch(BranchLineContent),
    File(FileLineContent),
    Uncommitted(UncommittedLineContent),
}

#[derive(Debug, Default, Clone)]
pub struct CommitLineContent {
    pub change_id: Vec<Span<'static>>,
    pub sha: Vec<Span<'static>>,
    pub author: Vec<Span<'static>>,
    pub message: Vec<Span<'static>>,
    pub suffix: Vec<Span<'static>>,
}

/// Consdering the example "dp [dp-branch-1] (no commits)" see the field docs for what exactly they
/// correspond to.
#[derive(Debug, Default, Clone)]
pub struct BranchLineContent {
    /// "dp" in the example
    pub id: Vec<Span<'static>>,
    /// " [" in the example
    pub decoration_start: Vec<Span<'static>>,
    /// "dp-branch-1" in the example
    pub branch_name: Vec<Span<'static>>,
    /// "] " in the example
    pub decoration_end: Vec<Span<'static>>,
    /// "(no commits)" in the example
    pub suffix: Vec<Span<'static>>,
}

/// Consdering the example "ae:sv A a/b/c.rs" see the field docs for what exactly they
/// correspond to.
#[derive(Debug, Default, Clone)]
pub struct FileLineContent {
    /// "ae:sv" in the example
    pub id: Vec<Span<'static>>,
    /// "A" in the example
    pub status: Vec<Span<'static>>,
    /// "a/b/c.rs" in the example
    pub path: Vec<Span<'static>>,
}

/// Considering the example "zz [uncommitted] (no changes)" see the field docs for what exactly
/// they correspond to.
#[derive(Debug, Default, Clone)]
pub struct UncommittedLineContent {
    /// "zz" in the example
    pub id: Vec<Span<'static>>,
    /// " [" in the example
    pub decoration_start: Vec<Span<'static>>,
    /// "uncommitted" in the example
    pub label: Vec<Span<'static>>,
    /// "]" in the example
    pub decoration_end: Vec<Span<'static>>,
    /// " (no changes)" in the example
    pub suffix: Vec<Span<'static>>,
}

#[derive(Debug, Clone)]
pub struct StatusOutputLine {
    /// The span holding the connector, if any, for this line. Includes padding and indicators that
    /// might be shown along side the connector.
    ///
    /// Example:
    ///
    /// ╭┄zz [uncommitted]                                      | Some("╭┄")
    /// ┊   ur M flake.nix                                              | Some("┊   ")
    /// ┊                                                               | Some("┊ ")
    /// ┊╭┄dp [dp-branch-4]                                             | Some("┊╭┄")
    /// ┊●   3dd0f00 (no commit message) (no changes)                   | Some("┊●   ")
    /// ├╯                                                              | Some("├╯ ")
    /// ┊                                                               | Some("┊ ")
    /// ┊● 7cd07f6 (upstream: origin/main) 1 new commit (checked 34 seconds ago) | Some("┊● ")
    /// ├╯ 8678259 [origin/main] 2026-03-11 nix                         | Some("├╯ ")
    pub connector: Option<Vec<Span<'static>>>,
    /// The content of the line such as the commit, branch, or file.
    pub content: StatusOutputContent,
    /// The backing data associated with this line.
    ///
    /// This tells the TUI what data the actual line is showing. Used for performing operations on
    /// the line.
    pub data: StatusOutputLineData,
}

impl StatusOutputLine {
    pub fn is_selectable(&self) -> bool {
        match &self.data {
            StatusOutputLineData::Commit { classification, .. } => match classification {
                CommitClassification::LocalOnly
                | CommitClassification::Pushed
                | CommitClassification::Modified => true,
                CommitClassification::Upstream | CommitClassification::Integrated => false,
            },
            StatusOutputLineData::Branch {
                is_merged_upstream, ..
            } => !*is_merged_upstream,
            StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UncommittedChanges { .. }
            | StatusOutputLineData::WorktreeUncommittedChanges { .. }
            | StatusOutputLineData::UncommittedFile { .. }
            | StatusOutputLineData::CommitMessage
            | StatusOutputLineData::MergeBase
            | StatusOutputLineData::File { .. } => true,
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::BetweenStacks
            | StatusOutputLineData::Connector
            | StatusOutputLineData::Warning
            | StatusOutputLineData::Hint
            | StatusOutputLineData::NoAssignmentsUnstaged
            | StatusOutputLineData::UpstreamChanges
            | StatusOutputLineData::EmptyCommitMessage => false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StatusOutputLineData {
    UpdateNotice,
    Connector,
    BetweenStacks,
    StagedChanges {
        cli_id: Arc<CliId>,
    },
    StagedFile {
        cli_id: Arc<CliId>,
    },
    UncommittedChanges {
        cli_id: Arc<CliId>,
    },
    WorktreeUncommittedChanges {
        cli_id: Arc<CliId>,
    },
    UncommittedFile {
        cli_id: Arc<CliId>,
    },
    Branch {
        cli_id: Arc<CliId>,
        is_merged_upstream: bool,
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
    pub fn cli_id(&self) -> Option<&Arc<CliId>> {
        match self {
            StatusOutputLineData::UncommittedChanges { cli_id }
            | StatusOutputLineData::WorktreeUncommittedChanges { cli_id }
            | StatusOutputLineData::UncommittedFile { cli_id }
            | StatusOutputLineData::Branch { cli_id, .. }
            | StatusOutputLineData::StagedChanges { cli_id }
            | StatusOutputLineData::StagedFile { cli_id }
            | StatusOutputLineData::Commit { cli_id, .. }
            | StatusOutputLineData::File { cli_id } => Some(cli_id),
            StatusOutputLineData::UpdateNotice
            | StatusOutputLineData::Connector
            | StatusOutputLineData::BetweenStacks
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
