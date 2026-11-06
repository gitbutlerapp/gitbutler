//! What the main checkout's uncommitted hunks build on: the commits of its stacks they
//! overlap. A hunk amends only into one of those or a commit above them, as any lower would
//! leave the commits it builds on out of the commit it's in.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use but_core::{HunkHeader, TreeChange};
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;

use super::{CommitId, Selected, Workspace};

/// The main checkout's hunks that build on commits, by their file's path.
#[derive(Debug, Default, PartialEq)]
pub struct Locks(HashMap<String, Vec<Lock>>);

/// A hunk, as a diff without context has it, and the commits it builds on.
#[derive(Debug, PartialEq)]
pub struct Lock {
    hunk: HunkHeader,
    pub commits: Vec<CommitId>,
}

impl Workspace {
    /// The main checkout's uncommitted changes that locks are read from.
    pub fn lock_input(&self) -> Vec<TreeChange> {
        (self.uncommitted.iter())
            .filter(|file| !file.conflicted)
            .map(|file| file.change.clone())
            .collect()
    }

    /// Whether `commit` is `base` or above it in its lane, so amending a hunk that builds on
    /// `base` into it keeps it above what it builds on. A `base` the workspace doesn't have,
    /// as locks read before a rewrite name, holds nothing back.
    pub fn builds_on(&self, commit: CommitId, base: CommitId) -> bool {
        let Some(lane) = self.lane_commits(base) else {
            return true;
        };
        let at = |id| lane.iter().position(|c| *c == id);
        at(commit)
            .zip(at(base))
            .is_some_and(|(commit, base)| commit <= base)
    }
}

impl Locks {
    /// The hunks of the file at `path` that build on commits.
    pub fn of_file(&self, path: &str) -> &[Lock] {
        self.0.get(path).map_or(&[], Vec::as_slice)
    }

    /// The commits what's `selected` builds on: all its file's hunks', or its hunks'.
    pub fn of(&self, selected: &Selected) -> impl Iterator<Item = CommitId> {
        let locks = self.of_file(&selected.file.path).iter();
        let taken = move |lock: &&Lock| match &selected.hunks {
            None => true,
            Some(keys) => keys.iter().any(|key| lock.hunk.overlaps(key.0)),
        };
        locks
            .filter(taken)
            .flat_map(|lock| lock.commits.iter().copied())
    }
}

impl Locks {
    /// Whether amending the main checkout's `files` into commit `into` of `workspace` keeps
    /// them above every commit they build on.
    pub fn amend_keeps(&self, workspace: &Workspace, files: &[Selected], into: CommitId) -> bool {
        (files.iter())
            .flat_map(|file| self.of(file))
            .all(|base| workspace.builds_on(into, base))
    }
}

/// The locks of `changes` on the commits of the project at `path`'s stacks.
pub fn read_locks(path: &Path, changes: Vec<TreeChange>) -> Result<Locks> {
    if changes.is_empty() {
        return Ok(Locks::default());
    }
    let ctx = super::open(path)?;
    let (_guard, repo, ws, _db) = ctx.workspace_and_db()?;
    let found = hunk_dependencies_for_workspace_changes_by_worktree_dir(&repo, &ws, Some(changes))?;
    let mut locks = HashMap::<String, Vec<Lock>>::new();
    for (path, hunk, by) in found.diffs {
        locks.entry(path).or_default().push(Lock {
            hunk: HunkHeader::from(&hunk),
            commits: by.iter().map(|lock| CommitId(lock.commit_id)).collect(),
        });
    }
    Ok(Locks(locks))
}

#[cfg(test)]
mod tests {
    use crate::backend::ops::tests::{git, head, name_at, workspace_with_an_edit};
    use crate::backend::{Kept, Selected, apply, load_workspace};

    #[test]
    fn an_edit_of_a_committed_line_builds_on_its_commit_and_those_above() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, edit, _, mut edited) = workspace_with_an_edit();
            let path = dir.path();
            git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
            let second = head(path);
            apply(path, name_at(second, "feature")).unwrap();
            edited[4] = "five c\n".into();
            std::fs::write(path.join("file"), edited.concat()).unwrap();
            std::fs::write(path.join("new"), "new\n").unwrap();

            let ws = load_workspace(path, Kept::default()).unwrap();
            let locks = super::read_locks(path, ws.lock_input()).unwrap();
            let of = |name: &str| {
                let file = ws.uncommitted.iter().find(|f| f.path == name).unwrap();
                locks.of(&Selected::whole(file)).collect::<Vec<_>>()
            };
            assert_eq!(of("file"), [edit], "it rewrites a line `edit` wrote");
            assert_eq!(of("new"), [], "a new file builds on nothing");
            assert!(ws.builds_on(edit, edit), "it amends into what it builds on");
            assert!(ws.builds_on(second, edit), "and into a commit above it");
            assert!(!ws.builds_on(edit, second), "but nothing below");
        })
    }

    #[test]
    fn locked_files_amend_only_into_what_they_build_on_and_above() {
        use crate::backend::model::build::{branch, id, workspace};
        // ui's commits newest first: 3, 2, 1; api's: 5, 4.
        let ws = workspace(
            vec![vec![branch("ui", &[3, 2, 1])], vec![branch("api", &[5, 4])]],
            vec![],
        );
        let file = crate::backend::FileChange::bare("file");
        let lock = |commits: Vec<_>| super::Lock {
            hunk: but_core::HunkHeader {
                old_start: 5,
                old_lines: 1,
                new_start: 5,
                new_lines: 1,
            },
            commits,
        };
        let locks = super::Locks([("file".into(), vec![lock(vec![id(2)])])].into());
        let files = [Selected::whole(&file)];
        let into = |n| locks.amend_keeps(&ws, &files, id(n));
        assert!(into(2) && into(3), "into what it builds on, and above it");
        assert!(!into(1), "not below it");
        assert!(!into(5), "nor into another stack");

        let unrelated = [Selected::whole(&crate::backend::FileChange::bare("other"))];
        assert!(
            locks.amend_keeps(&ws, &unrelated, id(1)),
            "a file that builds on nothing goes anywhere"
        );
        let gone = super::Locks([("file".into(), vec![lock(vec![id(9)])])].into());
        assert!(
            gone.amend_keeps(&ws, &files, id(1)),
            "a lock read before a rewrite, on a commit gone, holds nothing back"
        );
    }

    #[test]
    fn picked_lines_build_on_the_locks_they_overlap() {
        use crate::backend::model::build::id;
        let header = |old_start, old_lines, new_start, new_lines| but_core::HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        };
        let lock = |line, commit| super::Lock {
            hunk: header(line, 1, line, 1),
            commits: vec![id(commit)],
        };
        let locks = super::Locks([("file".into(), vec![lock(5, 1), lock(15, 2)])].into());
        let whole = Selected::whole(&crate::backend::FileChange::bare("file"));
        let picked = |line| Selected {
            hunks: Some(vec![crate::backend::HunkKey(header(0, 0, line, 1))]),
            ..whole.clone()
        };
        let on = |selected: &Selected| locks.of(selected).collect::<Vec<_>>();
        assert_eq!(on(&whole), [id(1), id(2)], "the whole file builds on both");
        assert_eq!(
            on(&picked(15)),
            [id(2)],
            "a line of one builds on its commit"
        );
        assert_eq!(on(&picked(10)), [], "a line between them on neither");
    }
}
