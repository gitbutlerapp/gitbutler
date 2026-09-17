use but_core::RefMetadata;
use but_rebase::graph_rebase::mutate::InsertSide;

use crate::support::{assert_workspace_ref, repo_with_feature_branch};

#[test]
fn branch_create_independent_respects_insertion_order() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    but_api::branch::apply_only(&mut ctx, feature.as_ref())?;

    for (name, order, expected_index) in [
        ("first", Some(0), 0),
        ("second", Some(0), 0),
        ("middle", Some(1), 1),
        ("last", None, 4),
        ("clamped", Some(100), 5),
    ] {
        let new_ref = gix::refs::FullName::try_from(format!("refs/heads/{name}"))?;
        but_api::branch::branch_create(
            &mut ctx,
            Some(new_ref.clone()),
            but_api::branch::json::BranchCreatePlacement::Independent { order },
        )?;
        let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
        assert_eq!(
            ws.find_segment_owner_indexes_by_refname(new_ref.as_ref())
                .map(|(stack, _)| stack),
            Some(expected_index),
            "new independent branches must respect the requested insertion order"
        );
    }
    Ok(())
}

#[test]
fn branch_create_above_checked_out_ref_checks_out_new_ref_in_ad_hoc_workspace() -> anyhow::Result<()>
{
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let new_ref = gix::refs::FullName::try_from("refs/heads/top")?;
    let anchor_ref = gix::refs::FullName::try_from("refs/heads/main")?;

    let result = but_api::branch::branch_create(
        &mut ctx,
        Some(new_ref.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(anchor_ref.clone()),
            side: InsertSide::Above,
        },
    )?;

    let repo = ctx.repo.get()?;
    let head_name = repo
        .head_name()?
        .expect("creating above checked-out branch checks out the new ref");
    assert_eq!(head_name, new_ref);
    assert_workspace_ref(&result.workspace, "refs/heads/top");

    let order = ctx
        .meta()?
        .branch_stack_order(anchor_ref.as_ref())?
        .expect("ad-hoc branch creation above a local ref persists branch order");
    assert_eq!(order, vec![new_ref, anchor_ref]);

    Ok(())
}

#[test]
fn branch_create_below_checked_out_ref_keeps_head_in_ad_hoc_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let new_ref = gix::refs::FullName::try_from("refs/heads/bottom")?;
    let anchor_ref = gix::refs::FullName::try_from("refs/heads/main")?;

    let result = but_api::branch::branch_create(
        &mut ctx,
        Some(new_ref.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(anchor_ref.clone()),
            side: InsertSide::Below,
        },
    )?;

    // Creating below the checked-out branch checks nothing out: HEAD stays on the anchor.
    let repo = ctx.repo.get()?;
    let head_name = repo
        .head_name()?
        .expect("HEAD remains symbolic after create-below");
    assert_eq!(head_name, anchor_ref);
    assert_workspace_ref(&result.workspace, "refs/heads/main");
    assert!(repo.try_find_reference(new_ref.as_ref())?.is_some());

    let order = ctx
        .meta()?
        .branch_stack_order(anchor_ref.as_ref())?
        .expect("ad-hoc branch creation below a local ref persists branch order");
    assert_eq!(order, vec![anchor_ref, new_ref]);

    Ok(())
}

/// A managed workspace with one stack of three segments, each owning two commits:
/// `C` (top) over `B` over `A`, with `A`'s bottom commit sitting directly on the target.
fn context_with_managed_two_commit_segments()
-> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    use but_testsupport::{CommandExt, git_at_dir, open_repo};

    let tmp = tempfile::tempdir()?;
    git_at_dir(tmp.path()).args(["init", "-b", "main"]).run();
    git_at_dir(tmp.path())
        .args(["config", "user.name", "GitButler"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "user.email", "gitbutler@example.com"])
        .run();
    crate::support::write_file(tmp.path(), "base.txt", "base\n")?;
    git_at_dir(tmp.path()).args(["add", "base.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "base"]).run();
    git_at_dir(tmp.path())
        .args(["config", "remote.origin.url", "../origin"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    for branch in ["A", "B", "C"] {
        git_at_dir(tmp.path())
            .args(["checkout", "-b", branch])
            .run();
        for n in ["1", "2"] {
            let file_name = format!("{branch}{n}.txt");
            crate::support::write_file(tmp.path(), &file_name, &format!("{branch}{n}\n"))?;
            git_at_dir(tmp.path()).args(["add", &file_name]).run();
            git_at_dir(tmp.path())
                .args(["commit", "-m", &format!("{branch}{n}")])
                .run();
        }
    }
    git_at_dir(tmp.path()).args(["checkout", "main"]).run();

    let repo = open_repo(tmp.path())?;
    crate::support::persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::branch::apply_only(&mut ctx, "refs/heads/C".try_into()?)?;
    Ok((ctx, tmp))
}

/// The segments of the single stack in the cached workspace projection, top to bottom,
/// as `(ref name, commit ids)`.
fn cached_segments(ctx: &but_ctx::Context) -> anyhow::Result<Vec<(String, Vec<gix::ObjectId>)>> {
    let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
    let [stack] = ws.stacks.as_slice() else {
        anyhow::bail!("expected exactly one stack, got {}", ws.stacks.len());
    };
    Ok(stack
        .segments
        .iter()
        .map(|segment| {
            (
                segment
                    .ref_name()
                    .map(|name| name.shorten().to_string())
                    .unwrap_or_default(),
                segment.commits.iter().map(|commit| commit.id).collect(),
            )
        })
        .collect())
}

/// Create `new_ref` below the bottom commit of `selected`, the request the desktop
/// "Create branch below" menu sends for a managed segment that owns commits.
fn create_below_bottom_commit(
    ctx: &mut but_ctx::Context,
    selected: &str,
    new_ref: &str,
) -> anyhow::Result<()> {
    let bottom = cached_segments(ctx)?
        .into_iter()
        .find(|(name, _)| name == selected)
        .and_then(|(_, commits)| commits.last().copied())
        .expect("selected segment owns commits");
    but_api::branch::branch_create(
        ctx,
        Some(new_ref.try_into()?),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Commit(bottom),
            side: InsertSide::Below,
        },
    )?;
    Ok(())
}

#[test]
fn branch_create_below_bottom_commit_of_bottom_segment_keeps_its_commits() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_managed_two_commit_segments()?;
    let before = cached_segments(&ctx)?;

    create_below_bottom_commit(&mut ctx, "A", "refs/heads/below-A")?;

    let after = cached_segments(&ctx)?;
    let mut expected = before;
    expected.push(("below-A".into(), vec![]));
    // The new segment is empty, sits directly below `A`, and every existing segment keeps
    // exactly its commits even though `A`'s bottom commit rests on the target.
    assert_eq!(
        after,
        expected,
        "{}",
        crate::support::workspace_graph(&ctx)?
    );
    Ok(())
}

#[test]
fn branch_create_below_bottom_commit_of_middle_segment_keeps_neighbours() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_managed_two_commit_segments()?;
    let before = cached_segments(&ctx)?;

    create_below_bottom_commit(&mut ctx, "B", "refs/heads/below-B")?;

    let after = cached_segments(&ctx)?;
    let mut expected = before;
    expected.insert(2, ("below-B".into(), vec![]));
    // The new segment is empty and sits between `B` and its lower neighbour `A`; both keep
    // exactly their commits.
    assert_eq!(
        after,
        expected,
        "{}",
        crate::support::workspace_graph(&ctx)?
    );
    Ok(())
}
