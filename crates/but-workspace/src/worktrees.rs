//! Listing and metadata operations for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//!
//! Enumeration and archived-state reconciliation are centralized in `but-ctx` -
//! callers pass the result in as [`WorktreeSource`]s so this crate stays independent
//! of it. The `worktree_meta` table only stores *explicitly set* archived state
//! plus the one-time adoption marker; a worktree without a row is active.

use std::path::PathBuf;

use anyhow::Context as _;
use bstr::{BStr, BString};
use serde::Serialize;

/// A non-archived linked worktree, presented like a single-branch stack.
///
/// This is intentionally slimmer than a workspace stack - linked worktrees have no
/// push status or remote tracking information of their own, and their commits
/// against the target are not computed yet.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStack {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    pub name: BString,
    /// The worktree checkout directory.
    #[serde(with = "but_serde::path_lossy")]
    pub path: PathBuf,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    #[serde(with = "but_serde::fullname_lossy_opt")]
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    #[serde(with = "but_serde::object_id")]
    pub head: gix::ObjectId,
}

/// What a linked worktree's own commits are resting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeBase {
    /// The base commit is owned by one of the workspace stacks, so the worktree branches
    /// off the workspace and belongs *inside* that stack when presented.
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

/// An archived linked worktree, listed with identity information only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedWorktree {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    pub name: BString,
    /// The worktree checkout directory.
    #[serde(with = "but_serde::path_lossy")]
    pub path: PathBuf,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    #[serde(with = "but_serde::fullname_lossy_opt")]
    pub ref_name: Option<gix::refs::FullName>,
}

/// All usable linked worktrees, separated by archived state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeListing {
    /// Non-archived worktrees.
    pub active: Vec<WorktreeStack>,
    /// Archived worktrees, hidden from the workspace but still on disk.
    pub archived: Vec<ArchivedWorktree>,
}

/// A usable linked worktree as input to [`list_worktrees()`].
///
/// Callers typically map this from `but-ctx`'s reconciled worktree enumeration.
#[derive(Debug, Clone)]
pub struct WorktreeSource {
    /// Whether the worktree is archived.
    pub archived: bool,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub name: BString,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    pub head: gix::ObjectId,
}

/// Produce a listing of all worktrees in `sources`, splitting them by archived state.
pub fn list_worktrees(sources: Vec<WorktreeSource>) -> WorktreeListing {
    let mut active = Vec::new();
    let mut archived = Vec::new();
    for source in sources {
        let WorktreeSource {
            archived: is_archived,
            path,
            name,
            ref_name,
            head,
        } = source;
        if is_archived {
            archived.push(ArchivedWorktree {
                name,
                path,
                ref_name,
            });
        } else {
            active.push(WorktreeStack {
                name,
                path,
                ref_name,
                head,
            });
        }
    }
    WorktreeListing { active, archived }
}

/// Project the linked worktrees that seeded `workspace`'s traversal into the commits they own.
///
/// Returns an empty list when the traversal wasn't seeded with worktree tips, which is how the
/// `worktreeManipulation` feature flag switches this off - the flag decides whether `but-ctx` sets
/// [`worktrees`](but_graph::init::Options::worktrees), and everything here rides on that.
///
/// The tips are the ones the traversal itself resolved, so a ref that vanished by then was
/// already dropped rather than resurrected at its stale commit.
pub fn worktree_infos(
    workspace: &but_graph::Workspace,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<WorktreeInfo>> {
    let graph = &workspace.graph;
    if graph.worktree_tips.is_empty() {
        return Ok(Vec::new());
    }

    let workspace_commits: gix::hashtable::HashSet<gix::ObjectId> = workspace
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
        let (commits, base) = commits_and_base(graph, sidx, head, &workspace_commits, workspace);
        out.push(WorktreeInfo {
            name: tip.name.clone(),
            ref_name: tip.ref_name.clone(),
            head,
            base,
            commits: commits
                .iter()
                .map(|commit| crate::ref_info::LocalCommit::try_from_stack_commit(commit, repo))
                .collect::<anyhow::Result<_>>()?,
        });
    }
    Ok(out)
}

/// Walk down from `head` (owned by `sidx`) along the first parent, collecting commits until
/// reaching a commit in `workspace_commits` or the target of `workspace`.
fn commits_and_base(
    graph: &but_graph::Graph,
    sidx: but_graph::SegmentIndex,
    head: gix::ObjectId,
    workspace_commits: &gix::hashtable::HashSet<gix::ObjectId>,
    workspace: &but_graph::Workspace,
) -> (Vec<but_graph::workspace::StackCommit>, Option<WorktreeBase>) {
    let target_commit_id = workspace.target_commit.as_ref().map(|t| t.commit_id);
    // Generations grow downwards, so anything past the target's is below it. This catches worktrees
    // that branch off below the target without passing through the target commit itself.
    let target_generation = workspace
        .target_commit
        .as_ref()
        .map(|t| graph[t.segment_index].generation);

    let mut commits = Vec::new();
    let mut base = None;
    // `head` can sit in the middle of its segment when another tip owns the segment's first commit.
    let mut before_head = true;
    let mut below_target = false;
    graph.visit_segments_downward_along_first_parent_include_start(sidx, |segment| {
        below_target |= target_generation.is_some_and(|generation| segment.generation > generation);
        for commit in &segment.commits {
            if before_head {
                if commit.id != head {
                    continue;
                }
                before_head = false;
            }
            if workspace_commits.contains(&commit.id) {
                base = Some(WorktreeBase::InWorkspace(commit.id));
                return true;
            }
            if below_target || target_commit_id == Some(commit.id) {
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
