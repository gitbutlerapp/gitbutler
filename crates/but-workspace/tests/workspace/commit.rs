mod from_new_merge_with_metadata {
    use bstr::ByteSlice;
    use but_graph::init::{Options, Overlay};
    use but_testsupport::{visualize_commit_graph_all, visualize_tree};
    use but_workspace::WorkspaceCommit;
    use gix::{prelude::ObjectIdExt, refs::Target};

    use crate::ref_info::with_workspace_commit::utils::{
        named_read_only_in_memory_scenario, named_writable_scenario_with_description_and_graph,
    };

    #[test]
    fn without_conflict_journey() -> anyhow::Result<()> {
        let (repo, mut meta) =
            named_read_only_in_memory_scenario("various-heads-for-clean-merge", "")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * d3cce74 (add-A) add A
        | * 115e41b (add-B) add B
        |/  
        | * 34c4591 (add-C) add C
        |/  
        | * 27ab782 (HEAD -> add-D) add D
        |/  
        * 85efbe4 (main, gitbutler/workspace) M
        ");

        let stacks = ["add-A"];
        add_stacks(&mut meta, stacks);
        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(stacks),
            None,
            &graph,
            &repo,
            None,
        )?;
        let commit = out.workspace_commit_id.attach(&repo).object()?;
        // This commit is never signed.
        insta::assert_snapshot!(commit.data.as_bstr(), @r"
        tree f53c91092dbda83f3565e78c285f3e2ab0cfd968
        parent d3cce74a69ee3b0e1cbea65b53908d602d6bda26
        author GitButler <gitbutler@gitbutler.com> 946771200 +0000
        committer GitButler <gitbutler@gitbutler.com> 946771200 +0000
        encoding UTF-8

        GitButler Workspace Commit

        This is a merge commit of the virtual branches in your workspace.

        Due to GitButler managing multiple virtual branches, you cannot switch back and
        forth between git branches and virtual branches easily. 

        If you switch to another branch, GitButler will need to be reinitialized.
        If you commit on this branch, GitButler will throw it away.

        Here are the branches that are currently applied:
         - add-A
           branch head: d3cce74a69ee3b0e1cbea65b53908d602d6bda26
        For more information about what we're doing here, check out our docs:
        https://docs.gitbutler.com/features/branch-management/integration-branch
        ");
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(31f3d99d4b12a57c1c21053ab3ae5da160247044),
            stacks: [
                Stack { tip: d3cce74, name: "add-A" },
            ],
            missing_stacks: [],
            conflicting_stacks: [],
        }
        "#);
        insta::assert_snapshot!(visualize_tree(commit.peel_to_tree()?.id()), @r#"
        f53c910
        └── A:100644:f70f10e "A\n"
        "#);

        let stacks = ["add-D", "add-A", "add-C", "add-B"];
        add_stacks(&mut meta, stacks);
        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(stacks),
            None,
            &graph,
            &repo,
            Some("refs/heads/has-no-effect-outside-conflicts".try_into()?),
        )?;
        // It retains order.
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(391a453230c141ac5f81d7203ac90c7e66ea9acf),
            stacks: [
                Stack { tip: 27ab782, name: "add-D" },
                Stack { tip: d3cce74, name: "add-A" },
                Stack { tip: 34c4591, name: "add-C" },
                Stack { tip: 115e41b, name: "add-B" },
            ],
            missing_stacks: [],
            conflicting_stacks: [],
        }
        "#);
        let commit = out.workspace_commit_id.attach(&repo).object()?;
        // This commit is never signed.
        insta::assert_snapshot!(commit.data.as_bstr(), @r"
        tree 94e1f0c26d5b13dc3a95a88e64d82155373b5780
        parent 27ab782831b1145249092d54c520a15bb6425cda
        parent d3cce74a69ee3b0e1cbea65b53908d602d6bda26
        parent 34c4591eac5ade7cdf094c4fc48dea798ab73bbb
        parent 115e41b0ffb7fcb56f91a9fb64cf4a7b786c1bea
        author GitButler <gitbutler@gitbutler.com> 946771200 +0000
        committer GitButler <gitbutler@gitbutler.com> 946771200 +0000
        encoding UTF-8

        GitButler Workspace Commit

        This is a merge commit of the virtual branches in your workspace.

        Due to GitButler managing multiple virtual branches, you cannot switch back and
        forth between git branches and virtual branches easily. 

        If you switch to another branch, GitButler will need to be reinitialized.
        If you commit on this branch, GitButler will throw it away.

        Here are the branches that are currently applied:
         - add-D
           branch head: 27ab782831b1145249092d54c520a15bb6425cda
         - add-A
           branch head: d3cce74a69ee3b0e1cbea65b53908d602d6bda26
         - add-C
           branch head: 34c4591eac5ade7cdf094c4fc48dea798ab73bbb
         - add-B
           branch head: 115e41b0ffb7fcb56f91a9fb64cf4a7b786c1bea
        For more information about what we're doing here, check out our docs:
        https://docs.gitbutler.com/features/branch-management/integration-branch
        ");
        // Order isn't visible in the merged tree.
        insta::assert_snapshot!(visualize_tree(commit.peel_to_tree()?.id()), @r#"
        94e1f0c
        ├── A:100644:f70f10e "A\n"
        ├── B:100644:223b783 "B\n"
        ├── C:100644:3cc58df "C\n"
        └── D:100644:1784810 "D\n"
        "#);
        Ok(())
    }

    #[test]
    fn with_multi_line_conflict_journey() -> anyhow::Result<()> {
        let (repo, mut meta) =
            named_read_only_in_memory_scenario("various-heads-for-multi-line-merge-conflict", "")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * d3cce74 (clean-A) add A
        | * 115e41b (clean-B) add B
        |/  
        | * 34c4591 (clean-C) add C
        |/  
        | * bf09eae (conflict-F1) add F1
        |/  
        | * f2ce66d (conflict-F2) add F2
        |/  
        | * 4bbb93c (HEAD -> conflict-hero) add conflicting-F2
        | * 98519e9 add conflicting-F1
        |/  
        * 85efbe4 (main, gitbutler/workspace) M
        ");

        let stacks = [
            "clean-A",
            "conflict-F1",
            "clean-B",
            "conflict-F2",
            "clean-C",
            "conflict-hero",
            "clean-A",
        ];
        add_stacks(&mut meta, stacks);
        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;

        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(stacks),
            None,
            &graph,
            &repo,
            Some("refs/heads/conflict-hero".try_into()?),
        )?;
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(b8ba7bd37ac1a1f9a0e7f29ddf83acc249d7b866),
            stacks: [
                Stack { tip: d3cce74, name: "clean-A" },
                Stack { tip: 115e41b, name: "clean-B" },
                Stack { tip: 34c4591, name: "clean-C" },
                Stack { tip: 4bbb93c, name: "conflict-hero" },
                Stack { tip: d3cce74, name: "clean-A" },
            ],
            missing_stacks: [],
            conflicting_stacks: [
                ConflictingStack {
                    tip: Sha1(bf09eaee36b845f0ee6af0b4e19731498b6a017b),
                    ref_name: FullName(
                        "refs/heads/conflict-F1",
                    ),
                },
                ConflictingStack {
                    tip: Sha1(f2ce66d01ec4227683e16ad679def2ee6aa0d282),
                    ref_name: FullName(
                        "refs/heads/conflict-F2",
                    ),
                },
            ],
        }
        "#);
        let commit = out.workspace_commit_id.attach(&repo).object()?;
        insta::assert_snapshot!(visualize_tree(commit.peel_to_tree()?.id()), @r#"
        45db176
        ├── A:100644:f70f10e "A\n"
        ├── B:100644:223b783 "B\n"
        ├── C:100644:3cc58df "C\n"
        ├── F1:100644:2fc7694 "conflicting-F1\n"
        └── F2:100644:ade95f4 "conflicting-F2\n"
        "#);

        // Just for show, see what happens if there is no hero.
        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(stacks),
            None,
            &graph,
            &repo,
            None,
        )?;
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(e444bfa38570217271f5df56c3fe26ed57a7e023),
            stacks: [
                Stack { tip: d3cce74, name: "clean-A" },
                Stack { tip: bf09eae, name: "conflict-F1" },
                Stack { tip: 115e41b, name: "clean-B" },
                Stack { tip: f2ce66d, name: "conflict-F2" },
                Stack { tip: 34c4591, name: "clean-C" },
                Stack { tip: d3cce74, name: "clean-A" },
            ],
            missing_stacks: [],
            conflicting_stacks: [
                ConflictingStack {
                    tip: Sha1(4bbb93c2e76f7ae0fe61183ac3774943284ba9af),
                    ref_name: FullName(
                        "refs/heads/conflict-hero",
                    ),
                },
            ],
        }
        "#);
        Ok(())
    }

    #[test]
    fn with_conflict_commits() -> anyhow::Result<()> {
        let (_tmp, mut graph, repo, mut meta, _description) =
            named_writable_scenario_with_description_and_graph("with-conflict", |_| {})?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 8450331 (HEAD -> main, tag: conflicted) GitButler WIP Commit
        * a047f81 (tag: normal) init
        ");
        but_testsupport::invoke_bash(
            r#"
            git branch tip-conflicted
            git reset --hard @~1
            git checkout -b unrelated
                touch unrelated-file && git add unrelated-file && git commit -m "unrelated"
            "#,
            &repo,
        );
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 8450331 (tag: conflicted, tip-conflicted) GitButler WIP Commit
        | * 8ab1c4d (HEAD -> unrelated) unrelated
        |/  
        * a047f81 (tag: normal, main) init
        ");

        let stacks = ["tip-conflicted", "unrelated"];
        add_stacks(&mut meta, stacks);

        graph = graph.redo_traversal_with_overlay(
            &repo,
            &meta,
            Overlay::default().with_references_if_new([
                repo.find_reference("unrelated")?.inner,
                // The workspace ref is needed so the workspace and its stacks are iterated as well.
                // Algorithms which work with simulation also have to be mindful about this.
                gix::refs::Reference {
                    name: "refs/heads/gitbutler/workspace".try_into()?,
                    target: Target::Object(repo.rev_parse_single("main")?.detach()),
                    peeled: None,
                },
            ]),
        )?;

        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(stacks),
            None,
            &graph,
            &repo,
            None,
        )?;
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(e25b36b3a4192701e5e91a00d1c2fe07b9888338),
            stacks: [
                Stack { tip: 8450331, name: "tip-conflicted" },
                Stack { tip: 8ab1c4d, name: "unrelated" },
            ],
            missing_stacks: [],
            conflicting_stacks: [],
        }
        "#);

        // There it auto-resolves the commit to not merge the actual tree structure.
        insta::assert_snapshot!(visualize_tree(
            out.workspace_commit_id.attach(&repo).object()?.into_commit().tree_id()?
        ), @r#"
        8882acc
        ├── file:100644:e69de29 ""
        └── unrelated-file:100644:e69de29 ""
        "#);

        Ok(())
    }

    #[test]
    fn with_conflict_journey() -> anyhow::Result<()> {
        let (repo, mut meta) =
            named_read_only_in_memory_scenario("various-heads-for-merge-conflict", "")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * d3cce74 (clean-A) add A
        | * 115e41b (clean-B) add B
        |/  
        | * 6777bd8 (conflict-C1) add C1
        |/  
        | * f8392d2 (HEAD -> conflict-C2) add C2
        |/  
        * 85efbe4 (main, gitbutler/workspace) M
        ");

        // NOTE: the caller would be expected to have prepared a graph that contains these branches.
        let stacks = ["clean-A", "conflict-C1", "clean-B", "conflict-C2"];
        add_stacks(&mut meta, stacks);
        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;

        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(stacks),
            None,
            &graph,
            &repo,
            None,
        )?;
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(4aede0de89327f3afde3db1ed9f83f368f67d501),
            stacks: [
                Stack { tip: d3cce74, name: "clean-A" },
                Stack { tip: 6777bd8, name: "conflict-C1" },
                Stack { tip: 115e41b, name: "clean-B" },
            ],
            missing_stacks: [],
            conflicting_stacks: [
                ConflictingStack {
                    tip: Sha1(f8392d239500de94b23f42c8ab5508dae1b3b657),
                    ref_name: FullName(
                        "refs/heads/conflict-C2",
                    ),
                },
            ],
        }
        "#);
        let commit = out.workspace_commit_id.attach(&repo).object()?;
        // In absence of a hero stack, the conflicting stack is simply not assigned.
        insta::assert_snapshot!(commit.data.as_bstr(), @r"
        tree fc2bf71918ed072ec412bdebb04ec7322f2cfb72
        parent d3cce74a69ee3b0e1cbea65b53908d602d6bda26
        parent 6777bd8aff28a87a07739e2f309d3699d93685f9
        parent 115e41b0ffb7fcb56f91a9fb64cf4a7b786c1bea
        author GitButler <gitbutler@gitbutler.com> 946771200 +0000
        committer GitButler <gitbutler@gitbutler.com> 946771200 +0000
        encoding UTF-8

        GitButler Workspace Commit

        This is a merge commit of the virtual branches in your workspace.

        Due to GitButler managing multiple virtual branches, you cannot switch back and
        forth between git branches and virtual branches easily. 

        If you switch to another branch, GitButler will need to be reinitialized.
        If you commit on this branch, GitButler will throw it away.

        Here are the branches that are currently applied:
         - clean-A
           branch head: d3cce74a69ee3b0e1cbea65b53908d602d6bda26
         - conflict-C1
           branch head: 6777bd8aff28a87a07739e2f309d3699d93685f9
         - clean-B
           branch head: 115e41b0ffb7fcb56f91a9fb64cf4a7b786c1bea
        For more information about what we're doing here, check out our docs:
        https://docs.gitbutler.com/features/branch-management/integration-branch
        ");
        insta::assert_snapshot!(visualize_tree(commit.peel_to_tree()?.id()), @r#"
        fc2bf71
        ├── A:100644:f70f10e "A\n"
        ├── B:100644:223b783 "B\n"
        └── C:100644:e2cf5e7 "C1\n"
        "#);

        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(stacks),
            None,
            &graph,
            &repo,
            Some("refs/heads/conflict-C2".try_into()?),
        )?;
        // TODO: make clean-B show up!
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(fd9723d257cd09656a2f8bb24d3dd2138f88b343),
            stacks: [
                Stack { tip: d3cce74, name: "clean-A" },
                Stack { tip: 115e41b, name: "clean-B" },
                Stack { tip: f8392d2, name: "conflict-C2" },
            ],
            missing_stacks: [],
            conflicting_stacks: [
                ConflictingStack {
                    tip: Sha1(6777bd8aff28a87a07739e2f309d3699d93685f9),
                    ref_name: FullName(
                        "refs/heads/conflict-C1",
                    ),
                },
            ],
        }
        "#);
        let commit = out.workspace_commit_id.attach(&repo).object()?;
        insta::assert_snapshot!(commit.data.as_bstr(), @r"
        tree 39ba52245958cf3a0544caf68c75665b9ad6ea4f
        parent d3cce74a69ee3b0e1cbea65b53908d602d6bda26
        parent 115e41b0ffb7fcb56f91a9fb64cf4a7b786c1bea
        parent f8392d239500de94b23f42c8ab5508dae1b3b657
        author GitButler <gitbutler@gitbutler.com> 946771200 +0000
        committer GitButler <gitbutler@gitbutler.com> 946771200 +0000
        encoding UTF-8

        GitButler Workspace Commit

        This is a merge commit of the virtual branches in your workspace.

        Due to GitButler managing multiple virtual branches, you cannot switch back and
        forth between git branches and virtual branches easily. 

        If you switch to another branch, GitButler will need to be reinitialized.
        If you commit on this branch, GitButler will throw it away.

        Here are the branches that are currently applied:
         - clean-A
           branch head: d3cce74a69ee3b0e1cbea65b53908d602d6bda26
         - clean-B
           branch head: 115e41b0ffb7fcb56f91a9fb64cf4a7b786c1bea
         - conflict-C2
           branch head: f8392d239500de94b23f42c8ab5508dae1b3b657
        For more information about what we're doing here, check out our docs:
        https://docs.gitbutler.com/features/branch-management/integration-branch
        ");
        insta::assert_snapshot!(visualize_tree(commit.peel_to_tree()?.id()), @r#"
        39ba522
        ├── A:100644:f70f10e "A\n"
        ├── B:100644:223b783 "B\n"
        └── C:100644:c4b2d41 "C2\n"
        "#);

        let out = WorkspaceCommit::from_new_merge_with_metadata(
            &to_stacks(["conflict-C2", "conflict-C2", "conflict-C1", "clean-A"]),
            None,
            &graph,
            &repo,
            Some("refs/heads/conflict-C1".try_into()?),
        )?;
        insta::assert_debug_snapshot!(out, @r#"
        Outcome {
            workspace_commit_id: Sha1(e297aee321879eb991bb5360859cbddb5891a316),
            stacks: [
                Stack { tip: 6777bd8, name: "conflict-C1" },
                Stack { tip: d3cce74, name: "clean-A" },
            ],
            missing_stacks: [],
            conflicting_stacks: [
                ConflictingStack {
                    tip: Sha1(f8392d239500de94b23f42c8ab5508dae1b3b657),
                    ref_name: FullName(
                        "refs/heads/conflict-C2",
                    ),
                },
                ConflictingStack {
                    tip: Sha1(f8392d239500de94b23f42c8ab5508dae1b3b657),
                    ref_name: FullName(
                        "refs/heads/conflict-C2",
                    ),
                },
            ],
        }
        "#);

        let commit = out.workspace_commit_id.attach(&repo).object()?;
        insta::assert_snapshot!(visualize_tree(commit.peel_to_tree()?.id()), @r#"
        5c730c4
        ├── A:100644:f70f10e "A\n"
        └── C:100644:e2cf5e7 "C1\n"
        "#);

        Ok(())
    }

    mod utils {
        use but_core::ref_metadata::{
            StackId, WorkspaceCommitRelation::Merged, WorkspaceStack, WorkspaceStackBranch,
        };
        use but_graph::VirtualBranchesTomlMetadata;
        use gix::refs::Category;

        use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack_with_segments};

        pub fn add_stacks(
            meta: &mut VirtualBranchesTomlMetadata,
            short_stack_names: impl IntoIterator<Item = &'static str>,
        ) {
            for (idx, stack_name) in short_stack_names.into_iter().enumerate() {
                add_stack_with_segments(
                    meta,
                    idx as u128 + 1,
                    stack_name,
                    StackState::InWorkspace,
                    &[],
                );
            }
        }

        pub fn to_stacks(
            short_stack_names: impl IntoIterator<Item = &'static str>,
        ) -> Vec<WorkspaceStack> {
            short_stack_names
                .into_iter()
                .map(|short_name| WorkspaceStack {
                    id: StackId::generate(),
                    workspacecommit_relation: Merged,
                    branches: vec![WorkspaceStackBranch {
                        ref_name: Category::LocalBranch
                            .to_full_name(short_name)
                            .expect("known good short ref name"),
                        archived: false,
                    }],
                })
                .collect()
        }
    }
    use utils::{add_stacks, to_stacks};
}
