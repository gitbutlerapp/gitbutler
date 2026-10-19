//! What the user has looked at: the commits seen, so new work by agents stands out.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use gpui::Context;

use super::{ProjectId, Selection, Store};
use crate::backend::{Commit, CommitId, Workspace};

/// What the user has looked at, so new work by agents stands out. Per project id.
#[derive(Default, Clone, Serialize, Deserialize)]
pub(super) struct Seen {
    /// Commit keys (see [`Commit::key`]) that have been looked at.
    pub(super) commits: HashMap<String, HashSet<String>>,
}

impl Store {
    pub fn is_unread(&self, project: &str, commit: &Commit) -> bool {
        self.seen
            .commits
            .get(project)
            .is_some_and(|seen| !seen.contains(&commit.key))
    }

    /// Count `ids` of `project` as seen, as opening them does.
    pub fn mark_read(
        &mut self,
        project: &ProjectId,
        ids: impl IntoIterator<Item = CommitId>,
        cx: &mut Context<Self>,
    ) {
        let keys: Vec<String> = (ids.into_iter())
            .filter_map(|id| self.key_of(project, id))
            .collect();
        let seen = self.seen.commits.entry(project.to_string()).or_default();
        seen.extend(keys);
        self.save_soon(cx);
        cx.notify();
    }

    /// Bring what's been seen of `id` up to date with the `workspace` it just loaded: what's
    /// gone is forgotten, and what's new is news unless the app made it. `latest` is false
    /// when a newer load is already queued behind this one.
    pub(super) fn update_seen(&mut self, id: &ProjectId, workspace: &Workspace, latest: bool) {
        let amended = self.amended_since_last_load(id, workspace);
        let commits: HashSet<&str> = workspace
            .commits()
            .map(|commit| commit.key.as_str())
            .collect();
        let first_sight = !self.seen.commits.contains_key(id.as_ref());
        let seen = self.seen.commits.entry(id.to_string()).or_default();
        seen.retain(|key| commits.contains(key.as_str()));
        // Nothing is news the first time a project shows up, and neither is what the
        // app's own operations created.
        let before = self.rebaseline.get(id);
        seen.extend(
            commits
                .iter()
                .filter(|key| first_sight || before.is_some_and(|before| !before.contains(**key)))
                .map(|key| key.to_string()),
        );
        // A commit amended since, by its change ID, is news again.
        for key in amended {
            seen.remove(&key);
        }
        // Nor is what was committed in a linked worktree: its changes were there to watch as
        // they were written. Kept by change ID, they stay seen when its branch is applied.
        let in_worktrees = (workspace.worktrees.iter())
            .flat_map(|worktree| &worktree.branches)
            .flat_map(|branch| &branch.commits);
        seen.extend(in_worktrees.map(|commit| commit.key.clone()));
        if latest {
            self.rebaseline.remove(id);
        }
    }

    /// The change IDs of `id`'s commits that `workspace` has amended since the workspace
    /// last loaded, as an agent amending one does: not the app's own amends, which the
    /// user made, nor the commit on screen, which shows them as they come.
    fn amended_since_last_load(&self, id: &ProjectId, workspace: &Workspace) -> Vec<String> {
        let (None, Some(old)) = (self.rebaseline.get(id), self.workspace(id)) else {
            return Vec::new();
        };
        let shown = match &self.selection {
            Some(Selection::Commit(project, commit)) if project == id => old
                .find_commit(*commit)
                .map(|(_, commit)| commit.key.as_str()),
            _ => None,
        };
        let before: HashMap<&str, &Commit> = old
            .commits()
            .map(|commit| (commit.key.as_str(), commit))
            .collect();
        (workspace.commits())
            .filter(|new| Some(new.key.as_str()) != shown)
            .filter(|new| {
                before
                    .get(new.key.as_str())
                    .is_some_and(|old| amended(old, new))
            })
            .map(|new| new.key.clone())
            .collect()
    }
}

/// Whether `new` is `old` amended: rewritten on the same parent, with other changes. One
/// rebased onto another parent only moved, and one reworded keeps its tree.
fn amended(old: &Commit, new: &Commit) -> bool {
    old.id != new.id && old.parent == new.parent && old.tree != new.tree
}

#[cfg(test)]
mod tests {
    use super::amended;
    use crate::backend::{Commit, CommitId, CommitState};

    fn commit(id: u8, parent: u8, tree: u8) -> Commit {
        let oid = |n: u8| gix::ObjectId::from_bytes_or_panic(&[n; 20]);
        Commit {
            id: CommitId(oid(id)),
            key: "change".into(),
            title: String::new(),
            message: String::new(),
            author: String::new(),
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
}
