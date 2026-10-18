//! What the user has looked at: the commits seen, so new work by agents stands out.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{ProjectId, Store};
use crate::backend::{Commit, Workspace};

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

    /// Bring what's been seen of `id` up to date with the `workspace` it just loaded: what's
    /// gone is forgotten, and what's new is news unless the app made it. `latest` is false
    /// when a newer load is already queued behind this one.
    pub(super) fn update_seen(&mut self, id: &ProjectId, workspace: &Workspace, latest: bool) {
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
}
