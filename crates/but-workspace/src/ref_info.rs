#![expect(clippy::indexing_slicing)]
// TODO: rename this module to `workspace`, make it private, and pub-use all content in the top-level, as we now literally
//       get the workspace, while possibly processing it for use in the UI.

use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use bstr::BString;
use but_core::ref_metadata;
use but_graph::{SegmentIndex, projection::StackCommitFlags};
use gix::Repository;

/// A commit with must useful information extracted from the Git commit itself.
///
/// Note that additional information can be computed and placed in the [`LocalCommit`] and [`RemoteCommit`]
#[derive(Clone, Eq, PartialEq)]
pub struct Commit {
    /// The hash of the commit.
    pub id: gix::ObjectId,
    /// The IDs of the parent commits, but may be empty if this is the first commit.
    pub parent_ids: Vec<gix::ObjectId>,
    /// The hash of the tree associated with the object.
    pub tree_id: gix::ObjectId,
    /// The complete message, verbatim.
    pub message: BString,
    /// The signature at which the commit was authored.
    pub author: gix::actor::Signature,
    /// The references pointing to this commit, even after dereferencing tag objects, along with workspace information.
    /// These can be names of tags and branches.
    pub refs: Vec<but_graph::RefInfo>,
    /// Additional properties to help classify this commit.
    pub flags: StackCommitFlags,
    /// Whether the commit is in a conflicted state, a GitButler concept.
    /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    ///
    /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
    pub has_conflicts: bool,
    /// The GitButler assigned change-id that we hold on to for convenience to avoid duplicate decoding of commits
    /// when trying to associate remote commits with local ones.
    pub change_id: Option<but_core::commit::ChangeId>,
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Commit({hash}, {msg:?}{flags})",
            hash = self.id.to_hex_with_len(7),
            msg = self.message,
            flags = self.flags.debug_string()
        )
    }
}

impl From<but_core::Commit<'_>> for Commit {
    fn from(value: but_core::Commit<'_>) -> Self {
        let has_conflicts = value.is_conflicted();
        let change_id = value.headers().map(|hdr| hdr.change_id);
        Commit {
            id: value.id.into(),
            tree_id: value.tree,
            parent_ids: value.parents.iter().cloned().collect(),
            message: value.inner.message,
            author: value.inner.author,
            has_conflicts,
            change_id,
            refs: Vec::new(),
            flags: StackCommitFlags::empty(),
        }
    }
}

impl Commit {
    /// A special constructor for very specific case.
    pub(crate) fn from_commit_ahead_of_workspace_commit(
        commit: gix::objs::Commit,
        graph_commit: &but_graph::Commit,
    ) -> Self {
        let hdr = but_core::commit::HeadersV2::try_from_commit(&commit);
        Commit {
            id: graph_commit.id,
            parent_ids: commit.parents.into_iter().collect(),
            tree_id: commit.tree,
            message: commit.message,
            has_conflicts: hdr.as_ref().is_some_and(|hdr| hdr.is_conflicted()),
            author: commit
                .author
                .to_ref(&mut gix::date::parse::TimeBuf::default())
                .into(),
            refs: graph_commit.refs.clone(),
            flags: graph_commit.flags.into(),
            change_id: hdr.map(|hdr| hdr.change_id),
        }
    }
}

/// A commit that is reachable through the *local tracking branch*, with additional, computed information.
#[derive(Clone, Eq, PartialEq)]
pub struct LocalCommit {
    /// The simple commit.
    pub inner: Commit,
    /// Provide additional information on how this commit relates to other points of reference, like its remote branch,
    /// or the target branch to integrate with.
    pub relation: LocalCommitRelation,
}

impl std::fmt::Debug for LocalCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let refs = self
            .refs
            .iter()
            .map(|ri| ri.debug_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "LocalCommit({conflict}{hash}, {msg:?}, {relation}{refs})",
            conflict = if self.has_conflicts { "💥" } else { "" },
            hash = self.id.to_hex_with_len(7),
            msg = self.message,
            relation = self.relation.display(self.id),
            refs = if refs.is_empty() {
                "".to_string()
            } else {
                format!(", {refs}")
            }
        )
    }
}

/// The state of the [local commit](LocalCommit) in relation to its remote tracking branch or its integration branch.
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy)]
pub enum LocalCommitRelation {
    /// The commit is only local
    #[default]
    LocalOnly,
    /// The commit is also present in the remote tracking branch.
    ///
    /// This is the case if:
    ///  - The commit has been pushed to the remote
    ///  - The commit has been copied from a remote commit (when applying a remote branch)
    ///
    /// This variant carries the remote commit id.
    /// The `remote_commit_id` may be the same as the `id` or it may be different if the local commit has been rebased
    /// or updated in another way.
    LocalAndRemote(gix::ObjectId),
    /// The commit is considered integrated, using the given hash as the commit that contains this one.
    /// Note that this can be a 1:1 relation in case of rebased commits, or an N:1 relation in case of squash commits.
    /// If the id of this value is the same as the owning commit, this means it's included in the ancestry
    /// of the target branch.
    /// This should happen when the commit or the contents of this commit is already part of the base.
    Integrated(gix::ObjectId),
}

impl LocalCommitRelation {
    /// Convert this relation into something displaying, mainly for debugging.
    pub fn display(&self, id: gix::ObjectId) -> Cow<'static, str> {
        match self {
            LocalCommitRelation::LocalOnly => Cow::Borrowed("local"),
            LocalCommitRelation::LocalAndRemote(remote_id) => {
                if *remote_id == id {
                    "local/remote(identity)".into()
                } else {
                    format!("local/remote({})", remote_id.to_hex_with_len(7)).into()
                }
            }
            LocalCommitRelation::Integrated(id) => {
                format!("integrated({})", id.to_hex_with_len(7)).into()
            }
        }
    }
}

impl Deref for LocalCommit {
    type Target = Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for LocalCommit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Additional workspace functionality that can't easily be implemented in `but-graph`.
pub trait WorkspaceExt {
    /// Return `true` if this workspace has a workspace commit that the workspace reference isn't directly pointing to.
    fn has_workspace_commit_in_ancestry(&self, repo: &gix::Repository) -> bool;
}

impl WorkspaceExt for but_graph::projection::Workspace<'_> {
    fn has_workspace_commit_in_ancestry(&self, repo: &Repository) -> bool {
        function::find_ancestor_workspace_commit(
            self.graph,
            repo,
            self.id,
            self.lower_bound_segment_id,
        )
        .is_some()
    }
}

/// Options for the [`ref_info()`](crate::ref_info) call.
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Control how to traverse the commit-graph as the basis for the workspace conversion.
    pub traversal: but_graph::init::Options,
    /// Perform expensive computations on a per-commit basis.
    ///
    /// Note that less expensive checks are still performed.
    pub expensive_commit_info: bool,
}

/// A segment of a commit graph, representing a set of commits exclusively.
#[derive(Clone, Eq, PartialEq)]
pub struct Segment {
    /// The unambiguous or disambiguated name of the branch at the tip of the segment, i.e. at the first commit,
    /// along with worktree information.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    /// Alternatively, the naming would have been ambiguous.
    /// Finally, this is `None` of the original name can be found searching upwards, finding exactly one
    /// named segment.
    pub ref_info: Option<but_graph::RefInfo>,
    /// An ID which can uniquely identify this segment among all segments within the graph that owned it.
    /// Note that it's not suitable to permanently identify the segment, so should not be persisted.
    pub id: SegmentIndex,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means that a remote is configured and that the stack content
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// The portion of commits that can be reached from the tip of the *branch* downwards, so that they are unique
    /// for that stack segment and not included in any other stack or stack segment.
    ///
    /// The list could be empty for when this is a dedicated empty segment as insertion position of commits.
    pub commits: Vec<LocalCommit>,
    /// Commits that are reachable from the remote-tracking branch associated with this branch,
    /// but are not reachable from this branch or duplicated by a commit in it.
    /// Note that commits that are also similar to commits in `commits` are pruned, and not present here.
    ///
    /// Note that remote commits along with their remote tracking branch should always retain a shared history
    /// with the local tracking branch. If these diverge, we can represent this in data, but currently there is
    /// no derived value to make this visible explicitly.
    pub commits_on_remote: Vec<Commit>,
    /// All commits *that are not workspace commits* reachable by (and including commits in) this segment.
    /// The list was created by walking all parents, not only the first parent.
    /// This means the segment needs fixing.
    pub commits_outside: Option<Vec<Commit>>,
    /// Read-only metadata with additional information about the branch naming the segment,
    /// or `None` if nothing was present.
    pub metadata: Option<ref_metadata::Branch>,
    /// This is `true` a segment in a workspace if the entrypoint of [the traversal](Graph::from_commit_traversal())
    /// is this segment, and the surrounding workspace is provided for context.
    ///
    /// This means one will see the entire workspace, while knowing the focus is on one specific segment.
    /// *Note* that this segment can be listed in *multiple stacks* as it's reachable from multiple 'ahead' segments.
    pub is_entrypoint: bool,
    /// A derived value to help the UI decide which functions to make available.
    pub push_status: crate::ui::PushStatus,
    /// This is always the `first()` commit in `commits` of the next stacksegment, or the first commit of
    /// the first ancestor segment.
    /// It can be imagined as the base upon which the segment is resting, or the connection point to the rest
    /// of the commit-graph along the first parent.
    /// It is `None` if the stack segment contains the first commit in the history, an orphan without ancestry,
    /// or if the history traversal was stopped early.
    pub base: Option<gix::ObjectId>,
}

/// Direct Access (without graph)
impl Segment {
    /// Return the top-most commit id of the segment.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.commits.first().map(|commit| commit.id)
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Segment {
            ref_info,
            id,
            commits,
            commits_on_remote,
            commits_outside,
            remote_tracking_ref_name,
            metadata,
            is_entrypoint,
            push_status,
            base,
        } = self;
        f.debug_struct(&format!(
            "{ep}ref_info::ui::Segment",
            ep = if *is_entrypoint { "👉" } else { "" }
        ))
        .field("id", &id)
        .field(
            "ref_name",
            &match ref_info.as_ref() {
                None => "None".to_string(),
                Some(ri) => ri.debug_string(),
            },
        )
        .field(
            "remote_tracking_ref_name",
            &match remote_tracking_ref_name.as_ref() {
                None => "None".to_string(),
                Some(name) => name.to_string(),
            },
        )
        .field("commits", &commits)
        .field("commits_on_remote", &commits_on_remote)
        .field("commits_outside", &commits_outside)
        .field(
            "metadata",
            match metadata {
                None => &"None",
                Some(m) => m,
            },
        )
        .field("push_status", push_status)
        .field(
            "base",
            &match base {
                None => Cow::Borrowed("None"),
                Some(id) => Cow::Owned(id.to_hex_with_len(7).to_string()),
            },
        )
        .finish()
    }
}

pub(crate) mod function {
    use anyhow::bail;
    use but_core::ref_metadata::ValueInfo;
    use but_graph::{
        Graph, SegmentIndex, is_workspace_ref_name,
        petgraph::Direction,
        projection::{StackCommit, WorkspaceKind},
    };
    use gix::prelude::ObjectIdExt;
    use tracing::instrument;

    use crate::{
        AncestorWorkspaceCommit, RefInfo, WorkspaceCommit, branch,
        ref_info::{LocalCommit, LocalCommitRelation},
        ui::PushStatus,
    };

    /// Gather information about the current `HEAD` and the workspace that might be associated with it,
    /// based on data in `repo` and `meta`. Use `options` to further configure the call.
    ///
    /// For details, see [`ref_info()`].
    pub fn head_info(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        opts: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let graph = Graph::from_head(repo, meta, opts.traversal.clone())?;
        graph_to_ref_info(graph, repo, opts)
    }

    /// Gather information about the commit at `existing_ref` and the workspace that might be associated with it,
    /// based on data in `repo` and `meta`.
    ///
    /// Use `options` to further configure the call.
    ///
    /// ### Performance
    ///
    /// Make sure the `repo` is initialized with a decently sized Object cache so querying the same commit multiple times will be cheap(er).
    #[instrument(level = tracing::Level::DEBUG, skip(meta), err(Debug))]
    pub fn ref_info(
        mut existing_ref: gix::Reference<'_>,
        meta: &impl but_core::RefMetadata,
        opts: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let id = existing_ref.peel_to_id()?;
        let repo = id.repo;
        let graph = Graph::from_commit_traversal(
            id,
            existing_ref.inner.name,
            meta,
            opts.traversal.clone(),
        )?;
        graph_to_ref_info(graph, repo, opts)
    }

    pub(crate) fn find_ancestor_workspace_commit(
        graph: &Graph,
        repo: &gix::Repository,
        workspace_id: SegmentIndex,
        lower_bound_segment_id: Option<SegmentIndex>,
    ) -> Option<AncestorWorkspaceCommit> {
        let lower_bound_generation = lower_bound_segment_id.map(|sidx| graph[sidx].generation);

        let mut commits_outside = Vec::new();
        let mut sidx_and_cidx = None;
        graph.visit_all_segments_excluding_start_until(workspace_id, Direction::Outgoing, |s| {
            if sidx_and_cidx.is_some()
                || lower_bound_generation.is_some_and(|max_gen| s.generation > max_gen)
            {
                return true;
            }
            for (cidx, graph_commit) in s.commits.iter().enumerate() {
                let Ok(commit) = WorkspaceCommit::from_id(graph_commit.id.attach(repo)) else {
                    continue;
                };
                if commit.is_managed() {
                    sidx_and_cidx = Some((s.id, cidx));
                    return true;
                }
                commits_outside.push(
                    crate::ref_info::Commit::from_commit_ahead_of_workspace_commit(
                        commit.inner,
                        graph_commit,
                    ),
                );
            }
            false
        });
        sidx_and_cidx.map(|(sidx, cidx)| AncestorWorkspaceCommit {
            commits_outside,
            segment_with_managed_commit: sidx,
            commit_index_of_managed_commit: cidx,
        })
    }

    fn graph_to_ref_info(
        graph: Graph,
        repo: &gix::Repository,
        opts: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let but_graph::projection::Workspace {
            graph,
            id,
            kind,
            stacks,
            target,
            extra_target,
            metadata,
            lower_bound: _,
            lower_bound_segment_id,
        } = graph.to_workspace()?;

        let (workspace_ref_info, is_managed_commit, ancestor_workspace_commit) = match kind {
            WorkspaceKind::Managed { ref_info } => (Some(ref_info), true, None),
            WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info: ref_name } => {
                let maybe_ancestor_workspace_commit =
                    find_ancestor_workspace_commit(graph, repo, id, lower_bound_segment_id);
                (Some(ref_name), false, maybe_ancestor_workspace_commit)
            }
            WorkspaceKind::AdHoc => (graph[id].ref_info.clone(), false, None),
        };
        let mut info = RefInfo {
            workspace_ref_info,
            lower_bound: lower_bound_segment_id,
            extra_target,
            stacks: stacks
                .into_iter()
                // `but-graph` produces the order as seen by the merge commit,
                // but GB traditionally shows it the other way around.
                // TODO: validate that this is still correct to do here if the workspace
                //       was generated from 'virtual' stacks only, i.e. stacks not from real
                //       merges.
                .rev()
                .map(|stack| branch::Stack::try_from_graph_stack(stack, repo))
                .collect::<anyhow::Result<_>>()?,
            target,
            is_managed_ref: metadata.is_some(),
            is_managed_commit,
            ancestor_workspace_commit,
            is_entrypoint: graph.lookup_entrypoint()?.segment_index == id,
        };

        if let Some(info) = &info.ancestor_workspace_commit {
            // This is the MVP version of what should be guided by the UI - just communicate through
            // an error message, which can only be recovered once the command is executed.
            let mut msg = format!(
                "Found {} commit(s) on top of the workspace commit.\n\n",
                info.commits_outside.len()
            );
            let ws_commit_id = graph[info.segment_with_managed_commit].commits
                [info.commit_index_of_managed_commit]
                .id;
            msg.push_str(
                    "Run the following command in your working directory to fix this while leaving your worktree unchanged.\n",
                );
            msg.push_str("Worktree changes need to be re-committed manually for now.\n\n");
            msg.push_str(&format!("    git reset --soft {ws_commit_id}"));
            bail!("{msg}");
        }
        info.compute_similarity(graph, repo, opts.expensive_commit_info)?;
        Ok(info)
    }

    impl branch::Stack {
        fn try_from_graph_stack(
            stack: but_graph::projection::Stack,
            repo: &gix::Repository,
        ) -> anyhow::Result<Self> {
            let base = stack.base();
            let but_graph::projection::Stack { segments, id } = stack;
            Ok(branch::Stack {
                id,
                base,
                segments: segments
                    .into_iter()
                    .map(|s| crate::ref_info::Segment::try_from_graph_segment(s, repo))
                    .collect::<anyhow::Result<_>>()?,
            })
        }
    }

    impl crate::ref_info::Segment {
        fn try_from_graph_segment(
            but_graph::projection::StackSegment {
                ref_info,
                base,
                base_segment_id: _,
                remote_tracking_ref_name,
                sibling_segment_id: _,
                id,
                commits,
                // TODO: make it visible in this this data structure.
                commits_outside,
                commits_on_remote,
                commits_by_segment: _,
                metadata,
                is_entrypoint,
            }: but_graph::projection::StackSegment,
            repo: &gix::Repository,
        ) -> anyhow::Result<Self> {
            let commits: Vec<_> = commits
                .into_iter()
                .map(|c| LocalCommit::try_from_stack_commit(c, repo))
                .collect::<anyhow::Result<_>>()?;
            let commits_on_remote: Vec<_> = commits_on_remote
                .into_iter()
                .map(|c| {
                    but_core::Commit::from_id(c.id.attach(repo)).map(crate::ref_info::Commit::from)
                })
                .collect::<Result<_, _>>()?;
            let commits_outside = commits_outside
                .map(|v| {
                    v.into_iter()
                        .map(|c| {
                            but_core::Commit::from_id(c.id.attach(repo))
                                .map(crate::ref_info::Commit::from)
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            Ok(Self {
                ref_info,
                id,
                remote_tracking_ref_name,
                commits,
                commits_on_remote,
                commits_outside,
                metadata,
                is_entrypoint,
                base,
                // To be set later.
                push_status: PushStatus::NothingToPush,
            })
        }
    }

    impl LocalCommit {
        // Note that commit-relationships here don't see remotes.
        fn try_from_stack_commit(c: StackCommit, repo: &gix::Repository) -> anyhow::Result<Self> {
            let StackCommit {
                id,
                parent_ids: _,
                flags,
                refs,
            } = c;
            use but_graph::projection::StackCommitFlags;
            let mut inner: crate::ref_info::Commit =
                but_core::Commit::from_id(id.attach(repo))?.into();
            inner.refs = refs;
            inner.flags = flags;
            Ok(LocalCommit {
                inner,
                relation: if flags.contains(StackCommitFlags::Integrated) {
                    LocalCommitRelation::Integrated(id)
                } else if flags.contains(StackCommitFlags::ReachableByRemote) {
                    LocalCommitRelation::LocalAndRemote(id)
                } else {
                    LocalCommitRelation::LocalOnly
                },
            })
        }
    }

    // Fetch non-default workspace information, but only if reference at `name` seems to be a workspace reference.
    pub fn workspace_data_of_workspace_branch(
        meta: &impl but_core::RefMetadata,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<but_core::ref_metadata::Workspace>> {
        if !is_workspace_ref_name(name) {
            return Ok(None);
        }

        let md = meta.workspace(name)?;
        Ok(if md.is_default() {
            None
        } else {
            Some((*md).clone())
        })
    }

    /// Like [`workspace_data_of_workspace_branch()`], but it will try the name of the default GitButler workspace branch.
    pub fn workspace_data_of_default_workspace_branch(
        meta: &impl but_core::RefMetadata,
    ) -> anyhow::Result<Option<but_core::ref_metadata::Workspace>> {
        workspace_data_of_workspace_branch(
            meta,
            "refs/heads/gitbutler/workspace"
                .try_into()
                .expect("statically known"),
        )
    }
}
