//! Plain data the UI renders. Nothing in here borrows from a repository, so it can be
//! built on a background thread and handed to the UI as is.

use std::path::PathBuf;

/// A commit's object id, opaque to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommitId(pub(crate) gix::ObjectId);

impl CommitId {
    pub fn short(&self) -> String {
        self.0.to_hex_with_len(7).to_string()
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    /// Stable id from GitButler's project registry.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// The branch everything integrates into, like `origin/main`.
    pub target: Option<Target>,
    pub stacks: Vec<Stack>,
    pub uncommitted: Vec<FileChange>,
}

impl Workspace {
    pub fn branches(&self) -> impl Iterator<Item = &Branch> {
        self.stacks.iter().flat_map(|stack| &stack.branches)
    }

    pub fn find_commit(&self, id: CommitId) -> Option<(&Branch, &Commit)> {
        self.branches().find_map(|branch| {
            branch
                .commits
                .iter()
                .find(|commit| commit.id == id)
                .map(|commit| (branch, commit))
        })
    }

    pub fn find_branch(&self, ref_name: &str) -> Option<&Branch> {
        self.branches().find(|branch| branch.ref_name == ref_name)
    }

    /// When anything last happened here, in seconds: a file written or a commit made.
    pub fn last_activity(&self) -> Option<i64> {
        let edited = self
            .uncommitted
            .iter()
            .filter_map(|file| file.modified.map(|ms| ms / 1000));
        let committed = self
            .branches()
            .filter_map(|branch| branch.commits.first().map(|c| c.time));
        edited.chain(committed).max()
    }

    pub fn unpushed_commits(&self) -> usize {
        self.branches()
            .flat_map(|branch| &branch.commits)
            .filter(|commit| commit.state == CommitState::Local)
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct Target {
    /// Short name, like `origin/main`.
    pub name: String,
    /// Commits on the target the workspace doesn't have yet.
    pub behind: usize,
    /// Whether updating from the target would change anything.
    pub needs_update: bool,
}

#[derive(Debug, Clone)]
pub struct Stack {
    /// Branches from the top of the stack down to its base.
    pub branches: Vec<Branch>,
}

#[derive(Debug, Clone)]
pub struct Branch {
    /// Short name, like `feature-x`.
    pub name: String,
    /// Full ref name, like `refs/heads/feature-x`. Empty for an anonymous segment.
    pub ref_name: String,
    /// Newest first.
    pub commits: Vec<Commit>,
    /// Commits on the remote branch that aren't local yet.
    pub upstream_only: usize,
    pub push: PushState,
    pub review: Option<Review>,
}

#[derive(Debug, Clone)]
pub struct Review {
    pub number: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushState {
    NothingToPush,
    Unpushed,
    NeedsForce,
    NeverPushed,
    Integrated,
}

impl PushState {
    pub fn can_push(self) -> bool {
        matches!(self, Self::Unpushed | Self::NeedsForce | Self::NeverPushed)
    }
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: CommitId,
    /// Survives rebases and rewording when the commit has a change id; the sha otherwise.
    pub key: String,
    pub title: String,
    pub message: String,
    pub author: String,
    /// Seconds since the Unix epoch.
    pub time: i64,
    pub state: CommitState,
    pub conflicted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitState {
    Local,
    Pushed,
    Integrated,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub previous_path: Option<String>,
    pub kind: ChangeKind,
    /// When the file was last written, in milliseconds since the Unix epoch. Only known
    /// for uncommitted changes, and not for deletions.
    pub modified: Option<i64>,
    /// What the backend needs to diff or move this change again.
    pub(crate) change: but_core::TreeChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl ChangeKind {
    pub fn letter(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub file: FileChange,
    pub body: DiffBody,
}

impl FileDiff {
    pub fn stats(&self) -> (u32, u32) {
        match &self.body {
            DiffBody::Hunks { added, removed, .. } => (*added, *removed),
            _ => (0, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DiffBody {
    Hunks {
        hunks: Vec<Hunk>,
        added: u32,
        removed: u32,
    },
    Binary,
    TooLarge {
        bytes: u64,
    },
    /// Mode changes and the like, with no lines to show.
    Empty,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub key: HunkKey,
    pub header: String,
    pub lines: Vec<DiffLine>,
    /// The scopes the hunk changes, outermost first, each as its first source line.
    pub scope: Vec<String>,
}

/// Identifies a hunk of a file's uncommitted changes, to commit just that part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HunkKey(pub(crate) but_core::HunkHeader);

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    /// Syntax token spans within `text`.
    pub spans: Vec<(std::ops::Range<usize>, Token)>,
    /// The words that changed, when the line pairs up with one on the other side.
    pub emph: Vec<std::ops::Range<usize>>,
}

pub use super::highlight::Token;

/// Part of the uncommitted changes picked for a commit.
#[derive(Debug, Clone)]
pub struct Selected {
    pub file: FileChange,
    /// `None` takes the whole file.
    pub hunks: Option<Vec<HunkKey>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}
