//! What the user has looked at: the commits seen, so new work by agents stands out.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use gpui::Context;

use super::{ProjectId, Selection, Store};
use crate::backend::{Branch, Commit, CommitId, RemoteCommit, Workspace};

/// What the user has looked at, so new work by agents stands out. Per project id.
#[derive(Default, Clone, Serialize, Deserialize)]
pub(super) struct Seen {
    /// What the commits that have been looked at go by: [`Commit::key`] and
    /// [`Commit::authored`].
    pub(super) commits: HashMap<String, HashSet<String>>,
}

/// How long a commit's rows glow once it shows up.
pub const GLOW: Duration = Duration::from_millis(2400);

/// A commit that can be read: a branch's own, or one only its remote has.
pub trait Seeable {
    fn id(&self) -> CommitId;
    /// What it goes by among those seen: its change ID, and what a rebase by Git, which
    /// drops that, keeps.
    fn ids(&self) -> [&str; 2];
}

impl Seeable for Commit {
    fn id(&self) -> CommitId {
        self.id
    }
    fn ids(&self) -> [&str; 2] {
        [&self.key, &self.authored]
    }
}

impl Seeable for RemoteCommit {
    fn id(&self) -> CommitId {
        self.id
    }
    fn ids(&self) -> [&str; 2] {
        [&self.key, &self.authored]
    }
}

/// Every commit `workspace` shows, and those only its branches' remotes have, by what each
/// goes by among those seen.
pub(super) fn shown(workspace: &Workspace) -> impl Iterator<Item = (CommitId, [&str; 2])> {
    let local = workspace.commits().map(|commit| (commit.id, commit.ids()));
    local.chain((workspace.remote_commits()).map(|commit| (commit.id, commit.ids())))
}

/// The commit a branch's or a stack's page shows, when it's their only one: their diff is
/// just its changes, so opening them reads it.
pub(super) fn lone_commit(workspace: &Workspace, selection: &Selection) -> Option<CommitId> {
    let branches = match selection {
        Selection::Branch(_, ref_name) => {
            std::slice::from_ref(workspace.find_branch(ref_name.as_ref())?)
        }
        Selection::Stack(_, bottom) => &workspace.lane(bottom.as_ref())?.branches,
        _ => return None,
    };
    let mut commits = branches.iter().flat_map(|branch| &branch.commits);
    let lone = commits.next()?;
    commits.next().is_none().then_some(lone.id)
}

/// Whether a commit going by `ids` was seen, by either.
fn was_seen(seen: &HashSet<String>, ids: [&str; 2]) -> bool {
    ids.iter().any(|id| seen.contains(*id))
}

impl Store {
    pub fn is_unread(&self, project: &str, commit: &impl Seeable) -> bool {
        self.seen
            .commits
            .get(project)
            .is_some_and(|seen| !was_seen(seen, commit.ids()))
    }

    /// Whether commit `id` showed up unread in the last [`GLOW`], so the rows listing it glow
    /// once: an agent just made it.
    pub fn glows(&self, id: CommitId) -> bool {
        (self.arrived.get(&id)).is_some_and(|at| at.elapsed() < GLOW)
    }

    /// Note when the unread commits `workspace` brings to project `id` showed up: those it
    /// didn't have by either id, so a rebase or an amend isn't news here, nor anything the
    /// first time it loads.
    pub(super) fn note_arrivals(&mut self, id: &ProjectId, workspace: &Workspace) {
        self.arrived.retain(|_, at| at.elapsed() < GLOW);
        let Some(old) = self.workspace(id) else {
            return;
        };
        let had: HashSet<&str> = old.commits().flat_map(Seeable::ids).collect();
        let new: Vec<CommitId> = (workspace.commits())
            .filter(|commit| !commit.ids().iter().any(|key| had.contains(key)))
            .filter(|commit| self.is_unread(id, *commit))
            .map(|commit| commit.id)
            .collect();
        let now = Instant::now();
        self.arrived
            .extend(new.into_iter().map(|commit| (commit, now)));
    }

    /// Whether any of `branch`'s commits is unread, or any only its remote has.
    pub fn branch_unread(&self, project: &str, branch: &Branch) -> bool {
        (branch.commits.iter()).any(|commit| self.is_unread(project, commit))
            || (branch.incoming.iter()).any(|commit| self.is_unread(project, commit))
    }

    /// Count `ids` of `project` as seen, as opening them does, or with `read` false as not
    /// seen yet, to come back to.
    pub fn mark_read(
        &mut self,
        project: &ProjectId,
        ids: impl IntoIterator<Item = CommitId>,
        read: bool,
        cx: &mut Context<Self>,
    ) {
        let ids: HashSet<CommitId> = ids.into_iter().collect();
        let keys: Vec<String> = (self.workspace(project).into_iter())
            .flat_map(shown)
            .filter(|(id, _)| ids.contains(id))
            .flat_map(|(_, keys)| keys.map(str::to_owned))
            .collect();
        let seen = self.seen.commits.entry(project.to_string()).or_default();
        match read {
            true => seen.extend(keys),
            false => keys.iter().for_each(|key| {
                seen.remove(key);
            }),
        }
        self.save_soon(cx);
        cx.notify();
    }

    /// What marking `selection` read does, as its right-click menu offers: the commits it
    /// marks, and whether read. A commit, the picks it's among, or a branch remote's commit
    /// turn read, or unread when they all are already; a branch or a project has its unread
    /// commits, and its remotes', turn read. None with nothing to mark.
    pub fn read_marks_of(&self, selection: &Selection) -> Option<(Vec<CommitId>, bool)> {
        let project = selection.project();
        let workspace = self.workspace(project)?;
        let (ids, toggles): (HashSet<CommitId>, bool) = match selection {
            Selection::Commit(_, id) => (
                self.with_picks(project, *id).iter().copied().collect(),
                true,
            ),
            Selection::Upstream(_, Some(_), id) => ([*id].into(), true),
            Selection::Branch(..) => {
                let branch = self.branch_for(selection)?;
                let incoming = branch.incoming.iter().map(|commit| commit.id);
                let ids = branch
                    .commits
                    .iter()
                    .map(|commit| commit.id)
                    .chain(incoming);
                (ids.collect(), false)
            }
            Selection::Project(_) => (shown(workspace).map(|(id, _)| id).collect(), false),
            _ => return None,
        };
        let seen = self.seen.commits.get(project.as_ref())?;
        let unread: Vec<CommitId> = (shown(workspace))
            .filter(|(id, keys)| ids.contains(id) && !was_seen(seen, *keys))
            .map(|(id, _)| id)
            .collect();
        match unread.is_empty() {
            false => Some((unread, true)),
            true => toggles.then(|| (ids.into_iter().collect(), false)),
        }
    }

    /// Mark `selection` read, or unread, as [`Self::read_marks_of`] says.
    pub fn toggle_read(&mut self, selection: &Selection, cx: &mut Context<Self>) {
        if let Some((ids, read)) = self.read_marks_of(selection) {
            self.mark_read(selection.project(), ids, read, cx);
        }
    }

    /// Bring what's been seen of `id` up to date with the `workspace` it just loaded: what's
    /// new is news unless the app made it, and what's gone is forgotten once there's much of
    /// it. `fresh` is false for a load that started before the app's operation finished,
    /// which the reload after it still compares with what was there before. Says whether
    /// what's seen changed, as it's saved then.
    pub(super) fn update_seen(
        &mut self,
        id: &ProjectId,
        workspace: &Workspace,
        fresh: bool,
    ) -> bool {
        let amended = self.amended_since_last_load(id, workspace);
        let commits: HashSet<String> = (shown(workspace))
            .flat_map(|(_, keys)| keys.map(str::to_owned))
            .collect();
        let first_sight = !self.seen.commits.contains_key(id.as_ref());
        let seen = self.seen.commits.entry(id.to_string()).or_default();
        let had = seen.len();
        // A branch unapplied for a while, or the default branch checked out in a terminal,
        // comes back read: what isn't shown is kept, up to 4000 keys, two a commit.
        const KEPT: usize = 4000;
        if seen.len() > KEPT {
            seen.retain(|key| commits.contains(key));
        }
        // Nothing is news the first time a project shows up, and neither is what the
        // app's own operations created. A commit seen by one of its ids is seen by both, so
        // it stays seen when a rebase by Git changes its key, or a reword its message.
        // Only those in the checkouts it made commits in: an agent's in another are news.
        let before = self.rebaseline.get(id).map(|(before, checkouts)| {
            let there: HashSet<&str> = (checkouts.iter())
                .flat_map(|checkout| workspace.branches_in(checkout))
                .flat_map(|branch| &branch.commits)
                .map(|commit| commit.key.as_str())
                .collect();
            (before, there)
        });
        let now_seen: Vec<String> = (shown(workspace))
            .filter(|(_, [key, authored])| {
                first_sight
                    || before.as_ref().is_some_and(|(before, there)| {
                        !before.contains(*key) && there.contains(key)
                    })
                    || was_seen(seen, [key, authored])
            })
            .flat_map(|(_, keys)| keys.map(str::to_owned))
            .collect();
        seen.extend(now_seen);
        // A commit amended since is news again.
        let mut unread = false;
        for key in amended.iter().flat_map(|commit| commit.ids()) {
            unread |= seen.remove(key);
        }
        if fresh {
            self.rebaseline.remove(id);
        }
        first_sight || unread || seen.len() != had
    }

    /// The commits of `id` that `workspace` has amended since the workspace last loaded, as
    /// an agent amending one does: not the app's own amends, which the user made, nor the
    /// commit on screen, which shows them as they come.
    fn amended_since_last_load<'a>(
        &self,
        id: &ProjectId,
        workspace: &'a Workspace,
    ) -> Vec<&'a Commit> {
        let (None, Some(old)) = (self.rebaseline.get(id), self.workspace(id)) else {
            return Vec::new();
        };
        let shown = match &self.selection {
            Some(Selection::Commit(project, commit)) if project == id => Some(*commit),
            _ => None,
        };
        amended_since(old.commits(), workspace.commits(), shown)
    }
}

/// The commits of `now` amended since `before`, other than `shown`'s. Each is paired with
/// what it was by its change ID, or after a rebase by Git dropped that, by what it keeps.
fn amended_since<'a, 'b>(
    before: impl Iterator<Item = &'b Commit>,
    now: impl Iterator<Item = &'a Commit>,
    shown: Option<CommitId>,
) -> Vec<&'a Commit> {
    let mut by_id: HashMap<&str, &Commit> = HashMap::new();
    for commit in before {
        by_id.insert(&commit.key, commit);
        by_id.entry(&commit.authored).or_insert(commit);
    }
    now.filter(|new| {
        let old = (by_id.get(new.key.as_str())).or_else(|| by_id.get(new.authored.as_str()));
        old.is_some_and(|old| Some(old.id) != shown && amended(old, new))
    })
    .collect()
}

/// Whether `new` is `old` amended: rewritten on the same parent, with other changes. One
/// rebased onto another parent only moved, and one reworded keeps its tree.
fn amended(old: &Commit, new: &Commit) -> bool {
    old.id != new.id && old.parent == new.parent && old.tree != new.tree
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{Seeable, amended, amended_since, lone_commit, was_seen};
    use crate::backend::model::build::{branch, full, workspace};
    use crate::backend::{Commit, CommitId};
    use crate::state::Selection;

    fn commit(id: u8, parent: u8, tree: u8) -> Commit {
        let oid = |n: u8| gix::ObjectId::from_bytes_or_panic(&[n; 20]);
        Commit {
            key: "change".into(),
            authored: "authored".into(),
            parent: Some(CommitId(oid(parent))),
            tree: oid(tree),
            ..Commit::bare(oid(id))
        }
    }

    #[test]
    fn only_new_changes_on_the_same_parent_count_as_an_amend() {
        let old = commit(1, 9, 1);
        assert!(
            amended(&old, &commit(2, 9, 2)),
            "same parent, other changes"
        );
        assert!(
            !amended(&old, &commit(2, 8, 2)),
            "on another parent it was rebased"
        );
        assert!(
            !amended(&old, &commit(2, 9, 1)),
            "the same tree was only reworded"
        );
        assert!(!amended(&old, &old.clone()), "not rewritten at all");
    }

    /// `commit` as `git rebase` or `git commit --amend` leaves one without a change ID: a key
    /// made from its new sha, and its author, time and message as they were.
    fn without_change_id(commit: Commit) -> Commit {
        Commit {
            key: format!("made from {:?}", commit.id),
            ..commit
        }
    }

    #[test]
    fn a_commit_rebased_by_git_is_still_seen() {
        let seen: HashSet<String> = commit(1, 9, 1).ids().map(str::to_owned).into();
        let rebased = without_change_id(commit(2, 8, 1));
        assert!(
            was_seen(&seen, rebased.ids()),
            "Git dropped its change ID, but it has the same author, time and message"
        );
        let other = Commit {
            authored: "someone else's".into(),
            ..rebased
        };
        assert!(!was_seen(&seen, other.ids()), "another commit isn't");
    }

    #[test]
    fn a_branch_or_stack_of_one_commit_reads_it() {
        let ws = workspace(
            vec![
                vec![branch("empty", &[]), branch("lone", &[1])],
                vec![branch("two", &[2, 3])],
            ],
            vec![],
        );
        let lone = |selection| lone_commit(&ws, &selection);
        let branch =
            |name: &str| Selection::Branch("p".into(), Some(full(&format!("refs/heads/{name}"))));
        let stack = |bottom: &str| {
            Selection::Stack("p".into(), Some(full(&format!("refs/heads/{bottom}"))))
        };
        let one = Some(
            ws.find_branch(Some(&full("refs/heads/lone")))
                .unwrap()
                .commits[0]
                .id,
        );
        assert_eq!(lone(branch("lone")), one, "its diff is that commit's");
        assert_eq!(lone(stack("lone")), one, "a branch with none adds nothing");
        assert_eq!(
            lone(branch("two")),
            None,
            "two commits' diff isn't either's"
        );
        assert_eq!(lone(branch("empty")), None, "nothing to read");
    }

    #[test]
    fn a_commit_amended_by_git_is_news_but_one_only_rebased_is_not() {
        let before = [commit(1, 9, 1)];
        let amended_by_git = [without_change_id(commit(2, 9, 2))];
        let found = amended_since(before.iter(), amended_by_git.iter(), None);
        assert_eq!(
            found.len(),
            1,
            "paired up by what Git kept, it has new changes on the same parent"
        );
        let rebased_by_git = [without_change_id(commit(2, 8, 2))];
        assert!(
            amended_since(before.iter(), rebased_by_git.iter(), None).is_empty(),
            "on another parent it was only rebased"
        );
        assert!(
            amended_since(before.iter(), amended_by_git.iter(), Some(before[0].id)).is_empty(),
            "the commit on screen shows its changes as they come"
        );
    }
}
