#![allow(clippy::indexing_slicing)]

/// Options for the [`head_info()`](crate::ref_info) call.
#[derive(Default, Debug, Copy, Clone)]
pub struct Options {
    /// The maximum amount of commits to list *per stack*. Note that a [`StackSegment`](crate::branch::StackSegment) will always have a single commit, if available,
    ///  even if this exhausts the commit limit in that stack.
    /// `0` means the limit is disabled.
    ///
    /// NOTE: Currently, to fetch more commits, make this call again with a higher limit.
    /// Additionally, this is only effective if there is an open-ended graph, for example, when `HEAD` points to `main` with
    /// a lot of commits without a discernible base.
    ///
    /// Callers can check for the limit by looking as the oldest commit - if it has no parents, then the limit wasn't hit, or if it is
    /// connected to a merge-base.
    pub stack_commit_limit: usize,

    /// Perform expensive computations on a per-commit basis.
    ///
    /// Note that less expensive checks are still performed.
    pub expensive_commit_info: bool,
}

pub(crate) mod function {
    use crate::branch::{LocalCommit, LocalCommitRelation, RefLocation, Stack, StackSegment};
    use crate::integrated::{IsCommitIntegrated, MergeBaseCommitGraph};
    use crate::{RefInfo, branch};
    use anyhow::bail;
    use bstr::BString;
    use but_core::ref_metadata::ValueInfo;
    use gitbutler_oxidize::ObjectIdExt as _;
    use gix::prelude::{ObjectIdExt, ReferenceExt};
    use gix::revision::walk::Sorting;
    use std::collections::hash_map::Entry;
    use std::collections::{HashMap, HashSet};
    use tracing::instrument;

    /// Gather information about the current `HEAD` and the workspace that might be associated with it, based on data in `repo` and `meta`.
    /// Use `options` to further configure the call.
    ///
    /// For details, see [`ref_info_at()`].
    pub fn ref_info(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        opts: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let head = repo.head()?;
        let existing_ref = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                return Ok(RefInfo {
                    workspace_ref_name: None,
                    target_ref: workspace_data_of_workspace_branch(meta, ref_name.as_ref())?
                        .and_then(|ws| ws.target_ref),
                    stacks: vec![Stack {
                        index: 0,
                        tip: None,
                        base: None,
                        segments: vec![StackSegment {
                            commits_unique_from_tip: vec![],
                            commits_unique_in_remote_tracking_branch: vec![],
                            remote_tracking_ref_name: None,
                            metadata: branch_metadata_opt(meta, ref_name.as_ref())?,
                            ref_location: Some(RefLocation::OutsideOfWorkspace),
                            ref_name: Some(ref_name),
                        }],
                        stash_status: None,
                    }],
                });
            }
            gix::head::Kind::Detached { .. } => {
                return Ok(RefInfo {
                    workspace_ref_name: None,
                    stacks: vec![],
                    target_ref: None,
                });
            }
            gix::head::Kind::Symbolic(name) => name.attach(repo),
        };
        ref_info_at(existing_ref, meta, opts)
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
    pub fn ref_info_at(
        mut existing_ref: gix::Reference<'_>,
        meta: &impl but_core::RefMetadata,
        super::Options {
            stack_commit_limit,
            expensive_commit_info,
        }: super::Options,
    ) -> anyhow::Result<RefInfo> {
        let ws_data = workspace_data_of_workspace_branch(meta, existing_ref.name())?;
        let (workspace_ref_name, target_ref) = if let Some(data) = ws_data {
            // TODO: figure out what to do with workspace information, consolidate it with what's there as well
            //       to know which branch is where.
            (Some(existing_ref.name().to_owned()), data.target_ref)
        } else {
            // We'd want to assure we don't overcount commits even if we are handed a non-workspace ref, so we always have to
            // search for known workspaces.
            // Do get the first known target ref for now.
            let mut target_refs =
                meta.iter()
                    .filter_map(Result::ok)
                    .filter_map(|(ref_name, item)| {
                        item.downcast::<but_core::ref_metadata::Workspace>()
                            .ok()
                            .and_then(|ws| ws.target_ref.map(|target| (ref_name, target)))
                    });
            let first_target = target_refs.next();
            if target_refs.next().is_some() {
                bail!(
                    "BUG: found more than one workspaces in branch-metadata, and we'd want to make this code multi-workspace compatible"
                )
            }
            first_target
                .map(|(a, b)| (Some(a), Some(b)))
                .unwrap_or_default()
        };

        let ref_commit = existing_ref.peel_to_commit()?;
        let ref_commit = crate::WorkspaceCommit {
            id: ref_commit.id(),
            inner: ref_commit.decode()?.to_owned(),
        };
        let repo = existing_ref.repo;
        let refs_by_id = collect_refs_by_commit_id(repo)?;
        let target_ref_id = target_ref
            .as_ref()
            .and_then(|rn| try_refname_to_id(repo, rn.as_ref()).transpose())
            .transpose()?;
        let cache = repo.commit_graph_if_enabled()?;
        let mut graph = repo.revision_graph(cache.as_ref());
        let base: Option<_> = if target_ref_id.is_none() {
            repo.merge_base_octopus_with_graph(ref_commit.parents.iter().cloned(), &mut graph)?
                .into()
        } else {
            None
        };
        let mut boundary = gix::hashtable::HashSet::default();
        let mut stacks = if ref_commit.is_managed() {
            // The commits we have already associated with a stack segment.
            let mut stacks = Vec::new();
            for (index, commit_id) in ref_commit.parents.iter().enumerate() {
                let tip = *commit_id;
                let base = base
                    .map(Ok)
                    .or_else(|| {
                        target_ref_id
                            .map(|target_id| repo.merge_base_with_graph(target_id, tip, &mut graph))
                    })
                    .transpose()?
                    .map(|base| base.detach());
                boundary.extend(base);
                let segments = collect_stack_segments(
                    tip.attach(repo),
                    refs_by_id
                        .get(&tip)
                        .and_then(|refs| refs.first().map(|r| r.as_ref())),
                    Some(RefLocation::ReachableFromWorkspaceCommit),
                    &boundary,
                    // TODO: get from workspace information maybe?
                    &[], /* preferred refs */
                    stack_commit_limit,
                    &refs_by_id,
                    meta,
                )?;

                boundary.extend(segments.iter().flat_map(|segment| {
                    segment.commits_unique_from_tip.iter().map(|c| c.id).chain(
                        segment
                            .commits_unique_in_remote_tracking_branch
                            .iter()
                            .map(|c| c.id),
                    )
                }));

                stacks.push(Stack {
                    index,
                    tip: Some(tip),
                    segments,
                    base,
                    // TODO: but as part of the commits.
                    stash_status: None,
                })
            }
            stacks
        } else {
            // Discover all references that actually point to the reachable graph.
            let tip = ref_commit.id;
            let base = target_ref_id
                .map(|target_id| repo.merge_base_with_graph(target_id, tip, &mut graph))
                .transpose()?
                .map(|base| base.detach());
            let boundary = {
                let mut hs = gix::hashtable::HashSet::default();
                hs.extend(base);
                hs
            };
            let segments = collect_stack_segments(
                tip,
                Some(existing_ref.name()),
                Some(match workspace_ref_name.as_ref().zip(target_ref_id) {
                    None => RefLocation::OutsideOfWorkspace,
                    Some((ws_ref, target_id)) => {
                        let ws_commits = walk_commits(repo, ws_ref.as_ref(), target_id)?;
                        if ws_commits.contains(&*tip) {
                            RefLocation::ReachableFromWorkspaceCommit
                        } else {
                            RefLocation::OutsideOfWorkspace
                        }
                    }
                }),
                &boundary,                         /* boundary commits */
                &[existing_ref.name().to_owned()], /* preferred refs */
                stack_commit_limit,
                &refs_by_id,
                meta,
            )?;

            vec![Stack {
                index: 0,
                tip: Some(tip.detach()),
                // TODO: compute base if target-ref is available, but only if this isn't the target ref!
                base,
                segments,
                stash_status: None,
            }]
        };

        if expensive_commit_info {
            populate_commit_info(target_ref.as_ref(), &mut stacks, repo, &mut graph)?;
        }

        Ok(RefInfo {
            workspace_ref_name,
            stacks,
            target_ref,
        })
    }

    /// Akin to `log()`, but less powerful.
    // TODO: replace with something better, and also use `.hide()`.
    fn walk_commits(
        repo: &gix::Repository,
        from: &gix::refs::FullNameRef,
        hide: gix::ObjectId,
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
            // TODO: use 'hide()'
            .with_boundary(Some(hide))
            .all()?
            .filter_map(Result::ok)
            .map(|info| info.id)
            .collect())
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
        target_ref_name: Option<&gix::refs::FullName>,
        stacks: &mut [Stack],
        repo: &'repo gix::Repository,
        merge_graph: &mut MergeBaseCommitGraph<'repo, '_>,
    ) -> anyhow::Result<()> {
        fn find_remote_ref_tip(
            repo: &gix::Repository,
            ref_name: &gix::refs::FullNameRef,
        ) -> anyhow::Result<Option<gix::ObjectId>> {
            let Some(remote_ref_name) = repo
                .branch_remote_tracking_ref_name(ref_name, gix::remote::Direction::Fetch)
                .transpose()?
            else {
                return Ok(None);
            };

            try_refname_to_id(repo, remote_ref_name.as_ref())
        }

        #[derive(Hash, Clone, Eq, PartialEq)]
        enum ChangeIdOrCommitData {
            ChangeId(String),
            CommitData {
                author: gix::actor::Signature,
                message: BString,
            },
        }
        let mut boundary = gix::hashtable::HashSet::default();
        let mut ambiguous_commits = HashSet::<ChangeIdOrCommitData>::new();
        let mut similarity_lut = HashMap::<ChangeIdOrCommitData, gix::ObjectId>::new();
        let git2_repo = git2::Repository::open(repo.path())?;
        for stack in stacks {
            boundary.clear();
            boundary.extend(stack.base);

            let segments_with_remote_ref_tips: Vec<_> = stack
                .segments
                .iter()
                .enumerate()
                .map(|(index, segment)| {
                    (
                        index,
                        segment.ref_name.as_ref().and_then(|ref_name| {
                            find_remote_ref_tip(repo, ref_name.as_ref()).ok().flatten()
                        }),
                    )
                })
                .collect();
            // Start the remote commit collection on the segment with the first remote,
            // and stop commit-status handling at the first segment which has a remote (as it would be a new starting point.
            let segments_with_remote_ref_tips_and_base: Vec<_> = segments_with_remote_ref_tips
                .iter()
                // TODO: a test for this: remote_ref_tip selects the start, and the base is always the next start's tip or the stack base.
                .filter_map(|(index, remote_ref_tip)| {
                    remote_ref_tip.and_then(|tip| {
                        segments_with_remote_ref_tips
                            .get((index + 1)..)
                            .and_then(|slice| {
                                slice.iter().find_map(|(index, remote_ref_tip)| {
                                    remote_ref_tip.and_then(|_| stack.segments[*index].tip())
                                })
                            })
                            .or(stack.base)
                            .map(|base| (index, tip, base))
                    })
                })
                .collect();

            for (segment_index, remote_ref_tip, base) in segments_with_remote_ref_tips_and_base {
                boundary.insert(base);

                let segment = &mut stack.segments[*segment_index];
                let local_commit_ids: gix::hashtable::HashSet = segment
                    .commits_unique_from_tip
                    .iter()
                    .map(|c| c.id)
                    .collect();

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

                for info in remote_ref_tip
                    .attach(repo)
                    .ancestors()
                    .first_parent_only()
                    .sorting(Sorting::BreadthFirst)
                    // TODO: boundary should be 'hide'.
                    .selected(|commit_id_to_yield| !boundary.contains(commit_id_to_yield))?
                {
                    let info = info?;
                    // Don't break, maybe the local commits are reachable through multiple avenues.
                    if local_commit_ids.contains(&info.id) {
                        for local_commit in &mut segment.commits_unique_from_tip {
                            local_commit.relation =
                                LocalCommitRelation::LocalAndRemote(local_commit.id);
                        }
                    } else {
                        let commit = but_core::Commit::from_id(info.id())?;
                        let has_conflicts = commit.is_conflicted();
                        if let Some(hdr) = commit.headers() {
                            insert_or_expell_ambiguous(
                                ChangeIdOrCommitData::ChangeId(hdr.change_id),
                                commit.id.detach(),
                            );
                        }
                        insert_or_expell_ambiguous(
                            ChangeIdOrCommitData::CommitData {
                                author: commit.author.clone(),
                                message: commit.message.clone(),
                            },
                            commit.id.detach(),
                        );
                        segment.commits_unique_in_remote_tracking_branch.push(
                            branch::RemoteCommit {
                                inner: commit.into(),
                                has_conflicts,
                            },
                        );
                    }
                }

                // Find duplicates harder by change-ids by commit-data.
                for local_commit in &mut segment.commits_unique_from_tip {
                    let commit = but_core::Commit::from_id(local_commit.id.attach(repo))?;
                    if let Some(hdr) = commit.headers() {
                        if let Some(remote_commit_id) = similarity_lut
                            .get(&ChangeIdOrCommitData::ChangeId(hdr.change_id))
                            .or_else(|| {
                                similarity_lut.get(&ChangeIdOrCommitData::CommitData {
                                    author: commit.author.clone(),
                                    message: commit.message.clone(),
                                })
                            })
                        {
                            local_commit.relation =
                                LocalCommitRelation::LocalAndRemote(*remote_commit_id);
                        }
                    }
                }
            }

            // Finally, check for integration into the target if available.
            // TODO: This can probably be more efficient if this is staged, by first trying
            //       to check if the tip is merged, to flag everything else as merged.
            let mut is_integrated = false;
            if let Some(target_ref_name) = target_ref_name {
                let mut check_commit = IsCommitIntegrated::new2(
                    repo,
                    &git2_repo,
                    target_ref_name.as_ref(),
                    merge_graph,
                )?;
                // TODO: remote commits could also be integrated, this seems overly simplified.
                // For now, just emulate the current implementation (hopefully).
                for local_commit in stack
                    .segments
                    .iter_mut()
                    .flat_map(|segment| &mut segment.commits_unique_from_tip)
                {
                    if is_integrated || {
                        let commit = git2_repo.find_commit(local_commit.id.to_git2())?;
                        check_commit.is_integrated(&commit)
                    }? {
                        is_integrated = true;
                        local_commit.relation = LocalCommitRelation::Integrated;
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
        ref_location: Option<RefLocation>,
        boundary_commits: &gix::hashtable::HashSet,
        preferred_refs: &[gix::refs::FullName],
        mut limit: usize,
        refs_by_id: &RefsById,
        meta: &impl but_core::RefMetadata,
    ) -> anyhow::Result<Vec<StackSegment>> {
        let mut out = Vec::new();
        let mut segment = Some(StackSegment {
            ref_name: tip_ref.map(ToOwned::to_owned),
            ref_location,
            // the tip is part of the walk.
            ..Default::default()
        });
        for (count, info) in tip
            .ancestors()
            .first_parent_only()
            .sorting(Sorting::BreadthFirst)
            // TODO: boundary should be 'hide'.
            .selected(|id_to_yield| !boundary_commits.contains(id_to_yield))?
            .enumerate()
        {
            let segment_ref = segment.as_mut().expect("a segment is always present here");

            if limit != 0 && count >= limit {
                if segment_ref.commits_unique_from_tip.is_empty() {
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
                    .find(|rn| preferred_refs.iter().any(|orn| orn == *rn))
                    .or_else(|| refs.first())
                    .map(|rn| rn.to_owned());
                if ref_at_commit.as_ref().map(|rn| rn.as_ref()) == tip_ref {
                    segment_ref
                        .commits_unique_from_tip
                        .push(LocalCommit::new_from_id(info.id())?);
                    continue;
                }
                out.extend(segment);
                segment = Some(StackSegment {
                    ref_name: ref_at_commit,
                    ref_location,
                    commits_unique_from_tip: vec![LocalCommit::new_from_id(info.id())?],
                    commits_unique_in_remote_tracking_branch: vec![],
                    remote_tracking_ref_name: None,
                    metadata: None,
                });
                continue;
            } else {
                segment_ref
                    .commits_unique_from_tip
                    .push(LocalCommit::new_from_id(info.id())?);
            }
        }
        out.extend(segment);

        for segment in out.iter_mut() {
            let Some(ref_name) = segment.ref_name.as_ref() else {
                continue;
            };
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
    fn collect_refs_by_commit_id(repo: &gix::Repository) -> anyhow::Result<RefsById> {
        let mut all_refs_by_id = gix::hashtable::HashMap::<_, Vec<_>>::default();
        for (commit_id, git_reference) in repo
            .references()?
            .prefixed("refs/heads/")?
            .filter_map(Result::ok)
            .filter_map(|r| r.try_id().map(|id| (id.detach(), r.inner.name)))
        {
            all_refs_by_id
                .entry(commit_id)
                .or_default()
                .push(git_reference);
        }
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
        if !is_gitbutler_workspace_ref(name) {
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

    fn is_gitbutler_workspace_ref(name: &gix::refs::FullNameRef) -> bool {
        name.as_bstr() == "refs/heads/gitbutler/workspace"
    }
}
