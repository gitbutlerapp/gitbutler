use std::sync::Arc;

use ratatui::text::Span;

use crate::CliId;

#[derive(Default)]
pub(super) struct StatusOutput {
    pub(super) lines: Vec<StatusOutputLine>,
}

impl StatusOutput {
    fn push_line(
        &mut self,
        connector: Option<Vec<Span<'static>>>,
        line: Vec<Span<'static>>,
        data: StatusOutputLineData,
    ) {
        self.lines.push(StatusOutputLine {
            connector,
            line,
            data,
        });
    }

    pub(super) fn update_notice(&mut self, line: Vec<Span<'static>>) {
        self.push_line(None, line, StatusOutputLineData::UpdateNotice);
    }

    pub(super) fn connector(&mut self, connector: Vec<Span<'static>>) {
        self.push_line(
            Some(connector),
            <_>::default(),
            StatusOutputLineData::Connector,
        );
    }

    pub(super) fn staged_changes(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::StagedChanges {
                cli_id: Arc::new(id),
            },
        );
    }

    pub(super) fn staged_file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::StagedFile {
                cli_id: Arc::new(id),
            },
        );
    }

    pub(super) fn unstaged_changes(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::UnstagedChanges {
                cli_id: Arc::new(id),
            },
        );
    }

    pub(super) fn unstaged_file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::UnstagedFile {
                cli_id: Arc::new(id),
            },
        );
    }

    pub(super) fn branch(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::Branch {
                cli_id: Arc::new(id),
            },
        );
    }

    pub(super) fn file(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::File {
                cli_id: Arc::new(id),
            },
        );
    }

    pub(super) fn commit(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
        id: CliId,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::Commit {
                cli_id: Arc::new(id),
            },
        );
    }

    pub(super) fn commit_message(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) {
        self.push_line(Some(connector), line, StatusOutputLineData::CommitMessage);
    }

    pub(super) fn empty_commit_message(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::EmptyCommitMessage,
        );
    }

    pub(super) fn warning(&mut self, line: Vec<Span<'static>>) {
        self.push_line(None, line, StatusOutputLineData::Warning);
    }

    pub(super) fn hint(&mut self, line: Vec<Span<'static>>) {
        self.push_line(None, line, StatusOutputLineData::Hint);
    }

    pub(super) fn no_assignments_unstaged(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) {
        self.push_line(
            Some(connector),
            line,
            StatusOutputLineData::NoAssignmentsUnstaged,
        );
    }

    pub(super) fn merge_base(&mut self, connector: Vec<Span<'static>>, line: Vec<Span<'static>>) {
        self.push_line(Some(connector), line, StatusOutputLineData::MergeBase);
    }

    pub(super) fn upstream_changes(
        &mut self,
        connector: Vec<Span<'static>>,
        line: Vec<Span<'static>>,
    ) {
        self.push_line(Some(connector), line, StatusOutputLineData::UpstreamChanges);
    }
}

#[derive(Debug)]
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
    /// The content of the line such as the commit, branch, etc.
    pub(super) line: Vec<Span<'static>>,
    /// The backing data associated with this line.
    ///
    /// This tells the TUI what data the actual line is showing. Used for performing operations on
    /// the line.
    pub(super) data: StatusOutputLineData,
}

impl StatusOutputLine {
    pub(super) fn is_selectable(&self) -> bool {
        match &self.data {
            StatusOutputLineData::StagedChanges { .. }
            | StatusOutputLineData::StagedFile { .. }
            | StatusOutputLineData::UnstagedChanges { .. }
            | StatusOutputLineData::UnstagedFile { .. }
            | StatusOutputLineData::Branch { .. }
            | StatusOutputLineData::Commit { .. }
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

#[derive(Debug)]
pub(super) enum StatusOutputLineData {
    UpdateNotice,
    Connector,
    StagedChanges { cli_id: Arc<CliId> },
    StagedFile { cli_id: Arc<CliId> },
    UnstagedChanges { cli_id: Arc<CliId> },
    UnstagedFile { cli_id: Arc<CliId> },
    Branch { cli_id: Arc<CliId> },
    Commit { cli_id: Arc<CliId> },
    CommitMessage,
    EmptyCommitMessage,
    File { cli_id: Arc<CliId> },
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
            | StatusOutputLineData::Commit { cli_id }
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
