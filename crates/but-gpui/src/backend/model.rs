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
    /// When Git last recorded something done in it, in seconds since the epoch: a commit,
    /// checkout or rebase, or the index changing, in its main checkout or a linked one.
    pub last_active: Option<i64>,
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
    pub stacks: Vec<Stack>,
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
    pub target_branch: Option<String>,
    /// The Git command a rebase, merge, cherry-pick or revert that stopped partway was
    /// started with, like `git rebase`, to finish it with.
    pub git_operation: Option<&'static str>,
    /// `HEAD` is detached or has no commits yet, which GitButler's history can't record or
    /// go back to: what's done in the main checkout can't be undone.
    pub unrecorded: bool,
    /// The commit checked out to edit, while the project is in edit mode. There are no
    /// stacks to show then.
    pub editing: Option<super::edit::Editing>,
    /// Its linked worktrees, like ones agents work in, when GitButler's worktree support is
    /// on: newest first, archived ones left out.
    pub worktrees: Vec<Worktree>,
    /// The names of the worktrees archived, to bring back.
    pub archived_worktrees: Vec<std::sync::Arc<str>>,
    /// A fingerprint of the repository's local and remote branches and of the pull requests
    /// the forge cache knows, which the Branches page lists again when it changes.
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
    pub name: std::sync::Arc<str>,
    pub path: std::path::PathBuf,
    /// The branch it has checked out, or `None` for a detached `HEAD`.
    pub branch: Option<String>,
    /// The branches of its lane, from the one checked out down to where it started, without
    /// the commits the target already has: what was committed in it.
    pub branches: Vec<Branch>,
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
    /// The last commit of the bottom branch of its lane, which a branch moves below to go
    /// under the worktree's branches. `None` when that branch has no commits of its own.
    pub fn lane_end(&self) -> Option<CommitId> {
        let bottom = (self.branches.iter().rev()).find(|branch| !branch.ref_name.is_empty())?;
        bottom.commits.last().map(|commit| commit.id)
    }

    /// When one of its uncommitted files was last written, in seconds since the epoch.
    pub fn last_activity(&self) -> Option<i64> {
        (self.uncommitted.iter())
            .filter_map(|file| file.modified.map(|ms| ms / 1000))
            .max()
    }
}

impl Workspace {
    /// The workspace's own branches, its stacks', which commits and moves go to.
    pub fn branches(&self) -> impl Iterator<Item = &Branch> {
        self.stacks.iter().flat_map(|stack| &stack.branches)
    }

    /// Every branch shown: the workspace's, then its linked worktrees'.
    fn shown_branches(&self) -> impl Iterator<Item = &Branch> {
        (self.branches()).chain(self.worktrees.iter().flat_map(|wt| &wt.branches))
    }

    /// Every commit shown, its linked worktrees' too.
    pub fn commits(&self) -> impl Iterator<Item = &Commit> {
        self.shown_branches().flat_map(|branch| &branch.commits)
    }

    pub fn find_commit(&self, id: CommitId) -> Option<(&Branch, &Commit)> {
        self.shown_branches().find_map(|branch| {
            branch
                .commits
                .iter()
                .find(|commit| commit.id == id)
                .map(|commit| (branch, commit))
        })
    }

    /// The commits only shown branches' remotes have.
    pub fn remote_commits(&self) -> impl Iterator<Item = &RemoteCommit> {
        self.shown_branches().flat_map(|branch| &branch.incoming)
    }

    /// The commits only a remote has: branch `ref_name`'s remote, or with `None` the
    /// target's that the workspace doesn't have yet, newest first.
    pub fn upstream(&self, ref_name: Option<&str>) -> &[RemoteCommit] {
        match ref_name {
            Some(ref_name) => self.find_branch(ref_name).map_or(&[], |b| &b.incoming),
            None => self.target.as_ref().map_or(&[], |t| &t.incoming),
        }
    }

    pub fn find_branch(&self, ref_name: &str) -> Option<&Branch> {
        self.shown_branches()
            .find(|branch| branch.ref_name == ref_name)
    }

    pub fn find_worktree(&self, name: &str) -> Option<&Worktree> {
        self.worktrees
            .iter()
            .find(|worktree| &*worktree.name == name)
    }

    /// The uncommitted files of the linked worktree `name`, or of the main checkout.
    pub fn uncommitted_in(&self, name: Option<&str>) -> Option<&[FileChange]> {
        match name {
            None => Some(&self.uncommitted),
            Some(name) => Some(&self.find_worktree(name)?.uncommitted),
        }
    }

    /// The branches of each stack, then of each linked worktree's lane, top first.
    pub fn lanes(&self) -> impl Iterator<Item = &[Branch]> {
        (self.stacks.iter().map(|stack| &stack.branches[..]))
            .chain(self.worktrees.iter().map(|wt| &wt.branches[..]))
    }

    /// The branches of the stack, or linked worktree's lane, whose bottom branch is `bottom`.
    pub fn lane(&self, bottom: &str) -> Option<&[Branch]> {
        self.lanes().find(|branches| {
            branches
                .last()
                .is_some_and(|branch| branch.ref_name == bottom)
        })
    }

    /// The lane holding the branch `ref_name`, a stack's or a linked worktree's, and the
    /// branch's place in it from the top.
    pub fn lane_of(&self, ref_name: &str) -> Option<(&[Branch], usize)> {
        self.lanes().find_map(|lane| {
            let at = lane.iter().position(|b| b.ref_name == ref_name)?;
            Some((lane, at))
        })
    }

    /// The stack holding the branch `ref_name`, and the branch's place in it from the top.
    pub fn stack_of(&self, ref_name: &str) -> Option<(&Stack, usize)> {
        self.stacks.iter().find_map(|stack| {
            let at = stack.branches.iter().position(|b| b.ref_name == ref_name)?;
            Some((stack, at))
        })
    }

    /// `ref_name`'s branch and the ones below it in its stack, which pushing it pushes too.
    fn pushed_with(&self, ref_name: &str) -> &[Branch] {
        self.stack_of(ref_name)
            .map_or(&[], |(stack, at)| &stack.branches[at..])
    }

    /// Whether pushing `ref_name` has commits to send. An unnamed branch has no ref to push.
    pub fn can_push(&self, ref_name: &str) -> bool {
        !ref_name.is_empty() && self.pushed_with(ref_name).iter().any(Branch::can_push)
    }

    /// The commits only the remote has that pushing `ref_name` drops: a forced push replaces
    /// what's there.
    pub fn push_drops(&self, ref_name: &str) -> Vec<CommitId> {
        if !self.needs_force(ref_name) {
            return Vec::new();
        }
        (self.pushed_with(ref_name).iter())
            .flat_map(|b| b.incoming.iter().map(|commit| commit.id))
            .collect()
    }

    /// Why `ref_name` can't be pushed yet, though it has commits to send: pushing goes to
    /// the remote of the target's upstream, and a conflicted commit would go along.
    pub fn push_refused(&self, ref_name: &str) -> Option<&'static str> {
        let pushed = self.pushed_with(ref_name);
        if self.target.is_none() && self.target_branch.is_some() {
            Some("The target branch isn't on the remote yet: push it once, with `git push -u`")
        } else if self.target.is_none() {
            Some("Pick the project's target branch first")
        } else if (pushed.iter()).any(|b| b.commits.iter().any(|c| c.conflicted)) {
            Some("Resolve the conflicted commits first")
        } else {
            None
        }
    }

    /// Whether pushing `ref_name` has to force: it or a branch below it was rewritten since
    /// it was pushed.
    pub fn needs_force(&self, ref_name: &str) -> bool {
        (self.pushed_with(ref_name).iter()).any(|branch| branch.push == PushState::NeedsForce)
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
        // Linked worktrees' too, as an agent committing in one clears its uncommitted files.
        let committed = self
            .shown_branches()
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
    /// The CI checks on the branch, once any have been seen.
    pub checks: Option<Checks>,
    /// Each check, failed ones first, then those still running.
    pub check_runs: Vec<CheckRun>,
    pub labels: Vec<String>,
    /// Who's been asked to review it, bots left out. GitHub lists only those who haven't
    /// reviewed yet; GitLab and Bitbucket keep the others.
    pub reviewers: Vec<String>,
    pub author: Option<String>,
    /// The branch it merges into, as the forge names it.
    pub target_branch: String,
    /// Its description, as written.
    pub body: String,
    /// When it last changed on the forge, in seconds since the epoch.
    pub updated_at: Option<i64>,
    /// When the forge was last asked about it, in seconds since the epoch.
    pub synced_at: Option<i64>,
}

/// A CI check on a pull request's branch.
#[derive(Debug, Clone)]
pub struct CheckRun {
    pub name: String,
    /// Where to read its log.
    pub url: String,
    /// Whether it failed, once it finished.
    pub failed: Option<bool>,
    /// How it finished or how far it got, as the forge says: "success", "in progress".
    pub status: String,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
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
    /// pushed. One only behind its remote has none, though Git couldn't push it without force.
    pub fn can_push(&self) -> bool {
        self.commits.iter().any(|c| c.state == CommitState::Local)
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
pub fn byline(
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
pub fn authored(author: &gix::actor::Signature, message: &[u8]) -> String {
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
    /// The lines it adds and removes, counted for uncommitted files while there are few
    /// enough to read them all.
    pub lines: Option<(u32, u32)>,
    /// What the backend needs to diff or move this change again.
    pub(crate) change: but_core::TreeChange,
}

impl FileChange {
    /// Whether it can only be taken whole, as GitButler can't take some of its hunks: it's
    /// added or deleted, or a side of it isn't a plain file, as a link isn't.
    pub fn whole_only(&self) -> bool {
        use gix::object::tree::EntryKind as K;
        let plain =
            |state: &but_core::ChangeState| matches!(state.kind, K::Blob | K::BlobExecutable);
        match &self.change.status {
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
        let (S::Modification { flags, .. } | S::Rename { flags, .. }) = &self.change.status else {
            return None;
        };
        Some(match (*flags)? {
            F::ExecutableBitAdded => "Made executable",
            F::ExecutableBitRemoved => "No longer executable",
            F::TypeChangeFileToLink => "Turned into a symbolic link",
            F::TypeChangeLinkToFile => "Turned from a symbolic link into a file",
            F::TypeChange => "Changed type",
        })
    }

    /// Where a submodule points now and pointed before, which is all there is to show of one.
    pub fn submodule(&self) -> Option<String> {
        use but_core::TreeStatus as S;
        let at = |state: &but_core::ChangeState| {
            (state.kind == gix::object::tree::EntryKind::Commit).then(|| CommitId(state.id).short())
        };
        Some(match &self.change.status {
            // A repository added in the worktree, not yet recorded, has no commit read for it.
            S::Addition { state, .. } if state.id.is_null() => {
                at(state).map(|_| "New submodule".to_owned())?
            }
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
    /// What a merge's markers call the two sides, for conflicts it left in a file.
    pub labels: [String; 2],
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub key: HunkKey,
    pub header: String,
    pub lines: Vec<DiffLine>,
    /// The scopes the hunk changes, outermost first, each as its first source line.
    pub scope: Vec<String>,
    /// Unchanged lines before and after `lines`, out to the edges of the innermost scope it
    /// changes, when that's only a few: shown, never acted on.
    pub lead: Vec<DiffLine>,
    pub tail: Vec<DiffLine>,
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
    use std::collections::BTreeSet;

    use but_core::HunkHeader;

    use super::{
        Branch, Commit, CommitId, CommitState, DiffLine, Hunk, HunkKey, LineKind, PushState,
        RemoteCommit,
    };

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
        let line = |kind, old, new| DiffLine {
            kind,
            old,
            new,
            text: "".into(),
            spans: Vec::new(),
            emph: Vec::new(),
        };
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
            ref_name: "refs/heads/feat".into(),
            commits: (states.iter())
                .map(|state| Commit {
                    id: CommitId(id),
                    key: String::new(),
                    authored: String::new(),
                    title: String::new(),
                    message: String::new(),
                    author: String::new(),
                    email: String::new(),
                    agent: None,
                    time: 0,
                    state: *state,
                    conflicted: false,
                    parent: None,
                    tree: id,
                })
                .collect(),
            incoming: vec![
                RemoteCommit {
                    id: CommitId(id),
                    key: String::new(),
                    authored: String::new(),
                    title: String::new(),
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
        use CommitState::{Local, Pushed};
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
        let ((name, _), agent) = super::byline(codex.clone(), byron.clone());
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
            id: CommitId(id),
            key,
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
}
