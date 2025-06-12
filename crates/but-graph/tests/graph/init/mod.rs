use crate::graph_tree;
use but_graph::Graph;
use but_graph::init::{Options, Segmentation};
use but_testsupport::visualize_commit_graph_all;

#[test]
fn unborn() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;

    for segmentation in all_segmentations() {
        let graph = Graph::from_head(
            &repo,
            &*meta,
            Options {
                segmentation,
                ..standard_options()
            },
        )?;
        insta::allow_duplicates! {
            insta::assert_snapshot!(graph_tree(&graph), @"└── 👉►:0:refs/heads/main");
            insta::assert_debug_snapshot!(graph, @r#"
            Graph {
                inner: Graph {
                    Ty: "Directed",
                    node_count: 1,
                    edge_count: 0,
                    node weights: {
                        0: StackSegment {
                            id: 0,
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                        },
                    },
                    edge weights: {},
                },
                entrypoint: Some(
                    (
                        NodeIndex(0),
                        None,
                    ),
                ),
            }
            "#);
        }
    }
    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 541396b (HEAD -> main, tag: release/v1, tag: annotated) first
    * fafd9d0 (other) init
    ");

    for segmentation in all_segmentations() {
        let graph = Graph::from_head(
            &repo,
            &*meta,
            Options {
                segmentation,
                ..standard_options()
            },
        )?;
        insta::allow_duplicates! {
            insta::assert_snapshot!(graph_tree(&graph), @r#"
            └── 👉►:0:refs/heads/main
                └── 🔵541396b❱"first" ►tags/annotated, ►tags/release/v1
                    └── ►:1:refs/heads/other
                        └── 🔵fafd9d0❱"init"
            "#);
            insta::assert_debug_snapshot!(graph, @r#"
            Graph {
                inner: Graph {
                    Ty: "Directed",
                    node_count: 2,
                    edge_count: 1,
                    edges: (0, 1),
                    node weights: {
                        0: StackSegment {
                            id: 0,
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(541396b, "first\n", local, ►annotated, ►release/v1),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                        },
                        1: StackSegment {
                            id: 1,
                            ref_name: "refs/heads/other",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(fafd9d0, "init\n", local),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                        },
                    },
                    edge weights: {
                        0: Edge {
                            src: Some(
                                0,
                            ),
                            dst: Some(
                                0,
                            ),
                        },
                    },
                },
                entrypoint: Some(
                    (
                        NodeIndex(0),
                        Some(
                            0,
                        ),
                    ),
                ),
            }
            "#);
        }
    }
    Ok(())
}

#[test]
fn multi_root() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("multi-root")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c6c8c05 (HEAD -> main) Merge branch 'C'
    |\  
    | *   8631946 (C) Merge branch 'D' into C
    | |\  
    | | * f4955b6 (D) D
    | * 00fab2a C
    *   76fc5c4 Merge branch 'B'
    |\  
    | * 366d496 (B) B
    * e5d0542 A
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:refs/heads/main
        └── 🔵c6c8c05❱"Merge branch \'C\'"
            ├── ►:2:refs/heads/C
            │   └── 🔵8631946❱"Merge branch \'D\' into C"
            │       ├── ►:6:refs/heads/D
            │       │   └── 🔵f4955b6❱"D"
            │       └── ►:5:anon:
            │           └── 🔵00fab2a❱"C"
            └── ►:1:anon:
                └── 🔵76fc5c4❱"Merge branch \'B\'"
                    ├── ►:4:refs/heads/B
                    │   └── 🔵366d496❱"B"
                    └── ►:3:anon:
                        └── 🔵e5d0542❱"A"
    "#);
    assert_eq!(
        graph.tip_segments().count(),
        1,
        "all leads to a single merge-commit"
    );
    assert_eq!(
        graph.base_segments().count(),
        4,
        "there are 4 orphaned bases"
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Options {
            segmentation: Segmentation::FirstParentPriority,
            ..standard_options()
        },
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:refs/heads/main
        ├── 🔵c6c8c05❱"Merge branch \'C\'"
        │   └── ►:1:refs/heads/C
        │       ├── 🔵8631946❱"Merge branch \'D\' into C"
        │       │   └── ►:3:refs/heads/D
        │       │       └── 🔵f4955b6❱"D"
        │       └── 🔵00fab2a❱"C"
        ├── 🔵76fc5c4❱"Merge branch \'B\'"
        │   └── ►:2:refs/heads/B
        │       └── 🔵366d496❱"B"
        └── 🔵e5d0542❱"A"
    "#);
    Ok(())
}

#[test]
fn four_diamond() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   8a6c109 (HEAD -> merged) Merge branch 'C' into merged
    |\  
    | *   7ed512a (C) Merge branch 'D' into C
    | |\  
    | | * ecb1877 (D) D
    | * | 35ee481 C
    | |/  
    * |   62b409a (A) Merge branch 'B' into A
    |\ \  
    | * | f16dddf (B) B
    | |/  
    * / 592abec A
    |/  
    * 965998b (main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:refs/heads/merged
        └── 🔵8a6c109❱"Merge branch \'C\' into merged"
            ├── ►:2:refs/heads/C
            │   └── 🔵7ed512a❱"Merge branch \'D\' into C"
            │       ├── ►:6:refs/heads/D
            │       │   └── 🔵ecb1877❱"D"
            │       │       └── ►:7:refs/heads/main
            │       │           └── 🔵965998b❱"base"
            │       └── ►:5:anon:
            │           └── 🔵35ee481❱"C"
            │               └── ERROR: Reached segment 7 for a second time: Some("refs/heads/main")
            └── ►:1:refs/heads/A
                └── 🔵62b409a❱"Merge branch \'B\' into A"
                    ├── ►:4:refs/heads/B
                    │   └── 🔵f16dddf❱"B"
                    │       └── ERROR: Reached segment 7 for a second time: Some("refs/heads/main")
                    └── ►:3:anon:
                        └── 🔵592abec❱"A"
                            └── ERROR: Reached segment 7 for a second time: Some("refs/heads/main")
    "#);

    assert_eq!(
        graph.num_segments(),
        8,
        "just as many as are displayed in the tree"
    );
    assert_eq!(
        graph.num_edges(),
        10,
        "however, we see only a portion of the edges as the tree can only show simple stacks"
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Options {
            segmentation: Segmentation::FirstParentPriority,
            ..standard_options()
        },
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:refs/heads/merged
        └── 🔵8a6c109❱"Merge branch \'C\' into merged"
            ├── ►:2:refs/heads/C
            │   ├── 🔵7ed512a❱"Merge branch \'D\' into C"
            │   │   └── ►:4:refs/heads/D
            │   │       └── 🔵ecb1877❱"D"
            │   │           └── ►:5:refs/heads/main
            │   │               └── 🔵965998b❱"base"
            │   └── 🔵35ee481❱"C"
            │       └── ERROR: Reached segment 5 for a second time: Some("refs/heads/main")
            └── ►:1:refs/heads/A
                ├── 🔵62b409a❱"Merge branch \'B\' into A"
                │   └── ►:3:refs/heads/B
                │       └── 🔵f16dddf❱"B"
                │           └── ERROR: Reached segment 5 for a second time: Some("refs/heads/main")
                └── 🔵592abec❱"A"
                    └── ERROR: Reached segment 5 for a second time: Some("refs/heads/main")
    "#);
    Ok(())
}

mod with_workspace;

fn standard_options() -> but_graph::init::Options {
    but_graph::init::Options {
        collect_tags: true,
        ..Default::default()
    }
}

fn all_segmentations() -> [Segmentation; 2] {
    [
        Segmentation::AtMergeCommits,
        Segmentation::FirstParentPriority,
    ]
}

mod utils {
    use but_graph::VirtualBranchesTomlMetadata;
    use gitbutler_stack::{StackId, Target};

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        named_read_only_in_memory_scenario("scenarios", name)
    }

    fn named_read_only_in_memory_scenario(
        script: &str,
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let repo = read_only_in_memory_scenario_named(script, name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, std::mem::ManuallyDrop::new(meta)))
    }

    /// Provide a scenario but assure the returned repository will write objects to memory, in a subdirectory `dirname`.
    pub fn read_only_in_memory_scenario_named(
        script_name: &str,
        dirname: &str,
    ) -> anyhow::Result<gix::Repository> {
        let root = gix_testtools::scripted_fixture_read_only(format!("{script_name}.sh"))
            .map_err(anyhow::Error::from_boxed)?;
        let repo = gix::open_opts(root.join(dirname), gix::open::Options::isolated())?
            .with_object_memory();
        Ok(repo)
    }

    pub enum StackState {
        #[allow(dead_code)]
        InWorkspace,
        Inactive,
    }

    pub fn add_workspace(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack(
            meta,
            StackId::from_number_for_testing(u128::MAX),
            "definitely outside of the workspace just to have it",
            StackState::Inactive,
        );
    }

    pub fn add_stack(
        meta: &mut VirtualBranchesTomlMetadata,
        stack_id: StackId,
        stack_name: &str,
        state: StackState,
    ) -> StackId {
        add_stack_with_segments(meta, stack_id, stack_name, state, &[])
    }

    // Add parameters as needed.
    pub fn add_stack_with_segments(
        meta: &mut VirtualBranchesTomlMetadata,
        stack_id: StackId,
        stack_name: &str,
        state: StackState,
        segments: &[&str],
    ) -> StackId {
        let mut stack = gitbutler_stack::Stack::new_with_just_heads(
            segments
                .iter()
                .rev()
                .map(|stack_name| {
                    gitbutler_stack::StackBranch::new_with_zero_head(
                        (*stack_name).into(),
                        None,
                        None,
                        None,
                        false,
                    )
                })
                .chain(std::iter::once(
                    gitbutler_stack::StackBranch::new_with_zero_head(
                        stack_name.into(),
                        None,
                        None,
                        None,
                        false,
                    ),
                ))
                .collect(),
            0,
            meta.data().branches.len(),
            match state {
                StackState::InWorkspace => true,
                StackState::Inactive => false,
            },
        );
        stack.id = stack_id;
        meta.data_mut().branches.insert(stack_id, stack);
        // Assure we have a target set.
        meta.data_mut().default_target = Some(Target {
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            remote_url: "does not matter".to_string(),
            sha: git2::Oid::zero(),
            push_remote_name: None,
        });
        stack_id
    }
}
pub use utils::{StackState, add_stack_with_segments, add_workspace, read_only_in_memory_scenario};
