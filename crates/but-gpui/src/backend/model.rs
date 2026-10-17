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

/// A branch's name without `refs/heads/`.
pub fn short_ref(ref_name: &str) -> &str {
    ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name)
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
    /// The forge that hosts the target, when there is one to make pull requests on.
    pub forge: Option<Forge>,
    pub stacks: Vec<Stack>,
    pub uncommitted: Vec<FileChange>,
    /// `HEAD` is GitButler's workspace. Otherwise one branch is checked out, as in a new
    /// project or after a plain `git checkout`, and the stacks are just that branch.
    pub managed: bool,
    /// The branch checked out instead of the workspace, by its short name. The stacks leave
    /// it out when it's the target's own branch, as in a new project.
    pub checked_out: Option<String>,
    /// The commit checked out to edit, while the project is in edit mode. There are no
    /// stacks to show then.
    pub editing: Option<super::edit::Editing>,
    /// Its linked worktrees, like ones agents work in, when GitButler's worktree support is
    /// on: newest first, archived ones left out.
    pub worktrees: Vec<Worktree>,
    /// The names of the worktrees archived, to bring back.
    pub archived_worktrees: Vec<std::sync::Arc<str>>,
    /// Whether the worktrees were listed: GitButler's worktree support is on and listing
    /// them worked. Otherwise the lists say nothing about which worktrees are gone.
    pub worktrees_listed: bool,
}

/// A forge: where a project's pull requests live, and what a forge account signs in to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    GitLab,
    Bitbucket,
    Azure,
}

impl Forge {
    pub fn name(self) -> &'static str {
        match self {
            Forge::GitHub => "GitHub",
            Forge::GitLab => "GitLab",
            Forge::Bitbucket => "Bitbucket",
            Forge::Azure => "Azure",
        }
    }
}

/// A linked worktree and its uncommitted changes.
#[derive(Debug, Clone)]
pub struct Worktree {
    /// Its stable name, the folder name under `.git/worktrees/`.
    pub name: std::sync::Arc<str>,
    pub path: std::path::PathBuf,
    /// The branch it has checked out, or `None` for a detached `HEAD`.
    pub branch: Option<String>,
    /// The last commit of the bottom branch of its lane, which a branch moves below to go
    /// under the worktree's branches. `None` when that branch has no commits of its own.
    pub lane_end: Option<CommitId>,
    pub uncommitted: Vec<FileChange>,
    /// When it or its branch last moved, as a commit or checkout does, in seconds since the
    /// epoch, from their reflogs.
    pub updated_at: Option<i64>,
}

impl Worktree {
    /// When one of its uncommitted files was last written, in seconds since the epoch.
    pub fn last_activity(&self) -> Option<i64> {
        (self.uncommitted.iter())
            .filter_map(|file| file.modified.map(|ms| ms / 1000))
            .max()
    }
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

    /// Whether pushing `ref_name`, which pushes the branches below it too, has to force:
    /// it or one of those was rewritten since it was pushed.
    pub fn needs_force(&self, ref_name: &str) -> bool {
        self.stack_of(ref_name).is_some_and(|(stack, at)| {
            (stack.branches[at..].iter()).any(|branch| branch.push == PushState::NeedsForce)
        })
    }

    /// Whether `ref_name` can pull the commits only its remote has: it has some, and no
    /// branches stacked on it. Pulling into a branch with others above moves its reference
    /// to the top of the stack, so the app doesn't offer it.
    pub fn can_pull(&self, ref_name: &str) -> bool {
        self.stack_of(ref_name)
            .is_some_and(|(stack, at)| at == 0 && !stack.branches[0].incoming.is_empty())
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
        let in_worktrees = self.worktrees.iter().filter_map(Worktree::last_activity);
        edited.chain(committed).chain(in_worktrees).max()
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
    /// The titles of the commits only the branch's remote has, newest first.
    pub incoming: Vec<String>,
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
    /// The checks that failed: each one's name, and where to read why.
    pub failed_checks: Vec<(String, String)>,
    pub labels: Vec<String>,
    /// Who's been asked to review it, bots left out. GitHub lists only those who haven't
    /// reviewed yet; GitLab and Bitbucket keep the others.
    pub reviewers: Vec<String>,
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
    /// An uncommitted file a merge left conflicted, markers and all, as an update can when
    /// the target changed the same lines. It's kept out of commits until marked resolved.
    pub conflicted: bool,
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

    /// Its hunks, or none when its body has no lines, like a binary file's.
    pub fn hunks(&self) -> &[Hunk] {
        match &self.body {
            DiffBody::Hunks { hunks, .. } => hunks,
            _ => &[],
        }
    }

    /// Its conflicts, in file order.
    pub fn conflicts(&self) -> &[Conflict] {
        match &self.body {
            DiffBody::Hunks { conflicts, .. } => conflicts,
            _ => &[],
        }
    }
}

/// The columns `text` takes in a monospace font: one a character, and two for those of three
/// bytes or more, which counts the wide ones, like CJK and emoji, generously.
pub fn columns(text: &str) -> u32 {
    text.chars()
        .map(|c| if c.len_utf8() < 3 { 1 } else { 2 })
        .sum()
}

#[derive(Debug, Clone)]
pub enum DiffBody {
    Hunks {
        hunks: Vec<Hunk>,
        added: u32,
        removed: u32,
        /// The widest line on each side, old then new, in [`columns`].
        widest: [u32; 2],
        /// Where a conflicted commit and its new base both changed the file, in file order.
        conflicts: Vec<Conflict>,
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
    /// A conflict the app can't split into lines to pick between, like a file one side
    /// deleted, and why.
    Unresolvable(String),
}

/// Lines a conflicted commit and its new base both changed: what each has there, with the
/// unchanged lines around them, numbered as the file's diff numbers its new side.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// Stays the same while resolving others rewrites the commit.
    pub id: String,
    /// Its place among the file's conflicts, which resolving addresses it by.
    pub index: usize,
    /// The new-side line of the file's diff it starts at.
    pub at: u32,
    pub before: Vec<DiffLine>,
    /// What the commit now sits on has these lines.
    pub new_base: Vec<DiffLine>,
    /// The commit's own lines.
    pub commit: Vec<DiffLine>,
    /// The lines before either side changed them, when the merge knows.
    pub ancestor: Option<Vec<DiffLine>>,
    pub after: Vec<DiffLine>,
    /// Each side as the file has it, to keep both or edit from.
    pub new_base_text: String,
    pub commit_text: String,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub key: HunkKey,
    pub header: String,
    pub lines: Vec<DiffLine>,
    /// The scopes the hunk changes, outermost first, each as its first source line.
    pub scope: Vec<String>,
}

impl Hunk {
    /// Sub-hunks taking only the changed lines at `picks`: each run of picked added lines,
    /// or removed ones, marked as one side only with `0,0` on the other, as the commit
    /// engine expects.
    pub fn picked(&self, picks: &std::collections::BTreeSet<usize>) -> Vec<HunkKey> {
        let mut runs: Vec<but_core::HunkHeader> = Vec::new();
        let mut previous = None;
        for (ix, line) in self.lines.iter().enumerate() {
            let kind = (picks.contains(&ix) && line.kind != LineKind::Context).then_some(line.kind);
            match (kind, runs.last_mut()) {
                (Some(LineKind::Added), Some(run)) if previous == kind => run.new_lines += 1,
                (Some(LineKind::Removed), Some(run)) if previous == kind => run.old_lines += 1,
                (Some(LineKind::Added), _) => runs.push(but_core::HunkHeader {
                    old_start: 0,
                    old_lines: 0,
                    new_start: line.new.unwrap_or_default(),
                    new_lines: 1,
                }),
                (Some(_), _) => runs.push(but_core::HunkHeader {
                    old_start: line.old.unwrap_or_default(),
                    old_lines: 1,
                    new_start: 0,
                    new_lines: 0,
                }),
                (None, _) => {}
            }
            previous = kind;
        }
        runs.into_iter().map(HunkKey).collect()
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use but_core::HunkHeader;

    use super::{DiffLine, Hunk, HunkKey, LineKind};

    fn header(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> HunkHeader {
        HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        }
    }

    #[test]
    fn picked_lines_become_one_sided_runs() {
        let line = |kind, old, new| DiffLine {
            kind,
            old,
            new,
            text: "".into(),
            spans: Vec::new(),
            emph: Vec::new(),
        };
        let hunk = Hunk {
            key: HunkKey(header(1, 4, 1, 4)),
            header: String::new(),
            lines: vec![
                line(LineKind::Context, Some(1), Some(1)),
                line(LineKind::Removed, Some(2), None),
                line(LineKind::Removed, Some(3), None),
                line(LineKind::Added, None, Some(2)),
                line(LineKind::Added, None, Some(3)),
                line(LineKind::Context, Some(4), Some(4)),
            ],
            scope: Vec::new(),
        };
        let picks = BTreeSet::from([1, 2, 4]);
        assert_eq!(
            hunk.picked(&picks),
            vec![HunkKey(header(2, 2, 0, 0)), HunkKey(header(0, 0, 3, 1))],
            "both removals run together, the second addition stands alone"
        );
        let picks = BTreeSet::from([1, 3]);
        assert_eq!(
            hunk.picked(&picks),
            vec![HunkKey(header(2, 1, 0, 0)), HunkKey(header(0, 0, 2, 1))],
            "a removal and an addition never share a run"
        );
    }
}
