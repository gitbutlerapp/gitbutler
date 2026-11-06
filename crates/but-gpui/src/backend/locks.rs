//! What the main checkout's uncommitted hunks build on: the commits of its stacks they
//! overlap. A hunk lands cleanly, amended or committed, on one of those or a commit above them
//! in their stack, once every other stack it builds on is stacked below that one. Any lower in
//! its own stack leaves the commits it builds on out of the commit it's in, which conflicts it.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use but_core::{HunkHeader, TreeChange};
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;

use super::{
    Branch, Checkout, CommitId, CommitState, CommitTo, Conflicts, FullName, Lane, Selected,
    Stacking, Workspace,
};

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

/// What changes amended or committed sit on in the workspace's history: in the lane at `lane`,
/// its commit `on`, or its bottom with `None`; with no lane, a new stack of their own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Footing {
    lane: Option<usize>,
    pub on: Option<CommitId>,
}

impl Workspace {
    /// What amending changes into commit `into` puts them on.
    pub fn footing_into(&self, into: CommitId) -> Option<Footing> {
        let lane = (self.lanes.iter()).position(|lane| lane_has(lane, into))?;
        Some(Footing {
            lane: Some(lane),
            on: Some(into),
        })
    }

    /// What committing changes to `to` puts them on.
    pub fn footing(&self, to: &CommitTo) -> Option<Footing> {
        let (lane, from) = match to {
            CommitTo::Branch(branch) => self.branch_at(branch)?,
            CommitTo::NewBranchAt {
                beside: Some((branch, below)),
                ..
            } => {
                let (lane, at) = self.branch_at(branch)?;
                (lane, at + usize::from(*below))
            }
            CommitTo::NewBranchAt { beside: None, .. } => {
                return Some(Footing {
                    lane: None,
                    on: None,
                });
            }
            CommitTo::BelowCommit { commit, .. } => {
                let lane = (self.lanes.iter()).position(|lane| lane_has(lane, *commit))?;
                let mut below = lane_commits(self.lanes.get(lane)?, 0)
                    .skip_while(|id| id != commit)
                    .skip(1);
                return Some(Footing {
                    lane: Some(lane),
                    on: below.next(),
                });
            }
        };
        Some(Footing {
            lane: Some(lane),
            on: lane_commits(self.lanes.get(lane)?, from).next(),
        })
    }

    /// The lane holding `branch`, and its place from the lane's top.
    fn branch_at(&self, branch: &FullName) -> Option<(usize, usize)> {
        (self.lanes.iter().enumerate()).find_map(|(ix, lane)| {
            let at = (lane.branches.iter()).position(|b| b.ref_name.as_ref() == Some(branch))?;
            Some((ix, at))
        })
    }
}

fn lane_has(lane: &Lane, id: CommitId) -> bool {
    lane_commits(lane, 0).any(|c| c == id)
}

/// The commits of `lane`'s branches from its `from`th down, newest first.
fn lane_commits(lane: &Lane, from: usize) -> impl Iterator<Item = CommitId> {
    (lane.branches.iter().skip(from))
        .flat_map(|b| &b.commits)
        .map(|c| c.id)
}

/// How putting changes on a footing sits with the commits they build on.
#[derive(Debug, PartialEq)]
pub struct Fit {
    /// The stacking that puts every other stack they build on below the one they land in, if
    /// one has to go there: the first stays, the rest go on it whole, the footing's own last.
    pub restack: Option<Stacking>,
    /// Whether what doesn't merge where they land is kept, leaving the commit they land in
    /// conflicted, as one they build on is above it in its own lane, or in a stack that can't
    /// go below it.
    pub conflicts: Conflicts,
}

impl Fit {
    /// Whether landing them asks the user first: it restacks, or leaves a commit conflicted.
    pub fn asks(&self) -> bool {
        self.restack.is_some() || self.conflicts == Conflicts::Keep
    }
}

impl Locks {
    /// How putting the main checkout's `files` on `footing` in `workspace` sits with what they
    /// build on. A commit the workspace doesn't have, as locks read before a rewrite name,
    /// holds nothing back. Other lanes that can't be stacked, as a linked worktree's or below
    /// a new stack, leave it conflicted.
    pub fn fit(&self, workspace: &Workspace, files: &[Selected], footing: Footing) -> Fit {
        let lane_of = |id| (workspace.lanes.iter()).position(|lane| lane_has(lane, id));
        let own = footing.lane.and_then(|ix| workspace.lanes.get(ix));
        let under: Vec<CommitId> = (own.into_iter())
            .flat_map(|lane| lane_commits(lane, 0))
            .skip_while(|id| Some(*id) != footing.on)
            .collect();
        let mut conflicts = false;
        let mut below = Vec::new();
        for base in files.iter().flat_map(|file| self.of(file)) {
            match lane_of(base) {
                Some(lane) if Some(lane) == footing.lane => conflicts |= !under.contains(&base),
                Some(lane) if !below.contains(&lane) => below.push(lane),
                Some(_) | None => {}
            }
        }
        below.sort_unstable();
        let restack = match (footing.lane, below.is_empty()) {
            (Some(own), false) => {
                stacking((below.iter().chain([&own])).filter_map(|ix| workspace.lanes.get(*ix)))
            }
            (Some(_), true) | (None, _) => None,
        };
        Fit {
            conflicts: match conflicts || (!below.is_empty() && restack.is_none()) {
                true => Conflicts::Keep,
                false => Conflicts::Reject,
            },
            restack,
        }
    }
}

/// `lanes` stacked whole, each on the one before, if they're all the main checkout's stacks
/// with named branches.
fn stacking<'a>(mut lanes: impl Iterator<Item = &'a Lane>) -> Option<Stacking> {
    let named = |lane: &Lane| -> Option<Vec<FullName>> {
        (lane.checkout == Checkout::Main).then_some(())?;
        lane.branches
            .iter()
            .rev()
            .map(|b| b.ref_name.clone())
            .collect()
    };
    let stays = lanes.next()?;
    let onto = named(stays)?.pop()?;
    let mut branches = Vec::new();
    let mut pushed = Vec::new();
    for lane in lanes {
        branches.extend(named(lane)?);
        let was_pushed = |b: &&Branch| b.commits.iter().any(|c| c.state == CommitState::Pushed);
        pushed.extend(
            lane.branches
                .iter()
                .filter(was_pushed)
                .filter_map(|b| b.ref_name.clone()),
        );
    }
    Some(Stacking {
        onto,
        top: branches.last()?.clone(),
        branches,
        pushed,
    })
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
        })
    }

    #[test]
    fn locked_files_restack_other_stacks_below_and_conflict_below_their_lock() {
        use crate::backend::model::build::{branch, id, workspace};
        // Newest first: ui's 3, 2, 1; api's 5, 4; db's 7, 6.
        let ws = workspace(
            vec![
                vec![branch("ui", &[3, 2, 1])],
                vec![branch("api", &[5, 4])],
                vec![branch("db", &[7, 6])],
            ],
            vec![],
        );
        let file = |name: &str| crate::backend::FileChange::bare(name);
        let lock = |line, commit| super::Lock {
            hunk: but_core::HunkHeader {
                old_start: line,
                old_lines: 1,
                new_start: line,
                new_lines: 1,
            },
            commits: vec![id(commit)],
        };
        let locks = super::Locks(
            [
                ("ui".into(), vec![lock(5, 2)]),
                ("ui+api".into(), vec![lock(5, 2), lock(9, 4)]),
                ("gone".into(), vec![lock(5, 9)]),
            ]
            .into(),
        );
        let fit = |name: &str, into| {
            let footing = ws.footing_into(id(into)).unwrap();
            let fit = locks.fit(&ws, &[Selected::whole(&file(name))], footing);
            let restack = fit.restack.map(|stacking| {
                let names = |refs: &[crate::backend::FullName]| {
                    refs.iter()
                        .map(|r| r.shorten().to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                format!(
                    "{} <- {} ({})",
                    stacking.onto.shorten(),
                    names(&stacking.branches),
                    stacking.top.shorten()
                )
            });
            (restack, fit.conflicts == super::Conflicts::Keep)
        };
        assert_eq!(fit("ui", 2), (None, false), "into what it builds on");
        assert_eq!(fit("ui", 3), (None, false), "and above it");
        assert_eq!(fit("ui", 1), (None, true), "below it, it conflicts");
        assert_eq!(
            fit("ui", 5),
            (Some("ui <- api (api)".into()), false),
            "in another stack, that one goes on top of the one it builds on"
        );
        assert_eq!(
            fit("ui+api", 3),
            (Some("api <- ui (ui)".into()), false),
            "on two stacks, the other goes below the one it lands in"
        );
        assert_eq!(
            fit("ui+api", 1),
            (Some("api <- ui (ui)".into()), true),
            "below its lock in its own stack, it conflicts as well"
        );
        assert_eq!(
            fit("ui+api", 7),
            (Some("ui <- api db (db)".into()), false),
            "in a third stack, both go below it, whole"
        );
        assert_eq!(
            fit("other", 1),
            (None, false),
            "a file that builds on nothing goes anywhere"
        );
        assert_eq!(
            fit("gone", 1),
            (None, false),
            "a lock on a commit gone holds nothing back"
        );
    }

    #[test]
    fn locked_files_committed_conflict_below_their_lock_and_in_a_new_stack() {
        use crate::backend::model::build::{branch, id, workspace};
        use crate::backend::{CommitTo, Conflicts, FullName};
        // Newest first: ui's 3, 2, 1; api's 5, 4.
        let ws = workspace(
            vec![vec![branch("ui", &[3, 2, 1])], vec![branch("api", &[5, 4])]],
            vec![],
        );
        let lock = super::Lock {
            hunk: but_core::HunkHeader {
                old_start: 5,
                old_lines: 1,
                new_start: 5,
                new_lines: 1,
            },
            commits: vec![id(2)],
        };
        let locks = super::Locks([("file".into(), vec![lock])].into());
        let files = [Selected::whole(&crate::backend::FileChange::bare("file"))];
        let ui = FullName::try_from("refs/heads/ui").unwrap();
        let fit = |to: CommitTo| {
            let fit = locks.fit(&ws, &files, ws.footing(&to).unwrap());
            (fit.restack.is_some(), fit.conflicts == Conflicts::Keep)
        };
        let below = |n| CommitTo::BelowCommit {
            commit: id(n),
            branch: ui.clone(),
        };
        let new_branch = |beside| CommitTo::NewBranchAt {
            beside,
            first: false,
            name: None,
        };
        assert_eq!(
            fit(CommitTo::Branch(ui.clone())),
            (false, false),
            "on top of it"
        );
        assert_eq!(
            fit(below(3)),
            (false, false),
            "right above what it builds on"
        );
        assert_eq!(fit(below(2)), (false, true), "below it, it conflicts");
        assert_eq!(fit(below(1)), (false, true), "at the stack's bottom too");
        assert_eq!(
            fit(new_branch(Some((ui.clone(), false)))),
            (false, false),
            "in a new branch above"
        );
        assert_eq!(
            fit(new_branch(Some((ui.clone(), true)))),
            (false, true),
            "in a new branch below, it conflicts"
        );
        assert_eq!(
            fit(CommitTo::Branch(
                FullName::try_from("refs/heads/api").unwrap()
            )),
            (true, false),
            "in another stack, it restacks"
        );
        assert_eq!(
            fit(new_branch(None)),
            (false, true),
            "in a new stack, which nothing can go below, it conflicts"
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
