//! What the user has looked at: the commits seen, so new work by agents stands out, and the
//! uncommitted files reviewed.

use std::collections::{HashMap, HashSet};

use gpui::Context;
use serde::{Deserialize, Serialize};

use super::{ProjectId, Store, StoreEvent};
use crate::backend::{Commit, FileChange, Workspace};

/// What the user has looked at, so new work by agents stands out. Per project id.
#[derive(Default, Clone, Serialize, Deserialize)]
pub(super) struct Seen {
    /// Commit keys (see [`Commit::key`]) that have been looked at.
    pub(super) commits: HashMap<String, HashSet<String>>,
    /// Uncommitted files marked reviewed, with the file version reviewed.
    files: HashMap<String, HashMap<String, i64>>,
}

/// Where an uncommitted file stands in review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    NotReviewed,
    Reviewed,
    /// Reviewed, then changed again.
    Changed,
}

/// Where the review marks of linked worktree `name`'s files are kept, apart from those of
/// the project's own checkout, which are kept under the project.
pub fn worktree_review_scope(project: &str, name: &str) -> ProjectId {
    format!("{project}#{name}").into()
}

impl Store {
    pub fn is_unread(&self, project: &str, commit: &Commit) -> bool {
        self.seen
            .commits
            .get(project)
            .is_some_and(|seen| !seen.contains(&commit.key))
    }

    pub fn review(&self, project: &str, file: &FileChange) -> Review {
        match self
            .seen
            .files
            .get(project)
            .and_then(|files| files.get(&file.path))
        {
            None => Review::NotReviewed,
            Some(version) if *version == file.modified.unwrap_or(0) => Review::Reviewed,
            Some(_) => Review::Changed,
        }
    }

    /// How many of `files`, whose review marks are kept under `scope`, still want a look.
    pub fn unreviewed(&self, scope: &str, files: &[FileChange]) -> usize {
        (files.iter())
            .filter(|file| self.review(scope, file) != Review::Reviewed)
            .count()
    }

    pub fn set_reviewed(
        &mut self,
        project: &ProjectId,
        files: &[FileChange],
        reviewed: bool,
        cx: &mut Context<Self>,
    ) {
        let entry = self.seen.files.entry(project.to_string()).or_default();
        for file in files {
            if reviewed {
                entry.insert(file.path.clone(), file.modified.unwrap_or(0));
            } else {
                entry.remove(&file.path);
            }
        }
        cx.emit(StoreEvent::Reviewed(project.clone()));
        self.save_soon(cx);
        cx.notify();
    }

    /// Forget the review marks of `id`'s worktrees that a full load listed as gone, neither
    /// active nor archived, as when an agent removes one: one recreated under its name
    /// starts over.
    pub(super) fn forget_gone_worktrees(&mut self, id: &ProjectId, workspace: &Workspace) {
        let prefix = format!("{id}#");
        let names: HashSet<&str> = (workspace.worktrees.iter())
            .map(|worktree| &*worktree.name)
            .chain(workspace.archived_worktrees.iter().map(|name| &**name))
            .collect();
        self.seen.files.retain(|scope, _| {
            (scope.strip_prefix(&prefix)).is_none_or(|name| names.contains(name))
        });
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
        if latest {
            self.rebaseline.remove(id);
        }
        // Review marks go with their files, in the main checkout and in each linked worktree.
        let checkouts = std::iter::once((id.clone(), &workspace.uncommitted)).chain(
            (workspace.worktrees.iter()).map(|worktree| {
                let scope = worktree_review_scope(id, &worktree.name);
                (scope, &worktree.uncommitted)
            }),
        );
        for (scope, uncommitted) in checkouts {
            if let Some(reviewed) = self.seen.files.get_mut(scope.as_ref()) {
                let files: HashSet<&str> = uncommitted.iter().map(|f| f.path.as_str()).collect();
                reviewed.retain(|path, _| files.contains(path.as_str()));
            }
        }
    }
}
