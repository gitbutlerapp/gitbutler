#![allow(clippy::indexing_slicing)]
// TODO: rename this module to `workspace`, make it private, and pub-use all content in the top-level, as we now literally
//       get the workspace, while possibly processing it for use in the UI.

/// Options for the [`ref_info()`](crate::ref_info) call.
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// The maximum amount of commits to list *per stack*. Note that a [`StackSegment`](crate::branch::Segment) will always have a single commit, if available,
    ///  even if this exhausts the commit limit in that stack.
    /// `0` means the limit is disabled.
    ///
    /// NOTE: Currently, to fetch more commits, make this call again with a higher limit.
    /// Additionally, this is only effective if there is an open-ended graph, for example, when `HEAD` points to `main` with
    /// a lot of commits without a discernible base.
    ///
    /// Callers can check for the limit by looking as the oldest commit - if it has no parents, then the limit wasn't hit, or if it is
    /// connected to a merge-base.
    /// TODO: remove this.
    pub stack_commit_limit: usize,
    /// Control how to traverse the commit-graph as the basis for the workspace conversion.
    pub traversal: but_graph::init::Options,
    /// Perform expensive computations on a per-commit basis.
    ///
    /// Note that less expensive checks are still performed.
    pub expensive_commit_info: bool,
}

/// Types driven by the user interface, not general purpose.
pub mod ui {
    use std::ops::{Deref, DerefMut};

    use bstr::BString;
    use but_core::ref_metadata;
    use but_graph::CommitFlags;

    /// A commit with must useful information extracted from the Git commit itself.
    ///
    /// Note that additional information can be computed and placed in the [`LocalCommit`] and [`RemoteCommit`]
    #[derive(Clone, Eq, PartialEq)]
    pub struct Commit {
        /// The hash of the commit.
        pub id: gix::ObjectId,
        /// The IDs of the parent commits, but may be empty if this is the first commit.
        pub parent_ids: Vec<gix::ObjectId>,
        /// The complete message, verbatim.
        pub message: BString,
        /// The signature at which the commit was authored.
        pub author: gix::actor::Signature,
        /// The references pointing to this commit, even after dereferencing tag objects.
        /// These can be names of tags and branches.
        pub refs: Vec<gix::refs::FullName>,
        /// Additional properties to help classify this commit.
        pub flags: CommitFlags,
        /// Whether the commit is in a conflicted state, a GitButler concept.
        /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
        /// Conflicts are resolved via the Edit Mode mechanism.
        ///
        /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
        pub has_conflicts: bool,
        /// The GitButler assigned change-id that we hold on to for convenience to avoid duplicate decoding of commits
        /// when trying to associate remote commits with local ones.
        /// TODO: Skip once this type is serialized to the UI directly.
        pub change_id: Option<String>,
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
                parent_ids: value.parents.iter().cloned().collect(),
                message: value.inner.message,
                author: value.inner.author,
                has_conflicts,
                change_id,
                refs: Vec::new(),
                flags: CommitFlags::empty(),
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
                .map(|rn| format!("►{}", rn.shorten()))
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
        /// The commit is considered integrated.
        /// This should happen when the commit or the contents of this commit is already part of the base.
        Integrated,
    }

    impl LocalCommitRelation {
        /// Convert this relation into something displaying, mainly for debugging.
        pub fn display(&self, id: gix::ObjectId) -> &'static str {
            match self {
                LocalCommitRelation::LocalOnly => "local",
                LocalCommitRelation::LocalAndRemote(remote_id) => {
                    if *remote_id == id {
                        "local/remote(identity)"
                    } else {
                        "local/remote(similarity)"
                    }
                }
                LocalCommitRelation::Integrated => "integrated",
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

    /// A segment of a commit graph, representing a set of commits exclusively.
    #[derive(Default, Clone, Eq, PartialEq)]
    pub struct Segment {
        /// The unambiguous or disambiguated name of the branch at the tip of the segment, i.e. at the first commit.
        ///
        /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
        /// a commit anymore that was reached by our rev-walk.
        /// This can happen if the ref is deleted, or if it was advanced by other means.
        /// Alternatively, the naming would have been ambiguous.
        /// Finally, this is `None` of the original name can be found searching upwards, finding exactly one
        /// named segment.
        pub ref_name: Option<gix::refs::FullName>,
        /// An ID which can uniquely identify this segment among all segments within the graph that owned it.
        /// Note that it's not suitable to permanently identify the segment, so should not be persisted.
        pub id: usize,
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
        pub commits_unique_in_remote_tracking_branch: Vec<Commit>,
        /// Read-only metadata with additional information about the branch naming the segment,
        /// or `None` if nothing was present.
        pub metadata: Option<ref_metadata::Branch>,
        /// This is `true` a segment in a workspace if the entrypoint of [the traversal](Graph::from_commit_traversal())
        /// is this segment, and the surrounding workspace is provided for context.
        ///
        /// This means one will see the entire workspace, while knowing the focus is on one specific segment.
        /// *Note* that this segment can be listed in *multiple stacks* as it's reachable from multiple 'ahead' segments.
        pub is_entrypoint: bool,
        // TODO: Add base?
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
                ref_name,
                id,
                commits,
                commits_unique_in_remote_tracking_branch,
                remote_tracking_ref_name,
                metadata,
                is_entrypoint,
            } = self;
            f.debug_struct(&format!(
                "{ep}ref_info::ui::Segment",
                ep = if *is_entrypoint { "👉" } else { "" }
            ))
            .field("id", &id)
            .field(
                "ref_name",
                &match ref_name.as_ref() {
                    None => "None".to_string(),
                    Some(name) => name.to_string(),
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
            .field(
                "commits_unique_in_remote_tracking_branch",
                &commits_unique_in_remote_tracking_branch,
            )
            .field(
                "metadata",
                match metadata {
                    None => &"None",
                    Some(m) => m,
                },
            )
            .finish()
        }
    }
}

pub(crate) mod function {
    use std::collections::{HashMap, HashSet, hash_map::Entry};

    use anyhow::bail;
    use bstr::BString;
    use but_core::ref_metadata::ValueInfo;
    use but_graph::{
        Graph, is_workspace_ref_name,
        projection::{StackCommit, WorkspaceKind},
    };
    use gix::{ObjectId, Repository, prelude::ObjectIdExt, refs::Category};
    use itertools::Itertools;
    use tracing::instrument;

    use super::ui::{LocalCommit, LocalCommitRelation};
    use crate::{
        RefInfo, WorkspaceCommit, branch,
        integrated::{IsCommitIntegrated, MergeBaseCommitGraph},
        ref_info::ui,
    };

    /// Gather information about the current `HEAD` and the workspace that might be associated with it,
    /// based on data in `repo` and `meta`. Use `options` to further configure the call.
    ///
    /// For details, see [`ref_info2()`].
    pub fn head_info2(
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
    /// Also, **IMPORTANT**, it must use in-memory objects to avoid leaking objects generated during test-merges to disk!
    #[instrument(level = tracing::Level::DEBUG, skip(meta), err(Debug))]
    pub fn ref_info2(
        mut existing_ref: gix::Reference<'_>,
        meta: &impl but_core::RefMetadata,
        opts: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let id = existing_ref.peel_to_id_in_place()?;
        let repo = id.repo;
        let graph = Graph::from_commit_traversal(
            id,
            existing_ref.inner.name,
            meta,
            opts.traversal.clone(),
        )?;
        graph_to_ref_info(graph, repo, opts)
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

        let cache = repo.commit_graph_if_enabled()?;
        let mut graph_cache = repo.revision_graph(cache.as_ref());

        let (workspace_ref_name, is_managed_commit) = match kind {
            WorkspaceKind::Managed { ref_name } => {
                let is_managed = try_refname_to_id(repo, ref_name.as_ref())?
                    .map(|id| WorkspaceCommit::from_id(id.attach(repo)))
                    .transpose()?
                    .is_some_and(|wsc| wsc.is_managed());
                (Some(ref_name), is_managed)
            }
            WorkspaceKind::AdHoc => (graph[id].ref_name.clone(), false),
        };
        Ok(RefInfo {
            workspace_ref_name,
            stacks: stacks
                .into_iter()
                // `but-graph` produces the order as seen by the merge commit,
                // but GB traditionally shows it the other way around.
                // TODO: validate that this is still correct to do here if the workspace
                //       was generated from 'virtual' stacks only, i.e. stacks not from real
                //       merges.
                .rev()
                .map(|stack| {
                    branch::Stack::try_from_graph_stack(
                        stack,
                        repo,
                        SimilarityContext {
                            target_ref_name: target.as_ref().map(|t| &t.ref_name),
                            upstream_commits: {
                                let topmost_target_sidx = target
                                    .as_ref()
                                    .map(|t| t.segment_index)
                                    .into_iter()
                                    .chain(extra_target)
                                    .sorted_by_key(|sidx| graph[*sidx].generation)
                                    .next();
                                let mut out = Vec::new();
                                if let Some(start) = topmost_target_sidx {
                                    graph.visit_all_segments_until(
                                        start,
                                        but_graph::petgraph::Direction::Outgoing,
                                        |s| {
                                            let prune = true;
                                            if Some(s.id) == lower_bound_segment_id {
                                                return prune;
                                            }
                                            for c in &s.commits {
                                                if !c.flags.is_remote() {
                                                    return prune;
                                                }
                                                out.push(c.id);
                                            }
                                            !prune
                                        },
                                    );
                                }
                                out
                            },
                            expensive: opts.expensive_commit_info,
                            graph_cache: &mut graph_cache,
                        },
                    )
                })
                .collect::<anyhow::Result<_>>()?,
            target_ref: target.map(|t| t.ref_name),
            is_managed_ref: metadata.is_some(),
            is_managed_commit,
            is_entrypoint: graph.lookup_entrypoint()?.segment_index == id,
        })
    }

    impl branch::Stack {
        fn try_from_graph_stack<'repo>(
            stack: but_graph::projection::Stack,
            repo: &'repo gix::Repository,
            mut ctx: SimilarityContext<'_, '_, 'repo, '_>,
        ) -> anyhow::Result<Self> {
            let base = stack.base();
            let but_graph::projection::Stack { segments } = stack;
            Ok(branch::Stack {
                base,
                segments: segments
                    .into_iter()
                    .map(|s| ui::Segment::try_from_graph_segment(s, repo, &mut ctx))
                    .collect::<anyhow::Result<_>>()?,
                stash_status: None,
            })
        }
    }

    impl ui::Segment {
        fn try_from_graph_segment<'repo>(
            but_graph::projection::StackSegment {
                ref_name,
                base: _,
                base_segment_id: _,
                remote_tracking_ref_name,
                sibling_segment_id: _,
                id,
                commits,
                commits_on_remote,
                commits_by_segment: _,
                metadata,
                is_entrypoint,
            }: but_graph::projection::StackSegment,
            repo: &'repo gix::Repository,
            ctx: &mut SimilarityContext<'_, '_, 'repo, '_>,
        ) -> anyhow::Result<Self> {
            let mut commits: Vec<_> = commits
                .into_iter()
                .map(|c| LocalCommit::try_from_graph_commit(c, repo))
                .collect::<anyhow::Result<_>>()?;
            let commits_unique_in_remote_tracking_branch = compute_commit_similarity(
                repo,
                &mut commits,
                commits_on_remote.iter().map(|c| c.id),
                ctx,
            )?;
            Ok(Self {
                ref_name,
                id: id.index(),
                remote_tracking_ref_name,
                commits,
                commits_unique_in_remote_tracking_branch,
                metadata,
                is_entrypoint,
            })
        }
    }

    impl LocalCommit {
        // Note that commit-relationships here don't see remotes.
        fn try_from_graph_commit(c: StackCommit, repo: &gix::Repository) -> anyhow::Result<Self> {
            use but_graph::projection::StackCommitFlags;
            let mut inner: ui::Commit = but_core::Commit::from_id(c.id.attach(repo))?.into();
            inner.refs = c.refs;
            Ok(LocalCommit {
                inner,
                relation: if c.flags.contains(StackCommitFlags::Integrated) {
                    LocalCommitRelation::Integrated
                } else if c.flags.contains(StackCommitFlags::ReachableByRemote) {
                    LocalCommitRelation::LocalAndRemote(c.id)
                } else {
                    LocalCommitRelation::LocalOnly
                },
            })
        }
    }

    struct SimilarityContext<'a, 'b, 'repo, 'cache> {
        target_ref_name: Option<&'a gix::refs::FullName>,
        /// Remote-only commits from the topmost target ref, but no further down than the lower bound of the workspace.
        upstream_commits: Vec<gix::ObjectId>,
        expensive: bool,
        graph_cache: &'b mut MergeBaseCommitGraph<'repo, 'cache>,
    }

    /// Given `local` commits, try to associate each of them to a `remote` commit by similarity, but only
    /// if they are considered local.
    /// Commits that are seen as duplicate in `local` won't be listed in `remote` anymore, and instead their
    /// hashes will be associated with each other.
    /// Perform expensive computation if `expensive` is `true`.
    // TODO(perf): there is a lot of duplicate computation going on here, revamp this completely. Mostly
    //             due to upstream commits, per segment processing, and better ways to do integration checks
    //             (probably with changeset-ids, which could also be cached).
    fn compute_commit_similarity<'repo>(
        repo: &'repo gix::Repository,
        local: &mut [LocalCommit],
        remote: impl Iterator<Item = gix::ObjectId>,
        SimilarityContext {
            target_ref_name,
            upstream_commits,
            expensive,
            graph_cache,
        }: &mut SimilarityContext<'_, '_, 'repo, '_>,
    ) -> anyhow::Result<Vec<ui::Commit>> {
        // NOTE: The check for similarity is currently run across all remote branches in the stack.
        //       Further, this doesn't handle reorderings/topology differences at all, it's just there or not.
        let (similarity_lut_remote, remote_commits_in_similarity_map) =
            create_similarity_lut(repo, remote)?;

        let (similarity_lut_upstream, _upstream_commits_in_similarity_map) =
            create_similarity_lut(repo, upstream_commits.iter().cloned())?;

        // At this stage, local commits are either matched by ID and reachable from our remote,
        // or integrated, our they are local.
        let mut matched_remote_commits = gix::hashtable::HashSet::default();
        for commit in local
            .iter_mut()
            .take_while(|c| c.relation == LocalCommitRelation::LocalOnly)
        {
            fn lookup_similar<'a>(
                map: &'a SimilarityLut,
                commit: &LocalCommit,
            ) -> Option<&'a gix::ObjectId> {
                commit
                    .change_id
                    .as_ref()
                    .and_then(|cid| map.get(&ChangeIdOrCommitData::ChangeId(cid.clone())))
                    .or_else(|| {
                        map.get(&ChangeIdOrCommitData::CommitData {
                            author: commit.author.clone().into(),
                            message: commit.message.clone(),
                        })
                    })
            }
            if let Some(upstream_commit_id) = lookup_similar(&similarity_lut_upstream, commit) {
                commit.relation = LocalCommitRelation::Integrated;
                // We prefer the integrated state, which currently doesn't track the matching commit. This probably
                // should be changed to make our life easier here.
                matched_remote_commits.insert(upstream_commit_id);
            } else if let Some(remote_commit_id) = lookup_similar(&similarity_lut_remote, commit) {
                commit.relation = LocalCommitRelation::LocalAndRemote(*remote_commit_id);
                matched_remote_commits.insert(remote_commit_id);
            }
        }

        if let Some(target_ref_name) = target_ref_name.filter(|_| *expensive) {
            // TODO: this should only operate on commits that are given to it, not compute these
            //       by itself. These commits must be remote-only commits as figured out by the graph.
            // TODO: remote commits could also be integrated this way, this seems overly simplified.
            //      For now, just emulate the current implementation (hopefully).
            let mut check_commit =
                IsCommitIntegrated::new_with_gix(repo, target_ref_name.as_ref(), graph_cache)?;
            let mut is_integrated = false;
            for local_commit in &mut *local {
                if is_integrated || check_commit.is_integrated_gix(local_commit.id)? {
                    is_integrated = true;
                    local_commit.relation = LocalCommitRelation::Integrated;
                }
            }
        }

        // Finally, assure we don't show remotes commits that are also visible locally.
        Ok(remote_commits_in_similarity_map
            .into_iter()
            .filter(|rc| {
                let is_used_in_local_commits = local.iter().any(|c| {
                    matches!(c.relation,  LocalCommitRelation::LocalAndRemote(rid) if rid == rc.id)
                        || matched_remote_commits.contains(&rc.id)
                });
                !is_used_in_local_commits
            })
            .collect())
    }

    #[derive(Hash, Clone, Eq, PartialEq)]
    enum ChangeIdOrCommitData {
        ChangeId(String),
        CommitData {
            author: gix::actor::Identity,
            message: BString,
        },
    }

    type SimilarityLut = HashMap<ChangeIdOrCommitData, ObjectId>;

    fn create_similarity_lut(
        repo: &Repository,
        commits: impl Iterator<Item = ObjectId>,
    ) -> anyhow::Result<(SimilarityLut, Vec<ui::Commit>)> {
        let mut similarity_lut = HashMap::<ChangeIdOrCommitData, gix::ObjectId>::new();
        let mut commits_not_in_upstream = Vec::<ui::Commit>::new();
        {
            let mut ambiguous_commits = HashSet::<ChangeIdOrCommitData>::new();
            let mut insert_or_expell_ambiguous = |k: ChangeIdOrCommitData, v: gix::ObjectId| {
                if ambiguous_commits.contains(&k) {
                    return;
                }
                match similarity_lut.entry(k) {
                    Entry::Occupied(ambiguous) => {
                        ambiguous_commits.insert(ambiguous.key().clone());
                        ambiguous.remove();
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(v);
                    }
                }
            };
            for commit_id in commits {
                let commit = but_core::Commit::from_id(commit_id.attach(repo))?;
                if let Some(hdr) = commit.headers() {
                    insert_or_expell_ambiguous(
                        ChangeIdOrCommitData::ChangeId(hdr.change_id),
                        commit.id.detach(),
                    );
                }
                insert_or_expell_ambiguous(
                    ChangeIdOrCommitData::CommitData {
                        author: commit.author.clone().into(),
                        message: commit.message.clone(),
                    },
                    commit.id.detach(),
                );
                commits_not_in_upstream.push(commit.into());
            }
        }
        Ok((similarity_lut, commits_not_in_upstream))
    }

    /// Returns `(remote_tracking_target_id, local_tracking_target_id )`, corresponding to `main` and `origin/main`
    /// for example, given only `target_ref` which can be either a remote or a local tracking branch.
    /// TODO: once we can handle local tracking branches, then one end is optional, but it would be harder bring them
    ///       in order outside of this function. Maybe return a Vec instead?
    pub(crate) fn remote_and_local_target_ids(
        repo: &gix::Repository,
        target_ref: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<(ObjectId, ObjectId)>> {
        let Some(Category::RemoteBranch) = target_ref.category() else {
            bail!(
                "Cannot handle {target_ref} target refs yet (but want to support local tracking branches as well)",
                target_ref = target_ref.as_bstr()
            )
        };
        let Some((local_tracking_name, _remote_name)) =
            repo.upstream_branch_and_remote_for_tracking_branch(target_ref)?
        else {
            return Ok(None);
        };
        let local_target_id = try_refname_to_id(repo, local_tracking_name.as_ref())?;
        let remote_target_id = try_refname_to_id(repo, target_ref)?;
        Ok(remote_target_id.zip(local_target_id))
    }

    pub(crate) fn try_refname_to_id(
        repo: &gix::Repository,
        refname: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<gix::ObjectId>> {
        Ok(repo
            .try_find_reference(refname)?
            .map(|mut r| r.peel_to_id_in_place())
            .transpose()?
            .map(|id| id.detach()))
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
