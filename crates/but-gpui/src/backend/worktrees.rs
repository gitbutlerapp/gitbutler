//! Linked worktrees, like the ones agents work in, each with its uncommitted changes. They
//! are listed when GitButler's experimental worktree support is on; archived ones are hidden.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use bstr::{BStr, BString, ByteSlice as _};
use but_ctx::Context;

use super::{
    FileChange, KeptWorktree, Workspace, Worktree, checkout, git_operation, head_tree, open,
    worktree_files,
};

/// The active linked worktrees with their uncommitted changes, and the names of the
/// archived ones with the branch each has checked out.
type Listing = (Vec<Worktree>, Vec<(BString, Option<super::FullName>)>);

/// The project's [`Listing`]. A worktree's files in `kept` are shown again rather than read,
/// while its `HEAD` has the tree they were read against.
pub(super) fn linked(ctx: &Context, kept: &mut HashMap<BString, KeptWorktree>) -> Result<Listing> {
    let listing = {
        let guard = ctx.shared_worktree_access();
        but_api::worktrees::worktrees_list_with_perm(ctx, guard.read_permission())?
    };
    let active = (listing.active.into_iter())
        .filter_map(|listed| {
            let name = listed.name;
            let checkout = opened(ctx, name.as_ref())?;
            let tree = head_tree(&checkout);
            // What Git is partway through is cheap to read, and changes without the files.
            let (uncommitted, git_operation) = match kept.remove(&name) {
                Some((files, read_against)) if read_against == tree => {
                    (files, git_operation(&checkout))
                }
                Some((files, _)) => read(&checkout, name.as_ref(), &files)?,
                None => read(&checkout, name.as_ref(), &[])?,
            };
            Some(Worktree {
                name,
                path: listed.path,
                branch: listed.ref_name,
                uncommitted,
                git_operation,
                tree,
                updated_at: listed.updated_at_ms.map(|ms| ms / 1000),
            })
        })
        .collect();
    let archived = (listing.archived.iter())
        .map(|listed| {
            let branch = listed.ref_name.clone();
            (listed.name.clone(), branch)
        })
        .collect();
    Ok((active, archived))
}

/// `shown` with the uncommitted files read again of the main checkout, with `main`, and of
/// the `linked` worktrees, for when only files changed.
pub fn reload_uncommitted(
    path: &Path,
    mut shown: Workspace,
    main: bool,
    linked: &HashSet<BString>,
) -> Result<Workspace> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    if main {
        shown.uncommitted = worktree_files(&repo, &shown.uncommitted)?;
        shown.tree = head_tree(&repo);
    }
    shown.worktrees.retain_mut(|worktree| {
        if !linked.contains(&worktree.name) {
            return true;
        }
        let Some(checkout) = opened(&ctx, worktree.name.as_ref()) else {
            return false;
        };
        match read(&checkout, worktree.name.as_ref(), &worktree.uncommitted) {
            Some((files, operation)) => {
                worktree.uncommitted = files;
                worktree.git_operation = operation;
                worktree.tree = head_tree(&checkout);
                true
            }
            None => false,
        }
    });
    tracing::debug!(elapsed = ?started.elapsed(), main, ?linked, "reloaded uncommitted files");
    Ok(shown)
}

/// The uncommitted files of worktree `name` and the Git command it's partway through, or
/// nothing when it can't be read, as when an agent just removed it: the next full load lists
/// the worktrees as they are.
fn read(
    checkout: &gix::Repository,
    name: &BStr,
    previous: &[FileChange],
) -> Option<(Vec<FileChange>, Option<&'static str>)> {
    (worktree_files(checkout, previous))
        .inspect_err(|err| tracing::warn!("couldn't read worktree {name}: {err:#}"))
        .ok()
        .map(|files| (files, git_operation(checkout)))
}

/// Worktree `name`'s own repository, to read its checkout from, or nothing when it can't be
/// opened, as [`read`] says.
fn opened(ctx: &Context, name: &BStr) -> Option<gix::Repository> {
    let repo = (checkout(ctx, Some(name)))
        .inspect_err(|err| tracing::warn!("couldn't open worktree {name}: {err:#}"))
        .ok()?;
    // Deleted without Git, as by `rm -rf`, Git still lists it, and its files as last read
    // would show on with nothing changing them.
    repo.workdir().is_some_and(Path::exists).then_some(repo)
}

/// What moved linked worktree `name`'s `HEAD` since `since`, in seconds since the epoch, other
/// than its commits, which its branch lists: checkouts, rebases, resets, merges and amends,
/// newest first, each with when, said as a sentence.
pub fn events(path: &Path, name: &BStr, since: i64) -> Result<Vec<(i64, String)>> {
    let repo = gix::open(path)?;
    let Some(worktree) = repo.worktree_proxy_by_id(name)? else {
        return Ok(Vec::new());
    };
    // Its log is in the repository's own folder, which outlasts the checkout's.
    let worktree = worktree.into_repo_with_possibly_inaccessible_worktree()?;
    let head = worktree.find_reference("HEAD")?;
    let mut log = head.log_iter();
    let Some(lines) = log.rev()? else {
        return Ok(Vec::new());
    };
    Ok((lines.filter_map(Result::ok))
        .take_while(|line| line.signature.time.seconds >= since)
        .filter_map(|line| {
            let said = event_said(line.message.to_str().ok()?)?;
            Some((line.signature.time.seconds, said))
        })
        .collect())
}

/// A reflog `message` said as an event, unless it's a commit, which the branch lists, or
/// says nothing, as `git stash`'s reset to `HEAD` doesn't.
fn event_said(message: &str) -> Option<String> {
    let after = |prefix: &str| message.strip_prefix(prefix);
    if let Some(moving) = after("checkout: moving from ") {
        return Some(format!("Checked out {}", moving.rsplit(" to ").next()?));
    }
    if let Some(returning) = after("rebase (finish): returning to ") {
        let branch = returning.strip_prefix("refs/heads/").unwrap_or(returning);
        return Some(format!("Rebased {branch}"));
    }
    if let Some(to) = after("reset: moving to ").filter(|to| *to != "HEAD") {
        return Some(format!("Reset to {to}"));
    }
    if let Some(title) = after("commit (amend): ") {
        return Some(format!("Amended \"{title}\""));
    }
    if let Some(merged) = after("merge ") {
        return Some(format!("Merged {}", merged.split(':').next()?));
    }
    if message.starts_with("pull") {
        return Some("Pulled".to_owned());
    }
    None
}

/// Where each linked worktree's `HEAD` points, the branch it has checked out and its commit,
/// to tell whether an operation moved one.
pub fn heads(path: &Path) -> Vec<Option<(Option<gix::refs::FullName>, gix::ObjectId)>> {
    let Ok(repo) = gix::open(path) else {
        return Vec::new();
    };
    (repo.worktrees().into_iter().flatten())
        .map(|wt| {
            let repo = wt.into_repo().ok()?;
            Some((repo.head_name().ok()?, repo.head_id().ok()?.detach()))
        })
        .collect()
}

/// Make a linked worktree on new branch `name`, at the workspace's base, as `but worktree new`
/// does. Returns the worktree's name.
pub fn create(path: &Path, name: &str) -> Result<BString> {
    let mut ctx = open(path)?;
    let ref_name = super::ops::branch_ref(name)?;
    let created = but_api::worktrees::worktree_new(&mut ctx, Some(ref_name))?;
    Ok(created.name)
}

/// Remove worktree `name` from disk as `git worktree remove` does, with the uncommitted
/// files `lost`, by their write times and sizes then, which the user agreed to lose. Its
/// branch stays. One with other uncommitted files, or those written to since, as by an agent
/// meanwhile, is refused, and so is one with `lost` files that can't be read to check them.
pub fn remove(path: &Path, name: &BStr, lost: &[(String, Option<i64>, Option<u64>)]) -> Result<()> {
    let mut ctx = open(path)?;
    let mut guard = ctx.exclusive_worktree_access();
    // With nothing to lose, nothing is forced, and Git refuses a worktree with changes, so
    // one that can't be opened, as when its folder is gone, can still be removed.
    let files = match checkout(&ctx, Some(name)) {
        Ok(worktree) => worktree_files(&worktree, &[])?,
        Err(_) if lost.is_empty() => Vec::new(),
        Err(err) => return Err(err.context(format!("Couldn't check {name}'s files"))),
    };
    let asked = |file: &FileChange| lost.contains(&(file.path.clone(), file.modified, file.size));
    if let Some(file) = files.iter().find(|file| !asked(file)) {
        anyhow::bail!(
            "{name} has a change you weren't asked about, {}. Look at it before deleting.",
            file.path
        );
    }
    but_api::worktrees::worktree_remove_with_perm(
        &ctx,
        name,
        !lost.is_empty(),
        guard.write_permission(),
    )
}

/// Hide worktree `name` from the workspace, or with `archived` false show it again. It
/// stays on disk either way.
pub fn set_archived(path: &Path, name: &BStr, archived: bool) -> Result<()> {
    let ctx = open(path)?;
    let guard = ctx.shared_worktree_access();
    but_api::worktrees::worktree_set_archived_with_perm(
        &ctx,
        name,
        archived,
        guard.read_permission(),
    )
}

#[cfg(test)]
mod tests {
    use crate::backend::ops::tests::{agent_worktree, git, repo_with};

    #[test]
    fn a_linked_worktrees_events_are_its_head_moves_newest_first() {
        let dir = repo_with("one\n");
        let path = dir.path();
        let (outside, linked) = agent_worktree(path);
        git(&linked, &["checkout", "-q", "-b", "other"]);
        git(&linked, &["commit", "-q", "--allow-empty", "-m", "work"]);
        git(&linked, &["reset", "-q", "--hard", "agent"]);
        git(path, &["checkout", "-q", "-b", "elsewhere"]);
        std::fs::rename(&linked, outside.path().join("moved")).unwrap();

        let said = |since| -> Vec<String> {
            let events = super::events(path, "agent".into(), since).unwrap();
            events.into_iter().map(|(_, said)| said).collect()
        };
        assert_eq!(
            said(0),
            ["Reset to agent", "Checked out other"],
            "its own moves but its commit, from its log, which outlasts its folder"
        );
        assert!(said(i64::MAX).is_empty(), "and none from before `since`");
    }
}
