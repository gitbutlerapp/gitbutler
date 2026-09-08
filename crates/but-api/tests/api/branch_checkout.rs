#[cfg(not(feature = "graph-workspace"))]
use snapbox::IntoData;

use but_testsupport::{CommandExt, git_at_dir};

use crate::support::{
    assert_workspace_ref, repo_with_feature_branch, set_project_target_to_feature,
};

#[test]
fn checkout_branch_switches_head_and_returns_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let branch = gix::refs::FullName::try_from("refs/heads/feature")?;
    let result = but_api::branch::branch_checkout(&mut ctx, branch)?;

    let repo = ctx.repo.get()?;
    let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
    assert_eq!(head_name, "refs/heads/feature");
    assert_workspace_ref(&result.workspace, "refs/heads/feature");

    Ok(())
}

#[test]
fn checkout_remote_ref_creates_local_tracking_branch() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    git_at_dir(tmp.path())
        .args([
            "update-ref",
            "refs/remotes/origin/topic",
            "refs/heads/feature",
        ])
        .run();
    let topic_commit_id = repo.rev_parse_single("refs/heads/feature")?.detach();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let branch = gix::refs::FullName::try_from("refs/remotes/origin/topic")?;
    let result = but_api::branch::branch_checkout(&mut ctx, branch)?;

    let repo = ctx.repo.get()?;
    let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
    assert_eq!(head_name, "refs/heads/topic");
    let mut created = repo.find_reference("refs/heads/topic")?;
    assert_eq!(created.peel_to_id()?, topic_commit_id);
    let config = repo.config_snapshot();
    let config_value = |key: &str| config.string(key).map(|value| value.to_string());
    assert_eq!(
        config_value("branch.topic.remote").as_deref(),
        Some("origin")
    );
    assert_eq!(
        config_value("branch.topic.merge").as_deref(),
        Some("refs/heads/topic")
    );
    assert_workspace_ref(&result.workspace, "refs/heads/topic");

    Ok(())
}

#[test]
fn checkout_remote_ref_switches_to_existing_local_tracking_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let main_commit_id = repo.rev_parse_single("refs/heads/main")?.detach();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::branch::branch_checkout(
        &mut ctx,
        gix::refs::FullName::try_from("refs/heads/feature")?,
    )?;

    let branch = gix::refs::FullName::try_from("refs/remotes/origin/main")?;
    let result = but_api::branch::branch_checkout(&mut ctx, branch)?;

    let repo = ctx.repo.get()?;
    let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
    assert_eq!(head_name, "refs/heads/main");
    // `main` is ahead of `origin/main` and must stay where it was.
    let mut main = repo.find_reference("refs/heads/main")?;
    assert_eq!(main.peel_to_id()?, main_commit_id);
    assert_workspace_ref(&result.workspace, "refs/heads/main");

    Ok(())
}

#[test]
fn checkout_rejects_symbolic_remote_head() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    git_at_dir(tmp.path())
        .args([
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ])
        .run();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let branch = gix::refs::FullName::try_from("refs/remotes/origin/HEAD")?;

    let err = but_api::branch::branch_checkout(&mut ctx, branch)
        .expect_err("a symbolic remote ref is not a branch to check out");
    assert_eq!(
        err.to_string(),
        "Refusing to check out symbolic ref 'origin/HEAD' due to potential ambiguity"
    );
    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference("refs/heads/HEAD")?.is_none(),
        "no local branch is made of the name the symbolic ref points through"
    );

    Ok(())
}

#[test]
fn checkout_remote_ref_completes_partial_tracking_config() -> anyhow::Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    git_at_dir(tmp.path())
        .args([
            "update-ref",
            "refs/remotes/origin/topic",
            "refs/heads/feature",
        ])
        .run();
    // The user's own keys, one of them tracking: both kept, the missing one filled in.
    git_at_dir(tmp.path())
        .args(["config", "branch.topic.description", "mine"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "branch.topic.merge", "refs/heads/other"])
        .run();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let branch = gix::refs::FullName::try_from("refs/remotes/origin/topic")?;
    but_api::branch::branch_checkout(&mut ctx, branch)?;

    let repo = ctx.repo.get()?;
    let config = repo.config_snapshot();
    let config_value = |key: &str| config.string(key).map(|value| value.to_string());
    assert_eq!(
        config_value("branch.topic.remote").as_deref(),
        Some("origin")
    );
    assert_eq!(
        config_value("branch.topic.merge").as_deref(),
        Some("refs/heads/other")
    );
    assert_eq!(
        config_value("branch.topic.description").as_deref(),
        Some("mine")
    );

    Ok(())
}

#[test]
fn checkout_rejects_refs_that_are_not_branches() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let branch = gix::refs::FullName::try_from("refs/tags/v1")?;

    let err = but_api::branch::branch_checkout(&mut ctx, branch)
        .expect_err("only branch refs can be checked out");
    assert_eq!(
        err.to_string(),
        "Can only check out local branches under refs/heads or remote-tracking branches under refs/remotes, got 'refs/tags/v1'"
    );

    Ok(())
}

#[test]
fn branch_checkout_new_creates_named_branch_at_target_and_checks_it_out() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let target_commit_id = set_project_target_to_feature(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let result = but_api::branch::branch_checkout_new(&mut ctx, Some("new branch".into()))?;

    let repo = ctx.repo.get()?;
    let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
    assert_eq!(head_name, "refs/heads/new-branch");
    let mut created = repo.find_reference("refs/heads/new-branch")?;
    assert_eq!(created.peel_to_id()?, target_commit_id);
    assert_workspace_ref(&result.workspace, "refs/heads/new-branch");

    Ok(())
}

#[test]
fn branch_checkout_new_rejects_existing_explicit_name() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let err = but_api::branch::branch_checkout_new(&mut ctx, Some("feature".into()))
        .expect_err("explicit names must not be uniquified");
    assert_eq!(
        err.to_string(),
        "Branch 'refs/heads/feature' already exists"
    );

    Ok(())
}

#[test]
fn checkout_returns_head_info_matching_fresh_head_info() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    crate::support::persist_default_target(&repo)?;

    snapbox::assert_data_eq!(
        crate::support::repository_graph(&repo)?,
        snapbox::str![["
* b720e1f (sibling) sibling
| * edd8381 (feature) feature
|/  
* 5374caf (HEAD -> main, origin/main) main

"]]
    );

    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let result = but_api::branch::branch_checkout(
        &mut ctx,
        gix::refs::FullName::try_from("refs/heads/feature")?,
    )?;

    {
        let repo = ctx.repo.get()?;
        let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
        assert_eq!(head_name, "refs/heads/feature");

        snapbox::assert_data_eq!(
            crate::support::repository_graph(&repo)?,
            snapbox::str![[r#"
* b720e1f (sibling) sibling
| * edd8381 (HEAD -> feature) feature
|/  
* 5374caf (origin/main, main, gitbutler/target) main

"#]]
        );
    }

    snapbox::assert_data_eq!(
        crate::support::workspace_graph(&ctx)?,
        snapbox::str![[r#"
⌂:feature[🌳] <> ✓refs/remotes/origin/main on 5374caf
└── ≡:feature[🌳] on 5374caf {1}
    └── :feature[🌳]
        └── ·edd8381

"#]]
    );

    #[cfg(feature = "graph-workspace")]
    {
        let returned = format!("{:#?}", result.workspace.graph_workspace);
        let fresh = format!("{:#?}", crate::support::fresh_graph_workspace(&ctx)?);
        assert_eq!(
            returned, fresh,
            "checkout API should return the same graph workspace a fresh post-checkout read sees"
        );
    }

    #[cfg(not(feature = "graph-workspace"))]
    {
        let returned_head_info = format!("{:#?}", result.workspace.head_info);
        let fresh_head_info = format!("{:#?}", crate::support::fresh_head_info(&ctx)?);
        let without_segment_indices = |value: &str| {
            value
                .lines()
                .filter(|line| !line.contains("NodeIndex("))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(
            without_segment_indices(&returned_head_info),
            without_segment_indices(&fresh_head_info),
            "checkout API should return the same head info a fresh post-checkout read sees"
        );

        snapbox::assert_data_eq!(
            without_segment_indices(&returned_head_info),
            snapbox::str![[r#"
RefInfo {
    symbolic_remote_names: {
        "origin",
    },
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: Some(
                Sha1(5374caf21933aee76b72bad8d6e30949c7a30e04),
            ),
            segments: [
                ref_info::ui::Segment {
                    ref_name: "►feature[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(edd8381, "feature\n", local),
                    ],
                    commits_on_remote: [],
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "5374caf",
                },
            ],
        },
    ],
    target_ref: Some(
        TargetRef {
            ref_name: FullName(
                "refs/remotes/origin/main",
            ),
            commits_ahead: 0,
        },
    ),
    target_commit: Some(
        TargetCommit {
            commit_id: Sha1(5374caf21933aee76b72bad8d6e30949c7a30e04),
        },
    ),
    is_target_current: true,
    lower_bound: Some(
    ),
    ancestor_workspace_commit: None,
}
"#]]
            .raw()
        );
    }

    Ok(())
}

#[test]
fn checkout_new_returns_head_info_matching_fresh_head_info() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    let target_commit_id = crate::support::persist_default_target(&repo)?;

    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let result = but_api::branch::branch_checkout_new(&mut ctx, Some("new branch".into()))?;

    {
        let repo = ctx.repo.get()?;
        let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
        assert_eq!(head_name, "refs/heads/new-branch");

        let mut created = repo.find_reference("refs/heads/new-branch")?;
        assert_eq!(
            created.peel_to_id()?,
            target_commit_id,
            "new branch should be created at the configured project target"
        );

        snapbox::assert_data_eq!(
            crate::support::repository_graph(&repo)?,
            snapbox::str![["
* b720e1f (sibling) sibling
| * edd8381 (feature) feature
|/  
* 5374caf (HEAD -> new-branch, origin/main, main) main

"]]
        );
    }

    snapbox::assert_data_eq!(
        crate::support::workspace_graph(&ctx)?,
        snapbox::str![[r#"
⌂:new-branch[🌳] <> ✓refs/remotes/origin/main on 5374caf
└── ≡:new-branch[🌳] on 5374caf {1}
    └── :new-branch[🌳]

"#]]
    );

    #[cfg(feature = "graph-workspace")]
    {
        let returned = format!("{:#?}", result.workspace.graph_workspace);
        let fresh = format!("{:#?}", crate::support::fresh_graph_workspace(&ctx)?);
        assert_eq!(
            returned, fresh,
            "checkout-new API should return the same graph workspace a fresh post-checkout read sees"
        );
    }

    #[cfg(not(feature = "graph-workspace"))]
    {
        let returned_head_info = format!("{:#?}", result.workspace.head_info);
        let fresh_head_info = format!("{:#?}", crate::support::fresh_head_info(&ctx)?);
        assert_eq!(
            returned_head_info, fresh_head_info,
            "checkout-new API should return the same head info a fresh post-checkout read sees"
        );

        snapbox::assert_data_eq!(
            returned_head_info,
            snapbox::str![[r#"
RefInfo {
    symbolic_remote_names: {
        "origin",
    },
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: Some(
                Sha1(5374caf21933aee76b72bad8d6e30949c7a30e04),
            ),
            segments: [
                ref_info::ui::Segment {
                    id: NodeIndex(0),
                    ref_name: "►new-branch[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [],
                    commits_on_remote: [],
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "5374caf",
                },
            ],
        },
    ],
    target_ref: Some(
        TargetRef {
            ref_name: FullName(
                "refs/remotes/origin/main",
            ),
            segment_index: NodeIndex(1),
            commits_ahead: 0,
        },
    ),
    target_commit: Some(
        TargetCommit {
            commit_id: Sha1(5374caf21933aee76b72bad8d6e30949c7a30e04),
            segment_index: NodeIndex(0),
        },
    ),
    is_target_current: true,
    lower_bound: Some(
        NodeIndex(0),
    ),
    ancestor_workspace_commit: None,
}
"#]]
            .raw()
        );
    }

    Ok(())
}
