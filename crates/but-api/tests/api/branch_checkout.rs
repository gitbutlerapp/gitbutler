use snapbox::IntoData;

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
