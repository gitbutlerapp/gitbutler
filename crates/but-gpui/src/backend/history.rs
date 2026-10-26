//! The operation log: what GitButler did to a project, newest first, each with the
//! snapshot taken just before, to look at and to restore. What an operation did to the
//! branches comes from comparing its snapshot with the next one, or with the workspace now
//! for the newest: the log itself rarely says which commit or branch it was.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Result, bail};
use but_api::legacy::oplog::{self, RestoreKind};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use gix::bstr::ByteSlice as _;
use serde::Deserialize;

use super::{CommitId, SnapshotId, open, short_ref, short_text};
use but_core::WORKSPACE_REF_NAME;

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
    /// Its first parent and its tree, which tell an amend from a rebase.
    pub parent: Option<CommitId>,
    pub tree: gix::ObjectId,
}

impl Recorded {
    pub fn title(&self) -> String {
        super::subject_and_body(&self.message).0
    }

    /// Whether it's `before` amended: on the same parent with other changes. One rebased
    /// onto another parent only moved.
    pub fn amends(&self, before: &Recorded) -> bool {
        self.parent == before.parent && self.tree != before.tree
    }
}

/// The workspace's branches at one point, top of each stack first, each by name with its
/// commits, newest first.
pub type Branches = Vec<(String, Vec<Recorded>)>;

/// The branches of `workspace` now, as a snapshot records them.
pub fn branches(workspace: &super::Workspace) -> Branches {
    (workspace.lanes_in(&super::Checkout::Main))
        .flat_map(|stack| &stack.branches)
        .map(|branch| {
            let commits = (branch.commits.iter()).map(|commit| Recorded {
                id: commit.id,
                key: commit.key.clone(),
                message: commit.message.clone(),
                parent: commit.parent,
                tree: commit.tree,
            });
            (branch.name.clone(), commits.collect())
        })
        .collect()
}

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
    /// A commit rewritten in its place on `branch`: amended, given a new message, or both.
    Edited {
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
    /// The target moved on by `commits` along its own line, as an update brings in.
    TargetMoved {
        target: String,
        commits: usize,
    },
}

/// Up to `limit` operations older than `after`, or the newest ones; and whether more remain.
/// `now` is the workspace's branches, which the newest operation left, when snapshots record
/// them as the workspace has them.
pub fn list(
    path: &Path,
    after: Option<SnapshotId>,
    limit: usize,
    now: Option<Branches>,
) -> Result<(Vec<Operation>, bool)> {
    let ctx = open(path)?;
    let mut snapshots = oplog::list_snapshots(&ctx, limit + 1, after.map(|s| s.0), None, None)?;
    let more = snapshots.len() > limit;
    snapshots.truncate(limit);
    let repo = ctx.repo.get()?;
    // Each commit's entry in a snapshot is read once: unchanged, it's the same in the next.
    let mut read = HashMap::new();
    // What came after the operation at hand: the one after it's snapshot, or the workspace.
    let meta = ctx.project_meta().ok();
    let target_ref = (meta.as_ref()).and_then(|meta| meta.target_ref.clone());
    let target_ref = target_ref.as_ref().map(|t| t.as_local_full_name_ref());
    let (mut later, mut later_target) = match after {
        Some(after) => {
            let target = snapshot_target(&repo, after.0);
            let branches = snapshot_branches(&repo, after.0, target, target_ref, &mut read);
            (branches, target)
        }
        None => (now, meta.as_ref().and_then(|meta| meta.target_commit_id)),
    };
    // By what updates come from, as the navigator names the target.
    let target_name = (meta.and_then(|meta| super::target_source(&repo, &meta))).map_or_else(
        || "the target".to_owned(),
        |name| name.shorten().to_string(),
    );
    let mut operations = Vec::with_capacity(snapshots.len());
    for snapshot in snapshots {
        let target = snapshot_target(&repo, snapshot.commit_id);
        let before = snapshot_branches(&repo, snapshot.commit_id, target, target_ref, &mut read);
        let mut changes = match (&before, &later) {
            (Some(before), Some(after)) => compare(before, after),
            _ => Vec::new(),
        };
        if let (Some(from), Some(to)) = (target, later_target)
            && let Some(commits) = super::first_parent_steps(&repo, to, from, 1000)
            && commits > 0
        {
            changes.push(Change::TargetMoved {
                target: target_name.clone(),
                commits,
            });
        }
        (later, later_target) = (before, target);
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
        // `but amend` records amending a commit as squashing the changes into it: one that
        // took no commit away amended one.
        let amended = |changes: &[Change]| {
            !changes.iter().any(|c| matches!(c, Change::Removed { .. }))
                && (changes.iter()).any(
                    |c| matches!(c, Change::Edited { before, after, .. } if after.amends(before)),
                )
        };
        let kind = (snapshot.details.as_ref()).map(|details| match details.operation {
            K::SquashCommit if amended(&changes) => K::AmendCommit,
            kind => kind,
        });
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
                            Some(short_text(line).to_owned())
                        });
                    let body = details.body.filter(|b| !b.trim().is_empty());
                    (
                        operation_title(kind.unwrap_or(details.operation)).to_owned(),
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

/// The workspace's branches as snapshot `id` recorded them, from the stacks it kept with
/// their heads and commits, or from the branch checked out outside the workspace; `None` for
/// a snapshot that kept neither, as an old one. `target` is the target commit it recorded,
/// and `target_ref` the project's target branch.
fn snapshot_branches(
    repo: &gix::Repository,
    id: gix::ObjectId,
    target: Option<gix::ObjectId>,
    target_ref: Option<&gix::refs::FullNameRef>,
    read: &mut HashMap<gix::ObjectId, Recorded>,
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
    if let Some(branches) = checkout_branches(repo, &tree, target, target_ref, read) {
        return Some(branches);
    }
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
                    Entry::Vacant(entry) => {
                        let data = repo
                            .find_tree(sub)
                            .ok()?
                            .find_entry("commit")?
                            .object()
                            .ok()?;
                        entry.insert(recorded(repo, commit, &data.data)?)
                    }
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
                let Some(commit) = commits.get(&id) else {
                    break;
                };
                taken.insert(id);
                list.push(commit.clone());
                at = commit.parent.map(|parent| parent.0);
            }
            own.push((head.name.clone(), list));
        }
        branches.extend(own.into_iter().rev());
    }
    Some(branches)
}

/// The branches of a snapshot taken with a branch checked out outside the workspace: that
/// branch, then each one it's stacked on, with the commits `target` doesn't have. A snapshot
/// keeps where the branches pointed but not their commits, so these are read from the
/// repository. `None` for one taken in the workspace, which records no checkout.
fn checkout_branches(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    target: Option<gix::ObjectId>,
    target_ref: Option<&gix::refs::FullNameRef>,
    read: &mut HashMap<gix::ObjectId, Recorded>,
) -> Option<Branches> {
    let text = |path: &str| -> Option<String> {
        let entry = tree.lookup_entry_by_path(path).ok()??;
        Some(entry.object().ok()?.data.to_str().ok()?.trim().to_owned())
    };
    let id = |path: &str| gix::ObjectId::from_hex(text(path)?.as_bytes()).ok();
    let checkout = text("checkout/ref")?;
    // While a commit is edited `HEAD` is on GitButler's own branch, and the stacks still say.
    if checkout.starts_with("refs/heads/gitbutler/") {
        return None;
    }
    let (tip, target) = (id("checkout/commit")?, target?);
    // The branches below it, by the commit each points to.
    let mut refs = HashMap::new();
    for ix in 0.. {
        let Some(name) = text(&format!("additional-refs/{ix}/name")) else {
            break;
        };
        if let Some(at) = id(&format!("additional-refs/{ix}/target")) {
            refs.insert(at, name);
        }
    }
    let mut branches = vec![(short_text(&checkout).to_owned(), Vec::new())];
    let walk = (repo.rev_walk([tip]).first_parent_only())
        .with_hidden([target])
        .all()
        .ok()?;
    // At most a thousand, so a branch far from the target doesn't hold up the list.
    for info in walk.take(1000) {
        let commit = info.ok()?.id;
        // One on the checked-out commit itself is part of the checked-out branch, as the
        // workspace shows it.
        if commit != tip
            && let Some(name) = refs.get(&commit)
        {
            branches.push((short_text(name).to_owned(), Vec::new()));
        }
        // Keyed by the commit itself, where a stack's entries are keyed by their subtree.
        let entry = match read.entry(commit) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let data = repo.find_object(commit).ok()?;
                entry.insert(recorded(repo, commit, &data.data)?)
            }
        };
        branches.last_mut()?.1.push(entry.clone());
    }
    // The target checked out with nothing of its own isn't listed, as the workspace has it.
    if target_ref.is_some_and(|t| t.as_bstr() == checkout.as_str())
        && (branches.first()).is_some_and(|(_, commits)| commits.is_empty())
    {
        branches.remove(0);
    }
    Some(branches)
}

/// Commit `commit` from its own `data`.
fn recorded(repo: &gix::Repository, commit: gix::ObjectId, data: &[u8]) -> Option<Recorded> {
    let parsed = gix::objs::CommitRef::from_bytes(data, repo.object_hash())
        .ok()?
        .into_owned()
        .ok()?;
    let key = but_core::commit::Headers::try_from_commit(&parsed)
        .unwrap_or_default()
        .ensure_change_id(commit)
        .change_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    Some(Recorded {
        id: CommitId(commit),
        key,
        // As the workspace's commits have it: a conflicted one's without its markers.
        message: but_core::commit::strip_conflict_markers(parsed.message.as_ref())
            .to_str_lossy()
            .trim_end()
            .to_owned(),
        parent: parsed.parents.first().copied().map(CommitId),
        tree: parsed.tree,
    })
}

/// The target commit the workspace was on when snapshot `id` was taken, for a snapshot
/// that records it.
fn snapshot_target(repo: &gix::Repository, id: gix::ObjectId) -> Option<gix::ObjectId> {
    // The stored target commit, or with none stored, the one it stood for then.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Meta {
        target_commit_id: Option<String>,
        resolved_target_commit_id: Option<String>,
    }
    let tree = repo.find_commit(id).ok()?.tree().ok()?;
    let meta = tree.lookup_entry_by_path("project_meta.toml").ok()??;
    let meta: Meta = toml::from_str(std::str::from_utf8(&meta.object().ok()?.data).ok()?).ok()?;
    let target = meta.target_commit_id.or(meta.resolved_target_commit_id)?;
    gix::ObjectId::from_hex(target.as_bytes()).ok()
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
    // Not the commit GitButler stacks on a conflicted one while it's conflicted, which no one
    // made.
    let made = |c: &&Recorded| {
        !but_core::commit::padding::is_padding_by_message(c.message.as_str().into())
    };
    let mut changes = Vec::new();
    for (name, commits) in after {
        for commit in commits.iter().filter(made) {
            match was.get(&commit.key) {
                None => changes.push(Change::Added {
                    commit: commit.clone(),
                    branch: name.clone(),
                }),
                Some((from, old)) => {
                    if old.message != commit.message || commit.amends(old) {
                        changes.push(Change::Edited {
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
        for commit in (commits.iter().filter(made)).filter(|c| !is.contains_key(&c.key)) {
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
    super::ops::refuse_unfinished(&*ctx.repo.get()?, false)?;
    // Each snapshot is made on the one before, so one taken since `id` has it as its base.
    let repo = ctx.repo.get()?;
    let since =
        |snapshot: &SnapshotId| super::is_ancestor(&repo, id.0, snapshot.0).unwrap_or(false);
    if kept.iter().any(since) {
        bail!(UNRECORDED);
    }
    drop(repo);
    oplog::restore_snapshot_with_kind(&mut ctx, RestoreKind::ExplicitRestoreFromSnapshot, id.0)
}

/// Record the project as it is now, with what it's for when `message` says.
pub fn take_snapshot(path: &Path, message: Option<String>) -> Result<()> {
    let mut ctx = open(path)?;
    oplog::create_snapshot(&mut ctx, message)?;
    Ok(())
}

/// Redo what the undo that recorded `undo` took back, while that's still the last undo and
/// isn't one of `kept`, or `None` when something changed the project since. It stops for
/// uncommitted files written since the undo outside `spans`, as undo does, or since `accepted`
/// when the user said to redo anyway then.
pub fn redo_snapshot(
    path: &Path,
    undo: SnapshotId,
    kept: &HashSet<SnapshotId>,
    spans: &[(i64, i64)],
    accepted: Option<i64>,
) -> Result<Option<Undone>> {
    let mut ctx = open(path)?;
    super::ops::refuse_unfinished(&*ctx.repo.get()?, false)?;
    let target = oplog::get_redo_target_snapshot(&ctx)?;
    let Some(target) = target.filter(|target| target.commit_id == undo.0) else {
        return Ok(None);
    };
    if kept.contains(&SnapshotId(target.commit_id)) {
        bail!(UNRECORDED);
    }
    let since = (target.created_at.seconds * 1000).max(accepted.unwrap_or_default());
    // As for undo, what a terminal did since would be taken back too.
    if moved_since(&ctx, since, spans)?.is_some() {
        return Ok(None);
    }
    let newer = written_since(&ctx, since, spans)?;
    if !newer.is_empty() {
        return Ok(Some(Undone::Newer(newer)));
    }
    let kind = RestoreKind::RestoreFromSnapshotViaRedo;
    restore_titled(&mut ctx, target.commit_id, target.details, kind)
        .map(|op| Some(Undone::Done(op)))
}

/// The newest entry in the project's operation log.
pub fn oplog_head(path: &Path) -> Option<SnapshotId> {
    let ctx = open(path).ok()?;
    let newest = oplog::list_snapshots(&ctx, 1, None, None, None).ok()?;
    newest
        .first()
        .map(|snapshot| SnapshotId(snapshot.commit_id))
}

/// What undoing an operation came to.
pub enum Undone {
    /// It was undone: what it was.
    Done(&'static str),
    /// Something came after it in the project, which undoing it would take back too.
    Behind(String),
    /// Something else undid it already, as `but undo` can: what it was.
    Already(&'static str),
    /// Uncommitted files written since it, which undoing it would put back as they were.
    Newer(Vec<String>),
}

/// Undo the operation that recorded `snapshot`, but only while it's the latest thing to
/// undo: the log is shared with the CLI and agents, and restoring anything else would take
/// back more than the user asked for. With `newer_than`, it also stops for what Git logged
/// since outside its spans, as [`moved_since`] reads it, and for the files [`written_since`].
pub fn undo_snapshot(
    path: &Path,
    snapshot: SnapshotId,
    newer_than: Option<(i64, &[(i64, i64)])>,
) -> Result<Undone> {
    let mut ctx = open(path)?;
    // Git would go on from where it stopped, past what the restore puts back.
    super::ops::refuse_unfinished(&*ctx.repo.get()?, false)?;
    // With nothing left to undo, or only what came before it, something undid it already.
    let before_it = |target: gix::ObjectId| -> Result<bool> {
        Ok(target != snapshot.0 && super::is_ancestor(&*ctx.repo.get()?, target, snapshot.0)?)
    };
    let target = match oplog::get_undo_target_snapshot(&ctx)? {
        Some(target) if !before_it(target.commit_id)? => target,
        _ => {
            let details = oplog::get_snapshot(&ctx, snapshot.0)?.details;
            return Ok(Undone::Already(titled(&ctx, snapshot.0, details)?));
        }
    };
    if target.commit_id != snapshot.0 {
        let ours = titled(
            &ctx,
            snapshot.0,
            oplog::get_snapshot(&ctx, snapshot.0)?.details,
        )?;
        let since = titled(&ctx, target.commit_id, target.details)?;
        return Ok(Undone::Behind(format!(
            "\"{ours}\" can't be undone on its own: \"{since}\" came after it. Operations \
             history can restore to before both."
        )));
    }
    if let Some((after, spans)) = newer_than {
        // As another client's operation would be: the undo would take it back too.
        if let Some(what) = moved_since(&ctx, after, spans)? {
            let ours = titled(&ctx, snapshot.0, target.details)?;
            return Ok(Undone::Behind(format!(
                "\"{ours}\" can't be undone on its own: {what}, outside the app, came after \
                 it. Operations history can restore to before both."
            )));
        }
        let newer = written_since(&ctx, after, spans)?;
        if !newer.is_empty() {
            return Ok(Undone::Newer(newer));
        }
    }
    let kind = RestoreKind::RestoreFromSnapshotViaUndo;
    restore_titled(&mut ctx, snapshot.0, target.details, kind).map(Undone::Done)
}

/// What a terminal or another tool did after `after`, in milliseconds since the Unix epoch,
/// outside `spans`, to what a restore puts back, as Git logged it: a commit, checkout, reset or
/// `update-ref` that moved HEAD or the workspace's branches. The app's own operations log
/// inside `spans`. Branches the workspace doesn't have applied, as agents' in linked worktrees,
/// aren't restored, and neither is the stash, which every worktree shares. A branch renamed
/// outside isn't noticed, as its old name's log goes with it.
fn moved_since(ctx: &but_ctx::Context, after: i64, spans: &[(i64, i64)]) -> Result<Option<String>> {
    use but_core::RefMetadata as _;
    let repo = ctx.repo.get()?;
    let workspace: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
    // As the workspace's metadata has them: one moved off the workspace no longer shows in it.
    let applied: Vec<gix::refs::FullName> =
        (ctx.meta()?.workspace(workspace.as_ref())?.stacks.iter())
            .filter(|stack| stack.is_in_workspace())
            .flat_map(|stack| &stack.branches)
            .map(|branch| branch.ref_name.clone())
            .collect();
    let checked_out = repo.head_name()?;
    let head: gix::refs::FullName = "HEAD".try_into()?;
    let names = [head.clone(), workspace]
        .into_iter()
        .chain(applied)
        .chain(checked_out);
    let mut newest: Option<(i64, String)> = None;
    for name in names {
        let Some(reference) = repo.try_find_reference(name.as_ref())? else {
            continue;
        };
        let mut log = reference.log_iter();
        let Some(lines) = log.rev()? else {
            // HEAD always has a log while Git keeps them: without, nothing here can tell.
            if name == head {
                anyhow::bail!(
                    "Git keeps no log of what moved HEAD here, as with core.logAllRefUpdates \
                     off, so undo can't tell what happened since. Operations history can still \
                     restore to before it."
                );
            }
            continue;
        };
        // Whole seconds, so a line logged in `after`'s second or a span's may be on either side.
        let line = (lines.filter_map(Result::ok))
            .map(|line| (line.signature.time.seconds * 1000, line))
            .take_while(|(at, _)| at + 1000 > after)
            // A stash, an abort or `git reset --hard HEAD` logs a reset to where it was, which a
            // restore doesn't take back; a rename or `git switch -c` moves nothing either, but
            // a restore would undo it.
            .filter(|(_, line)| {
                line.previous_oid != line.new_oid || !line.message.starts_with(b"reset:")
            })
            // The app never logs a "commit: …" or "commit (amend): …" line, so one Git logged in
            // a second wholly after `after` is an agent's, even in a later operation's second.
            .find(|(at, line)| {
                (*at > after && line.message.starts_with(b"commit"))
                    || !(spans.iter()).any(|(from, to)| (*from - 1000..=*to).contains(at))
            });
        if let Some((at, line)) = line
            && newest.as_ref().is_none_or(|(newest, _)| at > *newest)
        {
            let what = match line.message.trim().to_str_lossy() {
                message if message.is_empty() => format!("a change to {}", short_ref(&name)),
                message => format!("\"{message}\""),
            };
            newest = Some((at, what));
        }
    }
    Ok(newest.map(|(_, what)| what))
}

/// Uncommitted files written after `after`, in milliseconds since the Unix epoch, outside
/// `spans`, when the app's own operations wrote files.
fn written_since(ctx: &but_ctx::Context, after: i64, spans: &[(i64, i64)]) -> Result<Vec<String>> {
    let repo = ctx.repo.get()?;
    Ok((super::own_changes(&repo)?.changes.iter())
        .filter(|change| {
            let metadata = super::metadata(&repo, change.path.as_bstr());
            metadata.as_ref().and_then(super::millis).is_some_and(|at| {
                at > after && !spans.iter().any(|(from, to)| (*from..=*to).contains(&at))
            })
        })
        .map(|change| change.path.to_str_lossy().into_owned())
        .collect())
}

/// What the app calls an operation, where the oplog's name for it is another client's.
fn operation_title(kind: OperationKind) -> &'static str {
    use OperationKind as K;
    match kind {
        K::UndoCommit => "Uncommitted",
        K::UpdateCommitMessage => "Reworded commit",
        K::OnDemandSnapshot => "Manual snapshot",
        K::InsertBlankCommit => "Empty commit added",
        K::MergeUpstream | K::UpdateWorkspaceBase => "Updated from target",
        K::SetBaseBranch => "Changed target",
        other => other.title(),
    }
}

/// What the app calls the operation snapshot `id`, recorded with `details`, was taken for, as
/// the CLI names it: when `id` is itself an undo or redo, it's the operation that one restored.
fn titled(
    ctx: &but_ctx::Context,
    id: gix::ObjectId,
    details: Option<SnapshotDetails>,
) -> Result<&'static str> {
    let details = match oplog::peel_restore_snapshot(ctx, id)? {
        Some(peeled) => peeled.details,
        None => details,
    };
    Ok(details.map_or("Unknown operation", |details| {
        operation_title(details.operation)
    }))
}

/// Restore snapshot `id`, recorded with `details`, and name the operation it takes back.
fn restore_titled(
    ctx: &mut but_ctx::Context,
    id: gix::ObjectId,
    details: Option<SnapshotDetails>,
    kind: oplog::RestoreKind,
) -> Result<&'static str> {
    let title = titled(ctx, id, details)?;
    oplog::restore_snapshot_with_kind(ctx, kind, id)?;
    Ok(title)
}

#[cfg(test)]
mod tests {
    use super::{Branches, Change, CommitId, Recorded, compare};

    fn oid(n: u8) -> gix::ObjectId {
        gix::ObjectId::from_bytes_or_panic(&[n; 20])
    }

    /// Commit `n`, on a parent of its own, so a rewrite with another `n` moves it, with the
    /// same changes for the same `key`.
    fn commit(n: u8, key: &str, message: &str) -> Recorded {
        Recorded {
            id: CommitId(oid(n)),
            key: key.to_owned(),
            message: message.to_owned(),
            parent: Some(CommitId(oid(n.wrapping_add(100)))),
            tree: oid(key.as_bytes()[0]),
        }
    }

    /// Each change as a short line, to compare whole lists at once.
    fn lines(before: &Branches, after: &Branches) -> Vec<String> {
        let changes = compare(before, after);
        (changes.iter())
            .map(|change| match change {
                Change::Added { commit, branch } => format!("+ {} on {branch}", commit.title()),
                Change::Removed { commit, branch } => format!("- {} from {branch}", commit.title()),
                Change::Edited { before, after, .. } if after.amends(before) => {
                    format!("* {} amended", after.title())
                }
                Change::Edited { before, after, .. } => {
                    format!("~ {} was {}", after.title(), before.title())
                }
                Change::Moved { commit, from, to } => {
                    format!("> {} {from} to {to}", commit.title())
                }
                Change::BranchAdded(name) => format!("+ branch {name}"),
                Change::BranchRemoved(name) => format!("- branch {name}"),
                Change::Renamed { from, to } => format!("branch {from} now {to}"),
                Change::TargetMoved { .. } => panic!("compare leaves the target alone"),
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
        let padded: Branches = vec![
            (
                "feat".into(),
                vec![
                    commit(10, "p", but_core::commit::padding::TITLE),
                    commit(2, "b", "Two"),
                    commit(1, "a", "One"),
                ],
            ),
            ("fix".into(), vec![commit(3, "c", "Three")]),
        ];
        assert!(
            lines(&before, &padded).is_empty() && lines(&padded, &before).is_empty(),
            "the commit GitButler pads a conflicted one with comes and goes unlisted"
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

    #[test]
    fn an_amend_shows_but_a_rebase_does_not() {
        let before: Branches = vec![(
            "feat".into(),
            vec![commit(2, "b", "Two"), commit(1, "a", "One")],
        )];
        // "One" gets new changes in its place, and "Two" is rebased onto it.
        let amended = Recorded {
            id: CommitId(oid(3)),
            tree: oid(9),
            ..commit(1, "a", "One")
        };
        let after: Branches = vec![("feat".into(), vec![commit(4, "b", "Two"), amended])];
        assert_eq!(
            lines(&before, &after),
            ["* One amended"],
            "the commit above was only rebased onto the amended one"
        );
    }
}
