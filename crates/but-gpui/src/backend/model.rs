//! Plain data the UI renders. Nothing in here borrows from a repository, so it can be
//! built on a background thread and handed to the UI as is.

use std::path::PathBuf;
use std::sync::Arc;

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
    /// The forge that hosts the target, like "GitHub", when there is one to make pull
    /// requests on.
    pub forge: Option<String>,
    pub stacks: Vec<Stack>,
    pub uncommitted: Vec<FileChange>,
}

impl Workspace {
    pub fn branches(&self) -> impl Iterator<Item = &Branch> {
        self.stacks.iter().flat_map(|stack| &stack.branches)
    }

    pub fn commits(&self) -> impl Iterator<Item = &Commit> {
        self.branches().flat_map(|branch| &branch.commits)
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

    /// The stack holding the branch `ref_name`, and the branch's place in it from the top.
    pub fn stack_of(&self, ref_name: &str) -> Option<(&Stack, usize)> {
        self.stacks.iter().find_map(|stack| {
            let at = stack.branches.iter().position(|b| b.ref_name == ref_name)?;
            Some((stack, at))
        })
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
        self.commits()
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

/// An entry in a project's operation log, which Undo restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotId(pub(crate) gix::ObjectId);

/// A branch's pull request, as the forge cache last saw it.
#[derive(Debug, Clone)]
pub struct Review {
    pub number: u64,
    /// Empty until the forge has been asked about it.
    pub title: String,
    pub state: ReviewState,
    pub url: Option<String>,
    /// The CI checks on the branch, once any have been seen.
    pub checks: Option<Checks>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    Open,
    Draft,
    Merged,
    Closed,
}

/// How a branch's CI checks stand.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Checks {
    pub passed: usize,
    pub failed: usize,
    pub running: usize,
}

impl Checks {
    pub fn total(self) -> usize {
        self.passed + self.failed + self.running
    }
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

impl Commit {
    /// Whether `key` is a change ID rather than the commit's sha.
    pub fn has_change_id(&self) -> bool {
        gix::ObjectId::from_hex(self.key.as_bytes()).ok() != Some(self.id.0)
    }
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

impl FileChange {
    /// What the change is against: the file as committed, or nothing for an addition.
    /// Committing part of a file moves it without touching the file itself.
    pub fn base(&self) -> Option<gix::ObjectId> {
        use but_core::TreeStatus as S;
        match &self.change.status {
            S::Addition { .. } => None,
            S::Deletion { previous_state }
            | S::Modification { previous_state, .. }
            | S::Rename { previous_state, .. } => Some(previous_state.id),
        }
    }
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

    /// The change in a word, for screen readers.
    pub fn word(self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Modified => "modified",
            ChangeKind::Deleted => "deleted",
            ChangeKind::Renamed => "renamed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub file: FileChange,
    pub body: DiffBody,
    /// A hash of the patch, which tells a later load the file hasn't changed.
    pub fingerprint: u64,
    /// Highlighted and given scope names. A load can leave files past the first few
    /// plain, for a later one to fill in.
    pub annotated: bool,
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
    /// An image, shown before and after; a side is missing for an added or deleted file.
    Image {
        before: Option<std::sync::Arc<gpui::Image>>,
        after: Option<std::sync::Arc<gpui::Image>>,
    },
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
    /// Shared, so drawing the line doesn't copy it.
    pub text: Arc<str>,
    /// Syntax token spans within `text`.
    pub spans: Vec<(std::ops::Range<usize>, Token)>,
    /// The words that changed, when the line pairs up with one on the other side.
    pub emph: Vec<std::ops::Range<usize>>,
}

pub use super::highlight::Token;

/// A file's changes, or some of its hunks, picked for an operation.
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
