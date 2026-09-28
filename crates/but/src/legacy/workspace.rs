use anyhow::Context as _;
use but_core::{
    RepositoryExt, WORKSPACE_REF_NAME,
    ref_metadata::{StackId, StackKind, Workspace},
};
use but_ctx::Context;
use but_workspace::{
    RefInfo, branch,
    ref_info::{self, Segment},
    ui,
};

#[derive(Debug, Clone)]
pub struct HeadInfoStack {
    pub id: Option<StackId>,
    pub branches: Vec<HeadInfoBranch>,
}

#[derive(Debug, Clone)]
pub struct HeadInfoBranch {
    pub name: String,
    pub reference: gix::refs::FullName,
    pub tip: gix::ObjectId,
    pub base_commit: gix::ObjectId,
    pub review_id: Option<usize>,
    pub push_status: ui::PushStatus,
    pub commits: Vec<ui::Commit>,
    pub upstream_commits: Vec<ui::UpstreamCommit>,
}

impl HeadInfoStack {
    pub fn top_branch_name(&self) -> Option<&str> {
        self.branches.first().map(|branch| branch.name.as_str())
    }

    pub fn branch_names(&self) -> impl Iterator<Item = &str> {
        self.branches.iter().map(|branch| branch.name.as_str())
    }

    pub fn contains_branch(&self, branch_name: &str) -> bool {
        self.branch(branch_name).is_some()
    }

    pub fn branch(&self, branch_name: &str) -> Option<&HeadInfoBranch> {
        self.branches
            .iter()
            .find(|branch| branch.name == branch_name)
    }
}

fn head_info(
    ctx: &Context,
    expensive_commit_info: bool,
) -> anyhow::Result<(RefInfo, gix::hash::Kind)> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let object_hash = repo.object_hash();
    let meta = ctx.meta()?;
    let edit_mode_workspace_ref = edit_mode_workspace_ref(&repo)?;
    // The worktree-discovering database borrow must end before the gerrit handle
    // borrows the database again below. Only seed worktree tips when querying from
    // HEAD; during edit mode the workspace ref is queried without them.
    let ws = {
        let mut options = but_graph::init::Options::limited();
        options.worktrees =
            edit_mode_workspace_ref.is_none() && ctx.settings.feature_flags.worktree_manipulation;
        let mut db = ctx.db.get_cache_mut()?;
        let graph = match &edit_mode_workspace_ref {
            Some(ref_name) => {
                let mut reference = repo.find_reference(ref_name.as_ref())?;
                let id = reference.peel_to_id()?;
                but_graph::Graph::from_commit_traversal(
                    id,
                    reference.name().to_owned(),
                    &meta,
                    ctx.project_meta()?,
                    &mut db,
                    options,
                )?
            }
            None => {
                but_graph::Graph::from_head(&repo, &meta, ctx.project_meta()?, &mut db, options)?
            }
        };
        graph.into_workspace()?
    };
    let gerrit_mode_enabled = repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false);
    let db = gerrit_mode_enabled
        .then(|| ctx.db.get_cache())
        .transpose()?;
    let gerrit_mode = match db.as_ref() {
        Some(db) => ref_info::GerritMode::Enabled(db.gerrit_metadata()),
        None => ref_info::GerritMode::Disabled,
    };
    let options = ref_info::Options {
        project_meta: ctx.project_meta()?,
        traversal: but_graph::init::Options::limited(),
        expensive_commit_info,
        gerrit_mode,
    };
    let mut info = ref_info::graph_to_ref_info(&ws, &repo, options)?;

    // Enrich active associations from the forge cache while keeping durable
    // stored identity for integrated branches, mirroring desktop `head_info`.
    let review_cache = ctx.db.get_cache()?;
    let prs_by_head = but_forge::review_associations_by_head(&review_cache)?;
    info.apply_forge_review_associations(&repo, &prs_by_head);

    Ok((info, object_hash))
}

fn edit_mode_workspace_ref(repo: &gix::Repository) -> anyhow::Result<Option<gix::refs::FullName>> {
    let is_edit_mode = repo.head().ok().is_some_and(|head| {
        head.referent_name().is_some_and(|head_ref| {
            head_ref.as_bstr() == gitbutler_operating_modes::EDIT_BRANCH_REF
        })
    });
    if !is_edit_mode {
        return Ok(None);
    }
    for name in [
        gitbutler_operating_modes::WORKSPACE_BRANCH_REF,
        gitbutler_operating_modes::INTEGRATION_BRANCH_REF,
    ] {
        let ref_name: gix::refs::FullName = name.try_into()?;
        if repo.try_find_reference(ref_name.as_ref())?.is_some() {
            return Ok(Some(ref_name));
        }
    }
    Ok(None)
}

pub fn applied_stacks(ctx: &Context) -> anyhow::Result<Vec<HeadInfoStack>> {
    applied_stacks_with_options(ctx, false)
}

pub fn applied_stacks_with_expensive_commit_info(
    ctx: &Context,
) -> anyhow::Result<Vec<HeadInfoStack>> {
    applied_stacks_with_options(ctx, true)
}

fn applied_stacks_with_options(
    ctx: &Context,
    expensive_commit_info: bool,
) -> anyhow::Result<Vec<HeadInfoStack>> {
    let metadata = workspace_metadata(&ctx.meta()?)?;
    let (info, object_hash) = head_info(ctx, expensive_commit_info)?;
    Ok(head_info_stacks(
        &info,
        metadata.as_ref(),
        object_hash.null(),
        ctx.settings.feature_flags.single_branch,
    ))
}

/// Every lane: each stack, then each linked worktree in tip order without an id. Segments
/// without a ref are left out, as nothing can name them.
pub fn applied_lanes(ctx: &Context) -> anyhow::Result<Vec<HeadInfoStack>> {
    applied_lanes_with_options(ctx, false)
}

pub fn applied_lanes_with_expensive_commit_info(
    ctx: &Context,
) -> anyhow::Result<Vec<HeadInfoStack>> {
    applied_lanes_with_options(ctx, true)
}

fn applied_lanes_with_options(
    ctx: &Context,
    expensive_commit_info: bool,
) -> anyhow::Result<Vec<HeadInfoStack>> {
    let metadata = workspace_metadata(&ctx.meta()?)?;
    let (info, object_hash) = head_info(ctx, expensive_commit_info)?;
    let null_id = object_hash.null();
    let mut lanes = head_info_stacks(
        &info,
        metadata.as_ref(),
        null_id,
        ctx.settings.feature_flags.single_branch,
    );
    for worktree in &info.worktrees {
        lanes.push(HeadInfoStack {
            id: None,
            branches: worktree
                .segments
                .iter()
                .filter(|segment| segment.ref_info.is_some())
                .map(|segment| head_info_branch(segment, null_id))
                .collect::<Result<_, _>>()?,
        });
    }
    Ok(lanes)
}

/// Every commit a push of `branch` transfers, top-to-base, following the lane chain beneath a
/// worktree and including commits no branch names. `None` if `branch` is in no lane.
pub fn push_scope_with_expensive_commit_info(
    ctx: &Context,
    branch: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<Vec<ui::Commit>>> {
    let (info, _) = head_info(ctx, true)?;
    let segments = but_workspace::legacy::push::branch_and_ancestor_segments(&info, branch);
    Ok((!segments.is_empty()).then(|| {
        segments
            .values()
            .flat_map(|segment| &segment.commits)
            .map(Into::into)
            .collect()
    }))
}

/// The branches a review of `branch` stacks on followed by `branch` itself, base first: those
/// beneath it in its lane and in each lane it rests on. `None` if `branch` is in no lane.
pub fn review_chain(
    ctx: &Context,
    branch: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<Vec<HeadInfoBranch>>> {
    let (info, object_hash) = head_info(ctx, false)?;
    let segments = but_workspace::legacy::push::branch_and_ancestor_segments(&info, branch);
    if segments.is_empty() {
        return Ok(None);
    }
    segments
        .values()
        .rev()
        .filter(|segment| segment.ref_info.is_some())
        .map(|segment| head_info_branch(segment, object_hash.null()))
        .collect::<anyhow::Result<_>>()
        .map(Some)
}

fn workspace_metadata(meta: &impl but_core::RefMetadata) -> anyhow::Result<Option<Workspace>> {
    let workspace_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
    Ok(meta
        .workspace_opt(workspace_ref.as_ref())?
        .map(|workspace| (*workspace).clone()))
}

fn head_info_stacks(
    info: &RefInfo,
    metadata: Option<&Workspace>,
    null_id: gix::ObjectId,
    retain_single_branch_id: bool,
) -> Vec<HeadInfoStack> {
    info.stacks
        .iter()
        .filter_map(|stack| {
            match head_info_stack(stack, metadata, null_id, retain_single_branch_id) {
                Ok(stack) => Some(stack),
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        "Skipping head_info stack that the CLI cannot represent"
                    );
                    None
                }
            }
        })
        .collect()
}

fn head_info_stack(
    stack: &branch::Stack,
    metadata: Option<&Workspace>,
    null_id: gix::ObjectId,
    retain_single_branch_id: bool,
) -> anyhow::Result<HeadInfoStack> {
    let branches = stack
        .segments
        .iter()
        .map(|segment| head_info_branch(segment, null_id))
        .collect::<Result<Vec<_>, _>>()?;
    let metadata_id = metadata.and_then(|metadata| {
        stack
            .segments
            .iter()
            .filter_map(|segment| segment.ref_info.as_ref())
            .find_map(|ref_info| {
                metadata
                    .find_stack_with_branch(
                        ref_info.ref_name.as_ref(),
                        StackKind::AppliedAndUnapplied,
                    )
                    .map(|stack| stack.id)
            })
    });
    let projection_id = stack
        .id
        .filter(|id| retain_single_branch_id || *id != StackId::single_branch_id());
    let id = metadata_id.or(projection_id);
    Ok(HeadInfoStack { id, branches })
}

fn head_info_branch(segment: &Segment, null_id: gix::ObjectId) -> anyhow::Result<HeadInfoBranch> {
    let Segment {
        ref_info,
        commits: local_commits,
        commits_on_remote,
        metadata,
        push_status,
        base,
        ..
    } = segment;
    let ref_info = ref_info
        .clone()
        .context("Can't handle a stack yet whose tip isn't pointed to by a ref")?;
    let base_commit = base.unwrap_or(null_id);
    let tip = ref_info
        .commit_id
        .or_else(|| segment.tip())
        .unwrap_or(base_commit);
    Ok(HeadInfoBranch {
        name: ref_info.ref_name.shorten().to_string(),
        reference: ref_info.ref_name,
        tip,
        base_commit,
        review_id: metadata.as_ref().and_then(|meta| meta.review.pull_request),
        push_status: *push_status,
        commits: local_commits.iter().map(Into::into).collect(),
        upstream_commits: commits_on_remote.iter().map(Into::into).collect(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use but_core::ref_metadata::ProjectMeta;
    use but_ctx::Context;
    use but_testsupport::{CommandExt, git_at_dir, open_repo};

    use super::review_chain;

    /// `C` checked out over `B` over `A`, with a linked worktree on branch `W` resting on `B`, all
    /// but `C` pushed, and open reviews #1 on `A`, #2 on `B` and #3 on `W` in the forge cache.
    pub(crate) fn context_with_worktree_on_reviewed_stack()
    -> anyhow::Result<(Context, tempfile::TempDir)> {
        let tmp = tempfile::tempdir()?;
        let git = |args: &[&str]| git_at_dir(tmp.path()).args(args).run();
        git(&["init", "-b", "main"]);
        git(&["config", "user.name", "GitButler"]);
        git(&["config", "user.email", "gitbutler@example.com"]);
        git(&["commit", "--allow-empty", "-m", "base"]);
        git(&["config", "remote.origin.url", "../origin"]);
        git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        for branch in ["A", "B", "C"] {
            git(&["checkout", "-b", branch]);
            git(&["commit", "--allow-empty", "-m", branch]);
        }
        let worktree = tmp.path().join("worktrees").join("W");
        git_at_dir(tmp.path())
            .args(["worktree", "add", "-b", "W"])
            .arg(&worktree)
            .arg("B")
            .run();
        git_at_dir(&worktree)
            .args(["commit", "--allow-empty", "-m", "W"])
            .run();
        for branch in ["A", "B", "W"] {
            git(&[
                "update-ref",
                &format!("refs/remotes/origin/{branch}"),
                branch,
            ]);
        }

        let repo = open_repo(tmp.path())?;
        ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
            push_remote: Some("origin".into()),
        }
        .persist(&repo)?;
        let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        ctx.settings.feature_flags.worktree_manipulation = true;
        {
            let mut db = ctx.db.get_cache_mut()?;
            // Adoption already ran, so the worktree on disk counts as active.
            db.worktree_meta_mut().mark_adopted()?;
            for (number, branch) in [(1, "A"), (2, "B"), (3, "W")] {
                but_forge::cache_review(&mut db, &open_review(number, branch))?;
            }
        }
        Ok((ctx, tmp))
    }

    fn open_review(number: i64, source_branch: &str) -> but_forge::ForgeReview {
        but_forge::ForgeReview {
            html_url: String::new(),
            number,
            title: String::new(),
            body: None,
            author: None,
            labels: Vec::new(),
            draft: false,
            source_branch: source_branch.into(),
            target_branch: "main".into(),
            sha: String::new(),
            integration_commit_shas: Vec::new(),
            created_at: None,
            modified_at: None,
            merged_at: None,
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            head_repo_is_fork: false,
            auto_merge_enabled: false,
            reviewers: Vec::new(),
            unit_symbol: "#".into(),
            last_sync_at: Default::default(),
        }
    }

    #[test]
    fn a_review_chain_crosses_into_the_lane_a_worktree_rests_on() -> anyhow::Result<()> {
        let (ctx, _tmp) = context_with_worktree_on_reviewed_stack()?;
        let chain = |branch: &str| -> anyhow::Result<Option<Vec<String>>> {
            let branch = gix::refs::Category::LocalBranch.to_full_name(branch)?;
            Ok(review_chain(&ctx, branch.as_ref())?
                .map(|chain| chain.into_iter().map(|branch| branch.name).collect()))
        };

        assert_eq!(
            chain("W")?,
            Some(vec!["A".into(), "B".into(), "W".into()]),
            "the worktree's review stacks on the stack beneath it"
        );
        assert_eq!(
            chain("B")?,
            Some(vec!["A".into(), "B".into()]),
            "a stack branch does not reach up into the worktree resting on it"
        );
        assert_eq!(chain("main")?, None, "the target is in no lane");
        Ok(())
    }
}
