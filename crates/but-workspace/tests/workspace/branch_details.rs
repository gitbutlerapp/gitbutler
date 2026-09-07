mod without_workspace {
    use but_core::ref_metadata::ProjectMeta;
    use but_error::{AnyhowContextExt, Code};

    use crate::utils::read_only_in_memory_scenario_named;

    #[test]
    fn uses_the_project_target_as_the_traversal_boundary() -> anyhow::Result<()> {
        let repo =
            read_only_in_memory_scenario_named("with-remotes-no-workspace", "nothing-to-push")?;
        let project_meta = ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            ..Default::default()
        };

        let details = but_workspace::branch_details(
            &repo,
            "refs/heads/A".try_into()?,
            &Default::default(),
            &project_meta,
        )?;
        let target_id = repo.rev_parse_single("refs/remotes/origin/main")?.detach();

        assert_eq!(
            details.base_commit, target_id,
            "the configured project target bounds branch traversal"
        );
        assert_eq!(
            details.commits.len(),
            2,
            "only commits above the configured project target are returned"
        );
        Ok(())
    }

    #[test]
    fn classifies_a_missing_branch() -> anyhow::Result<()> {
        let repo =
            read_only_in_memory_scenario_named("with-remotes-no-workspace", "nothing-to-push")?;
        let project_meta = ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            ..Default::default()
        };

        let error = but_workspace::branch_details(
            &repo,
            "refs/heads/missing".try_into()?,
            &Default::default(),
            &project_meta,
        )
        .unwrap_err();

        assert_eq!(
            error.custom_context().map(|context| context.code),
            Some(Code::BranchNotFound),
            "missing branch refs should be recoverable by API consumers"
        );
        Ok(())
    }
}

/// All tests have a workspace present.
mod with_workspace {
    use snapbox::prelude::*;

    use but_core::ref_metadata::{Branch, ProjectMeta, RefInfo, Review};
    use but_testsupport::{visualize_commit_graph, visualize_commit_graph_all};

    use crate::utils::{read_only_in_memory_scenario, read_only_in_memory_scenario_named};

    fn refname(short_name: &str) -> gix::refs::FullName {
        if short_name.contains("/") {
            format!("refs/remotes/{short_name}").try_into().unwrap()
        } else {
            format!("refs/heads/{short_name}").try_into().unwrap()
        }
    }

    #[test]
    fn merge_with_two_branches() -> anyhow::Result<()> {
        let repo = read_only_in_memory_scenario("merge-with-two-branches-line-offset")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph(&repo, "HEAD")?,
            snapbox::str![[r#"
*   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
|\  
| * 7f389ed (A) add 10 to the beginning
* | 91ef6f6 (B) add 10 to the end
|/  
* ff045ef (main) init

"#]]
            .raw()
        );
        let project_meta = ProjectMeta {
            target_ref: Some(refname("B")),
            ..Default::default()
        };
        snapbox::assert_data_eq!(
            but_workspace::branch_details(
                &repo,
                refname("A").as_ref(),
                &reviewed_branch(),
                &project_meta
            )
            .unwrap()
            .to_debug(),
            snapbox::str![[r#"
BranchDetails {
    name: "A",
    reference: FullName(
        "refs/heads/A",
    ),
    remote_tracking_branch: None,
    pr_number: Some(
        42,
    ),
    review_id: Some(
        "uuid",
    ),
    tip: Sha1(7f389eda1b366f3d56ecc1300b3835727c3309b6),
    base_commit: Sha1(ff045efb99e8ee865f0fcded16ffbfff689aa667),
    push_status: CompletelyUnpushed,
    last_updated_at: Some(
        56000,
    ),
    authors: [
        author <author@example.com>,
        committer <committer@example.com>,
    ],
    is_conflicted: false,
    commits: [
        Commit(7f389ed, "add 10 to the beginning", local/remote(identity)),
    ],
    upstream_commits: [],
    is_remote_head: false,
}

"#]]
        );
        Ok(())
    }

    #[test]
    fn nothing_to_push() -> anyhow::Result<()> {
        let repo =
            read_only_in_memory_scenario_named("with-remotes-no-workspace", "nothing-to-push")?;

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 89cc2d3 (HEAD -> A, origin/A) change in A
* d79bba9 new file in A
* c166d42 (origin/main, origin/HEAD, main) init-integration

"#]]
        );
        let project_meta = ProjectMeta {
            target_ref: Some(refname("main")),
            ..Default::default()
        };
        snapbox::assert_data_eq!(
            but_workspace::branch_details(
                &repo,
                refname("A").as_ref(),
                &reviewed_branch(),
                &project_meta
            )
            .unwrap()
            .to_debug(),
            snapbox::str![[r#"
BranchDetails {
    name: "A",
    reference: FullName(
        "refs/heads/A",
    ),
    remote_tracking_branch: Some(
        "refs/remotes/origin/A",
    ),
    pr_number: Some(
        42,
    ),
    review_id: Some(
        "uuid",
    ),
    tip: Sha1(89cc2d303514654e9cab2d05b9af08b420a740c1),
    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
    push_status: NothingToPush,
    last_updated_at: Some(
        56000,
    ),
    authors: [
        author <author@example.com>,
        committer <committer@example.com>,
    ],
    is_conflicted: false,
    commits: [
        Commit(89cc2d3, "change in A", local/remote(identity)),
        Commit(d79bba9, "new file in A", local/remote(identity)),
    ],
    upstream_commits: [],
    is_remote_head: false,
}

"#]]
        );
        Ok(())
    }

    #[test]
    fn remote_tracking_advanced_ff() -> anyhow::Result<()> {
        let repo = read_only_in_memory_scenario_named(
            "with-remotes-no-workspace",
            "remote-tracking-advanced-ff",
        )?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 89cc2d3 (origin/A) change in A
* d79bba9 (HEAD -> A) new file in A
* c166d42 (origin/main, origin/HEAD, main) init-integration

"#]]
        );

        let project_meta = ProjectMeta {
            target_ref: Some(refname("main")),
            ..Default::default()
        };
        snapbox::assert_data_eq!(
            but_workspace::branch_details(
                &repo,
                refname("A").as_ref(),
                &reviewed_branch(),
                &project_meta
            )
            .unwrap()
            .to_debug(),
            snapbox::str![[r#"
BranchDetails {
    name: "A",
    reference: FullName(
        "refs/heads/A",
    ),
    remote_tracking_branch: Some(
        "refs/remotes/origin/A",
    ),
    pr_number: Some(
        42,
    ),
    review_id: Some(
        "uuid",
    ),
    tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
    push_status: UnpushedCommitsRequiringForce,
    last_updated_at: Some(
        56000,
    ),
    authors: [
        author <author@example.com>,
        committer <committer@example.com>,
    ],
    is_conflicted: false,
    commits: [
        Commit(d79bba9, "new file in A", local/remote(identity)),
    ],
    upstream_commits: [
        UpstreamCommit(89cc2d3, "change in A"),
    ],
    is_remote_head: false,
}

"#]]
        );

        // Remote tracking branches are OK to use as well.
        snapbox::assert_data_eq!(
            but_workspace::branch_details(
                &repo,
                refname("origin/A").as_ref(),
                &Branch::default(),
                &project_meta
            )
            .unwrap()
            .to_debug(),
            snapbox::str![[r#"
BranchDetails {
    name: "origin/A",
    reference: FullName(
        "refs/remotes/origin/A",
    ),
    remote_tracking_branch: None,
    pr_number: None,
    review_id: None,
    tip: Sha1(89cc2d303514654e9cab2d05b9af08b420a740c1),
    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
    push_status: NothingToPush,
    last_updated_at: None,
    authors: [
        author <author@example.com>,
        committer <committer@example.com>,
    ],
    is_conflicted: false,
    commits: [
        Commit(89cc2d3, "change in A", local/remote(identity)),
        Commit(d79bba9, "new file in A", local/remote(identity)),
    ],
    upstream_commits: [],
    is_remote_head: true,
}

"#]]
        );
        Ok(())
    }

    #[test]
    fn remote_tracking_diverged() -> anyhow::Result<()> {
        let repo =
            read_only_in_memory_scenario_named("with-remotes-no-workspace", "remote-diverged")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 1a265a4 (HEAD -> A) local change in A
| * 89cc2d3 (origin/A) change in A
|/  
* d79bba9 new file in A
* c166d42 (origin/main, origin/HEAD, main) init-integration

"#]]
        );

        let project_meta = ProjectMeta {
            target_ref: Some(refname("main")),
            ..Default::default()
        };
        snapbox::assert_data_eq!(
            but_workspace::branch_details(
                &repo,
                refname("A").as_ref(),
                &reviewed_branch(),
                &project_meta
            )
            .unwrap()
            .to_debug(),
            snapbox::str![[r#"
BranchDetails {
    name: "A",
    reference: FullName(
        "refs/heads/A",
    ),
    remote_tracking_branch: Some(
        "refs/remotes/origin/A",
    ),
    pr_number: Some(
        42,
    ),
    review_id: Some(
        "uuid",
    ),
    tip: Sha1(1a265a4374e58a2d5fc015d8ce3ce92025702273),
    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
    push_status: UnpushedCommitsRequiringForce,
    last_updated_at: Some(
        56000,
    ),
    authors: [
        author <author@example.com>,
        committer <committer@example.com>,
        local-user <local-user@example.com>,
    ],
    is_conflicted: false,
    commits: [
        Commit(1a265a4, "local change in A", local/remote(identity)),
        Commit(d79bba9, "new file in A", local/remote(identity)),
    ],
    upstream_commits: [
        UpstreamCommit(89cc2d3, "change in A"),
    ],
    is_remote_head: false,
}

"#]]
        );
        Ok(())
    }

    fn reviewed_branch() -> Branch {
        Branch {
            ref_info: RefInfo {
                created_at: None,
                updated_at: Some(gix::date::Time::new(56, 0)),
            },
            review: Review {
                pull_request: Some(42),
                review_id: Some("uuid".into()),
            },
        }
    }
}
