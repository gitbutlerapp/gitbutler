//! Helpers for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//! Enumeration, archived-state reconciliation, and `HEAD` resolution are
//! centralized in `but-ctx`, keeping this crate independent of it.

use std::path::Path;

use anyhow::{Context as _, bail};
use bstr::{BStr, BString};
use but_core::{DiffSpec, RepositoryExt};

/// What a linked worktree's own commits are resting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeBase {
    /// The base commit is owned by one of the workspace stacks - or by a worktree listed
    /// earlier in [tip order](but_graph::Graph::worktree_tips), for worktrees stacked on
    /// each other - so the worktree branches off the workspace and belongs *inside* it
    /// when presented.
    InWorkspace(gix::ObjectId),
    /// The base commit is outside the workspace, i.e. it is the target commit or below it,
    /// so the worktree stands on its own.
    Outside(gix::ObjectId),
}

impl WorktreeBase {
    /// The commit the worktree's own commits are resting on.
    pub fn commit_id(&self) -> gix::ObjectId {
        match self {
            WorktreeBase::InWorkspace(id) | WorktreeBase::Outside(id) => *id,
        }
    }
}

/// A non-archived linked worktree along with the commits it owns exclusively, i.e. the commits
/// between its `HEAD` and the workspace (or the target).
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub name: BString,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to, as re-resolved during traversal.
    pub head: gix::ObjectId,
    /// What [`Self::commits`] are resting on.
    ///
    /// This is `None` only if the traversal ran out of graph before reaching the workspace or the
    /// target, which happens for worktrees on unrelated history or when a traversal limit was hit.
    pub base: Option<WorktreeBase>,
    /// The commits owned by this worktree alone, from its `HEAD` down to (excluding) its
    /// [base](Self::base), along the first parent.
    ///
    /// Empty if the worktree `HEAD` is itself a workspace commit.
    pub commits: Vec<crate::ref_info::LocalCommit>,
}

/// Project the linked worktrees that seeded `workspace`'s traversal into the commits they own.
///
/// Returns an empty list when the traversal wasn't seeded with worktree tips, which is how the
/// `worktreeManipulation` feature flag switches this off - the flag decides whether
/// [`worktrees`](but_graph::init::Options::worktrees) is set, and everything here rides on that.
///
/// The tips are the ones the traversal itself resolved, so a ref that vanished by then was
/// already dropped rather than resurrected at its stale commit. A worktree that cannot be
/// projected - its tip is not in the graph, or one of its commits fails to load - is skipped
/// with a warning rather than failing the whole projection: worktree commits are decoration,
/// and a single broken worktree must not take down the workspace view.
pub fn worktree_infos(
    workspace: &but_graph::Workspace,
    repo: &gix::Repository,
) -> Vec<WorktreeInfo> {
    let graph = &workspace.graph;
    if graph.worktree_tips.is_empty() {
        return Vec::new();
    }

    // Commits no worktree may own: everything in the workspace stacks, plus - as tips are
    // processed - the commits already claimed by an earlier worktree, so worktrees stacked
    // on each other don't repeat each other's commits.
    let mut off_limits: gix::hashtable::HashSet<gix::ObjectId> = workspace
        .stacks
        .iter()
        .flat_map(|stack| stack.segments.iter())
        .flat_map(|segment| segment.commits.iter().map(|commit| commit.id))
        .collect();

    let mut out = Vec::new();
    for tip in &graph.worktree_tips {
        let head = tip.id;
        let Ok(sidx) = graph.segment_id_by_commit_id(head) else {
            // Another tip may have claimed the commit for a segment we can't walk from, or a
            // traversal limit cut it off. Either way there is nothing to show.
            tracing::warn!(
                worktree = %tip.name,
                %head,
                "Worktree tip is not part of the graph, skipping it"
            );
            continue;
        };
        let (commits, base) = commits_and_base(graph, sidx, head, &off_limits, workspace);
        let local_commits: Vec<_> = match commits
            .iter()
            .map(|commit| crate::ref_info::LocalCommit::try_from_stack_commit(commit, repo))
            .collect()
        {
            Ok(local_commits) => local_commits,
            Err(err) => {
                tracing::warn!(
                    worktree = %tip.name,
                    %head,
                    ?err,
                    "Failed to load a worktree commit, skipping the worktree"
                );
                continue;
            }
        };
        off_limits.extend(commits.iter().map(|commit| commit.id));
        out.push(WorktreeInfo {
            name: tip.name.clone(),
            ref_name: tip.ref_name.clone(),
            head,
            base,
            commits: local_commits,
        });
    }
    out
}

/// Walk down from `head` (owned by `sidx`) along the first parent, collecting commits until
/// reaching a commit in `off_limits` or the target of `workspace`.
fn commits_and_base(
    graph: &but_graph::Graph,
    sidx: but_graph::SegmentIndex,
    head: gix::ObjectId,
    off_limits: &gix::hashtable::HashSet<gix::ObjectId>,
    workspace: &but_graph::Workspace,
) -> (Vec<but_graph::workspace::StackCommit>, Option<WorktreeBase>) {
    let target_commit_id = workspace.target_commit.as_ref().map(|t| t.commit_id);

    let mut commits = Vec::new();
    let mut base = None;
    // `head` can sit in the middle of its segment when another tip owns the segment's first commit.
    let mut before_head = true;
    graph.visit_segments_downward_along_first_parent_include_start(sidx, |segment| {
        for commit in &segment.commits {
            if before_head {
                if commit.id != head {
                    continue;
                }
                before_head = false;
            }
            if off_limits.contains(&commit.id) {
                base = Some(WorktreeBase::InWorkspace(commit.id));
                return true;
            }
            // Reachable from the target branch means at or below the target - a per-commit
            // property, unlike global measures such as segment generations, which cannot tell
            // deep unrelated history apart from history below the target. The stored target
            // commit is checked as well in case the target branch was rewritten and no longer
            // reaches it.
            if commit.flags.contains(but_graph::CommitFlags::Integrated)
                || target_commit_id == Some(commit.id)
            {
                base = Some(WorktreeBase::Outside(commit.id));
                return true;
            }
            commits.push(but_graph::workspace::StackCommit::from_graph_commit(commit));
        }
        false
    });
    (commits, base)
}

/// Open the linked worktree named `name` as a from-disk repository.
///
/// It shares `repo`'s object database and has no object memory, so objects written
/// through it land loose on disk and are immediately visible to in-memory
/// repositories built on the same database - which is what makes it usable as the
/// source repository of a worktree-sourced commit or amend.
pub fn open_worktree_repo(repo: &gix::Repository, name: &BStr) -> anyhow::Result<gix::Repository> {
    let proxy = repo
        .worktrees()?
        .into_iter()
        .find(|proxy| proxy.id() == name)
        .with_context(|| format!("Worktree {name} does not exist"))?;
    proxy.into_repo().map_err(Into::into)
}

/// The time of the newest reflog entry of the linked worktree named `name`, across its own
/// `HEAD` log and the log of the branch it has checked out, or `None` if neither has one.
///
/// The branch log sees updates made from any checkout, while the `HEAD` log sees checkouts and
/// commits made inside the worktree and is all a detached worktree has.
pub fn updated_at(repo: &gix::Repository, name: &BStr) -> anyhow::Result<Option<gix::date::Time>> {
    let wt_repo = open_worktree_repo(repo, name)?;
    let mut newest: Option<gix::date::Time> = None;
    for ref_name in std::iter::once("HEAD".try_into()?).chain(wt_repo.head_name()?) {
        let Some(reference) = wt_repo.try_find_reference(ref_name.as_ref())? else {
            continue;
        };
        let mut log = reference.log_iter();
        let Some(mut lines) = log.rev()? else {
            continue;
        };
        let Some(line) = lines.next().transpose()? else {
            continue;
        };
        let time = line.signature.time;
        if newest.is_none_or(|newest| time.seconds > newest.seconds) {
            newest = Some(time);
        }
    }
    Ok(newest)
}

/// Remove the linked worktree checked out at `path` the way `git worktree remove` does, which
/// refuses a dirty checkout unless `force`, and a locked one until it is unlocked.
///
/// Git is invoked directly as it has the only implementation of this, and its own error
/// message is surfaced on failure.
pub fn remove(repo: &gix::Repository, path: &Path, force: bool) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new(gix::path::env::exe_invocation());
    // These would override `-C`.
    for var in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"] {
        cmd.env_remove(var);
    }
    cmd.arg("-C")
        .arg(repo.workdir().unwrap_or(repo.common_dir()))
        .args(["worktree", "remove"]);
    if force {
        cmd.arg("--force");
    }
    let output = cmd
        .arg("--")
        .arg(path)
        .output()
        .context("Failed to run `git worktree remove`")?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// The outcome of [`move_uncommitted_changes()`].
#[derive(Debug, Clone, Copy)]
pub struct MoveUncommittedChangesOutcome {
    /// Conflict markers were written into `main_repo`'s working directory and index.
    pub conflict_occurred: bool,
}

/// Move some or all of the uncommitted changes of the linked worktree `worktree_repo` into the
/// uncommitted changes of `main_repo`.
///
/// `worktree_repo` must share `main_repo`'s object database and have no object memory, as
/// returned by [`open_worktree_repo()`] - the same requirement `ChangeSource::Worktree`
/// (`crate::commit`) has, since this writes loose objects through it that `main_repo` must see
/// immediately.
///
/// If `selection` is `Some`, only those changes are moved - matched against the worktree's
/// current uncommitted changes the way `DiffSpec` selections are matched elsewhere, with
/// `context_lines` used to re-derive hunks and required to match whatever produced the
/// `DiffSpec`s. If `None`, every uncommitted change in the worktree is moved and `context_lines`
/// is unused.
pub fn move_uncommitted_changes(
    main_repo: &gix::Repository,
    worktree_repo: &gix::Repository,
    selection: Option<Vec<DiffSpec>>,
    context_lines: u32,
) -> anyhow::Result<MoveUncommittedChangesOutcome> {
    let worktree_head_tree = worktree_repo.head_tree_id_or_empty()?.detach();

    let moved_tree = match selection {
        None =>
        {
            #[expect(deprecated)]
            worktree_repo.create_wd_tree(0)?
        }
        Some(selection) => {
            if selection.is_empty() {
                bail!("No changes were selected to move");
            }
            let outcome = but_core::tree::create_tree(
                worktree_repo,
                worktree_head_tree,
                selection,
                context_lines,
            )?;
            let rejected: Vec<_> = outcome
                .rejected_specs
                .iter()
                .map(|(reason, spec)| format!("{reason:?}: {}", spec.path))
                .collect();
            if !rejected.is_empty() {
                bail!(
                    "Some selected changes no longer match the worktree's uncommitted changes: {}",
                    rejected.join(", ")
                );
            }
            outcome.destination_tree.context("No changes to move")?
        }
    };
    if moved_tree == worktree_head_tree {
        bail!("No changes to move");
    }

    let destination_outcome = but_core::worktree::safe_checkout_from_head(
        moved_tree,
        main_repo,
        but_core::worktree::checkout::Options {
            merge_base_override: Some(worktree_head_tree),
            allow_uncommitted_changes_to_conflict_with_new_head: true,
            ..Default::default()
        },
    )?;

    but_core::worktree::safe_checkout_from_head(
        worktree_head_tree,
        worktree_repo,
        but_core::worktree::checkout::Options {
            merge_base_override: Some(moved_tree),
            allow_uncommitted_changes_to_conflict_with_new_head: true,
            ..Default::default()
        },
    )?;

    Ok(MoveUncommittedChangesOutcome {
        conflict_occurred: destination_outcome.conflict_occurred,
    })
}
