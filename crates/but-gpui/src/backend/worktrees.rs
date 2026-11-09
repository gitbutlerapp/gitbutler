//! Linked worktrees, like the ones agents work in, each with its uncommitted changes. They
//! are listed when GitButler's experimental worktree support is on; archived ones are hidden.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use bstr::{BStr, BString, ByteSlice as _};
use but_ctx::Context;

use super::intent::Run;
use super::{
    FileChange, Intent, KeptWorktree, OpKind, Workspace, Worktree, checkout, git_operation,
    head_tree, open, worktree_files,
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
            let ((uncommitted, tree), git_operation) = match kept.remove(&name) {
                Some((files, read_against)) if read_against == tree => {
                    ((files, tree), git_operation(&checkout))
                }
                previous => read(&checkout, name.as_ref(), previous.unwrap_or_default()),
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
        let previous = (std::mem::take(&mut worktree.uncommitted), worktree.tree);
        let ((files, tree), operation) = read(&checkout, worktree.name.as_ref(), previous);
        worktree.uncommitted = files;
        worktree.tree = tree;
        worktree.git_operation = operation;
        true
    });
    tracing::debug!(elapsed = ?started.elapsed(), main, ?linked, "reloaded uncommitted files");
    Ok(shown)
}

/// The uncommitted files of worktree `name` with the tree they were read against, and the Git
/// command it's partway through. When they can't be read, as when a file goes while it's read,
/// they stay as `previous` was read: only [`opened`] and the listing say a worktree is gone.
fn read(
    checkout: &gix::Repository,
    name: &BStr,
    previous: KeptWorktree,
) -> (KeptWorktree, Option<&'static str>) {
    let tree = head_tree(checkout);
    let kept = match worktree_files(checkout, &previous.0) {
        Ok(files) => (files, tree),
        Err(err) => {
            tracing::warn!("couldn't read worktree {name}: {err:#}");
            previous
        }
    };
    (kept, git_operation(checkout))
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

/// Where each linked worktree's `HEAD` points, by the worktree's id: the branch it has checked
/// out and its commit, to tell whether an operation moved one.
#[derive(Debug, PartialEq)]
pub struct Heads(HashMap<BString, Option<(Option<gix::refs::FullName>, gix::ObjectId)>>);

impl Heads {
    /// Whether a worktree there both now and `before` points elsewhere. One added or removed
    /// meanwhile, as a worktree deleted beside the operation is, didn't move.
    pub fn moved_since(&self, before: &Heads) -> bool {
        (self.0.iter()).any(|(id, head)| before.0.get(id).is_some_and(|was| was != head))
    }

    /// The worktrees, by name, that had a branch an update found `merged` checked out, as these
    /// were before it, with work of their own and nothing uncommitted now. Work of their own is
    /// a newest commit that isn't on `main_line`, the target's own line from its tip down to
    /// where the workspace stood: one only caught up with the target, by `git pull` or made at
    /// its tip, has one of those, though the update counts it merged too. A branch merged by
    /// fast-forward has its commits on that line as well, so it stays.
    pub(super) fn merged_with_own_work(
        &self,
        ctx: &Context,
        merged: &[gix::refs::FullName],
        main_line: &HashSet<super::CommitId>,
    ) -> Vec<BString> {
        let own_work = |head: &Option<(Option<gix::refs::FullName>, gix::ObjectId)>| {
            head.as_ref().is_some_and(|(branch, tip)| {
                branch
                    .as_ref()
                    .is_some_and(|branch| merged.contains(branch))
                    && !main_line.contains(&super::CommitId(*tip))
            })
        };
        // The update gives conflict markers as uncommitted changes, so those stay too.
        let clean = |name: &BString| {
            (checkout(ctx, Some(name.as_ref())).ok())
                .and_then(|repo| worktree_files(&repo, &[]).ok())
                .is_some_and(|files| files.is_empty())
        };
        (self.0.iter())
            .filter(|(_, head)| own_work(head))
            .map(|(name, _)| name)
            .filter(|name| clean(name))
            .cloned()
            .collect()
    }
}

pub fn heads(repo: &gix::Repository) -> Heads {
    let heads = (repo.worktrees().into_iter().flatten()).filter_map(|wt| {
        let id = wt.id().ok()?.to_owned();
        let head = (|| {
            let repo = wt.into_repo().ok()?;
            Some((repo.head_name().ok()?, repo.head_id().ok()?.detach()))
        })();
        Some((id, head))
    });
    Heads(heads.collect())
}

/// Make a linked worktree, as `but worktree new` does: on a new branch at the workspace's base,
/// named or with a made-up name, or on a branch outside the workspace, a remote one's local
/// tracking branch made first as `git checkout` does. Done is the worktree's name.
pub enum CreateWorktree {
    New(Option<String>),
    On(super::FullName),
}

impl Intent for CreateWorktree {
    type Done = BString;

    fn apply(self, run: &mut Run) -> Result<BString> {
        let created = match self {
            Self::New(name) => {
                let ref_name = name.as_deref().map(super::ops::branch_ref).transpose()?;
                run.locked(|ctx, perm| {
                    but_api::worktrees::worktree_new_with_perm(ctx, ref_name, perm)
                })?
            }
            Self::On(branch) => run.locked(|ctx, perm| {
                let local = match branch.category() {
                    Some(gix::refs::Category::RemoteBranch) => {
                        but_workspace::branch::local_tracking_branch(
                            &*ctx.repo.get()?,
                            branch.as_ref(),
                        )?
                    }
                    _ => branch,
                };
                but_api::worktrees::worktree_new_on_branch_with_perm(ctx, local, perm)
            })?,
        };
        Ok(created.name)
    }
}

/// Remove worktree `name` from disk as `git worktree remove` does, with the uncommitted
/// files `lost`, by their write times and sizes then, which the user agreed to lose. Its
/// branch stays. One with other uncommitted files, or those written to since, as by an agent
/// meanwhile, is refused, and so is one with `lost` files that can't be read to check them.
/// With `keep_if_changed`, as for one an update found merged, one with any uncommitted file
/// stays instead, without an error.
pub struct DeleteWorktree {
    pub name: BString,
    pub lost: Vec<(String, Option<i64>, Option<u64>)>,
    pub keep_if_changed: bool,
}

impl Intent for DeleteWorktree {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        let Self {
            name,
            lost,
            keep_if_changed,
        } = self;
        run.locked(|ctx, perm| {
            // With nothing to lose, nothing is forced, and Git refuses a worktree with changes,
            // so one that can't be opened, as when its folder is gone, can still be removed.
            let files = match checkout(ctx, Some(name.as_ref())) {
                Ok(worktree) => worktree_files(&worktree, &[])?,
                Err(_) if lost.is_empty() => Vec::new(),
                Err(err) => return Err(err.context(format!("Couldn't check {name}'s files"))),
            };
            if keep_if_changed && !files.is_empty() {
                return Ok(());
            }
            let asked =
                |file: &FileChange| lost.contains(&(file.path.clone(), file.modified, file.size));
            if let Some(file) = files.iter().find(|file| !asked(file)) {
                anyhow::bail!(
                    "{name} has a change you weren't asked about, {}. Look at it before deleting.",
                    file.path
                );
            }
            but_api::worktrees::worktree_remove_with_perm(
                ctx,
                name.as_ref(),
                !lost.is_empty(),
                perm,
            )
        })
    }

    fn kind(&self) -> OpKind {
        OpKind::DeleteWorktree(self.name.clone())
    }
}

/// Hide worktree `name` from the workspace, or with `archived` false show it again. It
/// stays on disk either way.
pub struct SetArchived {
    pub name: BString,
    pub archived: bool,
}

impl Intent for SetArchived {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        run.locked(|ctx, perm| {
            but_api::worktrees::worktree_set_archived_with_perm(
                ctx,
                self.name.as_ref(),
                self.archived,
                perm.read_permission(),
            )
        })
    }
}

/// Hide worktrees `names` from the workspace, as an update that found their branches merged
/// leaves them, in one operation. They stay on disk.
pub struct ArchiveWorktrees(pub Vec<BString>);

impl Intent for ArchiveWorktrees {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        run.locked(|ctx, perm| {
            for name in &self.0 {
                but_api::worktrees::worktree_set_archived_with_perm(
                    ctx,
                    name.as_ref(),
                    true,
                    perm.read_permission(),
                )?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::backend::ops::tests::{agent_worktree, git, repo_with, target_origin_main};

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

    /// A repository on `main` targeting `origin/main`, whose linked worktrees GitButler lists.
    fn adopted_repo() -> tempfile::TempDir {
        let dir = repo_with("one\n");
        target_origin_main(dir.path());
        let ctx = crate::backend::open(dir.path()).unwrap();
        (ctx.db.get_cache_mut().unwrap())
            .worktree_meta_mut()
            .mark_adopted()
            .unwrap();
        dir
    }

    /// Update everything from the target, as the app's Update does: the branches it found
    /// merged, and the worktrees whose own work merged, by name in order.
    fn update(path: &std::path::Path) -> (Vec<String>, Vec<String>) {
        crate::backend::upstream::tests::without_askpass();
        let update = crate::backend::upstream::Update {
            scope: crate::backend::upstream::UpdateScope::All,
            accepted: None,
        };
        let outcome = crate::backend::apply(path, update).unwrap().0;
        let crate::backend::upstream::UpdateOutcome::Updated {
            merged,
            merged_worktrees,
            ..
        } = outcome
        else {
            panic!("{outcome:?}");
        };
        let mut merged: Vec<String> = merged.iter().map(|b| b.shorten().to_string()).collect();
        let mut worktrees: Vec<String> = merged_worktrees.iter().map(|n| n.to_string()).collect();
        merged.sort();
        worktrees.sort();
        (merged, worktrees)
    }

    #[test]
    fn an_update_merged_the_worktrees_whose_work_the_target_has() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = adopted_repo();
            let path = dir.path();
            let outside = tempfile::tempdir().unwrap();
            let at = |name: &str| outside.path().join(name);
            for name in ["merged", "squashed", "dirty", "open", "fresh"] {
                let dir = at(name);
                git(
                    path,
                    &["worktree", "add", "-q", "-b", name, dir.to_str().unwrap()],
                );
            }
            for name in ["merged", "squashed", "dirty", "open"] {
                std::fs::write(at(name).join(name), name).unwrap();
                git(&at(name), &["add", name]);
                git(&at(name), &["commit", "-q", "-m", name]);
            }
            git(path, &["checkout", "-q", "--detach"]);
            for name in ["merged", "dirty"] {
                git(path, &["merge", "-q", "--no-ff", "--no-edit", name]);
            }
            git(path, &["merge", "-q", "--squash", "squashed"]);
            git(path, &["commit", "-q", "-m", "squashed in"]);
            git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
            git(path, &["checkout", "-q", "main"]);
            std::fs::write(at("dirty").join("wip"), "wip").unwrap();
            // Made from the target's tip, past the workspace's target commit, as an agent might.
            let dir = at("ahead");
            let ahead = ["worktree", "add", "-q", "-b", "ahead"];
            git(
                path,
                &[&ahead[..], &[dir.to_str().unwrap(), "origin/main"]].concat(),
            );

            let (merged, worktrees) = update(path);
            assert!(
                merged.iter().any(|branch| branch == "ahead"),
                "the update counts ahead merged, as the target has all it has: {merged:?}"
            );
            assert_eq!(
                worktrees,
                ["merged", "squashed"],
                "the target has their commits, though neither branch was ever pushed; dirty has \
                 uncommitted files, open's commit is only here, fresh has none, and ahead's are \
                 the target's own"
            );
        });
    }

    #[test]
    fn a_worktree_only_caught_up_with_the_target_has_no_work_merged() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = adopted_repo();
            let path = dir.path();
            git(path, &["checkout", "-q", "--detach"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "teammate"]);
            git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
            git(path, &["checkout", "-q", "main"]);
            let (_outside, linked) = agent_worktree(path);
            // An agent catching up before starting work.
            git(&linked, &["merge", "-q", "--ff-only", "origin/main"]);

            let (merged, worktrees) = update(path);
            assert_eq!(
                merged,
                ["agent"],
                "the update counts it merged, as the target has all it has"
            );
            assert!(
                worktrees.is_empty(),
                "but its newest commit is the teammate's, on the target's own line"
            );
        });
    }

    #[test]
    fn a_worktree_merged_by_fast_forward_has_no_work_merged_either() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = adopted_repo();
            let path = dir.path();
            let (_outside, linked) = agent_worktree(path);
            git(&linked, &["commit", "-q", "--allow-empty", "-m", "work"]);
            // As `git push origin agent:main` does.
            git(path, &["update-ref", "refs/remotes/origin/main", "agent"]);

            let (merged, worktrees) = update(path);
            assert_eq!(merged, ["agent"], "the target has its commit");
            assert!(
                worktrees.is_empty(),
                "its commit is on the target's own line now, which a worktree only caught up \
                 has too, so it's kept rather than risk one that's being worked in"
            );
        });
    }

    #[test]
    fn a_linked_worktree_that_cant_be_read_keeps_its_files() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            let (_outside, linked) = agent_worktree(path);
            std::fs::write(linked.join("file"), "two\n").unwrap();
            let shown = crate::backend::Workspace {
                worktrees: vec![crate::backend::Worktree {
                    name: "agent".into(),
                    path: linked,
                    branch: None,
                    uncommitted: Vec::new(),
                    git_operation: None,
                    tree: None,
                    updated_at: None,
                }],
                ..Default::default()
            };
            let agent = std::collections::HashSet::from(["agent".into()]);
            let reload = |shown| super::reload_uncommitted(path, shown, false, &agent).unwrap();
            let shown = reload(shown);
            std::fs::write(path.join(".git/worktrees/agent/index"), "garbage").unwrap();

            let shown = reload(shown);
            let files: Vec<_> = (shown.worktrees.iter())
                .flat_map(|worktree| worktree.uncommitted.iter().map(|file| file.path.as_str()))
                .collect();
            assert_eq!(
                files,
                ["file"],
                "a read that fails keeps the worktree and its files, and so its terminals"
            );
        });
    }
}
