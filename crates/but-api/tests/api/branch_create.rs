use but_core::RefMetadata;
use but_rebase::graph_rebase::mutate::InsertSide;

use but_testsupport::{CommandExt as _, git_at_dir};

use crate::support::{assert_workspace_ref, repo_with_feature_branch, undo};

#[test]
fn independent_branch_creation_from_normal_branches_uses_workspace_setup_and_undo()
-> anyhow::Result<()> {
    for (starting_branch, expected_head) in [
        ("main", "refs/heads/new-branch"),
        ("feature", "refs/heads/gitbutler/workspace"),
    ] {
        let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
        let target = crate::support::persist_default_target(&repo)?;
        git_at_dir(repo.workdir().expect("fixture has a worktree"))
            .args(["checkout", starting_branch])
            .run();
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
        let before = crate::support::workspace_graph(&ctx)?;
        let new_ref = gix::refs::FullName::try_from("refs/heads/new-branch")?;

        let result = but_api::branch::branch_create(
            &mut ctx,
            Some(new_ref.clone()),
            but_api::branch::json::BranchCreatePlacement::Independent { order: Some(0) },
        )?;

        {
            let repo = ctx.repo.get()?;
            assert_eq!(
                repo.head_name()?,
                Some(expected_head.try_into()?),
                "creation checks out the new branch on the target and a workspace beside a feature"
            );
            assert_eq!(
                repo.rev_parse_single(new_ref.as_bstr())?.detach(),
                target,
                "the new independent branch starts at the configured target"
            );
        }
        assert_workspace_ref(&result.workspace, "refs/heads/new-branch");
        if starting_branch == "main" {
            assert_eq!(
                cached_segments(&ctx)?,
                vec![("new-branch".into(), vec![])],
                "the target at the cutoff must not become an empty stack member"
            );
        }

        undo(&mut ctx)?;
        assert_eq!(
            crate::support::workspace_graph(&ctx)?,
            before,
            "one undo restores the branch and workspace topology before creation"
        );
        assert!(
            ctx.repo
                .get()?
                .try_find_reference(new_ref.as_ref())?
                .is_none(),
            "undo also deletes the newly created reference"
        );
    }
    Ok(())
}

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
fn independent_branch_beside_a_checked_out_one_enters_the_workspace() -> anyhow::Result<()> {
    for (order, expected) in [
        (
            None,
            snapbox::str![[r#"
HEAD: gitbutler/workspace
workspace commit: GitButler Workspace Commit
parent: feature [feature]
parent: main [main, new-branch, origin/main]
metadata stack: feature (in workspace)
metadata stack: new-branch (in workspace)
target: main at main

"#]],
        ),
        (
            Some(0),
            snapbox::str![[r#"
HEAD: gitbutler/workspace
workspace commit: GitButler Workspace Commit
parent: main [main, new-branch, origin/main]
parent: feature [feature]
metadata stack: new-branch (in workspace)
metadata stack: feature (in workspace)
target: main at main

"#]],
        ),
    ] {
        let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
        crate::support::persist_default_target(&repo)?;
        git_at_dir(repo.workdir().expect("fixture has a worktree"))
            .args(["checkout", "feature"])
            .run();
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

        create_independent_branch(&mut ctx, "new-branch", order)?;

        snapbox::assert_data_eq!(entered_workspace(&ctx)?, expected);
        assert_eq!(
            ctx.project_meta()?.push_remote.as_deref(),
            Some("origin"),
            "entering the workspace keeps the remote branches are pushed to"
        );
    }
    Ok(())
}

#[test]
fn independent_branch_beside_one_based_past_the_target_starts_at_the_target() -> anyhow::Result<()>
{
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    let workdir = repo.workdir().expect("fixture has a worktree").to_owned();
    git_at_dir(&workdir).args(["checkout", "main"]).run();
    commit_file(&workdir, "main-ahead")?;
    git_at_dir(&workdir)
        .args(["checkout", "-b", "based-ahead"])
        .run();
    commit_file(&workdir, "based-ahead")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    create_independent_branch(&mut ctx, "new-branch", None)?;

    // The local target moved past the stored target commit, and the branch checked out is
    // based on it. The new branch starts at the stored target commit all the same.
    snapbox::assert_data_eq!(
        entered_workspace(&ctx)?,
        snapbox::str![[r#"
HEAD: gitbutler/workspace
workspace commit: GitButler Workspace Commit
parent: based-ahead [based-ahead]
parent: main [new-branch, origin/main]
metadata stack: based-ahead, main (in workspace)
metadata stack: new-branch (in workspace)
target: main at main

"#]]
    );
    Ok(())
}

#[test]
fn independent_branch_beside_a_checked_out_stack_keeps_its_empty_branches() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    git_at_dir(repo.workdir().expect("fixture has a worktree"))
        .args(["checkout", "feature"])
        .run();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    for (name, side) in [
        ("empty-below", InsertSide::Below),
        ("empty-above", InsertSide::Above),
    ] {
        but_api::branch::branch_create(
            &mut ctx,
            Some(format!("refs/heads/{name}").try_into()?),
            but_api::branch::json::BranchCreatePlacement::Dependent {
                relative_to: but_api::commit::json::RelativeTo::Reference(feature.clone()),
                side,
            },
        )?;
    }

    create_independent_branch(&mut ctx, "new-branch", None)?;

    snapbox::assert_data_eq!(
        entered_workspace(&ctx)?,
        snapbox::str![[r#"
HEAD: gitbutler/workspace
workspace commit: GitButler Workspace Commit
parent: feature [empty-above, empty-below, feature]
parent: main [main, new-branch, origin/main]
metadata stack: empty-above, feature, empty-below (in workspace)
metadata stack: new-branch (in workspace)
target: main at main

"#]]
    );
    Ok(())
}

fn create_independent_branch(
    ctx: &mut but_ctx::Context,
    name: &str,
    order: Option<usize>,
) -> anyhow::Result<()> {
    but_api::branch::branch_create(
        ctx,
        Some(format!("refs/heads/{name}").try_into()?),
        but_api::branch::json::BranchCreatePlacement::Independent { order },
    )?;
    Ok(())
}

fn commit_file(workdir: &std::path::Path, name: &str) -> anyhow::Result<()> {
    crate::support::write_file(workdir, name, name)?;
    git_at_dir(workdir).args(["add", name]).run();
    git_at_dir(workdir).args(["commit", "-m", name]).run();
    Ok(())
}

fn entered_workspace(ctx: &but_ctx::Context) -> anyhow::Result<String> {
    use gix::bstr::ByteSlice as _;
    use std::fmt::Write as _;

    let repo = ctx.repo.get()?;
    let mut out = String::new();
    let head = repo.head_name()?.expect("HEAD is on a branch");
    writeln!(out, "HEAD: {}", head.shorten())?;
    let workspace = repo
        .find_reference(but_core::WORKSPACE_REF_NAME)?
        .peel_to_commit()?;
    writeln!(
        out,
        "workspace commit: {}",
        workspace.message()?.title.trim().as_bstr()
    )?;
    for parent in workspace.parent_ids() {
        let mut branches = Vec::new();
        for reference in repo.references()?.all()? {
            let mut reference = reference.map_err(|err| anyhow::anyhow!("{err}"))?;
            let name = reference.name().to_owned();
            let is_branch = matches!(
                name.category(),
                Some(gix::refs::Category::LocalBranch | gix::refs::Category::RemoteBranch)
            );
            if is_branch
                && !name.as_bstr().starts_with(b"refs/heads/gitbutler/")
                && reference.peel_to_id()? == parent
            {
                branches.push(name.shorten().to_string());
            }
        }
        branches.sort();
        branches.dedup();
        writeln!(
            out,
            "parent: {} [{}]",
            parent
                .object()?
                .into_commit()
                .message()?
                .title
                .trim()
                .as_bstr(),
            branches.join(", ")
        )?;
    }
    let metadata = ctx
        .meta()?
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?;
    for stack in &metadata.stacks {
        let branches: Vec<_> = (stack.branches.iter())
            .map(|branch| branch.ref_name.shorten().to_string())
            .collect();
        let relation = if stack.is_in_workspace() {
            "in workspace"
        } else {
            "outside the workspace"
        };
        writeln!(out, "metadata stack: {} ({relation})", branches.join(", "))?;
    }
    let project_meta = ctx.project_meta()?;
    let target_commit = repo.find_commit(project_meta.target_commit_id_or_err()?)?;
    writeln!(
        out,
        "target: {} at {}",
        project_meta
            .target_ref_or_err()?
            .as_local_full_name_ref()
            .shorten(),
        target_commit.message()?.title.trim().as_bstr()
    )?;
    Ok(out)
}

#[test]
fn undoing_branch_create_deletes_the_new_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let feature = gix::refs::FullName::try_from("refs/heads/feature")?;
    but_api::branch::apply_only(&mut ctx, feature.as_ref())?;

    let new_ref = gix::refs::FullName::try_from("refs/heads/on-top")?;
    but_api::branch::branch_create(
        &mut ctx,
        Some(new_ref.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(feature),
            side: InsertSide::Above,
        },
    )?;
    undo(&mut ctx)?;

    assert!(
        ctx.repo
            .get()?
            .try_find_reference(new_ref.as_ref())?
            .is_none(),
        "a branch left behind would join a stack on the next change and take its commits"
    );
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
    assert_eq!(
        order,
        vec![
            new_ref,
            anchor_ref,
            gix::refs::FullName::try_from("refs/heads/feature")?
        ]
    );

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
    assert_eq!(
        order,
        vec![
            anchor_ref,
            new_ref,
            gix::refs::FullName::try_from("refs/heads/feature")?
        ]
    );

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
