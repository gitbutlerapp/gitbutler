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
            insta::assert_snapshot!(graph_tree(&graph), @"└── ►refs/heads/main(OUTSIDE)");
            insta::assert_debug_snapshot!(graph, @r#"
    Graph {
        inner: Graph {
            Ty: "Directed",
            node_count: 1,
            edge_count: 0,
            node weights: {
                0: StackSegment {
                    ref_name: "refs/heads/main",
                    remote_tracking_ref_name: "None",
                    ref_location: "OutsideOfWorkspace",
                    commits_unique_from_tip: [],
                    commits_unique_in_remote_tracking_branch: [],
                    metadata: None,
                },
            },
            edge weights: {},
        },
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
    └── ►refs/heads/main
        ├── 🔵fafd9d0❱"init" ►other
        └── 🔵541396b❱"first" ►annotated, ►release/v1
    "#);
            insta::assert_debug_snapshot!(graph, @r#"
    Graph {
        inner: Graph {
            Ty: "Directed",
            node_count: 1,
            edge_count: 0,
            node weights: {
                0: StackSegment {
                    ref_name: "refs/heads/main",
                    remote_tracking_ref_name: "None",
                    ref_location: "None",
                    commits_unique_from_tip: [
                        LocalCommit(541396b, "first\n", local, ►annotated, ►release/v1),
                        LocalCommit(fafd9d0, "init\n", local, ►other),
                    ],
                    commits_unique_in_remote_tracking_branch: [],
                    metadata: None,
                },
            },
            edge weights: {},
        },
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
    └── ►refs/heads/main
        └── 🔵c6c8c05❱"Merge branch \'C\'"
            ├── <anon>
            │   └── 🔵8631946❱"Merge branch \'D\' into C" ►C
            │       ├── <anon>
            │       │   └── 🔵f4955b6❱"D" ►D
            │       └── <anon>
            │           └── 🔵00fab2a❱"C"
            └── <anon>
                └── 🔵76fc5c4❱"Merge branch \'B\'"
                    ├── <anon>
                    │   └── 🔵366d496❱"B" ►B
                    └── <anon>
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
    └── ►refs/heads/main
        ├── 🔵e5d0542❱"A"
        ├── 🔵76fc5c4❱"Merge branch \'B\'"
        │   └── <anon>
        │       └── 🔵366d496❱"B" ►B
        └── 🔵c6c8c05❱"Merge branch \'C\'"
            └── <anon>
                ├── 🔵00fab2a❱"C"
                └── 🔵8631946❱"Merge branch \'D\' into C" ►C
                    └── <anon>
                        └── 🔵f4955b6❱"D" ►D
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
    └── ►refs/heads/merged
        └── 🔵8a6c109❱"Merge branch \'C\' into merged"
            ├── <anon>
            │   └── 🔵7ed512a❱"Merge branch \'D\' into C" ►C
            │       ├── <anon>
            │       │   └── 🔵ecb1877❱"D" ►D
            │       └── <anon>
            │           └── 🔵35ee481❱"C"
            └── <anon>
                └── 🔵62b409a❱"Merge branch \'B\' into A" ►A
                    ├── <anon>
                    │   └── 🔵f16dddf❱"B" ►B
                    └── <anon>
                        ├── 🔵965998b❱"base" ►main
                        └── 🔵592abec❱"A"
    "#);

    assert_eq!(
        graph.num_segments(),
        7,
        "just as many as are displayed in the tree"
    );
    assert_eq!(
        graph.num_edges(),
        9,
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
    └── ERROR: disconnected 4 nodes unreachable through base
        ├── ►refs/heads/merged
        │   ├── 🔵965998b❱"base" ►main
        │   ├── 🔵592abec❱"A"
        │   ├── 🔵62b409a❱"Merge branch \'B\' into A" ►A
        │   │   └── <anon>
        │   │       └── 🔵f16dddf❱"B" ►B
        │   └── 🔵8a6c109❱"Merge branch \'C\' into merged"
        │       └── <anon>
        │           ├── 🔵35ee481❱"C"
        │           └── 🔵7ed512a❱"Merge branch \'D\' into C" ►C
        │               └── <anon>
        │                   └── 🔵ecb1877❱"D" ►D
        ├── ERROR: Reached segment 1 for a second time: None
        ├── ERROR: Reached segment 2 for a second time: None
        └── ERROR: Reached segment 3 for a second time: None
    "#);
    Ok(())
}

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
}
pub use utils::read_only_in_memory_scenario;
