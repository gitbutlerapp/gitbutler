//! The operation log: what GitButler did to a project, newest first, each with the
//! snapshot taken just before, to look at and to restore. What an operation did to the
//! branches comes from comparing its snapshot with the next one, or with the workspace now
//! for the newest: the log itself rarely says which commit or branch it was.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use but_api::legacy::oplog::{self, RestoreKind};
use but_oplog::legacy::OperationKind;
use gix::bstr::ByteSlice as _;
use serde::Deserialize;

use super::{CommitId, SnapshotId, open, short_ref};

/// One operation in the log.
#[derive(Debug, Clone)]
pub struct Operation {
    /// The snapshot taken just before it.
    pub id: SnapshotId,
    /// What was done, like "Created commit".
    pub title: String,
    /// More about it: a snapshot's message, or the operation an undo took back.
    pub detail: Option<String>,
    /// When, in seconds since the epoch.
    pub time: i64,
    /// What kind of operation, when the oplog says.
    pub kind: Option<OperationKind>,
    /// What it did to the branches and their commits, when its snapshot says.
    pub changes: Vec<Change>,
}

/// A commit as a snapshot, or the workspace now, has it.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    pub id: CommitId,
    /// Its change ID, which a rebase or a new message keeps.
    pub key: String,
    pub message: String,
}

impl Recorded {
    pub fn title(&self) -> &str {
        self.message.lines().next().unwrap_or_default()
    }
}

/// The workspace's branches at one point, top of each stack first, each by name with its
/// commits, newest first.
pub type Branches = Vec<(String, Vec<Recorded>)>;

/// One thing an operation did to the branches.
#[derive(Debug, Clone)]
pub enum Change {
    /// A commit that wasn't there, now on `branch`.
    Added {
        commit: Recorded,
        branch: String,
    },
    /// A commit no longer there, which was on `branch`.
    Removed {
        commit: Recorded,
        branch: String,
    },
    /// A commit with a new message, on `branch`.
    Reworded {
        before: Recorded,
        after: Recorded,
        branch: String,
    },
    /// A commit now on another branch.
    Moved {
        commit: Recorded,
        from: String,
        to: String,
    },
    BranchAdded(String),
    BranchRemoved(String),
    Renamed {
        from: String,
        to: String,
    },
}

/// Up to `limit` operations older than `after`, or the newest ones; and whether more remain.
/// `now` is the workspace's branches, which the newest operation left.
pub fn list(
    path: &Path,
    after: Option<SnapshotId>,
    limit: usize,
    now: Branches,
) -> Result<(Vec<Operation>, bool)> {
    let ctx = open(path)?;
    let mut snapshots = oplog::list_snapshots(&ctx, limit + 1, after.map(|s| s.0), None, None)?;
    let more = snapshots.len() > limit;
    snapshots.truncate(limit);
    let repo = ctx.repo.get()?;
    // Each commit's entry in a snapshot is read once: unchanged, it's the same in the next.
    let mut read = HashMap::new();
    // What came after the operation at hand: the one after it's snapshot, or the workspace.
    let mut later = match after {
        Some(after) => snapshot_branches(&repo, after.0, &mut read),
        None => Some(now),
    };
    let mut operations = Vec::with_capacity(snapshots.len());
    for snapshot in snapshots {
        let before = snapshot_branches(&repo, snapshot.commit_id, &mut read);
        let changes = match (&before, &later) {
            (Some(before), Some(after)) => compare(before, after),
            _ => Vec::new(),
        };
        later = before;
        // An undo, redo or restore says which operation it took back.
        let restored = |title: &str| {
            let restored = oplog::peel_restore_snapshot(&ctx, snapshot.commit_id)
                .ok()
                .flatten()
                .and_then(|peeled| peeled.details)
                .map(|d| operation_title(d.operation).to_owned());
            (title.to_owned(), restored)
        };
        use OperationKind as K;
        let kind = snapshot.details.as_ref().map(|details| details.operation);
        let (title, detail) = match snapshot.details {
            Some(details) => match details.operation {
                K::RestoreFromSnapshotViaUndo => restored("Undo"),
                K::RestoreFromSnapshotViaRedo => restored("Redo"),
                K::RestoreFromSnapshot => restored("Restore"),
                _ => {
                    // What it was done to: a commit's title, a branch, a file.
                    let about = ["message", "name", "branch", "file"]
                        .iter()
                        .find_map(|key| {
                            let trailer = details.trailers.iter().find(|t| t.key() == *key)?;
                            let value = trailer.value();
                            let line = value.lines().next()?;
                            Some(short_ref(line).to_owned())
                        });
                    let body = details.body.filter(|b| !b.trim().is_empty());
                    (
                        operation_title(details.operation).to_owned(),
                        about.or(body),
                    )
                }
            },
            None => ("Unknown operation".to_owned(), None),
        };
        operations.push(Operation {
            id: SnapshotId(snapshot.commit_id),
            title,
            detail,
            time: snapshot.created_at.seconds,
            kind,
            changes,
        });
    }
    Ok((operations, more))
}

/// A commit's entry in a snapshot, read: the commit, and its first parent.
type Read = (Recorded, Option<gix::ObjectId>);

/// The workspace's branches as snapshot `id` recorded them, from the stacks it kept with
/// their heads and commits; `None` for a snapshot that kept none, as an old one.
fn snapshot_branches(
    repo: &gix::Repository,
    id: gix::ObjectId,
    read: &mut HashMap<gix::ObjectId, Read>,
) -> Option<Branches> {
    #[derive(Deserialize)]
    struct Meta {
        #[serde(default)]
        branches: HashMap<String, Stack>,
    }
    #[derive(Deserialize)]
    struct Stack {
        #[serde(default)]
        order: usize,
        #[serde(default = "yes")]
        in_workspace: bool,
        #[serde(default)]
        heads: Vec<Head>,
    }
    #[derive(Deserialize)]
    struct Head {
        name: String,
        head: HeadCommit,
    }
    #[derive(Deserialize)]
    struct HeadCommit {
        #[serde(rename = "CommitId")]
        commit: Option<String>,
    }
    fn yes() -> bool {
        true
    }
    let tree = repo.find_commit(id).ok()?.tree().ok()?;
    let meta = tree.lookup_entry_by_path("virtual_branches.toml").ok()??;
    let meta = meta.object().ok()?;
    let meta: Meta = toml::from_str(std::str::from_utf8(&meta.data).ok()?).ok()?;
    let mut stacks: Vec<_> = (meta.branches.iter())
        .filter(|(_, s)| s.in_workspace)
        .collect();
    stacks.sort_by_key(|(_, stack)| stack.order);
    let mut branches = Branches::new();
    for (stack_id, stack) in stacks {
        // A stack with no commits of its own keeps none.
        let mut commits = HashMap::new();
        let path = format!("virtual_branches/{stack_id}/commits");
        if let Some(entry) = tree.lookup_entry_by_path(&path).ok().flatten() {
            for entry in entry.object().ok()?.into_tree().iter() {
                let entry = entry.ok()?;
                let commit = gix::ObjectId::from_hex(entry.filename()).ok()?;
                let sub = entry.oid().to_owned();
                let entry = match read.entry(sub) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(read_commit(repo, commit, sub)?),
                };
                commits.insert(commit, entry.clone());
            }
        }
        // Bottom branch first, each down to the one below it.
        let mut taken = HashSet::new();
        let mut own: Vec<(String, Vec<Recorded>)> = Vec::new();
        for head in &stack.heads {
            let mut list = Vec::new();
            let mut at = (head.head.commit.as_deref())
                .and_then(|hex| gix::ObjectId::from_hex(hex.as_bytes()).ok());
            while let Some(id) = at.filter(|id| !taken.contains(id)) {
                let Some((commit, parent)) = commits.get(&id) else {
                    break;
                };
                taken.insert(id);
                list.push(commit.clone());
                at = *parent;
            }
            own.push((head.name.clone(), list));
        }
        branches.extend(own.into_iter().rev());
    }
    Some(branches)
}

/// Commit `commit` as entry `sub` of a snapshot's stack keeps it, with the commit's own data.
fn read_commit(repo: &gix::Repository, commit: gix::ObjectId, sub: gix::ObjectId) -> Option<Read> {
    let data = repo
        .find_tree(sub)
        .ok()?
        .find_entry("commit")?
        .object()
        .ok()?
        .detach()
        .data;
    let parsed = gix::objs::CommitRef::from_bytes(&data, repo.object_hash())
        .ok()?
        .into_owned()
        .ok()?;
    let key = but_core::commit::Headers::try_from_commit(&parsed)
        .unwrap_or_default()
        .ensure_change_id(commit)
        .change_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let recorded = Recorded {
        id: CommitId(commit),
        key,
        // As the workspace's commits have it: a conflicted one's without its markers.
        message: but_core::commit::strip_conflict_markers(parsed.message.as_ref())
            .to_str_lossy()
            .trim_end()
            .to_owned(),
    };
    Some((recorded, parsed.parents.first().copied()))
}

/// What changed from `before` to `after`: commits matched by their change ID, so a rebase
/// alone changes nothing, and a branch one of each side lacks a rename when it's the only
/// one either way.
fn compare(before: &Branches, after: &Branches) -> Vec<Change> {
    let names = |branches: &Branches| -> Vec<String> {
        branches.iter().map(|(name, _)| name.clone()).collect()
    };
    let (old, new) = (names(before), names(after));
    let gone: Vec<&String> = old.iter().filter(|name| !new.contains(name)).collect();
    let came: Vec<&String> = new.iter().filter(|name| !old.contains(name)).collect();
    let renamed = match (&gone[..], &came[..]) {
        ([from], [to]) => Some(((*from).clone(), (*to).clone())),
        _ => None,
    };
    // Where each commit is, its branch under the name it has after a rename.
    let index = |branches: &Branches, rename: bool| -> HashMap<String, (String, Recorded)> {
        let mut index = HashMap::new();
        for (name, commits) in branches {
            let name = match &renamed {
                Some((from, to)) if rename && from == name => to.clone(),
                _ => name.clone(),
            };
            for commit in commits {
                index
                    .entry(commit.key.clone())
                    .or_insert((name.clone(), commit.clone()));
            }
        }
        index
    };
    let (was, is) = (index(before, true), index(after, false));
    let mut changes = Vec::new();
    for (name, commits) in after {
        for commit in commits {
            match was.get(&commit.key) {
                None => changes.push(Change::Added {
                    commit: commit.clone(),
                    branch: name.clone(),
                }),
                Some((from, old)) => {
                    if old.message != commit.message {
                        changes.push(Change::Reworded {
                            before: old.clone(),
                            after: commit.clone(),
                            branch: name.clone(),
                        });
                    }
                    if from != name {
                        changes.push(Change::Moved {
                            commit: commit.clone(),
                            from: from.clone(),
                            to: name.clone(),
                        });
                    }
                }
            }
        }
    }
    for (name, commits) in before {
        for commit in commits.iter().filter(|c| !is.contains_key(&c.key)) {
            changes.push(Change::Removed {
                commit: commit.clone(),
                branch: name.clone(),
            });
        }
    }
    match renamed {
        Some((from, to)) => changes.push(Change::Renamed { from, to }),
        None => {
            changes.extend(came.into_iter().cloned().map(Change::BranchAdded));
            changes.extend(gone.into_iter().cloned().map(Change::BranchRemoved));
        }
    }
    changes
}

/// Why undo and restore refuse to take back one of the operations kept from them.
const UNRECORDED: &str = "GitButler's history records only the workspace and the main \
    checkout, not linked worktrees or branches outside it. Change it back by hand instead.";

/// Put the project back to how it was before the operation `id` was taken for, refusing
/// when that takes back one of `kept`, as undo does. The restore is itself recorded, so it
/// can be undone.
pub fn restore(path: &Path, id: SnapshotId, kept: &HashSet<SnapshotId>) -> Result<()> {
    let mut ctx = open(path)?;
    // Each snapshot is made on the one before, so one taken since `id` has it as its base.
    let repo = ctx.repo.get()?;
    let since = |snapshot: &SnapshotId| {
        snapshot.0 == id.0
            || repo
                .merge_base(id.0, snapshot.0)
                .is_ok_and(|base| base.is_some_and(|b| b == id.0))
    };
    if kept.iter().any(since) {
        bail!(UNRECORDED);
    }
    drop(repo);
    oplog::restore_snapshot_with_kind(&mut ctx, RestoreKind::ExplicitRestoreFromSnapshot, id.0)
}

/// Record the project as it is now.
pub fn take_snapshot(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    oplog::create_snapshot(&mut ctx, None)?;
    Ok(())
}

/// Restore the workspace to before the last operation, or with `redo` restore what the
/// last undo took back, refusing when that operation is one of `kept`. Returns what it was,
/// or `None` when there's nothing to undo or redo.
pub fn undo(path: &Path, redo: bool, kept: &HashSet<SnapshotId>) -> Result<Option<&'static str>> {
    let mut ctx = open(path)?;
    let (snapshot, kind) = if redo {
        let snapshot = oplog::get_redo_target_snapshot(&ctx)?;
        (snapshot, RestoreKind::RestoreFromSnapshotViaRedo)
    } else {
        let snapshot = oplog::get_undo_target_snapshot(&ctx)?;
        (snapshot, RestoreKind::RestoreFromSnapshotViaUndo)
    };
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    if kept.contains(&SnapshotId(snapshot.commit_id)) {
        bail!(UNRECORDED);
    }
    let title = snapshot
        .details
        .map(|details| operation_title(details.operation));
    restore_titled(&mut ctx, snapshot.commit_id, title, kind).map(Some)
}

/// The newest entry in the project's operation log.
pub fn oplog_head(path: &Path) -> Option<SnapshotId> {
    let ctx = open(path).ok()?;
    let newest = oplog::list_snapshots(&ctx, 1, None, None, None).ok()?;
    newest
        .first()
        .map(|snapshot| SnapshotId(snapshot.commit_id))
}

/// Undo the operation that recorded `snapshot`, but only while it's the latest thing to
/// undo: the log is shared with the CLI and agents, and restoring anything else would take
/// back more than the user asked for.
pub fn undo_snapshot(path: &Path, snapshot: SnapshotId) -> Result<&'static str> {
    let mut ctx = open(path)?;
    let target = oplog::get_undo_target_snapshot(&ctx)?
        .filter(|target| target.commit_id == snapshot.0)
        .context("Something else changed the workspace since. ⌘Z undoes one step at a time.")?;
    let title = target
        .details
        .map(|details| operation_title(details.operation));
    let kind = RestoreKind::RestoreFromSnapshotViaUndo;
    restore_titled(&mut ctx, snapshot.0, title, kind)
}

/// What the app calls an operation, where the oplog's name for it is another client's.
fn operation_title(kind: OperationKind) -> &'static str {
    use OperationKind as K;
    match kind {
        K::UndoCommit => "Uncommitted",
        K::InsertBlankCommit => "Empty commit added",
        K::MergeUpstream | K::UpdateWorkspaceBase => "Updated from target",
        K::SetBaseBranch => "Changed target",
        other => other.title(),
    }
}

/// Restore snapshot `id` and name the operation it takes back, as the CLI does: when `id`
/// is itself an undo or redo, it's the operation that one restored.
fn restore_titled(
    ctx: &mut but_ctx::Context,
    id: gix::ObjectId,
    title: Option<&'static str>,
    kind: oplog::RestoreKind,
) -> Result<&'static str> {
    let title = oplog::peel_restore_snapshot(ctx, id)?
        .map_or(title, |peeled| {
            peeled
                .details
                .map(|details| operation_title(details.operation))
        })
        .unwrap_or("Unknown operation");
    oplog::restore_snapshot_with_kind(ctx, kind, id)?;
    Ok(title)
}

#[cfg(test)]
mod tests {
    use super::{Branches, Change, CommitId, Recorded, compare};

    fn commit(n: u8, key: &str, message: &str) -> Recorded {
        Recorded {
            id: CommitId(gix::ObjectId::from_bytes_or_panic(&[n; 20])),
            key: key.to_owned(),
            message: message.to_owned(),
        }
    }

    /// Each change as a short line, to compare whole lists at once.
    fn lines(before: &Branches, after: &Branches) -> Vec<String> {
        let changes = compare(before, after);
        (changes.iter())
            .map(|change| match change {
                Change::Added { commit, branch } => format!("+ {} on {branch}", commit.title()),
                Change::Removed { commit, branch } => format!("- {} from {branch}", commit.title()),
                Change::Reworded { before, after, .. } => {
                    format!("~ {} was {}", after.title(), before.title())
                }
                Change::Moved { commit, from, to } => {
                    format!("> {} {from} to {to}", commit.title())
                }
                Change::BranchAdded(name) => format!("+ branch {name}"),
                Change::BranchRemoved(name) => format!("- branch {name}"),
                Change::Renamed { from, to } => format!("branch {from} now {to}"),
            })
            .collect()
    }

    #[test]
    fn commits_match_by_change_id_across_rewrites() {
        let before: Branches = vec![
            (
                "feat".into(),
                vec![commit(2, "b", "Two"), commit(1, "a", "One")],
            ),
            ("fix".into(), vec![commit(3, "c", "Three")]),
        ];
        // Rebased: every commit is new, none changed.
        let rebased: Branches = vec![
            (
                "feat".into(),
                vec![commit(5, "b", "Two"), commit(4, "a", "One")],
            ),
            ("fix".into(), vec![commit(6, "c", "Three")]),
        ];
        assert!(
            lines(&before, &rebased).is_empty(),
            "a rebase alone changes nothing"
        );

        let after: Branches = vec![
            (
                "feat".into(),
                vec![commit(7, "b", "Two, better"), commit(8, "d", "Four")],
            ),
            (
                "fix".into(),
                vec![commit(3, "c", "Three"), commit(9, "a", "One")],
            ),
        ];
        assert_eq!(
            lines(&before, &after),
            [
                "~ Two, better was Two",
                "+ Four on feat",
                "> One feat to fix"
            ],
            "a new message, a new commit and one moved to another branch"
        );

        let renamed: Branches = vec![
            (
                "feat".into(),
                vec![commit(2, "b", "Two"), commit(1, "a", "One")],
            ),
            ("hotfix".into(), vec![commit(3, "c", "Three")]),
        ];
        assert_eq!(
            lines(&before, &renamed),
            ["branch fix now hotfix"],
            "a renamed branch's commits didn't move"
        );
    }
}
