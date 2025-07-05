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

    impl Commit {
        /// Read the object of the `commit_id` and extract relevant values, while setting `flags` as well.
        pub(crate) fn new_from_id(
            commit_id: gix::Id<'_>,
            flags: CommitFlags,
            has_conflicts: bool,
        ) -> anyhow::Result<Self> {
            let commit = commit_id.object()?.into_commit();
            // Decode efficiently, no need to own this.
            let commit = commit.decode()?;
            Ok(Commit {
                id: commit_id.detach(),
                parent_ids: commit.parents().collect(),
                message: commit.message.to_owned(),
                author: commit.author.to_owned()?,
                refs: Vec::new(),
                flags,
                has_conflicts,
                change_id: None,
            })
        }
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

    impl LocalCommit {
        /// Create a new branch-commit, along with default values for the non-commit fields.
        // TODO: remove this function once ref_info code doesn't need it anymore (i.e. mapping is implemented).
        pub(crate) fn new_from_id(value: gix::Id<'_>, flags: CommitFlags) -> anyhow::Result<Self> {
            Ok(LocalCommit {
                inner: Commit::new_from_id(value, flags, false)?,
                relation: LocalCommitRelation::LocalOnly,
            })
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
    use std::collections::{BTreeSet, HashMap, HashSet, hash_map::Entry};

    use anyhow::bail;
    use bstr::BString;
    use but_core::ref_metadata::{ValueInfo, Workspace, WorkspaceStack};
    use but_graph::{
        CommitFlags, Graph, is_workspace_ref_name,
        projection::{HeadLocation, StackCommit},
    };
    use gix::{
        ObjectId,
        prelude::{ObjectIdExt, ReferenceExt},
        refs::{Category, FullName},
        revision::walk::Sorting,
        trace,
    };
    use tracing::instrument;

    use super::ui::{LocalCommit, LocalCommitRelation, Segment};
    use crate::{
        RefInfo, WorkspaceCommit, branch,
        branch::Stack,
        integrated::{IsCommitIntegrated, MergeBaseCommitGraph},
        ref_info::ui,
    };

    /// Gather information about the current `HEAD` and the workspace that might be associated with it, based on data in `repo` and `meta`.
    /// Use `options` to further configure the call.
    ///
    /// For details, see [`ref_info()`].
    pub fn head_info(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        opts: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let head = repo.head()?;
        let existing_ref = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                let is_managed = meta.workspace_opt(ref_name.as_ref())?.is_some();
                return Ok(RefInfo {
                    workspace_ref_name: Some(ref_name.clone()),
                    target_ref: workspace_data_of_workspace_branch(meta, ref_name.as_ref())?
                        .and_then(|ws| ws.target_ref),
                    stacks: vec![Stack {
                        base: None,
                        segments: vec![Segment {
                            id: 0,
                            commits: vec![],
                            commits_unique_in_remote_tracking_branch: vec![],
                            remote_tracking_ref_name: None,
                            metadata: branch_metadata_opt(meta, ref_name.as_ref())?,
                            ref_name: Some(ref_name),
                            is_entrypoint: true,
                        }],
                        stash_status: None,
                    }],
                    is_entrypoint: true,
                    is_managed_ref: is_managed,
                    is_managed_commit: false,
                });
            }
            gix::head::Kind::Detached { .. } => {
                return Ok(RefInfo {
                    workspace_ref_name: None,
                    stacks: vec![],
                    target_ref: None,
                    is_entrypoint: true,
                    is_managed_ref: false,
                    is_managed_commit: false,
                });
            }
            gix::head::Kind::Symbolic(name) => name.attach(repo),
        };
        ref_info(existing_ref, meta, opts)
    }

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
            head,
            stacks,
            target,
            metadata,
        } = graph.to_workspace()?;

        let cache = repo.commit_graph_if_enabled()?;
        let mut graph_cache = repo.revision_graph(cache.as_ref());

        let (workspace_ref_name, is_managed_commit) = match head {
            HeadLocation::Workspace { ref_name } => {
                let is_managed = try_refname_to_id(repo, ref_name.as_ref())?
                    .map(|id| WorkspaceCommit::from_id(id.attach(repo)))
                    .transpose()?
                    .is_some_and(|wsc| wsc.is_managed());
                (Some(ref_name), is_managed)
            }
            HeadLocation::Segment { segment_index } => {
                (graph[segment_index].ref_name.clone(), false)
            }
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
            but_graph::projection::Stack { base, segments }: but_graph::projection::Stack,
            repo: &'repo gix::Repository,
            mut ctx: SimilarityContext<'_, '_, 'repo, '_>,
        ) -> anyhow::Result<Self> {
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
            let commits_unique_in_remote_tracking_branch = if commits_on_remote.is_empty() {
                Vec::new()
            } else {
                compute_commit_similarity(
                    repo,
                    &mut commits,
                    commits_on_remote.iter().map(|c| c.id),
                    ctx,
                )?
            };
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
        expensive: bool,
        graph_cache: &'b mut MergeBaseCommitGraph<'repo, 'cache>,
    }

    /// Given `local` commits, try to associate each of them to a `remote` commit by similarity, but only
    /// if they are considered local.
    /// Commits that are seen as duplicate in `local` won't be listed in `remote` anymore, and instead their
    /// hashes will be associated with each other.
    /// Perform expensive computation if `expensive` is `true`.
    fn compute_commit_similarity<'repo>(
        repo: &'repo gix::Repository,
        local: &mut [LocalCommit],
        remote: impl Iterator<Item = gix::ObjectId>,
        SimilarityContext {
            target_ref_name,
            expensive,
            graph_cache,
        }: &mut SimilarityContext<'_, '_, 'repo, '_>,
    ) -> anyhow::Result<Vec<ui::Commit>> {
        #[derive(Hash, Clone, Eq, PartialEq)]
        enum ChangeIdOrCommitData {
            ChangeId(String),
            CommitData {
                author: gix::actor::Identity,
                message: BString,
            },
        }
        // NOTE: The check for similarity is currently run across all remote branches in the stack.
        //       Further, this doesn't handle reorderings/topology differences at all, it's just there or not.
        let mut similarity_lut = HashMap::<ChangeIdOrCommitData, gix::ObjectId>::new();
        let mut remote_commits_unpruned = Vec::<ui::Commit>::new();
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
            for commit_id in remote {
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
                remote_commits_unpruned.push(commit.into());
            }
        }

        // At this stage, local commits are either matched by ID and reachable from our remote,
        // or integrated, our they are local.
        for commit in local
            .iter_mut()
            .take_while(|c| c.relation == LocalCommitRelation::LocalOnly)
        {
            if let Some(remote_commit_id) = commit
                .change_id
                .as_ref()
                .and_then(|cid| similarity_lut.get(&ChangeIdOrCommitData::ChangeId(cid.clone())))
                .or_else(|| {
                    similarity_lut.get(&ChangeIdOrCommitData::CommitData {
                        author: commit.author.clone().into(),
                        message: commit.message.clone(),
                    })
                })
            {
                commit.relation = LocalCommitRelation::LocalAndRemote(*remote_commit_id);
            }
        }

        if let Some(target_ref_name) = target_ref_name.filter(|_| *expensive) {
            let mut check_commit =
                IsCommitIntegrated::new_with_gix(repo, target_ref_name.as_ref(), graph_cache)?;
            let mut is_integrated = false;
            // TODO: remote commits could also be integrated, this seems overly simplified.
            //      For now, just emulate the current implementation (hopefully).
            for local_commit in &mut *local {
                if is_integrated || { check_commit.is_integrated_gix(local_commit.id) }? {
                    is_integrated = true;
                    local_commit.relation = LocalCommitRelation::Integrated;
                }
            }
        }

        // Finally, assure we don't show remotes commits that are also visible locally.
        Ok(remote_commits_unpruned
            .into_iter()
            .filter(|rc| {
                let is_used_in_local_commits = local
                    .iter()
                    .any(|c| matches!(c.relation,  LocalCommitRelation::LocalAndRemote(rid) if rid == rc.id));
                !is_used_in_local_commits
            })
            .collect())
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
    pub fn ref_info(
        mut existing_ref: gix::Reference<'_>,
        meta: &impl but_core::RefMetadata,
        opts: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let ws_data = workspace_data_of_workspace_branch(meta, existing_ref.name())?;
        let (workspace_ref_name, target_ref, stored_workspace_stacks) =
            obtain_workspace_info(&existing_ref, meta, ws_data)?;
        let repo = existing_ref.repo;
        // If there are multiple choices for a ref that points to a commit we encounter, use one of these.
        let mut preferred_ref_names = stored_workspace_stacks
            .as_ref()
            .map(|stacks| {
                stacks
                    .iter()
                    .flat_map(|stack| stack.branches.iter().map(|b| b.ref_name.as_ref()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let target_symbolic_remote_name = target_ref
            .as_ref()
            .and_then(|rn| extract_remote_name(rn.as_ref(), &repo.remote_names()));

        let ref_commit = existing_ref.peel_to_commit()?;
        let ref_commit = WorkspaceCommit {
            id: ref_commit.id(),
            inner: ref_commit.decode()?.to_owned(),
        };
        let repo = existing_ref.repo;
        let refs_by_id = collect_refs_by_commit_id(repo)?;
        let target_ids = target_ref
            .as_ref()
            .and_then(|rn| remote_and_local_target_ids(repo, rn.as_ref()).transpose())
            .transpose()?;
        let cache = repo.commit_graph_if_enabled()?;
        let mut graph = repo.revision_graph(cache.as_ref());
        let mut boundary = gix::hashtable::HashSet::default();
        let configured_remote_tracking_branches = configured_remote_tracking_branches(repo)?;

        let mut stacks = if ref_commit.is_managed() {
            let base: Option<_> = match target_ids {
                None => match repo
                    .merge_base_octopus_with_graph(ref_commit.parents.iter().cloned(), &mut graph)
                {
                    Ok(id) => Some(id),
                    Err(err) => {
                        tracing::warn!(
                            "Parents of {existing_ref} are disjoint: {err}",
                            existing_ref = existing_ref.name().as_bstr(),
                        );
                        None
                    }
                },
                Some((remote_target_id, local_target_id)) => {
                    // We actually get the best results if we use the merge-base between the most recent target,
                    // the local one, and (in case we are behind somehow), our own parents.
                    // For now, we assume that we will find a base among all these commits.
                    match repo.merge_base_octopus_with_graph(
                        [remote_target_id, local_target_id]
                            .into_iter()
                            .chain(ref_commit.parents.iter().cloned()),
                        &mut graph,
                    ) {
                        Ok(id) => Some(id),
                        Err(err) => {
                            tracing::warn!(
                                "Parents of {existing_ref}, along with {remote_target_id} and {local_target_id} have no common merge base: {err}",
                                existing_ref = existing_ref.name().as_bstr(),
                            );
                            None
                        }
                    }
                }
            };
            // The commits we have already associated with a stack segment.
            let mut stacks = Vec::new();
            for commit_id in ref_commit.parents.iter() {
                let tip = *commit_id;
                let base = base.map(|base| base.detach());
                boundary.extend(base);
                let segments = collect_stack_segments(
                    tip.attach(repo),
                    refs_by_id.get(&tip).and_then(|refs| {
                        refs.iter()
                            .find(|rn| preferred_ref_names.iter().any(|orn| *orn == rn.as_ref()))
                            .or_else(|| refs.first())
                            .map(|rn| rn.as_ref())
                    }),
                    CommitFlags::InWorkspace,
                    &boundary,
                    &preferred_ref_names,
                    opts.stack_commit_limit,
                    &refs_by_id,
                    meta,
                    target_symbolic_remote_name.as_deref(),
                    &configured_remote_tracking_branches,
                )?;

                boundary.extend(segments.iter().flat_map(|segment| {
                    segment.commits.iter().map(|c| c.id).chain(
                        segment
                            .commits_unique_in_remote_tracking_branch
                            .iter()
                            .map(|c| c.id),
                    )
                }));

                stacks.push(Stack {
                    segments,
                    base,
                    // TODO: but as part of the commits.
                    stash_status: None,
                })
            }
            stacks
        } else {
            if is_workspace_ref_name(existing_ref.name()) {
                // TODO: assure we can recover from that.
                bail!(
                    "Workspace reference {name} didn't point to a managed commit anymore",
                    name = existing_ref.name().shorten()
                )
            }
            // Discover all references that actually point to the reachable graph.
            let tip = ref_commit.id;
            let base = target_ids
                .and_then(|(remote_target_id, _local_target_id)| {
                    match repo.merge_base_with_graph(remote_target_id, tip, &mut graph) {
                        Ok(id) => Some(id),
                        Err(err) => {
                            tracing::warn!(
                                "{existing_ref} and {target_ref} are disjoint: {err}",
                                existing_ref = existing_ref.name().as_bstr(),
                                target_ref = target_ref
                                    .as_ref()
                                    .expect("target_id is present, must have ref name then"),
                            );
                            None
                        }
                    }
                })
                .map(|base| base.detach());
            // If we have a workspace, then we have to use that as the basis for our traversal to assure
            // the commits and stacks are assigned consistently.
            if let Some(workspace_ref) = workspace_ref_name
                .as_ref()
                .filter(|workspace_ref| workspace_ref.as_ref() != existing_ref.name())
            {
                let workspace_contains_ref_tip =
                    walk_commits(repo, workspace_ref.as_ref(), base)?.contains(&*tip);
                let workspace_tip_is_managed = try_refname_to_id(repo, workspace_ref.as_ref())?
                    .map(|commit_id| WorkspaceCommit::from_id(commit_id.attach(repo)))
                    .transpose()?
                    .is_some_and(|c| c.is_managed());
                if workspace_contains_ref_tip && workspace_tip_is_managed {
                    // To assure the stack is counted consistently even when queried alone, redo the query.
                    // This should be avoided (i.e., the caller should consume the 'highest value'
                    // refs if possible, but that's not always the case.
                    // TODO(perf): add 'focus' to `opts` so it doesn't do expensive computations for stacks we drop later.
                    let mut info = ref_info(repo.find_reference(workspace_ref)?, meta, opts)?;
                    if let Some((stack_index, segment_index)) = info
                        .stacks
                        .iter()
                        .enumerate()
                        .find_map(|(stack_index, stack)| {
                            stack.segments.iter().enumerate().find_map(
                                |(segment_index, segment)| {
                                    segment
                                        .ref_name
                                        .as_ref()
                                        .is_some_and(|rn| rn.as_ref() == existing_ref.name())
                                        .then_some((stack_index, segment_index))
                                },
                            )
                        })
                    {
                        let mut curr_stack_idx = 0;
                        info.stacks.retain(|_| {
                            let retain = curr_stack_idx == stack_index;
                            curr_stack_idx += 1;
                            retain
                        });
                        let mut curr_segment_idx = 0;
                        info.stacks[0].segments.retain(|_| {
                            let retain = curr_segment_idx >= segment_index;
                            curr_segment_idx += 1;
                            retain
                        });
                    } else {
                        // TODO: a test for that, is it even desirable?
                        info.stacks.clear();
                        trace::warn!(
                            "Didn't find {ref_name} in ref-info, even though commit {tip} is reachable from {workspace_ref}",
                            ref_name = existing_ref.name().as_bstr(),
                        );
                    }
                    return Ok(info);
                }
            }
            let boundary = {
                let mut hs = gix::hashtable::HashSet::default();
                hs.extend(base);
                hs
            };

            preferred_ref_names.push(existing_ref.name());
            let segments = collect_stack_segments(
                tip,
                Some(existing_ref.name()),
                match workspace_ref_name.as_ref().zip(target_ids) {
                    None => CommitFlags::empty(),
                    Some((ws_ref, (remote_target_id, _local_target_id))) => {
                        let ws_commits =
                            walk_commits(repo, ws_ref.as_ref(), Some(remote_target_id))?;
                        if ws_commits.contains(&*tip) {
                            CommitFlags::InWorkspace
                        } else {
                            CommitFlags::empty()
                        }
                    }
                },
                &boundary, /* boundary commits */
                &preferred_ref_names,
                opts.stack_commit_limit,
                &refs_by_id,
                meta,
                target_symbolic_remote_name.as_deref(),
                &configured_remote_tracking_branches,
            )?;

            vec![Stack {
                // TODO: compute base if target-ref is available, but only if this isn't the target ref!
                base,
                segments,
                stash_status: None,
            }]
        };

        // Various cleanup functions to enforce constraints before spending time on classifying commits.
        enforce_constraints(&mut stacks);
        if let Some(ws_stacks) = stored_workspace_stacks.as_deref() {
            reconcile_with_workspace_stacks(
                &existing_ref,
                ws_stacks,
                &mut stacks,
                meta,
                target_symbolic_remote_name.as_deref(),
                &configured_remote_tracking_branches,
            )?;
        }

        if opts.expensive_commit_info {
            populate_commit_info(
                target_ref.as_ref().zip(target_ids),
                &mut stacks,
                repo,
                &mut graph,
            )?;
        }

        let is_managed = workspace_ref_name.is_some();
        Ok(RefInfo {
            workspace_ref_name,
            stacks,
            target_ref,
            is_entrypoint: false,
            is_managed_commit: false,
            is_managed_ref: is_managed,
        })
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

    #[allow(clippy::type_complexity)]
    fn obtain_workspace_info(
        existing_ref: &gix::Reference<'_>,
        meta: &impl but_core::RefMetadata,
        ws_data: Option<Workspace>,
    ) -> anyhow::Result<(
        Option<FullName>,
        Option<FullName>,
        Option<Vec<WorkspaceStack>>,
    )> {
        Ok(if let Some(ws_data) = ws_data {
            (
                Some(existing_ref.name().to_owned()),
                ws_data.target_ref,
                Some(ws_data.stacks),
            )
        } else {
            // We'd want to assure we don't overcount commits even if we are handed a non-workspace ref, so we always have to
            // search for known workspaces.
            // Do get the first known target ref for now.
            let ws_data_iter = meta
                .iter()
                .filter_map(Result::ok)
                .filter_map(|(ref_name, item)| {
                    item.downcast::<but_core::ref_metadata::Workspace>()
                        .ok()
                        .map(|ws| (ref_name, ws))
                });
            let mut target_refs =
                ws_data_iter.map(|(ref_name, ws)| (ref_name, ws.target_ref, ws.stacks));
            let first_target = target_refs.next();
            if target_refs.next().is_some() {
                bail!(
                    "BUG: found more than one workspaces in branch-metadata, and we'd want to make this code multi-workspace compatible"
                )
            }
            first_target
                .map(|(a, b, c)| (Some(a), b, Some(c)))
                .unwrap_or_default()
        })
    }

    /// Does the following:
    ///
    /// * a segment can be reachable from multiple stacks. If a segment is also a stack, remove it along with all segments
    ///   that follow as one can assume they are contained in the stack.
    fn enforce_constraints(stacks: &mut [Stack]) {
        let mut for_deletion = Vec::new();
        for (stack_idx, stack_name) in stacks
            .iter()
            .enumerate()
            .filter_map(|(idx, stack)| stack.name().map(|n| (idx, n)))
        {
            for (other_stack_idx, other_stack) in stacks
                .iter()
                .enumerate()
                .filter(|(idx, _stack)| *idx != stack_idx)
            {
                if let Some(matching_segment_idx) = other_stack
                    .segments
                    .iter()
                    .enumerate()
                    .find_map(|(idx, segment)| {
                        (segment.ref_name.as_ref().map(|rn| rn.as_ref()) == Some(stack_name))
                            .then_some(idx)
                    })
                {
                    for_deletion.push((other_stack_idx, matching_segment_idx));
                }
            }
        }

        for (stack_idx, first_segment_idx_to_delete) in for_deletion {
            stacks[stack_idx]
                .segments
                .drain(first_segment_idx_to_delete..);
        }
    }

    /// Given the desired stack configuration in `ws_stacks`, bring this information into `stacks` which is assumed
    /// to be what Git really sees.
    /// This is needed as empty stacks and segments, and particularly their order, can't be fully represented just
    /// with Git refs alone.
    fn reconcile_with_workspace_stacks(
        existing_ref: &gix::Reference<'_>,
        ws_stacks: &[WorkspaceStack],
        stacks: &mut Vec<Stack>,
        meta: &impl but_core::RefMetadata,
        symbolic_remote_name: Option<&str>,
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    ) -> anyhow::Result<()> {
        validate_workspace_stacks(ws_stacks)?;
        // Stacks that are genuinely reachable we have to show.
        // Empty ones are special as they don't have their own commits and aren't distinguishable by traversing a
        // workspace commit. For this, we have workspace metadata to tell us what is what.
        // The goal is to remove segments that we found by traversal and re-add them as individual stack if they are some.
        // TODO: remove this block once that is folded into the function below.
        let mut stack_idx_to_remove = Vec::new();
        for (idx, stack) in stacks
            .iter_mut()
            .enumerate()
            .filter(|(_, stack)| stack.name() != Some(existing_ref.name()))
        {
            // Find all empty segments that aren't listed in our workspace stacks metadata, and remove them.
            let desired_stack_segments = ws_stacks.iter().find(|ws_stack| {
                ws_stack
                    .branches
                    .first()
                    .is_some_and(|branch| Some(branch.ref_name.as_ref()) == stack.name())
            });
            let num_segments_to_keep = stack
                .segments
                .iter()
                .enumerate()
                .rev()
                .by_ref()
                .take_while(|(_idx, segment)| segment.commits.is_empty())
                .take_while(|(_idx, segment)| {
                    segment
                        .ref_name
                        .as_ref()
                        .zip(desired_stack_segments)
                        .is_none_or(|(srn, desired_stack)| {
                            // We don't let the desired order matter, just that an empty segment is (not) mentioned.
                            desired_stack
                                .branches
                                .iter()
                                .all(|branch| &branch.ref_name != srn)
                        })
                })
                .map(|t| t.0)
                .last();
            if let Some(keep) = num_segments_to_keep {
                stack.segments.drain(keep..);
            }

            if stack.segments.is_empty() {
                stack_idx_to_remove.push(idx);
            }
        }
        if !stack_idx_to_remove.is_empty() {
            let mut idx = 0;
            stacks.retain(|_stack| {
                let res = !stack_idx_to_remove.contains(&idx);
                idx += 1;
                res
            });
        }

        // Put the stacks into the right order, and create empty stacks for those that are completely virtual.
        sort_stacks_by_order_in_ws_stacks(
            existing_ref.repo,
            stacks,
            ws_stacks,
            meta,
            symbolic_remote_name,
            configured_remote_tracking_branches,
        )?;
        Ok(())
    }

    /// Basic validation for virtual workspaces that our processing builds upon.
    fn validate_workspace_stacks(stacks: &[WorkspaceStack]) -> anyhow::Result<()> {
        let mut seen = BTreeSet::new();
        for name in stacks
            .iter()
            .flat_map(|stack| stack.branches.iter().map(|branch| branch.ref_name.as_ref()))
        {
            let first = seen.insert(name);
            if !first {
                bail!(
                    "invalid workspace stack: duplicate ref name: {}",
                    name.as_bstr()
                )
            }
        }
        Ok(())
    }

    /// Brute-force insert missing stacks and segments as determined in `ordered`.
    /// Add missing stacks and segments as well so real stacks `unordered` match virtual stacks `ordered`.
    /// Note that `ordered` is assumed to be validated.
    fn sort_stacks_by_order_in_ws_stacks(
        repo: &gix::Repository,
        unordered: &mut Vec<Stack>,
        ordered: &[WorkspaceStack],
        meta: &impl but_core::RefMetadata,
        symbolic_remote_name: Option<&str>,
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    ) -> anyhow::Result<()> {
        // With this serialized set of desired segments, the tip can also be missing, and we still have
        // a somewhat expected order. Besides, it's easier to work with.
        // We also only include those that exist as ref (which is always a requirement now).
        let serialized_virtual_segments = {
            let mut v = Vec::new();
            for (is_stack_tip, existing_ws_ref) in ordered.iter().flat_map(|ws_stack| {
                ws_stack
                    .branches
                    .iter()
                    .enumerate()
                    .filter_map(|(branch_idx, branch)| {
                        repo.try_find_reference(branch.ref_name.as_ref())
                            .transpose()
                            .map(|rn| (branch_idx == 0, rn))
                    })
            }) {
                let mut existing_ws_ref = existing_ws_ref?;
                let id = existing_ws_ref.peel_to_id_in_place()?.detach();
                v.push((is_stack_tip, id, existing_ws_ref.inner.name));
            }
            v
        };

        let existing_virtual_stacks = serialized_virtual_segments.iter().enumerate().filter_map(
            |(segment_idx, (is_stack_tip, id, segment_ref_name))| {
                if !is_stack_tip {
                    return None;
                }
                let stack_tip = (*id, segment_ref_name);
                let segments = serialized_virtual_segments
                    .get(segment_idx + 1..)
                    .map(|slice| {
                        slice
                            .iter()
                            .take_while(|(is_stack, _, _)| !is_stack)
                            .map(|(_, id, rn)| (*id, rn))
                    })
                    .into_iter()
                    .flatten();
                Some(Some(stack_tip).into_iter().chain(segments))
            },
        );

        // Identify missing (existing) stacks in ordered and add them to the end of unordered.
        // Here we must match only the existing stack-heads.
        for virtual_segments in existing_virtual_stacks {
            // Find a real stack where one segment intersects with the desired stacks to know what that work on.
            let virtual_stack_is_known_as_real_stack = unordered.iter_mut().find(|stack| {
                stack.segments.iter().any(|s| {
                    virtual_segments
                        .clone()
                        .any(|(_, vs_name)| s.ref_name.as_ref() == Some(vs_name))
                })
            });
            if let Some(real_stack) = virtual_stack_is_known_as_real_stack {
                // We know there is one virtual ref name bonding the real stack with the virtual one.
                // Now all we have to do is to place the non-existing virtual segments into the right position
                // alongside their segments. It's notable that these virtual segments can be placed in any position.
                // They may also already exist, and may be stacked, so multiple empty ones are on top of each other.
                // At its core, we want to consume N consecutive segments and insert them into position X.
                // BUT ALSO NOTE: We cannot reorder real segments that are 'locked' to the real commit-graph, nor can
                //                we reorder real segments that are found at a certain commit, but empty.
                // TODO: also delete empty real segments

                let find_base = |start_idx: usize, segments: &[Segment]| -> Option<gix::ObjectId> {
                    segments.get(start_idx..).and_then(|slice| {
                        slice
                            .iter()
                            .find_map(|segment| segment.commits.first().map(|c| c.id))
                    })
                };
                let mut insert_position = 0;
                for (target_id, virtual_segment_ref_name) in virtual_segments {
                    let real_stack_idx =
                        real_stack
                            .segments
                            .iter()
                            .enumerate()
                            .find_map(|(idx, real_segment)| {
                                (real_segment.ref_name.as_ref() == Some(virtual_segment_ref_name))
                                    .then_some(idx)
                            });
                    match real_stack_idx {
                        None => {
                            if let Some(mismatched_base) =
                                find_base(insert_position, &real_stack.segments)
                                    .filter(|base| *base != target_id)
                            {
                                tracing::warn!(
                                    "Somehow virtual ref '{name}' was supposed to be at {}, but its closest insertion base was {}",
                                    target_id,
                                    mismatched_base,
                                    name = virtual_segment_ref_name.as_bstr(),
                                );
                                continue;
                            }
                            real_stack.segments.insert(
                                insert_position,
                                segment_from_ref_name(
                                    repo,
                                    meta,
                                    virtual_segment_ref_name.as_ref(),
                                    symbolic_remote_name,
                                    configured_remote_tracking_branches,
                                )?,
                            );
                            insert_position += 1;
                        }
                        Some(existing_idx) => {
                            if real_stack.segments[existing_idx].commits.is_empty() {
                                // TODO: do assure empty segments (despite real) are correctly sorted, and we can re-sort these
                            }
                            // Skip this one, it's already present
                            insert_position = existing_idx + 1;
                        }
                    }
                }
            } else {
                // We have a virtual stack that wasn't reachable in reality at all.
                // Add it as a separate stack then, reproducing each segment verbatim.
                // TODO: we actually have to assure that the recorded order still is compatible with the
                //       associated real-world IDs. This should be reconciled before using the virtual segments!!
                let mut segments = Vec::new();
                let mut last_seen_target_id_as_base = None;
                for (target_id, segment_ref_name) in virtual_segments {
                    last_seen_target_id_as_base = Some(target_id);
                    segments.push(segment_from_ref_name(
                        repo,
                        meta,
                        segment_ref_name.as_ref(),
                        symbolic_remote_name,
                        configured_remote_tracking_branches,
                    )?);
                }
                // From this segment
                unordered.push(Stack {
                    base: last_seen_target_id_as_base,
                    segments,
                    // TODO: set up
                    stash_status: None,
                });
            }
        }

        // Sort existing, and put those that aren't matched to the top as they are usually traversed,
        // and 'more real'.
        unordered.sort_by(|a, b| {
            let index_a = serialized_virtual_segments
                .iter()
                .enumerate()
                .find_map(|(idx, (_, _, segment_ref_name))| {
                    (Some(segment_ref_name) == a.ref_name()).then_some(idx)
                })
                .unwrap_or_default();
            let index_b = serialized_virtual_segments
                .iter()
                .enumerate()
                .find_map(|(idx, (_, _, segment_ref_name))| {
                    (Some(segment_ref_name) == b.ref_name()).then_some(idx)
                })
                .unwrap_or_default();
            index_a.cmp(&index_b)
        });
        // TODO: integrate segments into existing stacks.

        // TODO: log all stack segments that couldn't be matched, even though we should probably do something
        //       with them eventually.
        Ok(())
    }

    fn segment_from_ref_name(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        virtual_segment_ref_name: &gix::refs::FullNameRef,
        symbolic_remote_name: Option<&str>,
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    ) -> anyhow::Result<Segment> {
        Ok(Segment {
            id: 0,
            ref_name: Some(virtual_segment_ref_name.to_owned()),
            remote_tracking_ref_name: lookup_remote_tracking_branch_or_deduce_it(
                repo,
                virtual_segment_ref_name,
                symbolic_remote_name,
                configured_remote_tracking_branches,
            )?,
            // Always empty, otherwise we would have found the segment by traversal.
            commits: vec![],
            // Will be set when expensive data is computed.
            commits_unique_in_remote_tracking_branch: vec![],
            metadata: meta
                .branch_opt(virtual_segment_ref_name)?
                .map(|b| b.clone()),
            is_entrypoint: false,
        })
    }

    /// Akin to `log()`, but less powerful.
    fn walk_commits(
        repo: &gix::Repository,
        from: &gix::refs::FullNameRef,
        hide: Option<gix::ObjectId>,
    ) -> anyhow::Result<gix::hashtable::HashSet<gix::ObjectId>> {
        let Some(from_id) = repo
            .try_find_reference(from)?
            .and_then(|mut r| r.peel_to_id_in_place().ok())
        else {
            return Ok(Default::default());
        };
        Ok(from_id
            .ancestors()
            .sorting(Sorting::BreadthFirst)
            .with_hidden(hide)
            .all()?
            .filter_map(Result::ok)
            .map(|info| info.id)
            .collect())
    }

    fn lookup_remote_tracking_branch(
        repo: &gix::Repository,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<gix::refs::FullName>> {
        Ok(repo
            .branch_remote_tracking_ref_name(ref_name, gix::remote::Direction::Fetch)
            .transpose()?
            .map(|rn| rn.into_owned()))
    }

    /// Returns the unique names of all remote tracking branches that are configured in the repository.
    /// Useful to avoid claiming them for deduction.
    fn configured_remote_tracking_branches(
        repo: &gix::Repository,
    ) -> anyhow::Result<BTreeSet<gix::refs::FullName>> {
        let mut out = BTreeSet::default();
        for short_name in repo
            .config_snapshot()
            .sections_by_name("branch")
            .into_iter()
            .flatten()
            .filter_map(|s| s.header().subsection_name())
        {
            let Ok(full_name) = Category::LocalBranch.to_full_name(short_name) else {
                continue;
            };
            out.extend(lookup_remote_tracking_branch(repo, full_name.as_ref())?);
        }
        Ok(out)
    }

    fn lookup_remote_tracking_branch_or_deduce_it(
        repo: &gix::Repository,
        ref_name: &gix::refs::FullNameRef,
        symbolic_remote_name: Option<&str>,
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    ) -> anyhow::Result<Option<gix::refs::FullName>> {
        Ok(lookup_remote_tracking_branch(repo, ref_name)?.or_else(|| {
            let symbolic_remote_name = symbolic_remote_name?;
            // Deduce the ref-name as fallback.
            // TODO: remove this - this is only required to support legacy repos that
            //       didn't setup normal Git remotes.
            // let remote_name = target_
            let remote_tracking_ref_name = format!(
                "refs/remotes/{symbolic_remote_name}/{short_name}",
                short_name = ref_name.shorten()
            );
            let Ok(remote_tracking_ref_name) =
                gix::refs::FullName::try_from(remote_tracking_ref_name)
            else {
                return None;
            };
            if configured_remote_tracking_branches.contains(&remote_tracking_ref_name) {
                return None;
            }
            repo.find_reference(&remote_tracking_ref_name)
                .ok()
                .map(|remote_ref| remote_ref.name().to_owned())
        }))
    }

    fn extract_remote_name(
        ref_name: &gix::refs::FullNameRef,
        remotes: &gix::remote::Names<'_>,
    ) -> Option<String> {
        let (category, shorthand_name) = ref_name.category_and_short_name()?;
        if !matches!(category, Category::RemoteBranch) {
            return None;
        }

        let longest_remote = remotes
            .iter()
            .rfind(|reference_name| shorthand_name.starts_with(reference_name))
            .ok_or(anyhow::anyhow!(
                "Failed to find remote branch's corresponding remote"
            ))
            .ok()?;
        Some(longest_remote.to_string())
    }

    /// For each stack in `stacks`, and for each stack segment within it, check if a remote tracking branch is available
    /// and existing. Then find its commits and fill in commit-information of the commits that are reachable by the stack tips as well.
    ///
    /// `graph` is used to speed up merge-base queries.
    ///
    /// **IMPORTANT**: `repo` must use in-memory objects!
    /// TODO: have merge-graph based checks that can check if one commit is included in the ancestry of another tip. That way one can
    ///       quick perform is-integrated checks with the target branch.
    fn populate_commit_info<'repo>(
        target_ref_name_and_ids: Option<(&gix::refs::FullName, (gix::ObjectId, gix::ObjectId))>,
        stacks: &mut [Stack],
        repo: &'repo gix::Repository,
        merge_graph: &mut MergeBaseCommitGraph<'repo, '_>,
    ) -> anyhow::Result<()> {
        #[derive(Hash, Clone, Eq, PartialEq)]
        enum ChangeIdOrCommitData {
            ChangeId(String),
            CommitData {
                author: gix::actor::Identity,
                message: BString,
            },
        }
        let mut boundary = gix::hashtable::HashSet::default();
        let mut ambiguous_commits = HashSet::<ChangeIdOrCommitData>::new();
        // NOTE: The check for similarity is currently run across all remote branches in the stack.
        //       Further, this doesn't handle reorderings/topology differences at all, it's just there or not.
        let mut similarity_lut = HashMap::<ChangeIdOrCommitData, gix::ObjectId>::new();
        for stack in stacks {
            let segments_with_remote_ref_tips_and_base: Vec<_> = stack
                .segments
                .iter()
                .enumerate()
                .map(|(index, segment)| {
                    let remote_ref_tip =
                        segment
                            .remote_tracking_ref_name
                            .as_ref()
                            .and_then(|remote_ref_name| {
                                try_refname_to_id(repo, remote_ref_name.as_ref())
                                    .ok()
                                    .flatten()
                            });
                    (index, remote_ref_tip)
                })
                .collect();
            // Start the remote commit collection on the segment with the first remote,
            // and stop commit-status handling at the first segment which has a remote (as it would be a new starting point).
            let segments_with_remote_ref_tips_and_base: Vec<_> =
                segments_with_remote_ref_tips_and_base
                    .iter()
                    // TODO: a test for this: remote_ref_tip selects the start, and the base is always the next start's tip or the stack base.
                    .map(|(index, remote_ref_tip)| {
                        let remote_ref_tip_and_base = remote_ref_tip.and_then(|remote_ref_tip| {
                            segments_with_remote_ref_tips_and_base
                                .get((index + 1)..)
                                .and_then(|slice| {
                                    slice.iter().find_map(|(index, remote_ref_tip)| {
                                        remote_ref_tip.and_then(|_| stack.segments[*index].tip())
                                    })
                                })
                                .or(stack.base)
                                .map(|base| (remote_ref_tip, base))
                        });
                        (index, remote_ref_tip_and_base)
                    })
                    .collect();

            for (segment_index, remote_ref_tip_and_base) in segments_with_remote_ref_tips_and_base {
                boundary.clear();
                boundary.extend(stack.base);
                let segment = &mut stack.segments[*segment_index];
                if let Some((remote_ref_tip, base_for_remote)) = remote_ref_tip_and_base {
                    boundary.insert(base_for_remote);

                    let mut insert_or_expell_ambiguous =
                        |k: ChangeIdOrCommitData, v: gix::ObjectId| {
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

                    let walk = remote_ref_tip
                        .attach(repo)
                        .ancestors()
                        .first_parent_only()
                        .with_hidden(boundary.iter().cloned())
                        .all()?;
                    'remote_branch_traversal: for info in walk {
                        let id = info?.id;
                        if let Some(idx) = segment
                            .commits
                            .iter_mut()
                            .enumerate()
                            .find_map(|(idx, c)| (c.id == id).then_some(idx))
                        {
                            // Mark all commits from here as pushed.
                            for commit in &mut segment.commits[idx..] {
                                commit.relation = LocalCommitRelation::LocalAndRemote(commit.id);
                            }
                            // Don't break, maybe the local commits are reachable through multiple avenues.
                            continue 'remote_branch_traversal;
                        } else {
                            let commit = but_core::Commit::from_id(id.attach(repo))?;
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
                            segment
                                .commits_unique_in_remote_tracking_branch
                                .push(commit.into());
                        }
                    }
                }

                // Find duplicates harder by change-ids by commit-data.
                for local_commit in &mut segment.commits {
                    let commit = but_core::Commit::from_id(local_commit.id.attach(repo))?;
                    if let Some(remote_commit_id) = commit
                        .headers()
                        .and_then(|hdr| {
                            similarity_lut.get(&ChangeIdOrCommitData::ChangeId(hdr.change_id))
                        })
                        .or_else(|| {
                            similarity_lut.get(&ChangeIdOrCommitData::CommitData {
                                author: commit.author.clone().into(),
                                message: commit.message.clone(),
                            })
                        })
                    {
                        local_commit.relation =
                            LocalCommitRelation::LocalAndRemote(*remote_commit_id);
                    }
                    local_commit.has_conflicts = commit.is_conflicted();
                }

                // Prune upstream commits so they don't show if they are considered locally available as well.
                // This is kind of 'wrong', and we can hope that code doesn't rely on upstream commits.
                segment
                    .commits_unique_in_remote_tracking_branch
                    .retain(|remote_commit| {
                        let remote_commit_is_shared_in_local = segment
                            .commits
                            .iter()
                            .any(|c| matches!(c.relation,  LocalCommitRelation::LocalAndRemote(rid) if rid == remote_commit.id));
                        !remote_commit_is_shared_in_local
                    });
            }

            // Finally, check for integration into the target if available.
            // TODO: This can probably be more efficient if this is staged, by first trying
            //       to check if the tip is merged, to flag everything else as merged.
            if let Some((target_ref_name, (target_remote_id, target_local_id))) =
                target_ref_name_and_ids
            {
                let mut check_commit =
                    IsCommitIntegrated::new_with_gix(repo, target_ref_name.as_ref(), merge_graph)?;
                let mut is_integrated = false;
                // TODO: remote commits could also be integrated, this seems overly simplified.
                //      For now, just emulate the current implementation (hopefully).
                for local_commit in stack
                    .segments
                    .iter_mut()
                    .flat_map(|segment| &mut segment.commits)
                {
                    if is_integrated || { check_commit.is_integrated_gix(local_commit.id) }? {
                        is_integrated = true;
                        local_commit.relation = LocalCommitRelation::Integrated;
                    }
                }

                // Special case: if there are (previously) added empty segments, deref their tips to see if
                //               they are integrated.
                let merge_graph = check_commit.graph;
                for res in stack.segments.iter_mut().filter_map(|s| {
                    if s.commits.is_empty() {
                        s.ref_name
                            .as_ref()
                            .and_then(|name| try_refname_to_id(repo, name.as_ref()).transpose())
                            .map(|res| res.map(|id| (id, s)))
                    } else {
                        None
                    }
                }) {
                    let (tip, empty_segment) = res?;
                    if tip == target_local_id {
                        continue;
                    }
                    // TODO: make the is_integrated() check actually work for graph-based queries, maybe it would
                    //       but just doesn't have the necessary commits in this case.
                    //       Perform a simple commit-id based lookup instead.
                    let merge_base =
                        repo.merge_base_with_graph(target_remote_id, tip, merge_graph)?;
                    if merge_base == tip {
                        // TODO: this is a hack that arbitrarily adds this one commit so the state is observable.
                        //       This means segments needs its own integrated flag that should be set if one of its commits
                        //       or it itself is integrated.
                        empty_segment.commits.push(LocalCommit {
                            relation: LocalCommitRelation::Integrated,
                            ..LocalCommit::new_from_id(tip.attach(repo), CommitFlags::empty())?
                        })
                    }
                }
            }
        }
        Ok(())
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

    /// Walk down the commit-graph from `tip` until a `boundary_commits` is encountered, excluding it, or to the graph root if there is no boundary.
    /// Walk along the first parent, and return stack segments on its path using the `refs_by_commit_id` reverse mapping in walk order.
    /// `tip_ref` is the name of the reference pointing to `tip` if it's known.
    /// `ref_location` it the location of `tip_ref`
    /// `preferred_refs` is an arbitrarily sorted array of names that should be used in the returned segments if they are encountered during the traversal
    /// *and* there are more than one ref pointing to it.
    /// `symbolic_remote_name` is used to infer the name of the remote tracking ref in case `tip_ref` doesn't have a remote configured.
    ///
    /// Note that `boundary_commits` are sorted so binary-search can be used to quickly check membership.
    ///
    /// ### Important
    ///
    /// This function does *not* fill in remote information *nor* does it compute the per-commit status.
    /// TODO: also add `hidden` commits, for a list of special commits like the merge-base where all parents should be hidden as well.
    ///       Right now we are completely relying on (many) boundary commits which should work most of the time, but may not work if
    ///       branches have diverged a lot.
    #[allow(clippy::too_many_arguments)]
    fn collect_stack_segments(
        tip: gix::Id<'_>,
        tip_ref: Option<&gix::refs::FullNameRef>,
        flags: CommitFlags,
        boundary_commits: &gix::hashtable::HashSet,
        preferred_refs: &[&gix::refs::FullNameRef],
        mut limit: usize,
        refs_by_id: &RefsById,
        meta: &impl but_core::RefMetadata,
        symbolic_remote_name: Option<&str>,
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    ) -> anyhow::Result<Vec<Segment>> {
        let mut out = Vec::new();
        let mut segment = Some(Segment {
            ref_name: tip_ref.map(ToOwned::to_owned),
            // the tip is part of the walk.
            ..Default::default()
        });
        for (count, info) in tip
            .ancestors()
            .first_parent_only()
            .sorting(Sorting::BreadthFirst)
            .with_hidden(boundary_commits.iter().cloned())
            .all()?
            .enumerate()
        {
            let segment_ref = segment.as_mut().expect("a segment is always present here");

            if limit != 0 && count >= limit {
                if segment_ref.commits.is_empty() {
                    limit += 1;
                } else {
                    out.extend(segment.take());
                    break;
                }
            }
            let info = info?;
            if let Some(refs) = refs_by_id.get(&info.id) {
                let ref_at_commit = refs
                    .iter()
                    .find(|rn| preferred_refs.iter().any(|orn| *orn == rn.as_ref()))
                    .or_else(|| refs.first())
                    .map(|rn| rn.to_owned());
                if ref_at_commit.as_ref().map(|rn| rn.as_ref()) == tip_ref {
                    segment_ref
                        .commits
                        .push(LocalCommit::new_from_id(info.id(), flags)?);
                    continue;
                }
                out.extend(segment);
                segment = Some(Segment {
                    id: 0,
                    ref_name: ref_at_commit,
                    commits: vec![LocalCommit::new_from_id(info.id(), flags)?],
                    commits_unique_in_remote_tracking_branch: vec![],
                    // The fields that follow will be set later.
                    remote_tracking_ref_name: None,
                    metadata: None,
                    is_entrypoint: false,
                });
                continue;
            } else {
                segment_ref
                    .commits
                    .push(LocalCommit::new_from_id(info.id(), flags)?);
            }
        }
        out.extend(segment);

        let repo = tip.repo;
        for segment in out.iter_mut() {
            let Some(ref_name) = segment.ref_name.as_ref() else {
                continue;
            };
            segment.remote_tracking_ref_name = lookup_remote_tracking_branch_or_deduce_it(
                repo,
                ref_name.as_ref(),
                symbolic_remote_name,
                configured_remote_tracking_branches,
            )?;
            let branch_info = meta.branch(ref_name.as_ref())?;
            if !branch_info.is_default() {
                segment.metadata = Some((*branch_info).clone())
            }
        }
        Ok(out)
    }

    // A trait of the ref-names array is that these are sorted, as they are from a sorted traversal, giving us stable ordering.
    type RefsById = gix::hashtable::HashMap<gix::ObjectId, Vec<gix::refs::FullName>>;

    // Create a mapping of all heads to the object ids they point to.
    // No tags are used (yet), but maybe that's useful in the future.
    // We never pick up branches we consider to be part of the workspace.
    fn collect_refs_by_commit_id(repo: &gix::Repository) -> anyhow::Result<RefsById> {
        let mut all_refs_by_id = gix::hashtable::HashMap::<_, Vec<_>>::default();
        for (commit_id, git_reference) in repo
            .references()?
            .prefixed("refs/heads/")?
            .filter_map(Result::ok)
            .filter_map(|r| {
                if is_workspace_ref_name(r.name()) {
                    return None;
                }
                r.try_id().map(|id| (id.detach(), r.inner.name))
            })
        {
            all_refs_by_id
                .entry(commit_id)
                .or_default()
                .push(git_reference);
        }
        all_refs_by_id.values_mut().for_each(|v| v.sort());
        Ok(all_refs_by_id)
    }

    // TODO: Put this in `RefMetadataExt` if useful elsewhere.
    fn branch_metadata_opt(
        meta: &impl but_core::RefMetadata,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<but_core::ref_metadata::Branch>> {
        let md = meta.branch(name)?;
        Ok(if md.is_default() {
            None
        } else {
            Some((*md).clone())
        })
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
