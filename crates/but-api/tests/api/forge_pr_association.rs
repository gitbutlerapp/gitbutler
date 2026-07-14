use but_core::RefMetadata;
use gix::refs::transaction::PreviousValue;

const BRANCH: &str = "feature";
const PR_NUMBER: usize = 42;

#[test]
fn managed_branch_gets_pr_from_forge_cache() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_remote_branch()?;
    let branch: gix::refs::FullName = format!("refs/heads/{BRANCH}").try_into()?;
    but_api::branch::apply_only(&mut ctx, branch.as_ref())?;
    cache_review(&ctx, PR_NUMBER)?;

    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let segment = segment(&info);
    assert!(
        segment.metadata.is_some(),
        "applied branch should be managed"
    );
    assert_eq!(
        segment
            .metadata
            .as_ref()
            .and_then(|meta| meta.review.pull_request),
        Some(PR_NUMBER),
        "managed branch should resolve its PR from the forge cache"
    );
    Ok(())
}

#[test]
fn single_branch_gets_pr_without_stored_metadata() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_remote_branch()?;
    cache_review(&ctx, PR_NUMBER)?;

    let result =
        but_api::branch::branch_checkout(&mut ctx, format!("refs/heads/{BRANCH}").try_into()?)?;
    let segment = segment(&result.workspace.head_info);
    assert_eq!(
        segment
            .metadata
            .as_ref()
            .and_then(|meta| meta.review.pull_request),
        Some(PR_NUMBER),
        "a cache hit should synthesize projection metadata in single-branch mode"
    );

    let branch_name: gix::refs::FullName = format!("refs/heads/{BRANCH}").try_into()?;
    let stored = ctx.meta()?.branch_opt(branch_name.as_ref())?;
    assert!(
        stored.is_none(),
        "projection enrichment must not persist metadata for an ad-hoc branch"
    );
    Ok(())
}

#[test]
fn empty_cache_clears_a_stale_stored_pr() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_remote_branch()?;
    let branch_name: gix::refs::FullName = format!("refs/heads/{BRANCH}").try_into()?;
    but_api::branch::apply_only(&mut ctx, branch_name.as_ref())?;

    let mut meta = ctx.meta()?;
    let mut branch = meta.branch(branch_name.as_ref())?;
    branch.review.pull_request = Some(99);
    meta.set_branch(&branch)?;
    drop(meta);

    let info = but_api::legacy::workspace::head_info(&ctx)?;
    assert_eq!(
        segment(&info)
            .metadata
            .as_ref()
            .and_then(|meta| meta.review.pull_request),
        None,
        "an empty forge cache should clear stale persisted association in the projection"
    );
    Ok(())
}

#[test]
fn optimistic_cache_insert_is_visible_on_the_next_projection() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_remote_branch()?;
    let branch: gix::refs::FullName = format!("refs/heads/{BRANCH}").try_into()?;
    but_api::branch::apply_only(&mut ctx, branch.as_ref())?;

    assert_eq!(projected_pr(&ctx)?, None, "cache starts empty");
    cache_review(&ctx, PR_NUMBER)?;
    assert_eq!(
        projected_pr(&ctx)?,
        Some(PR_NUMBER),
        "a single optimistic upsert should be visible without a list sync"
    );
    Ok(())
}

fn context_with_remote_branch() -> anyhow::Result<(
    but_ctx::Context,
    but_testsupport::gix_testtools::tempfile::TempDir,
)> {
    let (repo, tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;

    let branch_name: gix::refs::FullName = format!("refs/heads/{BRANCH}").try_into()?;
    let branch_id = repo.find_reference(&branch_name)?.peel_to_id()?.detach();
    repo.reference(
        format!("refs/remotes/origin/{BRANCH}"),
        branch_id,
        PreviousValue::Any,
        "test remote-tracking branch",
    )?;

    Ok((
        but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache(),
        tmp,
    ))
}

fn cache_review(ctx: &but_ctx::Context, number: usize) -> anyhow::Result<()> {
    let mut db = ctx.db.get_cache_mut()?;
    but_forge::cache_review(&mut db, &review(number))
}

fn review(number: usize) -> but_forge::ForgeReview {
    but_forge::ForgeReview {
        html_url: format!("https://example.com/pull/{number}"),
        number: number as i64,
        title: "Cached review".into(),
        body: None,
        author: None,
        labels: vec![],
        draft: false,
        source_branch: BRANCH.into(),
        target_branch: "main".into(),
        sha: String::new(),
        integration_commit_shas: vec![],
        created_at: None,
        modified_at: None,
        merged_at: None,
        closed_at: None,
        repository_ssh_url: None,
        repository_https_url: None,
        repo_owner: None,
        head_repo_is_fork: false,
        reviewers: vec![],
        unit_symbol: "#".into(),
        last_sync_at: Default::default(),
    }
}

fn projected_pr(ctx: &but_ctx::Context) -> anyhow::Result<Option<usize>> {
    Ok(segment(&but_api::legacy::workspace::head_info(ctx)?)
        .metadata
        .as_ref()
        .and_then(|meta| meta.review.pull_request))
}

fn segment(info: &but_workspace::RefInfo) -> &but_workspace::ref_info::Segment {
    info.stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .find(|segment| {
            segment
                .ref_info
                .as_ref()
                .is_some_and(|info| info.ref_name.shorten() == BRANCH)
        })
        .expect("feature segment exists")
}
