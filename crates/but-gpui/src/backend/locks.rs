//! What the main checkout's uncommitted hunks build on: the commits of its stacks they
//! overlap. A hunk amends only into one of those or a commit above them, as any lower would
//! leave the commits it builds on out of the commit it's in.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use but_core::{RepositoryExt as _, TreeChange};
use but_hunk_dependency::ui::{HunkDependencies, HunkLockTarget};
use but_hunk_dependency::{InputStack, WorkspaceRanges, input_commit};

use super::{Checkout, CommitId, Selected, Workspace};

/// The main checkout's hunks that build on commits, by their file's path.
#[derive(Debug, Default, PartialEq)]
pub struct Locks(HashMap<String, Vec<Lock>>);

/// A hunk, as a diff without context has it, and the commits it builds on.
#[derive(Debug, PartialEq)]
pub struct Lock {
    old: (u32, u32),
    new: (u32, u32),
    pub commits: Vec<CommitId>,
}

/// What locks are read from, as a workspace was loaded: its stacks' commits, oldest first,
/// and the main checkout's uncommitted changes.
pub struct LockInput {
    stacks: Vec<Vec<CommitId>>,
    changes: Vec<TreeChange>,
}

impl Workspace {
    pub fn lock_input(&self) -> LockInput {
        let commits = |lane: &super::Lane| {
            let newest_first = lane.branches.iter().flat_map(|branch| &branch.commits);
            newest_first.rev().map(|commit| commit.id).collect()
        };
        LockInput {
            stacks: self.lanes_in(&Checkout::Main).map(commits).collect(),
            changes: (self.uncommitted.iter())
                .filter(|file| !file.conflicted)
                .map(|file| file.change.clone())
                .collect(),
        }
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
            Some(keys) => keys.iter().any(|key| lock.within(key.0)),
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

impl Lock {
    /// Whether it's in the hunk, or the lines of it, `header` names. A side a line pick
    /// leaves out is `(0, 0)`.
    fn within(&self, header: but_core::HunkHeader) -> bool {
        let overlaps = |(start, lines): (u32, u32), (other, other_lines): (u32, u32)| {
            let end = start + lines.max(1);
            let other_end = other + other_lines.max(1);
            (other, other_lines) != (0, 0) && start < other_end && other < end
        };
        overlaps(self.old, (header.old_start, header.old_lines))
            || overlaps(self.new, (header.new_start, header.new_lines))
    }
}

/// The locks of what `input` has, reading the commits' changes from the project at `path`.
pub fn read_locks(path: &Path, input: LockInput) -> Result<Locks> {
    if input.changes.is_empty() {
        return Ok(Locks::default());
    }
    let ctx = super::open(path)?;
    let repo = ctx
        .repo
        .get()?
        .clone()
        .for_tree_diffing()?
        .with_object_memory();
    let stack = |commits: Vec<CommitId>| -> Result<InputStack> {
        Ok(InputStack {
            target: HunkLockTarget::Unidentified,
            commits_from_base_to_tip: (commits.into_iter())
                .map(|id| input_commit(&repo, id.0))
                .collect::<Result<_>>()?,
        })
    };
    let stacks = input.stacks.into_iter().map(stack).collect::<Result<_>>()?;
    let ranges = WorkspaceRanges::try_from_stacks(stacks)?;
    let found = HunkDependencies::try_from_workspace_ranges(&repo, ranges, input.changes)?;
    let mut locks = HashMap::<String, Vec<Lock>>::new();
    for (path, hunk, by) in found.diffs {
        locks.entry(path).or_default().push(Lock {
            old: (hunk.old_start, hunk.old_lines),
            new: (hunk.new_start, hunk.new_lines),
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
            old: (5, 1),
            new: (5, 1),
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
    fn a_lock_is_within_the_hunk_or_line_pick_around_it() {
        let lock = super::Lock {
            old: (5, 1),
            new: (5, 2),
            commits: Vec::new(),
        };
        let header = |old_start, old_lines, new_start, new_lines| but_core::HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        };
        assert!(
            lock.within(header(2, 7, 2, 8)),
            "the hunk with context around it"
        );
        assert!(
            !lock.within(header(10, 3, 11, 3)),
            "another hunk further down"
        );
        assert!(
            lock.within(header(0, 0, 6, 1)),
            "an added line of it picked"
        );
        assert!(!lock.within(header(0, 0, 9, 1)), "a line picked elsewhere");
    }
}
