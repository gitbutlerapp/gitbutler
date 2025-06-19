use crate::graph_tree;
use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;

#[test]
fn unborn() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @"└── 👉►:0:main");
    insta::assert_debug_snapshot!(graph, @r#"
    Graph {
        inner: Graph {
            Ty: "Directed",
            node_count: 1,
            edge_count: 0,
            node weights: {
                0: StackSegment {
                    id: NodeIndex(0),
                    ref_name: "refs/heads/main",
                    remote_tracking_ref_name: "None",
                    commits: [],
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
        hard_limit_hit: false,
    }
    "#);
    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 541396b (HEAD -> main, tag: release/v1, tag: annotated) first
    * fafd9d0 (other) init
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:main
        └── ·541396b (⌂)❱"first" ►tags/annotated, ►tags/release/v1
            └── ►:1:other
                └── ·fafd9d0 (⌂)❱"init"
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
                    id: NodeIndex(0),
                    ref_name: "refs/heads/main",
                    remote_tracking_ref_name: "None",
                    commits: [
                        Commit(541396b, "first\n", ⌂),
                    ],
                    metadata: "None",
                },
                1: StackSegment {
                    id: NodeIndex(1),
                    ref_name: "refs/heads/other",
                    remote_tracking_ref_name: "None",
                    commits: [
                        Commit(fafd9d0, "init\n", ⌂),
                    ],
                    metadata: "None",
                },
            },
            edge weights: {
                0: Edge {
                    src: Some(
                        0,
                    ),
                    src_id: Some(
                        Sha1(541396b24e13b8ac45b7905c3fe8691c7fc5fbd0),
                    ),
                    dst: Some(
                        0,
                    ),
                    dst_id: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
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
        hard_limit_hit: false,
    }
    "#);
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
    └── 👉►:0:main
        └── ·c6c8c05 (⌂)❱"Merge branch \'C\'"
            ├── ►:2:C
            │   └── ·8631946 (⌂)❱"Merge branch \'D\' into C"
            │       ├── ►:6:D
            │       │   └── ·f4955b6 (⌂)❱"D"
            │       └── ►:5:anon:
            │           └── ·00fab2a (⌂)❱"C"
            └── ►:1:anon:
                └── ·76fc5c4 (⌂)❱"Merge branch \'B\'"
                    ├── ►:4:B
                    │   └── ·366d496 (⌂)❱"B"
                    └── ►:3:anon:
                        └── ·e5d0542 (⌂)❱"A"
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
    └── 👉►:0:merged
        └── ·8a6c109 (⌂)❱"Merge branch \'C\' into merged"
            ├── ►:2:C
            │   └── ·7ed512a (⌂)❱"Merge branch \'D\' into C"
            │       ├── ►:6:D
            │       │   └── ·ecb1877 (⌂)❱"D"
            │       │       └── ►:7:main
            │       │           └── ·965998b (⌂)❱"base"
            │       └── ►:5:anon:
            │           └── ·35ee481 (⌂)❱"C"
            │               └── →:7: (main)
            └── ►:1:A
                └── ·62b409a (⌂)❱"Merge branch \'B\' into A"
                    ├── ►:4:B
                    │   └── ·f16dddf (⌂)❱"B"
                    │       └── →:7: (main)
                    └── ►:3:anon:
                        └── ·592abec (⌂)❱"A"
                            └── →:7: (main)
    "#);

    assert_eq!(
        graph.num_segments(),
        8,
        "just as many as are displayed in the tree"
    );
    assert_eq!(graph.num_commits(), 8, "one commit per node");
    assert_eq!(
        graph.num_connections(),
        10,
        "however, we see only a portion of the edges as the tree can only show simple stacks"
    );
    Ok(())
}

#[test]
fn stacked_rebased_remotes() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("remote-includes-another-remote")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 682be32 (origin/B) B
    * e29c23d (origin/A) A
    | * 312f819 (HEAD -> B) B
    | * e255adc (A) A
    |/  
    * fafd9d0 (main) init
    ");

    // A remote will always be able to find their non-remotes so they don't seem cut-off.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:B <> origin/B
    │   └── ·312f819 (⌂)❱"B"
    │       └── ►:2:A <> origin/A
    │           └── ·e255adc (⌂)❱"A"
    │               └── ►:4:main
    │                   └── ·fafd9d0 (⌂)❱"init"
    └── ►:1:origin/B
        └── 🟣682be32❱"B"
            └── ►:3:origin/A
                └── 🟣e29c23d❱"A"
                    └── →:4: (main)
    "#);
    // The hard limit is always respected though.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_hard_limit(7))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:B
    │   └── ·312f819 (⌂)❱"B"
    │       └── ►:2:A
    │           └── ·e255adc (⌂)❱"A"
    │               └── ►:4:main
    │                   └── ·fafd9d0 (⌂)❱"init"
    ├── ►:1:origin/B
    │   └── ❌🟣682be32❱"B"
    └── ►:3:origin/A
    "#);

    // Everything we encounter is checked for remotes.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:B <> origin/B
    │   └── ·312f819 (⌂)❱"B"
    │       └── ►:2:A <> origin/A
    │           └── ·e255adc (⌂)❱"A"
    │               └── ►:4:main
    │                   └── ·fafd9d0 (⌂)❱"init"
    └── ►:1:origin/B
        └── 🟣682be32❱"B"
            └── ►:3:origin/A
                └── 🟣e29c23d❱"A"
                    └── →:4: (main)
    "#);

    // With a lower entrypoint, we don't see part of the graph.
    let (id, name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:A <> origin/A
    │   └── ·e255adc (⌂)❱"A"
    │       └── ►:2:main
    │           └── ·fafd9d0 (⌂)❱"init"
    └── ►:1:origin/A
        └── 🟣e29c23d❱"A"
            └── →:2: (main)
    "#);
    Ok(())
}

#[test]
fn with_limits() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("triple-merge")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   2a95729 (HEAD -> C) Merge branches 'A' and 'B' into C
    |\ \  
    | | * 9908c99 (B) B3
    | | * 60d9a56 B2
    | | * 9d171ff B1
    | * | 20a823c (A) A3
    | * | 442a12f A2
    | * | 686706b A1
    | |/  
    * | 6861158 C3
    * | 4f1f248 C2
    * | 487ffce C1
    |/  
    * edc4dee (main) 5
    * 01d0e1e 4
    * 4b3e5a8 3
    * 34d0715 2
    * eb5f731 1
    ");

    // Without limits
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:C
        └── ·2a95729 (⌂)❱"Merge branches \'A\' and \'B\' into C"
            ├── ►:3:B
            │   ├── ·9908c99 (⌂)❱"B3"
            │   ├── ·60d9a56 (⌂)❱"B2"
            │   └── ·9d171ff (⌂)❱"B1"
            │       └── ►:4:main
            │           ├── ·edc4dee (⌂)❱"5"
            │           ├── ·01d0e1e (⌂)❱"4"
            │           ├── ·4b3e5a8 (⌂)❱"3"
            │           ├── ·34d0715 (⌂)❱"2"
            │           └── ·eb5f731 (⌂)❱"1"
            ├── ►:2:A
            │   ├── ·20a823c (⌂)❱"A3"
            │   ├── ·442a12f (⌂)❱"A2"
            │   └── ·686706b (⌂)❱"A1"
            │       └── →:4: (main)
            └── ►:1:anon:
                ├── ·6861158 (⌂)❱"C3"
                ├── ·4f1f248 (⌂)❱"C2"
                └── ·487ffce (⌂)❱"C1"
                    └── →:4: (main)
    "#);

    // There is no empty starting points, we always traverse the first commit as we really want
    // to get to remote processing there.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(0))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:C
        └── ✂️·2a95729 (⌂)❱"Merge branches \'A\' and \'B\' into C"
    "#);

    // A single commit, the merge commit.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:C
        └── ·2a95729 (⌂)❱"Merge branches \'A\' and \'B\' into C"
            ├── ►:3:B
            │   └── ✂️·9908c99 (⌂)❱"B3"
            ├── ►:2:A
            │   └── ✂️·20a823c (⌂)❱"A3"
            └── ►:1:anon:
                └── ✂️·6861158 (⌂)❱"C3"
    "#);

    // The merge commit, then we witness lane-duplication of the limit so we get more than requested.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(2))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:C
        └── ·2a95729 (⌂)❱"Merge branches \'A\' and \'B\' into C"
            ├── ►:3:B
            │   ├── ·9908c99 (⌂)❱"B3"
            │   └── ✂️·60d9a56 (⌂)❱"B2"
            ├── ►:2:A
            │   ├── ·20a823c (⌂)❱"A3"
            │   └── ✂️·442a12f (⌂)❱"A2"
            └── ►:1:anon:
                ├── ·6861158 (⌂)❱"C3"
                └── ✂️·4f1f248 (⌂)❱"C2"
    "#);

    // Allow to see more commits just in the middle lane, the limit is reset,
    // and we see two more.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options()
            .with_limit_hint(2)
            .with_limit_extension_at(Some(id_by_rev(&repo, ":/A3").detach())),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:C
        └── ·2a95729 (⌂)❱"Merge branches \'A\' and \'B\' into C"
            ├── ►:3:B
            │   ├── ·9908c99 (⌂)❱"B3"
            │   └── ✂️·60d9a56 (⌂)❱"B2"
            ├── ►:2:A
            │   ├── ·20a823c (⌂)❱"A3"
            │   ├── ·442a12f (⌂)❱"A2"
            │   └── ✂️·686706b (⌂)❱"A1"
            └── ►:1:anon:
                ├── ·6861158 (⌂)❱"C3"
                └── ✂️·4f1f248 (⌂)❱"C2"
    "#);

    // Multiple extensions are fine as well.
    let id = |rev| id_by_rev(&repo, rev).detach();
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options()
            .with_limit_hint(2)
            .with_limit_extension_at([id(":/A3"), id(":/A1"), id(":/B3"), id(":/C3")]),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:C
        └── ·2a95729 (⌂)❱"Merge branches \'A\' and \'B\' into C"
            ├── ►:3:B
            │   ├── ·9908c99 (⌂)❱"B3"
            │   ├── ·60d9a56 (⌂)❱"B2"
            │   └── ✂️·9d171ff (⌂)❱"B1"
            ├── ►:2:A
            │   ├── ·20a823c (⌂)❱"A3"
            │   ├── ·442a12f (⌂)❱"A2"
            │   └── ·686706b (⌂)❱"A1"
            │       └── ►:4:main
            │           ├── ·edc4dee (⌂)❱"5"
            │           └── ✂️·01d0e1e (⌂)❱"4"
            └── ►:1:anon:
                ├── ·6861158 (⌂)❱"C3"
                ├── ·4f1f248 (⌂)❱"C2"
                └── ✂️·487ffce (⌂)❱"C1"
    "#);

    insta::assert_debug_snapshot!(graph.statistics(), @r"
    Statistics {
        segments: 5,
        segments_integrated: 0,
        segments_remote: 0,
        segments_with_remote_tracking_branch: 0,
        segments_empty: 0,
        segments_unnamed: 1,
        segments_in_workspace: 0,
        segments_in_workspace_and_integrated: 0,
        segments_with_workspace_metadata: 0,
        segments_with_branch_metadata: 0,
        entrypoint_in_workspace: Some(
            false,
        ),
        segments_behind_of_entrypoint: 4,
        segments_ahead_of_entrypoint: 0,
        connections: 4,
        commits: 12,
        commit_references: 0,
        commits_at_cutoff: 3,
    }
    ");
    Ok(())
}

mod with_workspace;

mod utils;
pub use utils::{
    StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev,
    read_only_in_memory_scenario, standard_options,
};
