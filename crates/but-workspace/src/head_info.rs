/// Options for the [`head_info()`](crate::head_info) call.
#[derive(Debug, Copy, Clone)]
pub struct Options {
    /// The maximum amount of commits to list *per stack*. Note that a [`StackSegment`](crate::branch::StackSegment) will always have a single commit, if available,
    ///  even if this would exhaust the commit limit in that stack.
    /// `0` means the limit is disabled.
    ///
    ///  NOTE: Currently, to fetch more commits, make this call again with a higher limit.
    pub stack_commit_limit: usize,
}
pub(crate) mod function {
    use crate::HeadInfo;
    use crate::branch::{RefLocation, Stack, StackSegment};
    use bstr::ByteSlice;
    use but_core::ref_metadata::ValueInfo;
    use gix::prelude::ReferenceExt;
    use gix::revision::walk::Sorting;

    /// Gather information about the current `HEAD` and the workspace that might be associated with it, based on data in `repo` and `meta`.
    ///
    /// Use `options` to further configure the call.
    ///
    /// ### Performance
    ///
    /// Make sure the `repo` is initialized with a decently sized Object cache so querying the same commit multiple times will be cheap(er).
    pub fn head_info(
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
        options: super::Options,
    ) -> anyhow::Result<HeadInfo> {
        let head = repo.head()?;
        let mut existing_ref = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                return Ok(HeadInfo {
                    target_ref: workspace_data_of_workspace_branch(meta, ref_name.as_ref())?
                        .and_then(|ws| ws.target_ref),
                    stacks: vec![Stack {
                        index: 0,
                        tip: None,
                        segments: vec![StackSegment {
                            commits_unique_from_tip: vec![],
                            commits_unintegratd_local: vec![],
                            commits_unintegrated_upstream: vec![],
                            remote_tracking_ref_name: None,
                            metadata: branch_metadata_opt(meta, ref_name.as_ref())?,
                            ref_location: Some(RefLocation::AtHead),
                            ref_name: Some(ref_name),
                        }],
                        stash_status: None,
                    }],
                });
            }
            gix::head::Kind::Detached { .. } => {
                return Ok(HeadInfo {
                    stacks: vec![],
                    target_ref: None,
                });
            }
            gix::head::Kind::Symbolic(name) => name.attach(repo),
        };

        let ws_data = workspace_data_of_workspace_branch(meta, existing_ref.name())?;
        let target_ref = if let Some(_data) = ws_data {
            todo!(
                "figure out what to do with workspace information, consolidate it with what's there as well"
            );
        } else {
            None
        };

        let head_commit = existing_ref.peel_to_commit()?;
        let head_commit = crate::WorkspaceCommit {
            id: head_commit.id(),
            inner: head_commit.decode()?.to_owned(),
        };
        if head_commit.is_managed() {
            todo!("deal with managed commits");
        } else {
            // Discover all references that actually point to the reachable graph.
            let refs_by_id = collect_refs_by_commit_id(repo)?;
            let segments = collect_stack_segments(
                head_commit.id,
                Some(existing_ref.name()),
                Some(RefLocation::AtHead),
                &[],                               /* boundary commits */
                &[existing_ref.name().to_owned()], /* preferred refs */
                options.stack_commit_limit,
                &refs_by_id,
            )?;
            Ok(HeadInfo {
                stacks: vec![Stack {
                    index: 0,
                    tip: segments
                        .first()
                        .and_then(|stack| Some(stack.commits_unique_from_tip.first()?.id)),
                    segments,
                    stash_status: None,
                }],
                target_ref,
            })
        }
    }

    /// Walk down the commit-graph from `tip` until a `boundary_commits` is encountered, excluding it, or to the graph root if there is no boundary.
    /// Walk along the first parent, and return stack segments on its path using the `refs_by_commit_id` reverse mapping in walk order.
    /// `tip_ref` is the name of the reference pointing to `tip` if it's known.
    /// `ref_location` it the location of `tip_ref`
    /// `preferred_refs` is an arbitrarily sorted array of names that should be used in the returned segments if they are encountered during the traversal
    /// *and* there are more than one ref pointing to it.
    ///
    /// Note that `boundary_commits` are sorted so binary-search can be used to quickly check membership.
    fn collect_stack_segments(
        tip: gix::Id<'_>,
        tip_ref: Option<&gix::refs::FullNameRef>,
        ref_location: Option<RefLocation>,
        boundary_commits: &[gix::ObjectId],
        preferred_refs: &[gix::refs::FullName],
        mut limit: usize,
        refs_by_id: &RefsById,
    ) -> anyhow::Result<Vec<StackSegment>> {
        let mut out = Vec::new();
        let mut segment = Some(StackSegment {
            ref_name: tip_ref.map(ToOwned::to_owned),
            ref_location,
            // the tip is part of the walk.
            ..Default::default()
        });
        for (count, info) in (tip
            .ancestors()
            .first_parent_only()
            .sorting(Sorting::BreadthFirst)
            .all()?)
        .enumerate()
        {
            if limit != 0 && count >= limit {
                if segment.as_ref().unwrap().commits_unique_from_tip.is_empty() {
                    limit += 1;
                } else {
                    out.extend(segment.take());
                    break;
                }
            }
            let info = info?;
            if boundary_commits.binary_search(&info.id).is_ok() {
                out.extend(segment.take());
                break;
            }

            if let Some(refs) = refs_by_id.get(&info.id) {
                let ref_name = refs
                    .iter()
                    .find(|rn| preferred_refs.iter().any(|orn| orn == *rn))
                    .or_else(|| refs.first())
                    .map(|rn| rn.to_owned());
                if ref_name.as_ref().map(|rn| rn.as_ref()) == tip_ref {
                    segment
                        .as_mut()
                        .expect("always present")
                        .commits_unique_from_tip
                        .push(info.id().try_into()?);
                    continue;
                }
                out.extend(segment);
                segment = Some(StackSegment {
                    ref_name,
                    ref_location,
                    commits_unique_from_tip: vec![info.id().try_into()?],
                    commits_unintegratd_local: vec![],
                    commits_unintegrated_upstream: vec![],
                    remote_tracking_ref_name: None,
                    metadata: None,
                });
                continue;
            } else {
                segment
                    .as_mut()
                    .unwrap()
                    .commits_unique_from_tip
                    .push(info.id().try_into()?);
            }
        }
        out.extend(segment);
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
    fn workspace_data_of_workspace_branch(
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

    fn is_gitbutler_workspace_ref(name: &gix::refs::FullNameRef) -> bool {
        name.shorten().starts_with_str("gitbutler/workspace/")
    }
}
