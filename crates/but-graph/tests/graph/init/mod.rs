use but_graph::Graph;
use but_testsupport::{graph_tree, graph_workspace, visualize_commit_graph_all};

#[test]
fn unborn() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:main[🌳]
    ");
    insta::assert_debug_snapshot!(graph, @r#"
    Graph {
        inner: StableGraph {
            Ty: "Directed",
            node_count: 1,
            edge_count: 0,
            node weights: {
                0: Segment {
                    id: NodeIndex(0),
                    generation: 0,
                    ref_info: "►main[🌳]",
                    remote_tracking_ref_name: "None",
                    sibling_segment_id: "None",
                    commits: [],
                    metadata: "None",
                },
            },
            edge weights: {},
            free_node: NodeIndex(4294967295),
            free_edge: EdgeIndex(4294967295),
        },
        entrypoint: Some(
            (
                NodeIndex(0),
                None,
            ),
        ),
        entrypoint_ref: None,
        extra_target: None,
        hard_limit_hit: false,
        options: Options {
            collect_tags: false,
            commits_limit_hint: None,
            commits_limit_recharge_location: [],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
        },
    }
    "#);

    assert!(
        graph.managed_entrypoint_commit(&repo)?.is_none(),
        "there is no commit it could return"
    );
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
    ");

    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 541396b (HEAD, tag: release/v1, tag: annotated, main) first
    * fafd9d0 (other) init
    ");

    // Detached branches are forcefully made anonymous, and it's something
    // we only know by examining `HEAD`.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── ►:0[0]:anon:
        └── 👉·541396b (⌂|1) ►tags/annotated, ►tags/release/v1, ►main
            └── ►:1[1]:other
                └── ·fafd9d0 (⌂|1)
    ");
    insta::assert_debug_snapshot!(graph, @r#"
    Graph {
        inner: StableGraph {
            Ty: "Directed",
            node_count: 2,
            edge_count: 1,
            edges: (0, 1),
            node weights: {
                0: Segment {
                    id: NodeIndex(0),
                    generation: 0,
                    ref_info: "None",
                    remote_tracking_ref_name: "None",
                    sibling_segment_id: "None",
                    commits: [
                        Commit(541396b, ⌂|1►annotated, ►release/v1, ►main),
                    ],
                    metadata: "None",
                },
                1: Segment {
                    id: NodeIndex(1),
                    generation: 1,
                    ref_info: "►other",
                    remote_tracking_ref_name: "None",
                    sibling_segment_id: "None",
                    commits: [
                        Commit(fafd9d0, ⌂|1),
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
            free_node: NodeIndex(4294967295),
            free_edge: EdgeIndex(4294967295),
        },
        entrypoint: Some(
            (
                NodeIndex(0),
                Some(
                    0,
                ),
            ),
        ),
        entrypoint_ref: None,
        extra_target: None,
        hard_limit_hit: false,
        options: Options {
            collect_tags: true,
            commits_limit_hint: None,
            commits_limit_recharge_location: [],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
        },
    }
    "#);

    assert!(
        graph.entrypoint_commit().map(|c| c.id).is_some(),
        "there is an entrypoint commit, detached or not"
    );
    assert!(
        graph.managed_entrypoint_commit(&repo)?.is_none(),
        "but it's not managed"
    );

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:DETACHED <> ✓!
    └── ≡:0:anon: {1}
        ├── :0:anon:
        │   └── ·541396b ►tags/annotated, ►tags/release/v1, ►main
        └── :1:other
            └── ·fafd9d0
    ");
    Ok(())
}

#[test]
fn main_advanced_remote_advanced() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("main-advanced-remote-advanced-two-shared")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 971953d (HEAD -> main) M2
    | * 5d29d62 (origin/main) RM1
    |/  
    * ce09734 M1
    * fafd9d0 init
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉►:0[0]:main[🌳] <> origin/main →:1:
    │   └── ·971953d (⌂|01)
    │       └── ►:2[1]:anon:
    │           ├── ·ce09734 (⌂|11)
    │           └── ·fafd9d0 (⌂|11)
    └── ►:1[0]:origin/main →:0:
        └── 🟣5d29d62 (0x0|10)
            └── →:2:
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main⇣1 on ce09734
    └── ≡:0:main[🌳] <> origin/main →:1:⇡1⇣1 on ce09734 {1}
        └── :0:main[🌳] <> origin/main →:1:⇡1⇣1
            ├── 🟣5d29d62
            └── ·971953d
    ");

    Ok(())
}

#[test]
fn only_remote_advanced() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("only-remote-advanced")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 085535d (origin/main) RM2
    * dd9f8d9 (origin/split-segment) RM1
    * 971953d (HEAD -> main) M2
    * ce09734 M1
    * fafd9d0 init
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── ►:1[0]:origin/main →:0:
        └── 🟣085535d (0x0|10)
            └── ►:2[1]:origin/split-segment
                └── 🟣dd9f8d9 (0x0|10)
                    └── 👉►:0[2]:main[🌳] <> origin/main →:1:
                        ├── ·971953d (⌂|11)
                        ├── ·ce09734 (⌂|11)
                        └── ·fafd9d0 (⌂|11)
    ");

    // TODO: it should detect that `main` has no own commits as it's fully integrated.
    //       This also affects the base which would have to be 085535d, the first commit.
    //       which is strange but maybe can work?
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main⇣2 on 971953d
    └── ≡:0:main[🌳] <> origin/main →:1:⇣1 {1}
        └── :0:main[🌳] <> origin/main →:1:⇣1
            └── 🟣085535d
    ");

    Ok(())
}

#[test]
fn only_remote_advanced_with_special_branch_name() -> anyhow::Result<()> {
    let (repo, meta) =
        read_only_in_memory_scenario("only-remote-advanced-with-special-branch-name")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 085535d (origin/main) RM2
    * dd9f8d9 (origin/split-segment) RM1
    * 971953d (HEAD -> main) M2
    * ce09734 (gitbutler/target) M1
    * fafd9d0 init
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── ►:1[0]:origin/main →:0:
        └── 🟣085535d (0x0|10)
            └── ►:3[1]:origin/split-segment
                └── 🟣dd9f8d9 (0x0|10)
                    └── 👉►:0[2]:main[🌳] <> origin/main →:1:
                        └── ·971953d (⌂|11)
                            └── ►:2[3]:gitbutler/target
                                ├── ·ce09734 (⌂|11)
                                └── ·fafd9d0 (⌂|11)
    ");

    // TODO: We'd actually have to recognise that the `origin/split-segment` branch
    //       isn't related to our stack and count its commits to `origin/main`.
    //       Right now we are missing dd9f8d9.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main⇣2 on 971953d
    └── ≡:0:main[🌳] <> origin/main →:1:⇣1 {1}
        └── :0:main[🌳] <> origin/main →:1:⇣1
            └── 🟣085535d
    ");

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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:main[🌳]
        └── ·c6c8c05 (⌂|1)
            ├── ►:1[1]:anon:
            │   └── ·76fc5c4 (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·e5d0542 (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·366d496 (⌂|1)
            └── ►:2[1]:C
                └── ·8631946 (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·00fab2a (⌂|1)
                    └── ►:6[2]:D
                        └── ·f4955b6 (⌂|1)
    ");
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
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            ├── ·c6c8c05
            ├── ·76fc5c4
            └── ·e5d0542
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── ·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

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

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:merged[🌳] <> ✓!
    └── ≡:0:merged[🌳] {1}
        ├── :0:merged[🌳]
        │   └── ·8a6c109
        ├── :1:A
        │   ├── ·62b409a
        │   └── ·592abec
        └── :7:main
            └── ·965998b
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉►:0[0]:B[🌳] <> origin/B →:1:
    │   └── ·312f819 (⌂|0001)
    │       └── ►:2[1]:A <> origin/A →:3:
    │           └── ·e255adc (⌂|0101)
    │               └── ►:4[2]:main
    │                   └── ·fafd9d0 (⌂|1111)
    └── ►:1[0]:origin/B →:0:
        └── 🟣682be32 (0x0|0010)
            └── ►:3[1]:origin/A →:2:
                └── 🟣e29c23d (0x0|1010)
                    └── →:4: (main)
    ");

    // 'main' is frozen because it connects to a 'foreign' remote, the commit was pushed.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:B[🌳] <> ✓refs/remotes/origin/B⇣2 on fafd9d0
    └── ≡:0:B[🌳] <> origin/B →:1:⇡1⇣1 on fafd9d0 {1}
        ├── :0:B[🌳] <> origin/B →:1:⇡1⇣1
        │   ├── 🟣682be32
        │   └── ·312f819
        └── :2:A <> origin/A →:3:⇡1⇣1
            ├── 🟣e29c23d
            └── ·e255adc
    ");

    // The hard limit is always respected though, despite yielding an incorrect result overall.
    // That's why it's the *hard* limit.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_hard_limit(7))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉►:0[0]:B[🌳] <> origin/B →:1:
    │   └── ·312f819 (⌂|001)
    │       └── ►:2[1]:A <> origin/A →:4:
    │           └── ❌·e255adc (⌂|101)
    ├── ►:1[0]:origin/B →:0:
    │   └── 🟣682be32 (0x0|010)
    │       └── ►:4[1]:origin/A →:2:
    │           └── ❌🟣e29c23d (0x0|010)
    └── ►:3[0]:origin/A
    ");
    // As the remotes don't connect, they are entirely unknown.
    // And if it's weird, it's due to the hard limit
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:B[🌳] <> ✓refs/remotes/origin/B⇣1 on 312f819
    └── ≡:0:B[🌳] <> origin/B →:1:⇣1 on e255adc {1}
        └── :0:B[🌳] <> origin/B →:1:⇣1
            └── 🟣682be32
    ");

    // Everything we encounter is checked for remotes (no limit)
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉►:0[0]:B[🌳] <> origin/B →:1:
    │   └── ·312f819 (⌂|0001)
    │       └── ►:2[1]:A <> origin/A →:3:
    │           └── ·e255adc (⌂|0101)
    │               └── ►:4[2]:main
    │                   └── ·fafd9d0 (⌂|1111)
    └── ►:1[0]:origin/B →:0:
        └── 🟣682be32 (0x0|0010)
            └── ►:3[1]:origin/A →:2:
                └── 🟣e29c23d (0x0|1010)
                    └── →:4: (main)
    ");

    // With a lower entrypoint, we don't see part of the graph.
    let (id, name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉►:0[0]:A <> origin/A →:1:
    │   └── ·e255adc (⌂|01)
    │       └── ►:2[1]:main
    │           └── ·fafd9d0 (⌂|11)
    └── ►:1[0]:origin/A →:0:
        └── 🟣e29c23d (0x0|10)
            └── →:2: (main)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:A <> ✓refs/remotes/origin/A⇣1 on fafd9d0
    └── ≡:0:A <> origin/A →:1:⇡1⇣1 on fafd9d0 {1}
        └── :0:A <> origin/A →:1:⇡1⇣1
            ├── 🟣e29c23d
            └── ·e255adc
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:C[🌳]
        └── ·2a95729 (⌂|1)
            ├── ►:1[1]:anon:
            │   ├── ·6861158 (⌂|1)
            │   ├── ·4f1f248 (⌂|1)
            │   └── ·487ffce (⌂|1)
            │       └── ►:4[2]:main
            │           ├── ·edc4dee (⌂|1)
            │           ├── ·01d0e1e (⌂|1)
            │           ├── ·4b3e5a8 (⌂|1)
            │           ├── ·34d0715 (⌂|1)
            │           └── ·eb5f731 (⌂|1)
            ├── ►:2[1]:A
            │   ├── ·20a823c (⌂|1)
            │   ├── ·442a12f (⌂|1)
            │   └── ·686706b (⌂|1)
            │       └── →:4: (main)
            └── ►:3[1]:B
                ├── ·9908c99 (⌂|1)
                ├── ·60d9a56 (⌂|1)
                └── ·9d171ff (⌂|1)
                    └── →:4: (main)
    ");
    // No limits list the first parent everywhere.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:C[🌳] <> ✓!
    └── ≡:0:C[🌳] {1}
        ├── :0:C[🌳]
        │   ├── ·2a95729
        │   ├── ·6861158
        │   ├── ·4f1f248
        │   └── ·487ffce
        └── :4:main
            ├── ·edc4dee
            ├── ·01d0e1e
            ├── ·4b3e5a8
            ├── ·34d0715
            └── ·eb5f731
    ");

    // There is no empty starting points, we always traverse the first commit as we really want
    // to get to remote processing there.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(0))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:C[🌳]
        └── ✂·2a95729 (⌂|1)
    ");
    // The cut by limit is also represented here.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:C[🌳] <> ✓!
    └── ≡:0:C[🌳] {1}
        └── :0:C[🌳]
            └── ✂️·2a95729
    ");

    // A single commit, the merge commit.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:C[🌳]
        └── ·2a95729 (⌂|1)
            ├── ►:1[1]:anon:
            │   └── ✂·6861158 (⌂|1)
            ├── ►:2[1]:A
            │   └── ✂·20a823c (⌂|1)
            └── ►:3[1]:B
                └── ✂·9908c99 (⌂|1)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:C[🌳] <> ✓!
    └── ≡:0:C[🌳] {1}
        └── :0:C[🌳]
            ├── ·2a95729
            └── ✂️·6861158
    ");

    // The merge commit, then we witness lane-duplication of the limit so we get more than requested.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(2))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:C[🌳]
        └── ·2a95729 (⌂|1)
            ├── ►:1[1]:anon:
            │   ├── ·6861158 (⌂|1)
            │   └── ✂·4f1f248 (⌂|1)
            ├── ►:2[1]:A
            │   ├── ·20a823c (⌂|1)
            │   └── ✂·442a12f (⌂|1)
            └── ►:3[1]:B
                ├── ·9908c99 (⌂|1)
                └── ✂·60d9a56 (⌂|1)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:C[🌳] <> ✓!
    └── ≡:0:C[🌳] {1}
        └── :0:C[🌳]
            ├── ·2a95729
            ├── ·6861158
            └── ✂️·4f1f248
    ");

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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:C[🌳]
        └── ·2a95729 (⌂|1)
            ├── ►:1[1]:anon:
            │   ├── ·6861158 (⌂|1)
            │   └── ✂·4f1f248 (⌂|1)
            ├── ►:2[1]:A
            │   ├── ·20a823c (⌂|1)
            │   ├── ·442a12f (⌂|1)
            │   └── ✂·686706b (⌂|1)
            └── ►:3[1]:B
                ├── ·9908c99 (⌂|1)
                └── ✂·60d9a56 (⌂|1)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:C[🌳] <> ✓!
    └── ≡:0:C[🌳] {1}
        └── :0:C[🌳]
            ├── ·2a95729
            ├── ·6861158
            └── ✂️·4f1f248
    ");

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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:C[🌳]
        └── ·2a95729 (⌂|1)
            ├── ►:1[1]:anon:
            │   ├── ·6861158 (⌂|1)
            │   ├── ·4f1f248 (⌂|1)
            │   └── ✂·487ffce (⌂|1)
            ├── ►:2[1]:A
            │   ├── ·20a823c (⌂|1)
            │   ├── ·442a12f (⌂|1)
            │   └── ·686706b (⌂|1)
            │       └── ►:4[2]:main
            │           ├── ·edc4dee (⌂|1)
            │           └── ✂·01d0e1e (⌂|1)
            └── ►:3[1]:B
                ├── ·9908c99 (⌂|1)
                ├── ·60d9a56 (⌂|1)
                └── ✂·9d171ff (⌂|1)
    ");
    insta::assert_debug_snapshot!(graph.statistics(), @r#"
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
        entrypoint: (
            NodeIndex(0),
            Some(
                0,
            ),
        ),
        segment_entrypoint_incoming: 0,
        segment_entrypoint_outgoing: 3,
        top_segments: [
            (
                Some(
                    FullName(
                        "refs/heads/C",
                    ),
                ),
                NodeIndex(0),
                Some(
                    CommitFlags(
                        NotInRemote | 0x8,
                    ),
                ),
            ),
        ],
        segments_at_bottom: 3,
        connections: 4,
        commits: 12,
        commit_references: 0,
        commits_at_cutoff: 3,
    }
    "#);

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:C[🌳] <> ✓!
    └── ≡:0:C[🌳] {1}
        └── :0:C[🌳]
            ├── ·2a95729
            ├── ·6861158
            ├── ·4f1f248
            └── ✂️·487ffce
    ");

    // We can specify any target, despite not having a workspace setup.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;

    // This limits the reach of the stack naturally.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:C[🌳]
        └── ·2a95729 (⌂|1)
            ├── ►:2[1]:anon:
            │   ├── ·6861158 (⌂|1)
            │   ├── ·4f1f248 (⌂|1)
            │   └── ·487ffce (⌂|1)
            │       └── ►:1[2]:main
            │           ├── ·edc4dee (⌂|✓|1)
            │           ├── ·01d0e1e (⌂|✓|1)
            │           ├── ·4b3e5a8 (⌂|✓|1)
            │           ├── ·34d0715 (⌂|✓|1)
            │           └── ·eb5f731 (⌂|✓|1)
            ├── ►:3[1]:A
            │   ├── ·20a823c (⌂|1)
            │   ├── ·442a12f (⌂|1)
            │   └── ·686706b (⌂|1)
            │       └── →:1: (main)
            └── ►:4[1]:B
                ├── ·9908c99 (⌂|1)
                ├── ·60d9a56 (⌂|1)
                └── ·9d171ff (⌂|1)
                    └── →:1: (main)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:C[🌳] <> ✓! on edc4dee
    └── ≡:0:C[🌳] on edc4dee {1}
        └── :0:C[🌳]
            ├── ·2a95729
            ├── ·6861158
            ├── ·4f1f248
            └── ·487ffce
    ");
    Ok(())
}

#[test]
fn special_branch_names_do_not_end_up_in_segment() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("special-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3686017 (HEAD -> main) top
    * 9725482 (gitbutler/edit) middle
    * fafd9d0 (gitbutler/target) init
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    // Standard handling after travrsal and post-processing.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:main[🌳]
        └── ·3686017 (⌂|1)
            └── ►:1[1]:gitbutler/edit
                └── ·9725482 (⌂|1)
                    └── ►:2[2]:gitbutler/target
                        └── ·fafd9d0 (⌂|1)
    ");

    // But special handling for workspace views.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            ├── ·3686017
            ├── ·9725482
            └── ·fafd9d0
    ");
    Ok(())
}

#[test]
fn ambiguous_worktrees() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("ambiguous-worktrees")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 85efbe4 (HEAD -> main, wt-outside-ambiguous-worktree, wt-inside-ambiguous-worktree) M");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:main[🌳]
        └── ·85efbe4 (⌂|1) ►wt-inside-ambiguous-worktree[📁], ►wt-outside-ambiguous-worktree[📁]
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            └── ·85efbe4 ►wt-inside-ambiguous-worktree[📁], ►wt-outside-ambiguous-worktree[📁]
    ");
    Ok(())
}

mod with_workspace;

mod utils;
pub use utils::{
    StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev,
    read_only_in_memory_scenario, standard_options,
};

use crate::init::utils::standard_options_with_extra_target;
