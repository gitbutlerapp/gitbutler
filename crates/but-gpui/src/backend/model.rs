//! Plain data the UI renders. Nothing in here borrows from a repository, so it can be
//! built on a background thread and handed to the UI as is.

#[cfg(test)]
pub(crate) mod build;
mod lanes;
mod push;

pub use lanes::{Checkout, Lane};

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

/// Saved as its hex, as Git writes it.
impl serde::Serialize for CommitId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for CommitId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let hex = <String as serde::Deserialize>::deserialize(d)?;
        (gix::ObjectId::from_hex(hex.as_bytes()).map(CommitId)).map_err(serde::de::Error::custom)
    }
}

pub use gix::refs::FullName;

/// A branch's name without `refs/heads/`.
pub fn short_ref(ref_name: &FullName) -> std::borrow::Cow<'_, str> {
    use bstr::ByteSlice as _;
    let name = ref_name.as_bstr();
    name.strip_prefix(b"refs/heads/")
        .unwrap_or(name)
        .to_str_lossy()
}

/// A branch's name without `refs/heads/`, from text that names it, as the oplog's.
pub fn short_text(ref_name: &str) -> &str {
    ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name)
}

#[derive(Debug, Clone)]
pub struct Project {
    /// Stable id from GitButler's project registry.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    /// When Git last recorded something done in it, in seconds since the epoch: a commit,
    /// checkout or rebase, or the index changing, in its main checkout or a linked one.
    pub last_active: Option<i64>,
    /// When GitButler last fetched it, or tried to, in seconds since the epoch. The desktop
    /// app fetches the project it has open, and `but` the one it runs in.
    pub last_fetch: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// The branch everything integrates into, like `origin/main`.
    pub target: Option<Target>,
    /// The target the project names but its repository no longer has, as when the remote
    /// renamed or deleted it, by its short name.
    pub lost_target: Option<String>,
    /// The forge that hosts the target, when there is one to make pull requests on.
    pub forge: Option<Forge>,
    /// The stacks, each a lane of the main checkout, then the lanes of the linked worktrees
    /// shown, in `worktrees`' order.
    pub lanes: Vec<Lane>,
    pub uncommitted: Vec<FileChange>,
    /// The tree of the main checkout's `HEAD` that `uncommitted` was read against. While it's
    /// the same, an operation on commits left the files as they were: see [`super::Kept`].
    pub tree: Option<gix::ObjectId>,
    /// `HEAD` is GitButler's workspace, merging two or more branches. Otherwise one branch is
    /// checked out, as in a new project or after a plain `git checkout`, and the stacks are
    /// just that branch, or none on the default branch with nothing of its own.
    pub managed: bool,
    /// The project's default branch, by its full name, like `refs/heads/main`: a new commit
    /// goes on a branch of its own rather than onto it, as `but commit` does.
    pub target_branch: Option<FullName>,
    /// The Git command a rebase, merge, cherry-pick or revert that stopped partway was
    /// started with, like `git rebase`, to finish it with.
    pub git_operation: Option<&'static str>,
    /// `HEAD` is detached or has no commits yet, or there's no target, which GitButler's
    /// history can't record or go back to: what's done in the main checkout can't be undone.
    pub unrecorded: bool,
    /// The repository has no remote, so there's nowhere to push to.
    pub remoteless: bool,
    /// The commit checked out to edit, while the project is in edit mode. There are no
    /// stacks to show then.
    pub editing: Option<super::edit::Editing>,
    /// Its linked worktrees, like ones agents work in, when GitButler's worktree support is
    /// on: newest first, archived ones left out.
    pub worktrees: Vec<Worktree>,
    /// The names of the worktrees archived, to bring back, with the branch each has checked
    /// out.
    pub archived_worktrees: Vec<(bstr::BString, Option<FullName>)>,
    /// A fingerprint of the repository's local and remote branches, of the pull requests the
    /// forge cache knows and of the branches linked worktrees shown have checked out, which
    /// the Branches page lists again when it changes.
    pub refs: u64,
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
    pub name: bstr::BString,
    pub path: std::path::PathBuf,
    /// The full ref of the branch it has checked out, or `None` for a detached `HEAD`.
    pub branch: Option<FullName>,
    pub uncommitted: Vec<FileChange>,
    /// The Git command it's stopped partway through, like `git rebase`.
    pub git_operation: Option<&'static str>,
    /// The tree of its `HEAD` that `uncommitted` was read against, as the main checkout's.
    pub tree: Option<gix::ObjectId>,
    /// When it or its branch last moved, as a commit or checkout does, in seconds since the
    /// epoch, from their reflogs.
    pub updated_at: Option<i64>,
}

impl Worktree {
    /// When its uncommitted files were written, and when it last moved, as a commit that
    /// takes them all does, in seconds since the epoch.
    pub fn activity(&self) -> impl Iterator<Item = i64> + '_ {
        (self.uncommitted.iter())
            .filter_map(|file| file.modified.map(|ms| ms / 1000))
            .chain(self.updated_at)
    }

    /// Nothing uncommitted, so not active either, and it hasn't moved in
    /// [`INACTIVE_AFTER`]. One with no reflog to date it stays active.
    pub fn is_inactive(&self) -> bool {
        self.uncommitted.is_empty()
            && (self.updated_at)
                .is_some_and(|at| crate::ui::now() - at > INACTIVE_AFTER.as_secs() as i64)
    }
}

/// How long a worktree with nothing uncommitted counts as active after it last moved.
const INACTIVE_AFTER: std::time::Duration = std::time::Duration::from_secs(12 * 60 * 60);

impl Workspace {
    /// Every commit shown, its linked worktrees' too.
    pub fn commits(&self) -> impl Iterator<Item = &Commit> {
        self.all_branches().flat_map(|branch| &branch.commits)
    }

    pub fn find_commit(&self, id: CommitId) -> Option<(&Branch, &Commit)> {
        self.all_branches().find_map(|branch| {
            branch
                .commits
                .iter()
                .find(|commit| commit.id == id)
                .map(|commit| (branch, commit))
        })
    }

    /// The commits only shown branches' remotes have.
    pub fn remote_commits(&self) -> impl Iterator<Item = &RemoteCommit> {
        self.all_branches().flat_map(|branch| &branch.incoming)
    }

    /// The target's name: its remote counterpart once there is one, else the branch picked,
    /// as one never pushed or of a repository with no remote.
    pub fn target_name(&self) -> Option<std::borrow::Cow<'_, str>> {
        match &self.target {
            Some(target) => Some(target.name.as_str().into()),
            None => self.target_branch.as_ref().map(super::short_ref),
        }
    }

    /// The commits only a remote has: branch `ref_name`'s remote, or with `None` the
    /// target's that the workspace doesn't have yet, newest first.
    pub fn upstream(&self, ref_name: Option<&FullName>) -> &[RemoteCommit] {
        match ref_name {
            Some(ref_name) => self
                .find_branch(Some(ref_name))
                .map_or(&[], |b| &b.incoming),
            None => self.target.as_ref().map_or(&[], |t| &t.incoming),
        }
    }

    pub fn find_branch(&self, ref_name: Option<&FullName>) -> Option<&Branch> {
        self.all_branches()
            .find(|branch| branch.ref_name.as_ref() == ref_name)
    }

    pub fn find_worktree(&self, name: &bstr::BStr) -> Option<&Worktree> {
        self.worktrees.iter().find(|worktree| worktree.name == name)
    }

    pub fn checked_out_in(&self, ref_name: &FullName) -> Option<&Worktree> {
        (self.worktrees.iter()).find(|worktree| worktree.branch.as_ref() == Some(ref_name))
    }

    /// The uncommitted files of `checkout`.
    pub fn uncommitted_in(&self, checkout: &Checkout) -> Option<&[FileChange]> {
        match checkout {
            Checkout::Main => Some(&self.uncommitted),
            Checkout::Linked(name) => Some(&self.find_worktree(name.as_ref())?.uncommitted),
        }
    }

    /// When anything last happened here, in seconds: a file written or a commit made.
    pub fn last_activity(&self) -> Option<i64> {
        let edited = self
            .uncommitted
            .iter()
            .filter_map(|file| file.modified.map(|ms| ms / 1000));
        // Linked worktrees' too, as an agent committing in one clears its uncommitted files.
        // Any commit, not just the top one, which can be an old one cherry-picked.
        let committed =
            (self.all_branches()).flat_map(|branch| branch.commits.iter().map(|c| c.time));
        let in_worktrees = self.worktrees.iter().flat_map(Worktree::activity);
        // Nothing dated ahead of the clock, as a machine whose clock is off dates its commits,
        // which would otherwise keep the project active until then.
        let now = crate::ui::now();
        (edited.chain(committed).chain(in_worktrees))
            .filter(|at| *at <= now)
            .max()
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
    /// The branch without its remote, like `main`, as pull requests name the branch they merge
    /// into.
    pub branch: String,
    /// It's the local target branch itself, like `main`, as it has no upstream, as in a
    /// repository with no remote: a land moves it rather than pushing it.
    pub local: bool,
    /// The remote branch of its name a `local` one doesn't track, like `origin/main` after
    /// `git push origin main` without `-u`: what others push there doesn't count here.
    pub untracked: Option<String>,
    /// Commits on the target's own line of history the workspace doesn't have yet: each pull
    /// request merged counts once, as the project's page lists them.
    pub behind: usize,
    /// Every commit the workspace doesn't have, those the merges brought in too.
    pub commits_behind: usize,
    /// Whether updating from the target would change anything.
    pub needs_update: bool,
    /// The newest of the `behind` commits, up to [`UPSTREAM_SHOWN`], named as the project's
    /// page names them.
    pub incoming: Vec<RemoteCommit>,
}

/// How many of the target's commits the workspace doesn't have yet are read with it.
pub const UPSTREAM_SHOWN: usize = 8;

#[derive(Debug, Clone)]
pub struct Branch {
    /// Short name, like `feature-x`.
    pub name: String,
    /// Full ref name, like `refs/heads/feature-x`. `None` for an anonymous segment.
    pub ref_name: Option<FullName>,
    /// Newest first.
    pub commits: Vec<Commit>,
    /// The commits only the branch's remote has, newest first.
    pub incoming: Vec<RemoteCommit>,
    /// The remote branch it tracks, like `origin/feature-x`.
    pub remote: Option<String>,
    pub push: PushState,
    pub review: Option<Review>,
}

/// An entry in a project's operation log, which Undo restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotId(pub(crate) gix::ObjectId);

/// A branch's pull request, as the forge cache last saw it.
#[derive(Debug, Clone, Default)]
pub struct Review {
    pub number: u64,
    /// Empty until the forge has been asked about it.
    pub title: String,
    pub state: ReviewState,
    pub url: Option<String>,
    /// It merges by itself once its required checks and reviews let it.
    pub auto_merge: bool,
    /// The CI checks on the branch, once any have been seen.
    pub checks: Option<Checks>,
    /// Each check, failed ones first, then those still running.
    pub check_runs: Vec<CheckRun>,
    pub labels: Vec<ReviewLabel>,
    /// Who's been asked to review it, bots left out. GitHub lists only those who haven't
    /// reviewed yet; GitLab and Bitbucket keep the others.
    pub reviewers: Vec<ForgeUser>,
    pub author: Option<ForgeUser>,
    /// The branch it merges from, as the forge names it.
    pub source_branch: String,
    /// The branch it merges into, as the forge names it.
    pub target_branch: String,
    /// Its description, as written.
    pub body: String,
    /// When it was opened, in seconds since the epoch.
    pub created_at: Option<i64>,
    /// When it last changed on the forge, in seconds since the epoch.
    pub updated_at: Option<i64>,
    /// When the forge was last asked about it, in seconds since the epoch.
    pub synced_at: Option<i64>,
}

/// A pull request's label, with its colour as `0xRRGGBB` when the forge gives one.
#[derive(Debug, Clone)]
pub struct ReviewLabel {
    pub name: String,
    pub color: Option<u32>,
}

/// Someone on the forge, with where their picture is when it has one.
#[derive(Debug, Clone)]
pub struct ForgeUser {
    pub login: String,
    pub avatar_url: Option<String>,
}

/// A CI check on a pull request's branch.
#[derive(Debug, Clone)]
pub struct CheckRun {
    pub name: String,
    /// Where to read its log.
    pub url: String,
    /// Whether it failed, once it finished.
    pub failed: Option<bool>,
    /// How it finished or how far it got, as the forge says: "Succeeded", "In progress".
    pub status: String,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
}

/// One part of what's been said on a pull request, as its conversation shows it.
#[derive(Debug, Clone)]
pub enum Remark {
    /// A comment on the pull request itself.
    Comment(Note),
    /// A review: its verdict and what it said. The threads it started on the diff follow it.
    Review {
        verdict: crate::backend::reviews::Verdict,
        note: Note,
    },
    /// A thread on the diff, `in_review` when it follows the review that started it.
    Thread { thread: Thread, in_review: bool },
}

/// Something someone wrote on a pull request, its words ready to draw.
#[derive(Debug, Clone)]
pub struct Note {
    pub author: Option<ForgeUser>,
    /// Written by a bot, as an agent's review is.
    pub bot: bool,
    pub body: Vec<crate::ui::markdown::Block>,
    /// When it was written, in seconds since the epoch.
    pub at: Option<i64>,
    /// Changed since it was written.
    pub edited: bool,
    /// Where it shows on the forge.
    pub url: String,
}

/// A conversation on lines of a pull request's diff.
#[derive(Debug, Clone)]
pub struct Thread {
    /// The forge's id for it.
    pub id: String,
    pub path: String,
    /// The first and last lines it was left on, the same for one line, as `excerpt` numbers
    /// them.
    pub lines: Option<(i64, i64)>,
    pub resolved: bool,
    /// The lines it was left on have changed since.
    pub outdated: bool,
    /// The diff's last lines up to where it was left, as GitHub quotes them.
    pub excerpt: Vec<DiffLine>,
    pub comments: Vec<Note>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReviewState {
    #[default]
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

impl Branch {
    /// Whether it has commits the remote doesn't: new ones, or ones rewritten since they were
    /// pushed, or, when it has no remote branch, as after a rename, ones the remote has under
    /// the old name. One only behind its remote has none, though Git couldn't push it without
    /// force, and neither has one merged whose remote branch was deleted.
    pub fn can_push(&self) -> bool {
        (self.commits.iter()).any(|c| c.state == CommitState::Local)
            || (self.push == PushState::NeverPushed
                && (self.commits.iter()).any(|c| c.state == CommitState::Pushed))
    }

    /// What its header says of it against its remote, short of being in the target: what's
    /// to pull and to push, or that it was rewritten since it was pushed. Git can't push one
    /// behind its remote without force either, but pulling is what it needs.
    pub fn remote_state(&self) -> String {
        let to_push = self
            .commits
            .iter()
            .filter(|c| c.state == CommitState::Local)
            .count();
        match (self.push, self.incoming.len()) {
            (PushState::NeverPushed, _) => "never pushed".to_owned(),
            (PushState::NeedsForce, 0) => "rewritten, needs a force push".to_owned(),
            (_, 0) if to_push > 0 => format!("{to_push} to push"),
            (_, 0) => "on the remote".to_owned(),
            (_, pull) if to_push > 0 => format!("{pull} to pull, {to_push} to push"),
            (_, pull) => format!("{pull} to pull"),
        }
    }
}

/// A commit only a branch's remote has, as a teammate or an agent elsewhere pushed it.
#[derive(Debug, Clone)]
pub struct RemoteCommit {
    pub id: CommitId,
    /// Its change ID and authored hash, as a [`Commit`]'s: what its read mark goes by, which
    /// it keeps once pulled. Empty for the target's new commits, which aren't marked.
    pub key: String,
    pub authored: String,
    pub title: String,
    /// The rest of its message, after the title.
    pub body: String,
    /// Who it's shown by, as [`byline`] picks.
    pub author: String,
    /// Theirs, for their picture.
    pub email: String,
    /// The agent that wrote it, when `author` is the person who committed it.
    pub agent: Option<Agent>,
    /// When it was written, in seconds since the Unix epoch.
    pub time: i64,
    /// It has more than one parent, bringing in the commits of the others.
    pub merge: bool,
    /// The pull request a target's commit landed, as the project's page names it.
    pub review: Option<Review>,
}

/// A coding agent, or a bot, that wrote a commit a person committed.
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    pub name: String,
    pub email: String,
}

/// Who a commit is shown by, from its author's and its committer's name and email: its author,
/// unless that's an agent and the committer a person, who then leads, with the agent. Not the
/// committer in general: a rebase or a cherry-pick makes whoever ran it that.
pub(super) fn byline(
    author: (String, String),
    committer: (String, String),
) -> ((String, String), Option<Agent>) {
    if is_person(&author.0, &author.1) || !is_person(&committer.0, &committer.1) {
        return (author, None);
    }
    let agent = Agent {
        name: author.0,
        email: author.1,
    };
    (committer, Some(agent))
}

/// Whether `name` <`email`> is a person: not a coding agent or a bot, nor GitHub committing a
/// pull request merged on its website.
fn is_person(name: &str, email: &str) -> bool {
    let email = email.to_ascii_lowercase();
    !(name.ends_with("[bot]")
        || email.ends_with("[bot]@users.noreply.github.com")
        || email.ends_with("+copilot@users.noreply.github.com")
        || matches!(
            email.as_str(),
            "codex@openai.com"
                | "noreply@anthropic.com"
                | "cursoragent@cursor.com"
                | "noreply@github.com"
        ))
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: CommitId,
    /// Its change ID, which survives rebases and rewording. One made without, as by plain
    /// Git, has the one the engine writes into it on its first rebase, made from its sha.
    pub key: String,
    /// Its author, when they wrote it and its message, hashed: what any rebase keeps, Git's
    /// own too, which drops the change ID.
    pub authored: String,
    pub title: String,
    pub message: String,
    /// Who it's shown by, as [`byline`] picks: its author, or the person who committed what an
    /// agent wrote.
    pub author: String,
    /// Theirs, for their picture.
    pub email: String,
    /// The agent that wrote it, when `author` is the person who committed it.
    pub agent: Option<Agent>,
    /// Seconds since the Unix epoch.
    pub time: i64,
    pub state: CommitState,
    pub conflicted: bool,
    /// Its first parent and its tree: rewritten on the same parent with another tree, it
    /// was amended, rather than only rebased.
    pub parent: Option<CommitId>,
    pub tree: gix::ObjectId,
}

/// What [`Commit::authored`] is made of.
pub(super) fn authored(author: &gix::actor::Signature, message: &[u8]) -> String {
    let time = author.time.seconds.to_string();
    let parts: [&[u8]; 4] = [&author.name, &author.email, time.as_bytes(), message];
    format!("{:x}", md5::compute(parts.join(&0)))
}

impl Commit {
    /// Whether the commit carries its change ID, rather than `key` being made from its sha.
    pub fn has_change_id(&self) -> bool {
        let made = but_core::commit::Headers::synthetic_change_id_from_commit_id(self.id.0);
        self.key != made.to_string()
    }

    /// A local commit `id` with nothing else to it, its tree `id` too.
    #[cfg(test)]
    pub fn bare(id: gix::ObjectId) -> Self {
        Self {
            id: CommitId(id),
            key: String::new(),
            authored: String::new(),
            title: String::new(),
            message: String::new(),
            author: String::new(),
            email: String::new(),
            agent: None,
            time: 0,
            state: CommitState::Local,
            conflicted: false,
            parent: None,
            tree: id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Its size in bytes when it was read, beside `modified`, for telling whether it changed
    /// since: a write within a coarse timestamp's tick, or one that keeps the old time, moves
    /// only the size.
    pub size: Option<u64>,
    /// An uncommitted file a merge left conflicted, markers and all, as an update can when
    /// the target changed the same lines. It's kept out of commits until marked resolved.
    pub conflicted: bool,
    /// The lines it adds and removes, counted for uncommitted files while there are few
    /// enough to read them all.
    pub lines: Option<(u32, u32)>,
    /// Its diff, as read, is a copy a `textconv` filter made for reading, as of an encrypted or
    /// binary file: its hunks aren't the file's own lines, so it's taken whole or not at all.
    pub converted: bool,
    /// What the backend needs to diff or move this change again.
    pub(crate) change: but_core::TreeChange,
}

impl FileChange {
    /// Whether it can only be taken whole, as GitButler can't take some of its hunks: it's
    /// added or deleted, a side of it isn't a plain file, as a link isn't, or it's `converted`.
    pub fn whole_only(&self) -> bool {
        use gix::object::tree::EntryKind as K;
        let plain =
            |state: &but_core::ChangeState| matches!(state.kind, K::Blob | K::BlobExecutable);
        self.converted
            || match &self.change.status {
                but_core::TreeStatus::Modification {
                    previous_state,
                    state,
                    ..
                }
                | but_core::TreeStatus::Rename {
                    previous_state,
                    state,
                    ..
                } => !plain(previous_state) || !plain(state),
                _ => true,
            }
    }

    /// What changed about the file other than its lines: its executable bit, or whether
    /// it's a link.
    pub fn mode_change(&self) -> Option<&'static str> {
        use but_core::{ModeFlags as F, TreeStatus as S};
        let status = &self.change.status;
        let flags = match status {
            S::Modification { flags, .. } | S::Rename { flags, .. } => *flags,
            _ => None,
        };
        // A link's one line is where it points, which reads like a file's content otherwise.
        let link = (status
            .state()
            .or(status.previous_state_and_path().map(|(state, _)| state)))
        .is_some_and(|state| state.kind == gix::object::tree::EntryKind::Link);
        Some(match flags {
            Some(F::ExecutableBitAdded) => "Made executable",
            Some(F::ExecutableBitRemoved) => "No longer executable",
            Some(F::TypeChangeFileToLink) => "Turned into a symbolic link",
            Some(F::TypeChangeLinkToFile) => "Turned from a symbolic link into a file",
            Some(F::TypeChange) => "Changed type",
            None if link => "Symbolic link",
            None => return None,
        })
    }

    /// A repository cloned into the worktree and not yet recorded, which has no commit read
    /// for it: committed, it would go in as a submodule with nowhere to clone it from, as
    /// `git add` warns.
    pub fn nested_repository(&self) -> bool {
        matches!(&self.change.status, but_core::TreeStatus::Addition { state, .. }
            if state.id.is_null() && state.kind == gix::object::tree::EntryKind::Commit)
    }

    /// Where a submodule points now and pointed before, which is all there is to show of one.
    pub fn submodule(&self) -> Option<String> {
        use but_core::TreeStatus as S;
        let at = |state: &but_core::ChangeState| {
            (state.kind == gix::object::tree::EntryKind::Commit).then(|| CommitId(state.id).short())
        };
        if self.nested_repository() {
            return Some("A repository of its own, not a submodule".to_owned());
        }
        Some(match &self.change.status {
            S::Addition { state, .. } => format!("Submodule at {}", at(state)?),
            S::Deletion { previous_state } => {
                format!("Removed submodule at {}", at(previous_state)?)
            }
            S::Modification {
                previous_state,
                state,
                ..
            }
            | S::Rename {
                previous_state,
                state,
                ..
            } => match (at(previous_state), at(state)?) {
                (Some(before), now) => format!("Submodule moved from {before} to {now}"),
                (None, now) => format!("Submodule at {now}"),
            },
        })
    }

    /// What the change is against: the file as committed, or nothing for an addition.
    /// Committing part of a file moves it without touching the file itself.
    pub fn base(&self) -> Option<gix::ObjectId> {
        (self.change.status.previous_state_and_path()).map(|(state, _)| state.id)
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
    /// The most lines its hunks' scopes could add when annotated, `None` with scopes off. A
    /// load with another limit annotates it again.
    pub scope_lines: Option<usize>,
    /// An SVG's two sides drawn, shown in place of its lines unless those are asked for.
    pub drawing: Option<Drawing>,
    /// Which of several commits shown one after another it's from, as for commits picked
    /// with others between them.
    pub commit: Option<CommitId>,
}

/// An SVG file's sides drawn as images are shown; a side is missing for an added or
/// deleted file.
#[derive(Debug, Clone)]
pub struct Drawing {
    pub before: Option<Picture>,
    pub after: Option<Picture>,
}

/// An image a diff shows, decoded or drawn, and the size it's drawn at, in points.
#[derive(Debug, Clone)]
pub struct Picture {
    pub image: std::sync::Arc<gpui::RenderImage>,
    pub width: f32,
    pub height: f32,
}

impl FileDiff {
    /// `hunks` of it to take, or all of it with `None`. All the hunks of a file that can only
    /// be taken whole are the file whole.
    pub fn part(&self, hunks: Option<Vec<HunkKey>>) -> Selected {
        let all = |keys: &Vec<HunkKey>| (self.hunks().iter()).all(|h| keys.contains(&h.key));
        Selected {
            file: self.file.clone(),
            hunks: hunks.filter(|keys| !(self.file.whole_only() && all(keys))),
        }
    }

    /// The images it shows: an image file's sides, or an SVG's drawn.
    pub fn pictures(&self) -> impl Iterator<Item = &Picture> {
        let image = match &self.body {
            DiffBody::Image { before, after } => [before.as_ref(), after.as_ref()],
            _ => [None, None],
        };
        let drawn = (self.drawing.as_ref()).map_or([None, None], |drawn| {
            [drawn.before.as_ref(), drawn.after.as_ref()]
        });
        image.into_iter().chain(drawn).flatten()
    }

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
        before: Option<Picture>,
        after: Option<Picture>,
    },
    TooLarge {
        bytes: u64,
    },
    /// Mode changes and the like, with no lines to show.
    Empty,
    /// A conflict the app can't split into lines to pick between, like a file one side
    /// deleted: why, and the command that resolves it.
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
    pub new_base_text: bstr::BString,
    pub commit_text: bstr::BString,
    /// What a merge's markers call the two sides, for conflicts it left in a file.
    pub labels: [String; 2],
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub key: HunkKey,
    pub header: String,
    pub lines: Vec<DiffLine>,
    /// The scopes the hunk changes, outermost first, each as [`scope_name`] gives its first
    /// source line.
    pub scope: Vec<String>,
    /// Unchanged lines before and after `lines`, out to the edges of the innermost scope it
    /// changes, when that's only a few: shown, never acted on.
    pub lead: Vec<DiffLine>,
    pub tail: Vec<DiffLine>,
}

/// A scope as a hunk names it: its first source `line`, without the indent or the brace
/// that opens its body.
pub fn scope_name(line: &str) -> &str {
    line.trim().trim_end_matches('{').trim_end()
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

    /// The runs `picked` gives, each against this hunk's other side: what taking those lines
    /// out of where they are means, to an uncommit, a move or a discard, as against adding
    /// them to a commit.
    pub fn picked_out(&self, picks: &std::collections::BTreeSet<usize>) -> Vec<HunkKey> {
        let whole = self.key.0;
        (self.picked(picks).into_iter())
            .map(|HunkKey(run)| {
                HunkKey(match run.old_lines {
                    0 => but_core::HunkHeader {
                        old_start: whole.old_start,
                        old_lines: whole.old_lines,
                        ..run
                    },
                    _ => but_core::HunkHeader {
                        new_start: whole.new_start,
                        new_lines: whole.new_lines,
                        ..run
                    },
                })
            })
            .collect()
    }
}

/// Identifies a hunk of a file's uncommitted changes, to commit just that part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HunkKey(pub(crate) but_core::HunkHeader);

impl HunkKey {
    /// Whether this key, of `hunk` or of part of it, takes `line` of it: all of its lines for
    /// the whole hunk; else the changed lines of the side it picks, named alone as a commit
    /// takes them, or against the whole of the hunk's other side as a discard does.
    pub fn takes(&self, hunk: &Hunk, line: &DiffLine) -> bool {
        let (key, whole) = (self.0, hunk.key.0);
        if key == whole {
            return true;
        }
        let within = |n: Option<u32>, start: u32, lines: u32| {
            n.is_some_and(|n| n >= start && n < start + lines)
        };
        let old_side = (key.new_start, key.new_lines) == (0, 0)
            || (key.new_start, key.new_lines) == (whole.new_start, whole.new_lines);
        let new_side = (key.old_start, key.old_lines) == (0, 0)
            || (key.old_start, key.old_lines) == (whole.old_start, whole.old_lines);
        match line.kind {
            LineKind::Context => false,
            LineKind::Removed => old_side && within(line.old, key.old_start, key.old_lines),
            LineKind::Added => new_side && within(line.new, key.new_start, key.new_lines),
        }
    }
}

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

impl DiffLine {
    /// A line without highlighting or marked words.
    pub fn new(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> Self {
        Self {
            kind,
            old,
            new,
            text: text.into(),
            spans: Vec::new(),
            emph: Vec::new(),
        }
    }
}

pub use super::highlight::Token;

/// A file's changes, or some of its hunks, picked for an operation.
#[derive(Debug, Clone)]
pub struct Selected {
    pub file: FileChange,
    /// `None` takes the whole file.
    pub hunks: Option<Vec<HunkKey>>,
}

impl Selected {
    /// All of `file`.
    pub fn whole(file: &FileChange) -> Self {
        Self {
            file: file.clone(),
            hunks: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[cfg(test)]
mod tests {
    use crate::backend::model::build::full;
    use std::collections::BTreeSet;

    use but_core::HunkHeader;

    use super::{
        Branch, Commit, CommitId, CommitState, DiffLine, Hunk, HunkKey, LineKind, PushState,
        RemoteCommit,
    };

    #[test]
    fn a_symbolic_link_says_so_where_its_target_reads_like_content() {
        let link = but_core::ChangeState {
            id: gix::ObjectId::null(gix::hash::Kind::Sha1),
            kind: gix::object::tree::EntryKind::Link,
        };
        let retargeted = crate::backend::file_change(but_core::TreeChange {
            path: "assets/current".into(),
            status: but_core::TreeStatus::Modification {
                previous_state: link,
                state: link,
                flags: None,
            },
        });
        assert_eq!(retargeted.mode_change(), Some("Symbolic link"));
    }

    #[test]
    fn a_repository_cloned_inside_is_told_apart_from_a_submodule() {
        let added = |id| {
            crate::backend::file_change(but_core::TreeChange {
                path: "scratch/sim".into(),
                status: but_core::TreeStatus::Addition {
                    state: but_core::ChangeState {
                        id,
                        kind: gix::object::tree::EntryKind::Commit,
                    },
                    is_untracked: true,
                },
            })
        };
        let cloned = added(gix::ObjectId::null(gix::hash::Kind::Sha1));
        assert!(cloned.nested_repository(), "status reads no commit for it");
        assert_eq!(
            cloned.submodule().as_deref(),
            Some("A repository of its own, not a submodule")
        );
        let added_as_submodule = added(gix::ObjectId::empty_tree(gix::hash::Kind::Sha1));
        assert!(
            !added_as_submodule.nested_repository(),
            "`git submodule add` records its commit"
        );
    }

    fn header(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> HunkHeader {
        HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        }
    }

    #[test]
    fn lines_picked_out_name_the_hunk_they_leave() {
        let line = |kind, old, new| DiffLine::new(kind, old, new, "");
        let hunk = Hunk {
            key: HunkKey(header(1, 3, 1, 3)),
            header: String::new(),
            lines: vec![
                line(LineKind::Context, Some(1), Some(1)),
                line(LineKind::Removed, Some(2), None),
                line(LineKind::Added, None, Some(2)),
                line(LineKind::Context, Some(3), Some(3)),
            ],
            scope: Vec::new(),
            lead: Vec::new(),
            tail: Vec::new(),
        };
        assert_eq!(
            hunk.picked_out(&BTreeSet::from([1, 2])),
            vec![HunkKey(header(2, 1, 1, 3)), HunkKey(header(1, 3, 2, 1))],
            "each side's run stands against the whole of the other side, as a discard reads it"
        );
        let taken = |key: HunkKey| -> Vec<bool> {
            hunk.lines
                .iter()
                .map(|line| key.takes(&hunk, line))
                .collect()
        };
        assert_eq!(
            taken(hunk.picked_out(&BTreeSet::from([2]))[0]),
            [false, false, true, false],
            "a picked line, against the other side, takes just itself"
        );
        assert_eq!(
            taken(hunk.picked(&BTreeSet::from([1]))[0]),
            [false, true, false, false],
            "and so does one named alone"
        );
        assert_eq!(taken(hunk.key), [true; 4], "the whole hunk takes all of it");
    }

    #[test]
    fn picked_lines_become_one_sided_runs() {
        let line = |kind, old, new| DiffLine::new(kind, old, new, "");
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
            lead: Vec::new(),
            tail: Vec::new(),
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

    #[test]
    fn a_branch_says_what_it_needs_against_its_remote() {
        let id = gix::ObjectId::from_bytes_or_panic(&[1; 20]);
        let branch = |push: PushState, states: &[CommitState], incoming: usize| Branch {
            name: "feat".into(),
            ref_name: Some(full("refs/heads/feat")),
            commits: (states.iter())
                .map(|state| Commit {
                    state: *state,
                    ..Commit::bare(id)
                })
                .collect(),
            incoming: vec![
                RemoteCommit {
                    id: CommitId(id),
                    key: String::new(),
                    authored: String::new(),
                    title: String::new(),
                    body: String::new(),
                    author: String::new(),
                    email: String::new(),
                    agent: None,
                    time: 0,
                    merge: false,
                    review: None,
                };
                incoming
            ],
            remote: None,
            push,
            review: None,
        };
        use CommitState::{Integrated, Local, Pushed};
        for (push, states, incoming, said, can_push, why) in [
            (
                PushState::NothingToPush,
                &[Pushed][..],
                0,
                "on the remote",
                false,
                "in step",
            ),
            (
                PushState::Unpushed,
                &[Local, Pushed],
                0,
                "1 to push",
                true,
                "ahead",
            ),
            (
                PushState::NeedsForce,
                &[Pushed, Pushed],
                2,
                "2 to pull",
                false,
                "only behind",
            ),
            (
                PushState::NeedsForce,
                &[Local, Pushed],
                2,
                "2 to pull, 1 to push",
                true,
                "diverged",
            ),
            (
                PushState::NeedsForce,
                &[Local],
                0,
                "rewritten, needs a force push",
                true,
                "rewritten",
            ),
            (
                PushState::NeverPushed,
                &[Pushed],
                0,
                "never pushed",
                true,
                "renamed since it was pushed, its commits there under the old name",
            ),
            (
                PushState::NeverPushed,
                &[Integrated],
                0,
                "never pushed",
                false,
                "merged, its remote branch deleted",
            ),
        ] {
            let branch = branch(push, states, incoming);
            assert_eq!(branch.remote_state(), said, "{why}");
            assert_eq!(
                branch.can_push(),
                can_push,
                "{why}: whether it has commits to send"
            );
        }
    }

    #[test]
    fn an_agents_commit_is_shown_by_the_person_who_committed_it() {
        let person = |name: &str, email: &str| (name.to_owned(), email.to_owned());
        let (codex, byron) = (
            person("Codex GPT-6", "codex@openai.com"),
            person("Byron", "b@x.io"),
        );
        let ((name, _), agent) = super::byline(codex.clone(), byron);
        assert_eq!(name, "Byron", "the person leads");
        assert_eq!(
            agent.map(|a| a.name).as_deref(),
            Some("Codex GPT-6"),
            "with the agent"
        );

        let (scott, kiril) = (person("Scott", "s@x.io"), person("Kiril", "k@x.io"));
        let ((name, _), agent) = super::byline(scott, kiril);
        assert!(
            name == "Scott" && agent.is_none(),
            "a person's commit stays theirs, rebased or not"
        );

        let web = person("GitHub", "noreply@github.com");
        let ((name, _), agent) = super::byline(codex, web);
        assert!(
            name == "Codex GPT-6" && agent.is_none(),
            "a merge on GitHub's website isn't a person"
        );
    }

    #[test]
    fn a_commit_made_by_plain_git_has_no_change_id_of_its_own() {
        let id = gix::ObjectId::from_hex(b"99a9648a9b2cf5a631d8ef586f97005604afaabc").unwrap();
        let made = but_core::commit::Headers::synthetic_change_id_from_commit_id(id);
        let commit = |key: String| Commit {
            key,
            ..Commit::bare(id)
        };
        assert!(
            !commit(made.to_string()).has_change_id(),
            "its key is the one made from its sha, which there's nothing to copy of"
        );
        assert!(
            commit("vtlrvnlmsxrmrsnrkvqlwrzqokpsoxzk".into()).has_change_id(),
            "it carries one"
        );
    }

    #[test]
    fn force_pushing_the_target_branch_asks_rather_than_refuses() {
        let id = gix::ObjectId::from_bytes_or_panic(&[1; 20]);
        let main = Branch {
            name: "main".into(),
            ref_name: Some(full("refs/heads/main")),
            commits: vec![Commit {
                state: CommitState::Local,
                ..Commit::bare(id)
            }],
            incoming: Vec::new(),
            remote: None,
            push: PushState::NeedsForce,
            review: None,
        };
        let ws = super::Workspace {
            lanes: vec![super::Lane {
                branches: vec![main],
                checkout: super::Checkout::Main,
                rests_on: None,
            }],
            target_branch: Some(full("refs/heads/main")),
            target: Some(super::Target {
                name: "origin/main".into(),
                branch: "main".into(),
                local: false,
                untracked: None,
                behind: 1,
                commits_behind: 1,
                needs_update: true,
                incoming: Vec::new(),
            }),
            ..super::Workspace::default()
        };
        assert_eq!(
            ws.push_refused(&full("refs/heads/main")),
            None,
            "rewriting your own main and forcing it is allowed"
        );
        assert!(
            ws.forced_target(&full("refs/heads/main")).is_some(),
            "though it always asks first, as others' work builds on it"
        );
    }

    #[test]
    fn activity_ahead_of_the_clock_is_left_out() {
        let now = crate::ui::now();
        let written = |path, at: i64| super::FileChange {
            modified: Some(at * 1000),
            ..super::FileChange::bare(path)
        };
        let ws = super::Workspace {
            uncommitted: vec![written("a", now - 60), written("b", now + 3600)],
            ..super::Workspace::default()
        };
        assert_eq!(
            ws.last_activity(),
            Some(now - 60),
            "a time in the future is a clock that's off, not activity"
        );
    }
}
