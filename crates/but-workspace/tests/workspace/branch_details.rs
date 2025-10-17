/// All tests have a workspace present.
mod with_workspace {
    use std::{
        any::Any,
        ops::{Deref, DerefMut},
    };

    use but_core::{
        RefMetadata,
        ref_metadata::{Branch, RefInfo, Review, Workspace},
    };
    use but_testsupport::{visualize_commit_graph, visualize_commit_graph_all};
    use gix::refs::{FullName, FullNameRef};

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
        insta::assert_snapshot!(visualize_commit_graph(&repo, "HEAD")?, @r"
        *   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
        |\  
        | * 7f389ed (A) add 10 to the beginning
        * | 91ef6f6 (B) add 10 to the end
        |/  
        * ff045ef (main) init
        ");
        let store = WorkspaceRefMetadataStore::default()
            .with_target("B")
            .with_named_branch("A");
        insta::assert_debug_snapshot!(
            but_workspace::branch_details_v3(&repo, refname("A").as_ref(), &store).unwrap(),
            @r#"
        BranchDetails {
            name: "refs/heads/A",
            remote_tracking_branch: None,
            description: Some(
                "A: description",
            ),
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
        "#,
        );
        Ok(())
    }

    #[test]
    fn nothing_to_push() -> anyhow::Result<()> {
        let repo =
            read_only_in_memory_scenario_named("with-remotes-no-workspace", "nothing-to-push")?;

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 89cc2d3 (HEAD -> A, origin/A) change in A
        * d79bba9 new file in A
        * c166d42 (origin/main, origin/HEAD, main) init-integration
        ");
        let store = WorkspaceRefMetadataStore::default()
            .with_target("main")
            .with_named_branch("A");
        insta::assert_debug_snapshot!(but_workspace::branch_details_v3(&repo, refname("A").as_ref(), &store).unwrap(), @r#"
        BranchDetails {
            name: "refs/heads/A",
            remote_tracking_branch: Some(
                "refs/remotes/origin/A",
            ),
            description: Some(
                "A: description",
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
        "#);
        Ok(())
    }

    #[test]
    fn remote_tracking_advanced_ff() -> anyhow::Result<()> {
        let repo = read_only_in_memory_scenario_named(
            "with-remotes-no-workspace",
            "remote-tracking-advanced-ff",
        )?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 89cc2d3 (origin/A) change in A
        * d79bba9 (HEAD -> A) new file in A
        * c166d42 (origin/main, origin/HEAD, main) init-integration
        ");

        let store = WorkspaceRefMetadataStore::default()
            .with_target("main")
            .with_named_branch("A");
        insta::assert_debug_snapshot!(but_workspace::branch_details_v3(&repo, refname("A").as_ref(), &store).unwrap(), @r#"
        BranchDetails {
            name: "refs/heads/A",
            remote_tracking_branch: Some(
                "refs/remotes/origin/A",
            ),
            description: Some(
                "A: description",
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
        "#);

        // Remote tracking branches are OK to use as well.
        insta::assert_debug_snapshot!(but_workspace::branch_details_v3(&repo, refname("origin/A").as_ref(), &store).unwrap(), @r#"
        BranchDetails {
            name: "refs/remotes/origin/A",
            remote_tracking_branch: None,
            description: None,
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
        "#);
        Ok(())
    }

    #[test]
    fn remote_tracking_diverged() -> anyhow::Result<()> {
        let repo =
            read_only_in_memory_scenario_named("with-remotes-no-workspace", "remote-diverged")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 1a265a4 (HEAD -> A) local change in A
        | * 89cc2d3 (origin/A) change in A
        |/  
        * d79bba9 new file in A
        * c166d42 (origin/main, origin/HEAD, main) init-integration
        ");

        let store = WorkspaceRefMetadataStore::default()
            .with_target("main")
            .with_named_branch("A");
        insta::assert_debug_snapshot!(but_workspace::branch_details_v3(&repo, refname("A").as_ref(), &store).unwrap(), @r#"
        BranchDetails {
            name: "refs/heads/A",
            remote_tracking_branch: Some(
                "refs/remotes/origin/A",
            ),
            description: Some(
                "A: description",
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
        "#);
        Ok(())
    }

    #[derive(Default)]
    struct WorkspaceRefMetadataStore {
        workspace: Workspace,
        branches: Vec<(FullName, but_core::ref_metadata::Branch)>,
    }

    impl WorkspaceRefMetadataStore {
        pub fn with_target(mut self, short_name: &str) -> Self {
            self.workspace = but_core::ref_metadata::Workspace {
                ref_info: Default::default(),
                stacks: vec![],
                target_ref: Some(refname(short_name)),
                push_remote: None,
            };
            self
        }

        pub fn with_branch(mut self, short_name: &str, branch: Branch) -> Self {
            self.branches.push((refname(short_name), branch));
            self
        }

        pub fn with_named_branch(self, short_name: &str) -> Self {
            let branch = Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(gix::date::Time::new(56, 0)),
                },
                description: Some(format!("{short_name}: description")),
                review: Review {
                    pull_request: Some(42),
                    review_id: Some("uuid".into()),
                },
            };
            self.with_branch(short_name, branch)
        }
    }

    struct NullHandle<T> {
        inner: T,
        is_default: bool,
        name: FullName,
    }

    impl<T> but_core::ref_metadata::ValueInfo for NullHandle<T> {
        fn is_default(&self) -> bool {
            self.is_default
        }
    }

    impl<T> Deref for NullHandle<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<T> DerefMut for NullHandle<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    impl<T> AsRef<FullNameRef> for NullHandle<T> {
        fn as_ref(&self) -> &FullNameRef {
            self.name.as_ref()
        }
    }

    impl RefMetadata for WorkspaceRefMetadataStore {
        type Handle<T> = NullHandle<T>;

        fn iter(&self) -> impl Iterator<Item = anyhow::Result<(FullName, Box<dyn Any>)>> + '_ {
            std::iter::empty()
        }

        fn workspace(&self, ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Workspace>> {
            Ok(NullHandle {
                inner: self.workspace.clone(),
                is_default: false,
                name: ref_name.into(),
            })
        }

        fn branch(&self, ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
            let mut is_default = true;
            let inner = self
                .branches
                .iter()
                .find_map(|(name, branch)| {
                    (name.as_ref() == ref_name).then(|| {
                        is_default = false;
                        branch
                    })
                })
                .cloned()
                .unwrap_or_default();
            Ok(NullHandle {
                inner,
                is_default: true,
                name: ref_name.into(),
            })
        }

        fn set_workspace(&mut self, _value: &Self::Handle<Workspace>) -> anyhow::Result<()> {
            unreachable!()
        }

        fn set_branch(&mut self, _value: &Self::Handle<Branch>) -> anyhow::Result<()> {
            unreachable!()
        }

        fn remove(&mut self, _ref_name: &FullNameRef) -> anyhow::Result<bool> {
            unreachable!()
        }
    }
}
