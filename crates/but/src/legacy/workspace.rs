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
    let gerrit_mode_enabled = repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false);
    let db = gerrit_mode_enabled
        .then(|| ctx.db.get_cache())
        .transpose()?;
    let gerrit_mode = match db.as_ref() {
        Some(db) => ref_info::GerritMode::Enabled(db.gerrit_metadata()),
        None => ref_info::GerritMode::Disabled,
    };
    let options = ref_info::Options {
        traversal: but_graph::init::Options::limited(),
        expensive_commit_info,
        gerrit_mode,
    };
    let info = match edit_mode_workspace_ref(&repo)? {
        Some(ref_name) => {
            but_workspace::ref_info(repo.find_reference(ref_name.as_ref())?, &meta, options)
        }
        None => but_workspace::head_info(&repo, &meta, options),
    }?
    .pruned_to_entrypoint();
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
        info,
        metadata.as_ref(),
        object_hash.null(),
    ))
}

pub fn applied_stack(ctx: &Context, stack_id: Option<StackId>) -> anyhow::Result<HeadInfoStack> {
    let stacks = applied_stacks(ctx)?;
    applied_stack_from_stacks(stacks, stack_id)
}

pub fn applied_stack_with_expensive_commit_info(
    ctx: &Context,
    stack_id: Option<StackId>,
) -> anyhow::Result<HeadInfoStack> {
    let stacks = applied_stacks_with_expensive_commit_info(ctx)?;
    applied_stack_from_stacks(stacks, stack_id)
}

fn applied_stack_from_stacks(
    stacks: Vec<HeadInfoStack>,
    stack_id: Option<StackId>,
) -> anyhow::Result<HeadInfoStack> {
    match stack_id {
        Some(stack_id) => stacks
            .into_iter()
            .find(|stack| stack.id == Some(stack_id))
            .with_context(|| format!("Stack {stack_id} not found in workspace")),
        None => stacks
            .into_iter()
            .next()
            .context("Expected at least one stack in workspace"),
    }
}

fn workspace_metadata(meta: &impl but_core::RefMetadata) -> anyhow::Result<Option<Workspace>> {
    let workspace_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
    Ok(meta
        .workspace_opt(workspace_ref.as_ref())?
        .map(|workspace| (*workspace).clone()))
}

fn head_info_stacks(
    info: RefInfo,
    metadata: Option<&Workspace>,
    null_id: gix::ObjectId,
) -> Vec<HeadInfoStack> {
    info.stacks
        .iter()
        .filter_map(|stack| match head_info_stack(stack, metadata, null_id) {
            Ok(stack) => Some(stack),
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "Skipping head_info stack that the CLI cannot represent"
                );
                None
            }
        })
        .collect()
}

fn head_info_stack(
    stack: &branch::Stack,
    metadata: Option<&Workspace>,
    null_id: gix::ObjectId,
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
    let projection_id = stack.id.filter(|id| *id != StackId::single_branch_id());
    let id = metadata_id.or(projection_id);
    Ok(HeadInfoStack { id, branches })
}

fn head_info_branch(segment: &Segment, null_id: gix::ObjectId) -> anyhow::Result<HeadInfoBranch> {
    let Segment {
        ref_info,
        commits: local_commits,
        commits_on_remote,
        commits_outside,
        metadata,
        push_status,
        base,
        ..
    } = segment;
    let ref_info = ref_info
        .clone()
        .context("Can't handle a stack yet whose tip isn't pointed to by a ref")?;
    if let Some(commits_outside) = commits_outside
        .as_ref()
        .filter(|commits| !commits.is_empty())
    {
        tracing::warn!(
            ignored_outside_commits = commits_outside.len(),
            stack_segment_ref = %ref_info.ref_name,
            "CLI head_info branch drops commits_outside for this stack segment"
        );
    }

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
