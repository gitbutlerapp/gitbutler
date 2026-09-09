use but_api::json::HexHash;
use but_testsupport::{CommandExt, git_at_dir, open_repo};

use crate::support::{repo_with_feature_branch, write_file};

fn incoming_target() -> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    let (repo, tmp) = repo_with_feature_branch()?;
    write_file(tmp.path(), "file.txt", "three\n")?;
    git_at_dir(tmp.path())
        .args(["commit", "-am", "three"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    git_at_dir(tmp.path()).args(["switch", "feature"]).run();
    git_at_dir(tmp.path())
        .args(["switch", "-c", "gitbutler/workspace"])
        .run();
    git_at_dir(tmp.path())
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "GitButler Workspace Commit",
        ])
        .run();
    drop(repo);

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    Ok((ctx, tmp))
}

#[test]
fn reports_clipping_and_accepts_a_zero_limit() -> anyhow::Result<()> {
    let (ctx, _tmp) = incoming_target()?;

    let zero = but_api::target_commits::workspace_target_commits(&ctx, None, Some(0))?;
    assert!(
        zero.commits.is_empty(),
        "a zero-sized page contains no commits"
    );
    assert!(
        zero.has_more,
        "the empty page is clipped before the workspace bound"
    );

    let first = but_api::target_commits::workspace_target_commits(&ctx, None, Some(1))?;
    assert_eq!(
        first.commits.len(),
        1,
        "the requested page size is respected"
    );
    assert!(
        first.has_more,
        "another relative commit remains after the first page"
    );

    let complete = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    assert_eq!(
        complete.commits.len(),
        3,
        "the complete page includes two incoming commits and the lower bound"
    );
    assert!(
        !complete.has_more,
        "the complete page reached its workspace bound"
    );
    Ok(())
}

#[test]
fn integration_returns_the_updated_target_page_only_after_materializing() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = incoming_target()?;
    let before = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    assert!(
        before.commits.iter().any(|entry| !entry.in_workspace),
        "the fixture has incoming target commits"
    );

    let preview = but_api::workspace::workspace_integrate_upstream_only(
        &mut ctx,
        vec![],
        but_core::DryRun::Yes,
    )?;
    let preview = serde_json::to_value(
        but_api::workspace::json::WorkspaceIntegrateUpstreamOutcome::try_from(preview)?,
    )?;
    assert_eq!(
        preview.get("targetCommits"),
        Some(&serde_json::Value::Null),
        "a dry run must not supply a page for the live target cache"
    );
    assert_eq!(
        serde_json::to_value(but_api::target_commits::workspace_target_commits(
            &ctx, None, None
        )?)?,
        serde_json::to_value(&before)?,
        "previewing leaves the persisted target listing unchanged"
    );

    let outcome = but_api::workspace::workspace_integrate_upstream_only(
        &mut ctx,
        vec![],
        but_core::DryRun::No,
    )?;
    let outcome = serde_json::to_value(
        but_api::workspace::json::WorkspaceIntegrateUpstreamOutcome::try_from(outcome)?,
    )?;
    let after = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    assert!(
        !after.commits.is_empty() && after.commits.iter().all(|entry| entry.in_workspace),
        "integration marks the target commits as part of the workspace"
    );
    assert_eq!(
        outcome.get("targetCommits"),
        Some(&serde_json::to_value(after)?),
        "the mutation returns the same page as a fresh target query"
    );
    Ok(())
}

#[test]
fn target_page_failure_preserves_integration_and_its_undo_entry() -> anyhow::Result<()> {
    use gix::prelude::Write;

    let (mut ctx, _tmp) = incoming_target()?;
    let target_id = {
        let repo = ctx.repo.get()?;
        let target = repo
            .find_reference("refs/remotes/origin/main")?
            .peel_to_commit()?;
        // Keep the signature structurally valid so graph loading and integration succeed.
        // Overflow fails only when the target page decodes the author's time; non-numeric text fails earlier.
        let data = format!(
            "tree {}\nparent {}\nauthor Broken <broken@example.com> 999999999999999999999 +0000\ncommitter GitButler <gitbutler@example.com> 1000000000 +0000\n\nbroken author time\n",
            target.tree_id()?,
            target.id
        );
        let id = repo
            .write_buf(gix::objs::Kind::Commit, data.as_bytes())
            .map_err(anyhow::Error::from_boxed)?;
        repo.reference(
            "refs/remotes/origin/main",
            id,
            gix::refs::transaction::PreviousValue::Any,
            "test malformed target author",
        )?;
        id
    };
    ctx.invalidate_workspace_cache()?;
    assert!(
        but_api::target_commits::workspace_target_commits(&ctx, None, None).is_err(),
        "the target page cannot decode the malformed author time"
    );

    let outcome =
        but_api::workspace::workspace_integrate_upstream(&mut ctx, vec![], but_core::DryRun::No)?;
    assert!(
        outcome.target_commits.is_none(),
        "an unreadable cache payload is omitted"
    );
    assert_eq!(
        ctx.project_meta()?.target_commit_id,
        Some(target_id),
        "integration still advances the workspace target"
    );
    let snapshots = but_api::legacy::oplog::list_snapshots(
        &ctx,
        10,
        None,
        None,
        Some(vec![but_oplog::legacy::OperationKind::MergeUpstream]),
    )?;
    assert_eq!(
        snapshots.len(),
        1,
        "the completed integration retains its undo entry"
    );
    Ok(())
}

/// Without a forge cache entry, a GitHub merge-button commit still names the
/// pull request it landed, read from its own message.
#[test]
fn github_merge_commit_names_its_pull_request_from_the_message() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    git_at_dir(tmp.path())
        .args([
            "config",
            "remote.origin.url",
            "git@github.com:owner/repo.git",
        ])
        .run();
    write_file(tmp.path(), "file.txt", "three\n")?;
    git_at_dir(tmp.path())
        .args([
            "commit",
            "-am",
            "Merge pull request #42 from owner/topic\n\nAdd the third line\n",
        ])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    git_at_dir(tmp.path()).args(["switch", "feature"]).run();
    git_at_dir(tmp.path())
        .args(["switch", "-c", "gitbutler/workspace"])
        .run();
    git_at_dir(tmp.path())
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "GitButler Workspace Commit",
        ])
        .run();
    drop(repo);

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;

    let page = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    let reviews: Vec<_> = page
        .commits
        .iter()
        .map(|commit| {
            commit.review.as_ref().map(|review| {
                (
                    review.number,
                    review.title.as_str(),
                    review.html_url.as_str(),
                    review.unit_symbol.as_str(),
                    review.source_branch.as_str(),
                )
            })
        })
        .collect();
    assert_eq!(
        reviews,
        [
            Some((
                42,
                "Add the third line",
                "https://github.com/owner/repo/pull/42",
                "#",
                "topic"
            )),
            None,
            None,
        ],
        "only the merge commit names a review; plain commits stay unannotated"
    );
    Ok(())
}

/// A still-applied stack landed upstream through a merge commit: the workspace
/// lower bound becomes the stack tip, which sits on the merge's *second*
/// parent and is never met by the first-parent walk. The stack's base bounds
/// the walk instead of running to the commit cap (or repository root).
#[test]
fn merge_integrated_stack_bounds_the_walk_at_its_base() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    git_at_dir(tmp.path()).args(["init"]).run();
    git_at_dir(tmp.path())
        .args(["config", "user.name", "GitButler"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "user.email", "gitbutler@example.com"])
        .run();
    write_file(tmp.path(), "file.txt", "zero\n")?;
    git_at_dir(tmp.path()).args(["add", "file.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "zero"]).run();
    write_file(tmp.path(), "file.txt", "one\n")?;
    git_at_dir(tmp.path()).args(["commit", "-am", "one"]).run();
    git_at_dir(tmp.path()).args(["branch", "feature"]).run();
    write_file(tmp.path(), "file.txt", "two\n")?;
    git_at_dir(tmp.path()).args(["commit", "-am", "two"]).run();
    git_at_dir(tmp.path()).args(["switch", "feature"]).run();
    write_file(tmp.path(), "feature.txt", "work\n")?;
    git_at_dir(tmp.path()).args(["add", "feature.txt"]).run();
    git_at_dir(tmp.path())
        .args(["commit", "-m", "feature-work"])
        .run();
    git_at_dir(tmp.path()).args(["switch", "main"]).run();
    git_at_dir(tmp.path())
        .args(["merge", "--no-ff", "feature", "-m", "merge feature"])
        .run();
    git_at_dir(tmp.path())
        .args(["remote", "add", "origin", "../origin"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    git_at_dir(tmp.path()).args(["switch", "feature"]).run();
    git_at_dir(tmp.path())
        .args(["switch", "-c", "gitbutler/workspace"])
        .run();
    git_at_dir(tmp.path())
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "GitButler Workspace Commit",
        ])
        .run();

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    let feature_ref = gix::refs::FullName::try_from("refs/heads/feature")?;
    but_api::branch::apply_only(&mut ctx, feature_ref.as_ref())?;

    let complete = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    let titles: Vec<_> = complete
        .commits
        .iter()
        .map(|entry| {
            let message = entry.commit.message.to_string();
            message.lines().next().unwrap_or_default().to_owned()
        })
        .collect();
    assert_eq!(
        titles,
        ["merge feature", "two", "one"],
        "the walk stops at the stack's fork point instead of running past it to the root"
    );
    assert!(
        !complete.has_more,
        "stopping at the fork point is a natural bound, not a clip"
    );
    Ok(())
}

#[test]
fn continuation_excludes_its_cursor_and_reports_more_history() -> anyhow::Result<()> {
    let (ctx, _tmp) = incoming_target()?;
    let complete = but_api::target_commits::workspace_target_commits(&ctx, None, None)?;
    let cursor = complete
        .commits
        .first()
        .expect("the relative page has a tip");

    let page = but_api::target_commits::workspace_target_commits(
        &ctx,
        Some(HexHash(cursor.commit.id)),
        Some(1),
    )?;
    assert_eq!(page.commits.len(), 1, "one older commit is returned");
    assert_ne!(
        page.commits[0].commit.id, cursor.commit.id,
        "continuation starts below its cursor"
    );
    assert!(
        page.has_more,
        "the repository has more history below this page"
    );
    Ok(())
}

/// Paging below the tip of a shallow clone stops at the missing parent instead of failing.
#[test]
fn continuation_stops_gracefully_in_a_shallow_clone() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let remote_dir = tmp.path().join("remote");
    std::fs::create_dir(&remote_dir)?;
    git_at_dir(&remote_dir)
        .args(["init", "-b", "main", "--object-format=sha1"])
        .run();
    write_file(&remote_dir, "file", "initial\n")?;
    git_at_dir(&remote_dir).args(["add", "file"]).run();
    git_at_dir(&remote_dir)
        .args(["commit", "-m", "initial"])
        .run();
    write_file(&remote_dir, "file", "second\n")?;
    git_at_dir(&remote_dir)
        .args(["commit", "-am", "second"])
        .run();

    // `--depth` is ignored for local path clones, so use the file protocol.
    let clone_dir = tmp.path().join("clone");
    let remote_url = format!("file://{}", remote_dir.display());
    git_at_dir(tmp.path())
        .args(["clone", "--depth", "1", &remote_url])
        .arg(&clone_dir)
        .run();

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(&clone_dir)?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    let head_id = ctx.repo.get()?.head_id()?.detach();

    let page =
        but_api::target_commits::workspace_target_commits(&ctx, Some(HexHash(head_id)), Some(100))?;
    assert!(
        page.commits.is_empty(),
        "the tip's parent is missing, so nothing lies below it"
    );
    assert!(!page.has_more, "a missing parent ends the history");
    Ok(())
}

/// With an ordinary branch checked out, the listing walks the persisted GitButler target,
/// not the checked-out branch's own upstream.
#[test]
fn ordinary_checkout_lists_the_persisted_target() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    drop(repo);
    // `feature` tracks a remote branch of its own, and the target moves on past it.
    for args in [
        vec!["switch", "feature"],
        vec!["update-ref", "refs/remotes/origin/feature", "feature"],
        vec!["config", "branch.feature.remote", "origin"],
        vec!["config", "branch.feature.merge", "refs/heads/feature"],
        vec!["update-ref", "refs/remotes/origin/main", "main"],
    ] {
        git_at_dir(tmp.path()).args(args).run();
    }

    let mut ctx =
        but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?.with_memory_app_cache();
    let target_ref = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    but_api::workspace::set_target_ref_and_init_project(&mut ctx, target_ref.as_ref(), None)?;
    let (main_tip, feature_tip) = {
        let repo = ctx.repo.get()?;
        (
            repo.rev_parse_single("refs/remotes/origin/main")?.detach(),
            repo.rev_parse_single("refs/remotes/origin/feature")?
                .detach(),
        )
    };
    assert_ne!(
        main_tip, feature_tip,
        "the branch upstream must differ from the target to tell them apart"
    );

    let page = but_api::target_commits::workspace_target_commits(&ctx, None, Some(1))?;
    assert_eq!(
        page.commits.first().map(|entry| entry.commit.id),
        Some(main_tip),
        "the target history starts at the persisted target, not the branch upstream"
    );
    Ok(())
}
