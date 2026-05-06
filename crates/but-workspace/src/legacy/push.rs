//! Workspace-derived data needed to push stack branches.

use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_core::{ref_metadata::StackId, sync::RepoShared};
use but_ctx::Context;
use gix::refs::Category;

/// The target and remote configuration used when pushing a stack.
#[derive(Clone)]
#[non_exhaustive]
pub struct PushTarget {
    target_ref_name: gix::refs::FullName,
    target_base_oid: gix::ObjectId,
    target_remote_name: String,
    push_remote_name: String,
    target_branch_name: String,
}

impl PushTarget {
    /// Read push target information from persisted default-target metadata.
    pub fn from_context(ctx: &Context) -> Result<Self> {
        let default_target = ctx.persisted_default_target()?;
        let target_remote_name = default_target.branch.remote().to_owned();
        let push_remote_name = default_target
            .push_remote_name
            .unwrap_or_else(|| target_remote_name.clone());
        Ok(Self {
            target_ref_name: default_target.branch.to_string().try_into()?,
            target_base_oid: default_target.sha,
            target_remote_name,
            push_remote_name,
            target_branch_name: default_target.branch.branch().to_owned(),
        })
    }

    /// The remote backing the integration target.
    pub fn target_remote_name(&self) -> &str {
        &self.target_remote_name
    }

    /// The remote to push stack branches to.
    pub fn push_remote_name(&self) -> &str {
        &self.push_remote_name
    }

    /// The remotes that should be fetched before planning a push.
    pub fn remote_names_to_fetch(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.target_remote_name()).chain(
            (self.push_remote_name() != self.target_remote_name())
                .then_some(self.push_remote_name()),
        )
    }

    /// The target branch short name, used for Gerrit push refspecs.
    pub fn target_branch_name(&self) -> &str {
        &self.target_branch_name
    }
}

/// A prepared stack push plan with branches that should actually be pushed.
#[non_exhaustive]
pub struct PushStackPlan {
    id: StackId,
    gerrit_metadata_range: CommitRange,
    branches: Vec<PushBranch>,
}

/// A first-parent commit range.
#[derive(Clone, Copy)]
pub struct CommitRange {
    /// Range start, included.
    pub head: gix::ObjectId,
    /// Range end, excluded.
    pub base: gix::ObjectId,
}

impl PushStackPlan {
    /// Prepare push data for branches in `stack_id`.
    ///
    /// This reads stack structure from the current workspace projection and filters out archived,
    /// no-op, and already-integrated branches. Callers are responsible for fetching
    /// [`PushTarget::remote_names_to_fetch()`] before invoking this method if they need up-to-date
    /// remote refs.
    pub fn from_workspace(
        ctx: &Context,
        stack_id: StackId,
        target: &PushTarget,
        stop_after_branch: &str,
        perm: &RepoShared,
    ) -> Result<Self> {
        let stack = WorkspaceStack::from_workspace(ctx, stack_id, perm)?;
        PushStackPlanner::new(ctx, target, stack.head_oid)?.plan(stack_id, stack, stop_after_branch)
    }

    /// The stable stack ID.
    pub fn id(&self) -> StackId {
        self.id
    }

    /// Branches to push, ordered from stack base toward stack tip.
    pub fn branches(&self) -> &[PushBranch] {
        &self.branches
    }

    /// The commit range used to record Gerrit push metadata.
    pub fn gerrit_metadata_range(&self) -> CommitRange {
        self.gerrit_metadata_range
    }
}

/// A branch to push.
#[non_exhaustive]
pub struct PushBranch {
    branch_name: String,
    remote_refname: gix::refs::FullName,
    local_sha: gix::ObjectId,
    before_sha: gix::ObjectId,
}

impl PushBranch {
    /// The local branch short name.
    pub fn name(&self) -> &str {
        &self.branch_name
    }

    /// The corresponding remote-tracking reference.
    pub fn remote_refname(&self) -> &gix::refs::FullNameRef {
        self.remote_refname.as_ref()
    }

    /// The local commit to push.
    pub fn local_sha(&self) -> gix::ObjectId {
        self.local_sha
    }

    /// The remote commit observed before pushing, or the null object ID for new branches.
    pub fn before_sha(&self) -> gix::ObjectId {
        self.before_sha
    }
}

struct WorkspaceStack {
    head_oid: gix::ObjectId,
    branches: Vec<WorkspaceStackBranch>,
}

struct WorkspaceStackBranch {
    branch_name: String,
    archived: bool,
    local_sha: gix::ObjectId,
}

impl WorkspaceStack {
    fn from_workspace(ctx: &Context, stack_id: StackId, perm: &RepoShared) -> Result<Self> {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
        let stack = ws.try_find_stack_by_id(stack_id)?;
        let archived_by_ref_name = ws
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.stacks.iter().find(|stack| stack.id == stack_id))
            .map(|stack| {
                stack
                    .branches
                    .iter()
                    .map(|branch| (branch.ref_name.clone(), branch.archived))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        let branches = stack
            .segments
            .iter()
            .rev()
            .map(|segment| -> Result<_> {
                let ref_name = segment
                    .ref_info
                    .as_ref()
                    .map(|ref_info| ref_info.ref_name.clone())
                    .context("Cannot push a stack segment without a local branch reference")?;
                let archived = archived_by_ref_name
                    .get(&ref_name)
                    .copied()
                    .unwrap_or(false);
                WorkspaceStackBranch::from_ref(&repo, ref_name, archived)
            })
            .collect::<Result<Vec<_>>>()?;
        let head_oid = branches
            .last()
            .map(|branch| branch.local_sha)
            .context("Cannot push an empty stack")?;

        Ok(Self { head_oid, branches })
    }
}

impl WorkspaceStackBranch {
    fn from_ref(
        repo: &gix::Repository,
        ref_name: gix::refs::FullName,
        archived: bool,
    ) -> Result<Self> {
        let branch_name = Self::branch_name(ref_name.as_ref())?;
        let local_sha = Self::tip(repo, ref_name.as_ref()).with_context(|| {
            format!(
                "Failed to resolve local branch '{}' for pushing",
                ref_name.shorten()
            )
        })?;
        Ok(Self {
            branch_name,
            archived,
            local_sha,
        })
    }

    fn branch_name(ref_name: &gix::refs::FullNameRef) -> Result<String> {
        let (category, short_name) = ref_name
            .category_and_short_name()
            .context("Branch reference could not be categorized")?;
        if !matches!(category, Category::LocalBranch) {
            bail!("Expected a local branch reference, got {ref_name}");
        }
        Ok(short_name.to_str_lossy().into_owned())
    }

    fn tip(repo: &gix::Repository, ref_name: &gix::refs::FullNameRef) -> Result<gix::ObjectId> {
        Ok(repo.find_reference(ref_name)?.peel_to_commit()?.id)
    }

    fn log_skipped(&self, reason: SkipBranchReason) {
        match reason {
            SkipBranchReason::Archived => {
                tracing::debug!(
                    branch = self.branch_name,
                    "skipping archived branch for pushing"
                );
            }
            SkipBranchReason::HeadAtMergeBase => {
                tracing::debug!(
                    branch = self.branch_name,
                    "nothing to push as head_oid == merge_base"
                );
            }
            SkipBranchReason::Integrated => {
                tracing::debug!(
                    branch = self.branch_name,
                    "Skipping push for integrated branch"
                );
            }
        }
    }
}

struct PushStackPlanner<'target> {
    target: &'target PushTarget,
    gix_repo: gix::Repository,
    commit_graph_cache: Option<gix::commitgraph::Graph>,
    merge_base_id: gix::ObjectId,
}

impl<'target> PushStackPlanner<'target> {
    fn new(
        ctx: &Context,
        target: &'target PushTarget,
        stack_head_oid: gix::ObjectId,
    ) -> Result<Self> {
        let gix_repo = ctx.clone_repo_for_merging_non_persisting()?;
        let merge_base_id = gix_repo
            .merge_base(stack_head_oid, target.target_base_oid)?
            .detach();
        let commit_graph_cache = gix_repo.commit_graph_if_enabled()?;

        Ok(Self {
            target,
            gix_repo,
            commit_graph_cache,
            merge_base_id,
        })
    }

    fn plan(
        &self,
        stack_id: StackId,
        stack: WorkspaceStack,
        stop_after_branch: &str,
    ) -> Result<PushStackPlan> {
        let mut branches = Vec::new();
        for branch in &stack.branches {
            let Some(branch) = self.prepare_branch(branch)? else {
                continue;
            };
            let should_stop = branch.branch_name == stop_after_branch;
            branches.push(branch);
            if should_stop {
                break;
            }
        }

        Ok(PushStackPlan {
            id: stack_id,
            gerrit_metadata_range: CommitRange {
                head: stack.head_oid,
                base: self.merge_base_id,
            },
            branches,
        })
    }

    fn prepare_branch(&self, branch: &WorkspaceStackBranch) -> Result<Option<PushBranch>> {
        if branch.archived {
            branch.log_skipped(SkipBranchReason::Archived);
            return Ok(None);
        }

        let local_sha = branch.local_sha;
        if let Some(reason) = self.skip_reason(local_sha)? {
            branch.log_skipped(reason);
            return Ok(None);
        }

        let remote_refname = self.remote_refname_for_branch(&branch.branch_name)?;
        let before_sha = self.remote_before_sha(remote_refname.as_ref())?;

        Ok(Some(PushBranch {
            branch_name: branch.branch_name.clone(),
            remote_refname,
            local_sha,
            before_sha,
        }))
    }

    fn skip_reason(&self, local_sha: gix::ObjectId) -> Result<Option<SkipBranchReason>> {
        if local_sha == self.merge_base_id {
            return Ok(Some(SkipBranchReason::HeadAtMergeBase));
        }

        let mut graph = self
            .gix_repo
            .revision_graph(self.commit_graph_cache.as_ref());
        let mut check_commit = super::integrated::IsCommitIntegrated::new_with_target(
            &self.gix_repo,
            self.target.target_ref_name.as_ref(),
            self.target.target_base_oid,
            &mut graph,
        )?;
        if check_commit.is_integrated(local_sha)? {
            return Ok(Some(SkipBranchReason::Integrated));
        }

        Ok(None)
    }

    fn remote_before_sha(&self, remote_refname: &gix::refs::FullNameRef) -> Result<gix::ObjectId> {
        Ok(self
            .gix_repo
            .try_find_reference(remote_refname)?
            .map(|mut reference| reference.peel_to_commit())
            .transpose()?
            .map(|commit| commit.id)
            .unwrap_or(self.gix_repo.object_hash().null()))
    }

    fn remote_refname_for_branch(&self, branch_name: &str) -> Result<gix::refs::FullName> {
        let remote_name = &self.target.push_remote_name;
        format!("refs/remotes/{remote_name}/{branch_name}")
            .try_into()
            .map_err(Into::into)
    }
}

enum SkipBranchReason {
    Archived,
    HeadAtMergeBase,
    Integrated,
}

#[cfg(test)]
mod tests {
    use but_core::ref_metadata::StackId;

    use super::*;

    #[test]
    fn target_fetch_remotes_include_target_and_push_remotes_once() -> Result<()> {
        let target = PushTarget {
            target_ref_name: "refs/remotes/origin/main".try_into()?,
            target_base_oid: gix::hash::Kind::Sha1.null(),
            target_remote_name: "origin".into(),
            push_remote_name: "fork".into(),
            target_branch_name: "main".into(),
        };

        assert_eq!(
            target.remote_names_to_fetch().collect::<Vec<_>>(),
            vec!["origin", "fork"]
        );

        let target = PushTarget {
            push_remote_name: "origin".into(),
            ..target
        };
        assert_eq!(
            target.remote_names_to_fetch().collect::<Vec<_>>(),
            vec!["origin"]
        );
        Ok(())
    }

    #[test]
    fn plan_orders_branches_from_base_to_tip_and_honors_stop_branch() -> Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("single-stack-two-segments");
        let repo = repo.with_object_memory();
        let target = target(&repo, "refs/remotes/origin/main", "main", "main")?;

        let stack = WorkspaceStack {
            head_oid: oid(&repo, "A2")?,
            branches: vec![branch(&repo, "A1")?, branch(&repo, "A2")?],
        };
        let plan = planner(&repo, &target, stack.head_oid)?.plan(
            StackId::from_number_for_testing(1),
            stack,
            "A2",
        )?;
        assert_eq!(branch_names(&plan), vec!["A1", "A2"]);
        assert_eq!(plan.gerrit_metadata_range().head, oid(&repo, "A2")?);
        assert_eq!(plan.gerrit_metadata_range().base, oid(&repo, "main")?);

        let stack = WorkspaceStack {
            head_oid: oid(&repo, "A2")?,
            branches: vec![branch(&repo, "A1")?, branch(&repo, "A2")?],
        };
        let plan = planner(&repo, &target, stack.head_oid)?.plan(
            StackId::from_number_for_testing(1),
            stack,
            "A1",
        )?;
        assert_eq!(branch_names(&plan), vec!["A1"]);
        Ok(())
    }

    #[test]
    fn plan_filters_archived_noop_and_integrated_branches() -> Result<()> {
        let (repo, _tmp) =
            but_testsupport::writable_scenario("diamond-partially-content-integrated");
        let repo = repo.with_object_memory();
        let target = target(&repo, "refs/remotes/origin/master", "o1", "master")?;
        let stack = WorkspaceStack {
            head_oid: oid(&repo, "E")?,
            branches: vec![
                WorkspaceStackBranch {
                    archived: true,
                    ..branch(&repo, "D")?
                },
                WorkspaceStackBranch {
                    branch_name: "noop".into(),
                    archived: false,
                    local_sha: oid(&repo, "o1")?,
                },
                WorkspaceStackBranch {
                    branch_name: "integrated".into(),
                    archived: false,
                    local_sha: oid(&repo, "refs/remotes/origin/master")?,
                },
                branch(&repo, "E")?,
            ],
        };

        let plan = planner(&repo, &target, stack.head_oid)?.plan(
            StackId::from_number_for_testing(1),
            stack,
            "E",
        )?;

        assert_eq!(branch_names(&plan), vec!["E"]);
        Ok(())
    }

    fn target(
        repo: &gix::Repository,
        target_ref_name: &str,
        target_base_rev: &str,
        target_branch_name: &str,
    ) -> Result<PushTarget> {
        Ok(PushTarget {
            target_ref_name: target_ref_name.try_into()?,
            target_base_oid: oid(repo, target_base_rev)?,
            target_remote_name: "origin".into(),
            push_remote_name: "origin".into(),
            target_branch_name: target_branch_name.into(),
        })
    }

    fn planner<'target>(
        repo: &gix::Repository,
        target: &'target PushTarget,
        stack_head_oid: gix::ObjectId,
    ) -> Result<PushStackPlanner<'target>> {
        Ok(PushStackPlanner {
            target,
            gix_repo: repo.clone(),
            commit_graph_cache: repo.commit_graph_if_enabled()?,
            merge_base_id: repo
                .merge_base(stack_head_oid, target.target_base_oid)?
                .detach(),
        })
    }

    fn branch(repo: &gix::Repository, name: &str) -> Result<WorkspaceStackBranch> {
        Ok(WorkspaceStackBranch {
            branch_name: name.into(),
            archived: false,
            local_sha: oid(repo, name)?,
        })
    }

    fn oid(repo: &gix::Repository, rev: &str) -> Result<gix::ObjectId> {
        Ok(repo.rev_parse_single(rev)?.detach())
    }

    fn branch_names(plan: &PushStackPlan) -> Vec<&str> {
        plan.branches().iter().map(PushBranch::name).collect()
    }
}
