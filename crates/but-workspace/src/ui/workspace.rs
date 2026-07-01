//! Frontend-facing detailed graph workspace types.

use bstr::{BString, ByteSlice};
use gix::date::parse::TimeBuf;
use serde::Serialize;

use crate::{ref_info, workspace as graph_workspace};

/// The rendered detailed graph workspace.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphWorkspace {
    /// The rendered stacks in the workspace.
    pub stacks: Vec<DetailedGraphStack>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphWorkspace);

impl From<graph_workspace::DetailedGraphWorkspace> for DetailedGraphWorkspace {
    fn from(value: graph_workspace::DetailedGraphWorkspace) -> Self {
        Self {
            stacks: value.stacks.into_iter().map(Into::into).collect(),
        }
    }
}

/// One rendered stack in a detailed graph workspace.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphStack {
    /// The rendered rows in this stack.
    pub rows: Vec<DetailedGraphRow>,
    /// Linear row runs split by reference rows and graph forks/merges.
    pub linear_segments: Vec<DetailedGraphLinearSegment>,
    /// Per-reference row runs, including shared commits for every reference
    /// that reaches them.
    pub reference_segments: Vec<DetailedGraphReferenceSegment>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphStack);

impl From<graph_workspace::Stack> for DetailedGraphStack {
    fn from(value: graph_workspace::Stack) -> Self {
        Self {
            rows: value.rows.into_iter().map(Into::into).collect(),
            linear_segments: value.linear_segments.into_iter().map(Into::into).collect(),
            reference_segments: value
                .reference_segments
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

/// One row in a rendered detailed graph stack.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphRow {
    /// The commit or reference represented by this row.
    pub data: DetailedGraphRowData,
    /// The node columns for this row.
    pub node_line: Vec<NodeLine>,
    /// The link columns for this row, if a link row is necessary.
    pub link_line: Option<Vec<LinkLine>>,
    /// The location of terminators, if a terminator row is necessary.
    pub term_line: Option<Vec<bool>>,
    /// The pad columns for this row.
    pub pad_lines: Vec<PadLine>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphRow);

impl From<graph_workspace::GraphRow> for DetailedGraphRow {
    fn from(value: graph_workspace::GraphRow) -> Self {
        Self {
            data: value.data.into(),
            node_line: value.node_line.into_iter().map(Into::into).collect(),
            link_line: value
                .link_line
                .map(|line| line.into_iter().map(Into::into).collect()),
            term_line: value.term_line,
            pad_lines: value.pad_lines.into_iter().map(Into::into).collect(),
        }
    }
}

/// The typed data represented by one detailed graph row.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject")]
pub enum DetailedGraphRowData {
    /// A commit row.
    Commit(super::Commit),
    /// A reference row.
    Reference(DetailedGraphReference),
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphRowData);

impl From<graph_workspace::GraphRowData> for DetailedGraphRowData {
    fn from(value: graph_workspace::GraphRowData) -> Self {
        match value {
            graph_workspace::GraphRowData::Commit { commit, state } => {
                Self::Commit(commit_for_ui(commit, state))
            }
            graph_workspace::GraphRowData::Reference {
                ref_name,
                additional_ref_info,
            } => Self::Reference(DetailedGraphReference {
                ref_name: ref_name.into(),
                status: additional_ref_info.map(Into::into),
            }),
        }
    }
}

/// A reference row in a detailed graph workspace.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphReference {
    /// The reference name rendered in this row.
    pub ref_name: DetailedGraphReferenceName,
    /// Derived status for this reference, when available.
    pub status: Option<DetailedGraphReferenceStatus>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphReference);

/// A Git reference name with both byte-preserving and frontend-friendly forms.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphReferenceName {
    /// The full ref name bytes, such as `refs/heads/feature`.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_bytes")
    )]
    pub full_name_bytes: BString,
    /// The full ref name as a string, such as `refs/heads/feature`.
    pub full_name: String,
    /// The shortened display name, such as `feature`.
    pub display_name: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphReferenceName);

impl From<gix::refs::FullName> for DetailedGraphReferenceName {
    fn from(value: gix::refs::FullName) -> Self {
        Self {
            full_name: value.as_bstr().to_string(),
            display_name: value.shorten().to_str_lossy().into_owned(),
            full_name_bytes: value.into_inner(),
        }
    }
}

/// Derived status for a detailed graph reference row.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphReferenceStatus {
    /// The remote-tracking reference this reference was compared with.
    pub remote_ref: Option<DetailedGraphReferenceName>,
    /// Push status for this reference.
    pub push_status: super::PushStatus,
    /// Push status including parent references below it in the stack.
    pub combined_push_status: super::PushStatus,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphReferenceStatus);

impl From<but_rebase::graph_rebase::workspace::ReferenceStatus> for DetailedGraphReferenceStatus {
    fn from(value: but_rebase::graph_rebase::workspace::ReferenceStatus) -> Self {
        Self {
            remote_ref: value.remote_ref.map(Into::into),
            push_status: value.push_status,
            combined_push_status: value.combined_push_status,
        }
    }
}

/// A linear run of detailed graph rows.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphLinearSegment {
    /// The reference row that starts this run, if any.
    pub reference_idx: Option<usize>,
    /// The row indices in this run.
    pub row_idxs: Vec<usize>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphLinearSegment);

impl From<graph_workspace::LinearSegment> for DetailedGraphLinearSegment {
    fn from(value: graph_workspace::LinearSegment) -> Self {
        Self {
            reference_idx: value.reference_idx,
            row_idxs: value.row_idxs,
        }
    }
}

/// A reference and the rows reachable from it down to the next reference.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetailedGraphReferenceSegment {
    /// The reference row index.
    pub reference_idx: usize,
    /// The row indices in this reference segment.
    pub row_idxs: Vec<usize>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DetailedGraphReferenceSegment);

impl From<graph_workspace::ReferenceSegment> for DetailedGraphReferenceSegment {
    fn from(value: graph_workspace::ReferenceSegment) -> Self {
        Self {
            reference_idx: value.reference_idx,
            row_idxs: value.row_idxs,
        }
    }
}

/// A column in a detailed graph node row.
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum NodeLine {
    /// Blank space.
    Blank,
    /// A vertical ancestor line.
    Ancestor,
    /// A vertical parent line.
    Parent,
    /// The node for this row.
    Node,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(NodeLine);

impl From<renderdag::NodeLine> for NodeLine {
    fn from(value: renderdag::NodeLine) -> Self {
        match value {
            renderdag::NodeLine::Blank => Self::Blank,
            renderdag::NodeLine::Ancestor => Self::Ancestor,
            renderdag::NodeLine::Parent => Self::Parent,
            renderdag::NodeLine::Node => Self::Node,
        }
    }
}

/// A column in a detailed graph padding row.
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum PadLine {
    /// Blank space.
    Blank,
    /// A vertical ancestor line.
    Ancestor,
    /// A vertical parent line.
    Parent,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PadLine);

impl From<renderdag::PadLine> for PadLine {
    fn from(value: renderdag::PadLine) -> Self {
        match value {
            renderdag::PadLine::Blank => Self::Blank,
            renderdag::PadLine::Ancestor => Self::Ancestor,
            renderdag::PadLine::Parent => Self::Parent,
        }
    }
}

/// Bit flags for one column in a detailed graph linking row.
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct LinkLine(u16);

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(LinkLine);

impl From<renderdag::LinkLine> for LinkLine {
    fn from(value: renderdag::LinkLine) -> Self {
        Self(value.bits())
    }
}

fn commit_for_ui(commit: ref_info::Commit, state: super::CommitState) -> super::Commit {
    let change_id = commit.change_id().to_string();
    super::Commit {
        id: commit.id,
        parent_ids: commit.parent_ids,
        message: commit.message,
        has_conflicts: commit.has_conflicts,
        state,
        authored_at: commit.author.time.seconds as i128 * 1000,
        committed_at: commit.committer.time.seconds as i128 * 1000,
        author: commit.author.to_ref(&mut TimeBuf::default()).into(),
        change_id,
        gerrit_review_url: commit.gerrit_review_url,
    }
}
