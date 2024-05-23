use anyhow::anyhow;
use anyhow::Result;
use itertools::Itertools;
use serde::Deserialize;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use strum::EnumString;

use serde::Serialize;

/// A snapshot of the repository and virtual branches state that GitButler can restore to.
/// It captures the state of the working directory, virtual branches and commits.
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// The sha of the commit that represents the snapshot
    pub id: String,
    /// Snapshot creation time in epoch milliseconds
    pub created_at: Duration,
    /// The number of working directory lines added in the snapshot
    pub lines_added: usize,
    /// The number of working directory lines removed in the snapshot
    pub lines_removed: usize,
    /// The list of working directory files that were changed in the snapshot
    pub files_changed: Vec<PathBuf>,
    /// Snapshot details as persisted in the commit message
    pub details: Option<SnapshotDetails>,
}

/// The payload of a snapshot commit
///
/// This is persisted as a commit message in the title, body and trailers format (https://git-scm.com/docs/git-interpret-trailers)
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDetails {
    /// The version of the snapshot format
    pub version: Version,
    /// The type of operation that was performed just before the snapshot was created
    pub operation: OperationType,
    /// The title / lablel of the snapshot
    pub title: String,
    /// Additional text describing the snapshot
    pub body: Option<String>,
    /// Additional key value pairs that describe the snapshot
    pub trailers: Vec<Trailer>,
}

impl SnapshotDetails {
    pub fn new(operation: OperationType) -> Self {
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
        let mut split: Vec<Vec<&str>> = message_lines
            .split(|line| line.is_empty())
            .map(|s| s.to_vec())
            .collect();
        let title = split.remove(0).join("\n");
        let mut trailers: Vec<Trailer> = split
            .pop()
            .ok_or(anyhow!("No trailers found on snapshot commit message"))?
            .iter()
            .map(|s| Trailer::from_str(s))
            .filter_map(Result::ok)
            .collect();
        let body = split.iter().map(|v| v.join("\n")).join("\n\n");
        let body = if body.is_empty() { None } else { Some(body) };

        let version = Version::from_str(
            &trailers
                .iter()
                .find(|t| t.key == "Version")
                .cloned()
                .ok_or(anyhow!("No version found on snapshot commit message"))?
                .value,
        )?;

        let operation = OperationType::from_str(
            &trailers
                .iter()
                .find(|t| t.key == "Operation")
                .cloned()
                .ok_or(anyhow!("No operation found on snapshot commit message"))?
                .value,
        )
        .unwrap_or(Default::default());

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
        writeln!(f, "{}", self.title)?;
        writeln!(f)?;
        if let Some(body) = &self.body {
            writeln!(f, "{}", body)?;
            writeln!(f)?;
        }
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Operation: {}", self.operation)?;
        for line in &self.trailers {
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum OperationType {
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
    #[default]
    Unknown,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct Version(u32);
impl Default for Version {
    fn default() -> Self {
        Version(1)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Version {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Version(u32::from_str(s)?))
    }
}

/// Represents a key value pair stored in a snapshot.
/// Using the git trailer format (https://git-scm.com/docs/git-interpret-trailers)
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
        write!(f, "{}: {}", self.key, self.value)
    }
}

impl FromStr for Trailer {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid trailer format"));
        }
        Ok(Self {
            key: parts[0].trim().to_string(),
            value: parts[1].trim().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trailer_display() {
        let trailer = Trailer {
            key: "foo".to_string(),
            value: "bar".to_string(),
        };
        assert_eq!(format!("{}", trailer), "foo: bar");
    }

    #[test]
    fn test_trailer_from_str() {
        let s = "foo: bar";
        let trailer = Trailer::from_str(s).unwrap();
        assert_eq!(trailer.key, "foo");
        assert_eq!(trailer.value, "bar");
    }

    #[test]
    fn test_trailer_from_str_invalid() {
        let s = "foobar";
        let result = Trailer::from_str(s);
        assert!(result.is_err());
    }

    #[test]
    fn test_version_from_trailer() {
        let s = "Version: 1";
        let trailer = Trailer::from_str(s).unwrap();
        let version = Version::from_str(&trailer.value).unwrap();
        assert_eq!(version.0, 1);
    }

    #[test]
    fn test_version_invalid() {
        let s = "Version: -1";
        let trailer = Trailer::from_str(s).unwrap();
        let version = Version::from_str(&trailer.value);
        assert!(version.is_err());
    }

    #[test]
    fn test_operation_type_from_trailer() {
        let s = "Operation: CreateCommit";
        let trailer = Trailer::from_str(s).unwrap();
        let operation = OperationType::from_str(&trailer.value).unwrap();
        assert_eq!(operation, OperationType::CreateCommit);
    }

    #[test]
    fn test_operation_unknown() {
        let commit_message = "Create a new snapshot\n\nBody text 1\nBody text2\n\nBody text 3\n\nVersion: 1\nOperation: Asdf\nFoo: Bar\n";
        let details = SnapshotDetails::from_str(commit_message).unwrap();
        assert_eq!(details.version.0, 1);
        assert_eq!(details.operation, OperationType::Unknown);
        assert_eq!(details.title, "Create a new snapshot");
        assert_eq!(
            details.body,
            Some("Body text 1\nBody text2\n\nBody text 3".to_string())
        );
        assert_eq!(
            details.trailers,
            vec![Trailer {
                key: "Foo".to_string(),
                value: "Bar".to_string(),
            }]
        );
    }

    #[test]
    fn test_new_snapshot() {
        let commit_sha = "1234567890".to_string();
        let commit_message =
            "Create a new snapshot\n\nBody text 1\nBody text2\n\nBody text 3\n\nVersion: 1\nOperation: CreateCommit\nFoo: Bar\n".to_string();
        let created_at = Duration::from_secs(1234567890);
        let details = SnapshotDetails::from_str(&commit_message.clone()).unwrap();
        let snapshot = Snapshot {
            id: commit_sha.clone(),
            created_at,
            lines_added: 1,
            lines_removed: 1,
            files_changed: vec![PathBuf::from("foo.txt")],
            details: Some(details),
        };
        assert_eq!(snapshot.id, commit_sha);
        assert_eq!(snapshot.created_at, created_at);
        let details = snapshot.details.unwrap();
        assert_eq!(details.version.0, 1);
        assert_eq!(details.operation, OperationType::CreateCommit);
        assert_eq!(details.title, "Create a new snapshot");
        assert_eq!(
            details.body,
            Some("Body text 1\nBody text2\n\nBody text 3".to_string())
        );
        assert_eq!(
            details.trailers,
            vec![Trailer {
                key: "Foo".to_string(),
                value: "Bar".to_string(),
            }]
        );
        assert_eq!(details.to_string(), commit_message);
    }
}
