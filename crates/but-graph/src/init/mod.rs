use bstr::BString;
use but_core::ref_metadata;
use petgraph::Direction;

use crate::{CommitIndex, Edge, Graph, Segment, SegmentIndex};

pub(crate) mod walk;

pub(crate) mod types;
use types::EdgeOwned;

pub(crate) use crate::init::overlay::{OverlayMetadata, OverlayRepo};

pub(crate) mod remotes;

mod overlay;
mod post;

pub(crate) type Entrypoint = Option<(gix::ObjectId, Option<gix::refs::FullName>)>;

/// A way to define information to be served from memory, instead of from the underlying data source, when
/// [building](crate::nosegment::segment_graph()) the graph.
#[derive(Debug, Default, Clone)]
pub struct Overlay {
    entrypoint: Entrypoint,
    nonoverriding_references: Vec<gix::refs::Reference>,
    overriding_references: Vec<gix::refs::Reference>,
    /// A list of references that should not be picked up anymore in the
    /// re-traversal.
    ///
    /// For example, if the `but_rebase::graph_rebase::Editor` converts a
    /// `Reference` step to a `None` step which is the equivalent of running
    /// `git update-ref -d`, it should no longer be part of the [`Graph`], so we
    /// would list the particular reference as a dropped reference.
    dropped_references: Vec<gix::refs::FullName>,
    meta_branches: Vec<(gix::refs::FullName, ref_metadata::Branch)>,
    branch_stack_orders: Vec<Vec<gix::refs::FullName>>,
    workspace: Option<(gix::refs::FullName, ref_metadata::Workspace)>,
}

pub(super) type PetGraph = petgraph::stable_graph::StableGraph<Segment, Edge>;

#[derive(Default, Debug, Clone)]
pub struct Options {
    pub collect_tags: bool,
    /// The (soft) maximum number of commits we should traverse.
    /// Workspaces with a target branch automatically have unlimited traversals as they rely on the target
    /// branch to eventually stop the traversal.
    ///
    /// If `None`, there is no limit, which typically means that when lacking a workspace, the traversal
    /// will end only when no commit is left to traverse.
    /// `Some(0)` means nothing but the first commit is going to be returned, but it should be avoided.
    ///
    /// Note that this doesn't affect the traversal of integrated commits, which is always stopped once there
    /// is nothing interesting left to traverse.
    ///
    /// Also note: This is a hint and not an exact measure, and it's always possible to receive a more commits
    /// for various reasons, for instance the need to let remote branches find their local branch independently
    /// of the limit.
    pub commits_limit_hint: Option<usize>,
    /// Discover active linked worktrees and seed their `HEAD`s as extra
    /// [traversal tips](Graph::worktree_tips).
    ///
    /// Discovery may run the one-time worktree adoption and thus write to the
    /// database.
    pub worktrees: bool,
}

/// A linked-worktree `HEAD` seeded as an extra traversal tip, see
/// [`Graph::worktree_tips`].
#[derive(Debug, Clone)]
pub struct WorktreeTip {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    ///
    /// Traversal does not consume it, but it travels with the graph for consumers
    /// that need to identify the linked worktree.
    pub name: BString,
    /// The branch the worktree has checked out, if its `HEAD` is symbolic.
    pub ref_name: Option<gix::refs::FullName>,
    /// The peeled `HEAD` commit at caller resolution time.
    pub id: gix::ObjectId,
}

impl Options {
    /// Return options that won't traverse the whole graph if there is no workspace, but will show
    /// more than enough commits by default.
    pub fn limited() -> Self {
        Options {
            collect_tags: false,
            commits_limit_hint: Some(300),
            ..Default::default()
        }
    }
}

impl Options {
    /// Set the maximum amount of commits that each lane in a tip may traverse, but that's less important
    /// than building consistent, connected graphs.
    pub fn with_limit_hint(mut self, limit: usize) -> Self {
        self.commits_limit_hint = Some(limit);
        self
    }
}

/// Discover the active linked worktrees of `repo` and resolve each `HEAD` freshly
/// into a traversal tip; with [collection disabled](Options::worktrees) there is
/// nothing to discover.
///
/// This is where graph construction reads the world: it runs the one-time worktree
/// adoption of [`but_db::worktrees::worktrees_with_state()`], so it may write to `db`.
/// Worktrees with nothing to seed - vanished, unborn, or on the workspace ref - are
/// skipped.
pub fn discover_worktree_tips(
    repo: &gix::Repository,
    db: &mut but_db::DbHandle,
    collect: bool,
) -> anyhow::Result<Vec<WorktreeTip>> {
    if !collect {
        return Ok(Vec::new());
    }
    let mut tips = Vec::new();
    for worktree in but_db::worktrees::worktrees_with_state(repo, db)? {
        if worktree.archived {
            continue;
        }
        let Some(head) = but_db::worktrees::worktree_head(repo, worktree.name.as_ref())? else {
            continue;
        };
        tips.push(WorktreeTip {
            name: worktree.name,
            ref_name: head.ref_name,
            id: head.id,
        });
    }
    Ok(tips)
}

impl Graph {
    #[expect(clippy::too_many_arguments)]
    pub(crate) fn connect_segments_with_ids(
        &mut self,
        src: SegmentIndex,
        src_commit: impl Into<Option<CommitIndex>>,
        src_id: Option<gix::ObjectId>,
        dst: SegmentIndex,
        dst_commit: impl Into<Option<CommitIndex>>,
        dst_id: Option<gix::ObjectId>,
        parent_order: u32,
    ) {
        let src_commit = src_commit.into();
        let dst_commit = dst_commit.into();
        let new_edge_id = self.inner.add_edge(
            src,
            dst,
            Edge {
                src: src_commit,
                src_id: src_id.or_else(|| self[src].commit_id_by_index(src_commit)),
                dst: dst_commit,
                dst_id: dst_id.or_else(|| self[dst].commit_id_by_index(dst_commit)),
                parent_order,
                workspace_parent_order: None,
            },
        );
        self.rebuild_outgoing_edges_for_traversal_order(src, new_edge_id);
    }

    fn rebuild_outgoing_edges_for_traversal_order(
        &mut self,
        src: SegmentIndex,
        new_edge_id: petgraph::stable_graph::EdgeIndex,
    ) {
        let mut new_edge = None;
        let mut outgoing_edges = Vec::new();
        for edge in self.inner.edges_directed(src, Direction::Outgoing) {
            let edge = EdgeOwned::from(edge);
            if edge.id == new_edge_id {
                new_edge = Some(edge);
            } else {
                outgoing_edges.push(edge);
            }
        }

        let Some(new_edge) = new_edge else {
            return;
        };
        if outgoing_edges.is_empty() {
            return;
        }

        let insert_at = outgoing_edges
            .partition_point(|edge| edge.weight.parent_order <= new_edge.weight.parent_order);
        outgoing_edges.insert(insert_at, new_edge);

        for edge in &outgoing_edges {
            self.inner.remove_edge(edge.id);
        }
        for edge in outgoing_edges.into_iter().rev() {
            self.inner.add_edge(edge.source, edge.target, edge.weight);
        }
    }
}
