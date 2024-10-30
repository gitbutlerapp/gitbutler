use std::{
    fmt,
    fmt::{Debug, Display, Formatter},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{anyhow, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::EnumString;

/// A snapshot of the repository and virtual branches state that GitButler can restore to.
/// It captures the state of the working directory, virtual branches and commits.
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The id of the commit that represents the snapshot
    #[serde(rename = "id", with = "gitbutler_serde::oid")]
    pub commit_id: git2::Oid,
    /// Snapshot creation time in seconds from Unix epoch seconds, based on a commit as `commit_id`.
    #[serde(serialize_with = "gitbutler_serde::as_time_seconds_from_unix_epoch")]
    pub created_at: git2::Time,
    /// The number of working directory lines added in the snapshot
    pub lines_added: usize,
    /// The number of working directory lines removed in the snapshot
    pub lines_removed: usize,
    /// The list of working directory files that were changed in the snapshot
    pub files_changed: Vec<PathBuf>,
    /// Snapshot details as persisted in the commit message, or `None` if the details couldn't be parsed.
    pub details: Option<SnapshotDetails>,
}

/// The payload of a snapshot commit
///
/// This is persisted as a commit message in the title, body and trailers format (<https://git-scm.com/docs/git-interpret-trailers>)
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDetails {
    /// The version of the snapshot format
    pub version: Version,
    /// The type of operation that was performed just before the snapshot was created
    pub operation: OperationKind,
    /// The title / label of the snapshot
    pub title: String,
    /// Additional text describing the snapshot
    pub body: Option<String>,
    /// Additional key value pairs that describe the snapshot
    pub trailers: Vec<Trailer>,
}

impl SnapshotDetails {
    pub fn new(operation: OperationKind) -> Self {
        let title = operation.to_string();
        SnapshotDetails {
            version: Default::default(),
            operation,
            title,
            body: None,
            trailers: vec![],
        }
    }
    pub fn with_trailers(mut self, trailers: Vec<Trailer>) -> Self {
        self.trailers = trailers;
        self
    }
}

impl FromStr for SnapshotDetails {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let message_lines: Vec<&str> = s.lines().collect();
        let mut split: Vec<&[&str]> = message_lines.split(|line| line.is_empty()).collect();
        let title = split.remove(0).join("\n");
        let mut trailers: Vec<Trailer> = split
            .pop()
            .ok_or(anyhow!("No trailers found on snapshot commit message"))?
            .iter()
            .filter_map(|s| Trailer::from_str(s).ok())
            .collect();
        let body = split.iter().map(|v| v.join("\n")).join("\n\n");
        let body = if body.is_empty() { None } else { Some(body) };

        let version = trailers
            .iter()
            .find(|t| t.key == "Version")
            .ok_or(anyhow!("No version found on snapshot commit message"))?
            .value
            .parse()?;

        let operation = trailers
            .iter()
            .find(|t| t.key == "Operation")
            .ok_or(anyhow!("No operation found on snapshot commit message"))?
            .value
            .parse()
            .unwrap_or_default();

        // remove the version and operation attributes from the trailers since they have dedicated fields
        trailers.retain(|t| t.key != "Version" && t.key != "Operation");

        Ok(SnapshotDetails {
            version,
            operation,
            title,
            body,
            trailers,
        })
    }
}

impl Display for SnapshotDetails {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{}\n", self.title)?;
        if let Some(body) = &self.body {
            writeln!(f, "{}\n", body)?;
        }
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Operation: {}", self.operation)?;
        for line in &self.trailers {
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize, EnumString, Default)]
pub enum OperationKind {
    CreateCommit,
    CreateBranch,
    SetBaseBranch,
    MergeUpstream,
    UpdateWorkspaceBase,
    MoveHunk,
    UpdateBranchName,
    UpdateBranchNotes,
    ReorderBranches,
    SelectDefaultVirtualBranch,
    UpdateBranchRemoteName,
    GenericBranchUpdate,
    DeleteBranch,
    ApplyBranch,
    DiscardHunk,
    DiscardFile,
    AmendCommit,
    UndoCommit,
    UnapplyBranch,
    CherryPick,
    SquashCommit,
    UpdateCommitMessage,
    MoveCommit,
    RestoreFromSnapshot,
    ReorderCommit,
    InsertBlankCommit,
    MoveCommitFile,
    FileChanges,
    EnterEditMode,
    SyncWorkspace,
    CreateDependentBranch,
    RemoveDependentBranch,
    UpdateDependentBranchName,
    UpdateDependentBranchDescription,
    UpdateDependentBranchForgeId,
    #[default]
    Unknown,
}

impl From<OperationKind> for SnapshotDetails {
    fn from(value: OperationKind) -> Self {
        SnapshotDetails::new(value)
    }
}

impl fmt::Display for OperationKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct Version(u32);
impl Default for Version {
    fn default() -> Self {
        Version(2)
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl FromStr for Version {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Version(u32::from_str(s)?))
    }
}

/// Represents a key value pair stored in a snapshot, like `key: value\n`
/// Using the git trailer format (<https://git-scm.com/docs/git-interpret-trailers>)
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trailer {
    /// Trailer key
    pub key: String,
    /// Trailer value
    pub value: String,
}

impl Display for Trailer {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let escaped_value = self.value.replace('\n', "\\n");
        write!(f, "{}: {}", self.key, escaped_value)
    }
}

impl FromStr for Trailer {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid trailer format, expected `key: value`"));
        }
        let unescaped_value = parts[1].trim().replace("\\n", "\n");
        Ok(Self {
            key: parts[0].trim().to_string(),
            value: unescaped_value,
        })
    }
}
