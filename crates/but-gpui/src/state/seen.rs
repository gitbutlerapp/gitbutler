//! What the user has looked at: the commits seen, so new work by agents stands out.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use gpui::Context;

use super::{ProjectId, Selection, Store};
use crate::backend::{Commit, CommitId, Workspace};

/// What the user has looked at, so new work by agents stands out. Per project id.
#[derive(Default, Clone, Serialize, Deserialize)]
pub(super) struct Seen {
    /// What the commits that have been looked at go by: [`Commit::key`] and
    /// [`Commit::authored`].
    pub(super) commits: HashMap<String, HashSet<String>>,
}

/// What `commit` goes by among those seen: its change ID, and what a rebase by Git, which
/// drops that, keeps.
fn ids(commit: &Commit) -> [String; 2] {
    [commit.key.clone(), commit.authored.clone()]
}

/// Whether `commit` was seen, by either of its [`ids`].
fn was_seen(seen: &HashSet<String>, commit: &Commit) -> bool {
    seen.contains(&commit.key) || seen.contains(&commit.authored)
}

impl Store {
    pub fn is_unread(&self, project: &str, commit: &Commit) -> bool {
        self.seen
            .commits
            .get(project)
            .is_some_and(|seen| !was_seen(seen, commit))
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
        let keys: Vec<String> = (ids.into_iter())
            .filter_map(|id| self.find_commit(project, id))
            .flat_map(|(_, commit)| self::ids(commit))
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

    /// Bring what's been seen of `id` up to date with the `workspace` it just loaded: what's
    /// gone is forgotten, and what's new is news unless the app made it. `latest` is false
    /// when a newer load is already queued behind this one.
    pub(super) fn update_seen(&mut self, id: &ProjectId, workspace: &Workspace, latest: bool) {
        let amended = self.amended_since_last_load(id, workspace);
        let commits: HashSet<String> = workspace.commits().flat_map(ids).collect();
        let first_sight = !self.seen.commits.contains_key(id.as_ref());
        let seen = self.seen.commits.entry(id.to_string()).or_default();
        seen.retain(|key| commits.contains(key));
        // Nothing is news the first time a project shows up, and neither is what the
        // app's own operations created. A commit seen by one of its ids is seen by both, so
        // it stays seen when a rebase by Git changes its key, or a reword its message.
        let before = self.rebaseline.get(id);
        let now_seen: Vec<String> = (workspace.commits())
            .filter(|commit| {
                first_sight
                    || before.is_some_and(|before| !before.contains(&commit.key))
                    || was_seen(seen, commit)
            })
            .flat_map(ids)
            .collect();
        seen.extend(now_seen);
        // A commit amended since is news again.
        for key in amended.iter().flat_map(|commit| ids(commit)) {
            seen.remove(&key);
        }
        if latest {
            self.rebaseline.remove(id);
        }
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

    use super::{amended, amended_since, ids, was_seen};
    use crate::backend::{Commit, CommitId, CommitState};

    fn commit(id: u8, parent: u8, tree: u8) -> Commit {
        let oid = |n: u8| gix::ObjectId::from_bytes_or_panic(&[n; 20]);
        Commit {
            id: CommitId(oid(id)),
            key: "change".into(),
            authored: "authored".into(),
            title: String::new(),
            message: String::new(),
            author: String::new(),
            email: String::new(),
            agent: None,
            time: 0,
            state: CommitState::Local,
            conflicted: false,
            parent: Some(CommitId(oid(parent))),
            tree: oid(tree),
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
        let seen: HashSet<String> = ids(&commit(1, 9, 1)).into();
        let rebased = without_change_id(commit(2, 8, 1));
        assert!(
            was_seen(&seen, &rebased),
            "Git dropped its change ID, but it has the same author, time and message"
        );
        let other = Commit {
            authored: "someone else's".into(),
            ..rebased
        };
        assert!(!was_seen(&seen, &other), "another commit isn't");
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
