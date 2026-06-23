use but_core::RefMetadata;
use snapbox::IntoData;

#[test]
fn legacy_create_reference_below_checked_out_empty_branch_is_visible_in_head_info()
-> anyhow::Result<()> {
    let (repo, _tmp) = single_branch_repo()?;
    persist_origin_main_target(&repo)?;
    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();

    but_api::branch::branch_create(
        &mut ctx,
        Some(gix::refs::FullName::try_from("refs/heads/empty-top")?),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(
                gix::refs::FullName::try_from("refs/heads/main")?,
            ),
            side: but_rebase::graph_rebase::mutate::InsertSide::Above,
        },
    )?;

    let repo = ctx.repo.get()?;
    let head_name = repo
        .head_name()?
        .expect("creating above the checked-out branch checks out the new ref");
    assert_eq!(head_name.as_bstr(), "refs/heads/empty-top");
    drop(repo);

    but_api::legacy::stack::create_reference(
        &mut ctx,
        but_api::legacy::stack::create_reference::Request {
            new_name: "bm-branch-1".into(),
            anchor: Some(
                but_api::legacy::stack::create_reference::Anchor::AtReference {
                    short_name: "empty-top".into(),
                    position: but_workspace::branch::create_reference::Position::Below,
                },
            ),
        },
    )?;

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference("refs/heads/bm-branch-1")?.is_some(),
        "legacy API should create the inserted branch ref"
    );
    drop(repo);

    let main_ref = gix::refs::FullName::try_from("refs/heads/main")?;
    let branch_order = ctx
        .meta()?
        .branch_stack_order(main_ref.as_ref())?
        .expect("legacy API should persist ad-hoc branch order");
    assert_eq!(
        branch_order,
        vec![
            gix::refs::FullName::try_from("refs/heads/empty-top")?,
            gix::refs::FullName::try_from("refs/heads/bm-branch-1")?,
            gix::refs::FullName::try_from("refs/heads/main")?,
        ]
    );

    let repo = ctx.repo.get()?;
    let meta = ctx.meta()?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        ctx.project_meta()?,
        but_graph::init::Options::limited(),
    )?;
    let graph_tree = but_testsupport::graph_tree(&graph).to_string();
    assert!(
        graph_tree.contains("bm-branch-1"),
        "graph should include inserted branch before workspace projection:\n{graph_tree}"
    );
    drop(meta);
    drop(repo);

    let head_info = but_api::legacy::workspace::head_info(&ctx)?;
    let rendered = format!("{head_info:#?}");
    let empty_top_idx = rendered
        .find("empty-top")
        .expect("head info should include the checked-out empty top branch");
    let inserted_idx = rendered.find("bm-branch-1").unwrap_or_else(|| {
        panic!("head info should include the inserted empty branch:\n{rendered}")
    });
    let main_idx = rendered
        .find("main")
        .expect("head info should include the bottom branch");
    assert!(
        empty_top_idx < inserted_idx && inserted_idx < main_idx,
        "head_info should preserve ad-hoc branch order, got:\n{rendered}"
    );

    but_api::legacy::stack::remove_branch(
        &mut ctx,
        but_core::ref_metadata::StackId::single_branch_id(),
        "empty-top".into(),
    )?;
    let repo = ctx.repo.get()?;
    let head_name = repo
        .head_name()?
        .expect("removing checked-out empty top should switch to the branch below");
    assert_eq!(head_name.as_bstr(), "refs/heads/bm-branch-1");
    assert!(repo.try_find_reference("refs/heads/empty-top")?.is_none());
    drop(repo);

    let main_ref = gix::refs::FullName::try_from("refs/heads/main")?;
    assert_eq!(
        ctx.meta()?.branch_stack_order(main_ref.as_ref())?,
        Some(vec![
            gix::refs::FullName::try_from("refs/heads/bm-branch-1")?,
            main_ref,
        ])
    );

    Ok(())
}

fn persist_origin_main_target(repo: &gix::Repository) -> anyhow::Result<gix::ObjectId> {
    let target_commit_id = repo.rev_parse_single("refs/remotes/origin/main")?.detach();
    but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some("origin".into()),
    }
    .persist_to_local_config(repo)?;
    Ok(target_commit_id)
}

fn single_branch_repo() -> anyhow::Result<(gix::Repository, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    git(tmp.path(), ["init", "-b", "main"])?;
    git(tmp.path(), ["config", "user.name", "GitButler"])?;
    git(
        tmp.path(),
        ["config", "user.email", "gitbutler@example.com"],
    )?;
    std::fs::write(tmp.path().join("base.txt"), "base\n")?;
    git(tmp.path(), ["add", "base.txt"])?;
    git(tmp.path(), ["commit", "-m", "base"])?;
    git(
        tmp.path(),
        ["update-ref", "refs/remotes/origin/main", "HEAD"],
    )?;

    for name in ["first", "second", "third"] {
        std::fs::write(tmp.path().join(format!("{name}.txt")), format!("{name}\n"))?;
        git(tmp.path(), ["add", "."])?;
        git(tmp.path(), ["commit", "-m", name])?;
    }

    Ok((gix::open(tmp.path())?, tmp))
}

fn git<const N: usize>(cwd: &std::path::Path, args: [&str; N]) -> anyhow::Result<()> {
    let status = std::process::Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(cwd)
        .status()?;
    anyhow::ensure!(status.success(), "git command failed");
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

    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
    let result = but_api::branch::branch_checkout(
        &mut ctx,
        gix::refs::FullName::try_from("refs/heads/feature")?,
    )?;

    {
        let repo = ctx.repo.get()?;
        let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
        assert_eq!(head_name.as_bstr(), "refs/heads/feature");

        snapbox::assert_data_eq!(
            crate::support::repository_graph(&repo)?,
            snapbox::str![["
* b720e1f (sibling) sibling
| * edd8381 (HEAD -> feature) feature
|/  
* 5374caf (origin/main, main) main

"]]
        );
    }

    snapbox::assert_data_eq!(
        crate::support::workspace_graph(&ctx)?,
        snapbox::str![[r"
⌂:0:feature[🌳] <> ✓refs/remotes/origin/main on 5374caf
└── ≡:0:feature[🌳] on 5374caf {1}
    └── :0:feature[🌳]
        └── ·edd8381

"]]
    );

    let returned_head_info = format!("{:#?}", result.workspace.head_info);
    let fresh_head_info = format!("{:#?}", crate::support::fresh_head_info(&ctx)?);
    assert_eq!(
        returned_head_info, fresh_head_info,
        "checkout API should return the same head info a fresh post-checkout read sees"
    );

    snapbox::assert_data_eq!(
        returned_head_info,
        snapbox::str![[r#"
RefInfo {
    workspace_ref_info: Some(
        RefInfo {
            ref_name: FullName(
                "refs/heads/feature",
            ),
            commit_id: Some(
                Sha1(edd838127f5665b9675a440e81f51bc5f170140f),
            ),
            worktree: Some(
                Worktree {
                    kind: Main,
                    owned_by_repo: true,
                },
            ),
        },
    ),
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
                    ref_name: "►feature[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [
                        LocalCommit(edd8381, "feature\n", local),
                    ],
                    commits_on_remote: [],
                    commits_outside: None,
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
            segment_index: NodeIndex(2),
            commits_ahead: 0,
        },
    ),
    target_commit: Some(
        TargetCommit {
            commit_id: Sha1(5374caf21933aee76b72bad8d6e30949c7a30e04),
            segment_index: NodeIndex(1),
        },
    ),
    lower_bound: Some(
        NodeIndex(1),
    ),
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}
"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn checkout_new_returns_head_info_matching_fresh_head_info() -> anyhow::Result<()> {
    let (repo, _tmp) = crate::support::writable_scenario("checkout-head-info");
    let target_commit_id = crate::support::persist_default_target(&repo)?;

    let mut ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
    let result = but_api::branch::branch_checkout_new(&mut ctx, Some("new branch".into()))?;

    {
        let repo = ctx.repo.get()?;
        let head_name = repo.head_name()?.expect("HEAD is symbolic after checkout");
        assert_eq!(head_name.as_bstr(), "refs/heads/new-branch");

        let mut created = repo.find_reference("refs/heads/new-branch")?;
        assert_eq!(
            created.peel_to_id()?.detach(),
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
        snapbox::str![[r"
⌂:0:new-branch[🌳] <> ✓refs/remotes/origin/main on 5374caf
└── ≡:0:new-branch[🌳] {1}
    └── :0:new-branch[🌳]

"]]
    );

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
    workspace_ref_info: Some(
        RefInfo {
            ref_name: FullName(
                "refs/heads/new-branch",
            ),
            commit_id: Some(
                Sha1(5374caf21933aee76b72bad8d6e30949c7a30e04),
            ),
            worktree: Some(
                Worktree {
                    kind: Main,
                    owned_by_repo: true,
                },
            ),
        },
    ),
    symbolic_remote_names: {
        "origin",
    },
    stacks: [
        Stack {
            id: Some(
                00000000-0000-0000-0000-000000000001,
            ),
            base: None,
            segments: [
                ref_info::ui::Segment {
                    id: NodeIndex(0),
                    ref_name: "►new-branch[🌳]",
                    remote_tracking_ref_name: "None",
                    commits: [],
                    commits_on_remote: [],
                    commits_outside: None,
                    metadata: "None",
                    push_status: CompletelyUnpushed,
                    base: "None",
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
    lower_bound: Some(
        NodeIndex(0),
    ),
    is_managed_ref: false,
    is_managed_commit: false,
    ancestor_workspace_commit: None,
    is_entrypoint: true,
}
"#]]
        .raw()
    );

    Ok(())
}
