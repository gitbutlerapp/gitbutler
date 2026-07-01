use but_graph::{CommitFlags, Graph, StopCondition, init::Tip};
use but_testsupport::{
    gix_testtools::{self, Creation, rust_fixture_writable},
    graph_tree, graph_workspace, visualize_commit_graph_all,
};

#[test]
fn unborn() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @"

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
                    remote_tracking_ref_name: None,
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
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
                Unborn,
            ),
        ),
        entrypoint_ref: None,
        traversal_tips: [],
        hard_limit_hit: false,
        options: Options {
            collect_tags: false,
            commits_limit_hint: None,
            commits_limit_recharge_location: [],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
        },
        project_meta: ProjectMeta {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        symbolic_remote_names: [],
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 541396b (HEAD, tag: release/v1, tag: annotated, main) first
    * fafd9d0 (other) init
    ");

    // Detached branches are forcefully made anonymous, and it's something
    // we only know by examining `HEAD`.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:0[0]:anon:
        └── 👉·541396b (⌂|1) ►tags/annotated, ►tags/release/v1, ►main
            └── ►:1[1]:other
                └── 🏁·fafd9d0 (⌂|1)
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
                    ref_info: None,
                    remote_tracking_ref_name: None,
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
                    commits: [
                        Commit(541396b, ⌂|1►annotated, ►release/v1, ►main),
                    ],
                    metadata: "None",
                },
                1: Segment {
                    id: NodeIndex(1),
                    generation: 1,
                    ref_info: "►other",
                    remote_tracking_ref_name: None,
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
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
                    parent_order: 0,
                },
            },
            free_node: NodeIndex(4294967295),
            free_edge: EdgeIndex(4294967295),
        },
        entrypoint: Some(
            (
                NodeIndex(0),
                AtCommit(
                    Sha1(541396b24e13b8ac45b7905c3fe8691c7fc5fbd0),
                ),
            ),
        ),
        entrypoint_ref: None,
        traversal_tips: [
            Tip {
                id: Sha1(541396b24e13b8ac45b7905c3fe8691c7fc5fbd0),
                ref_name: None,
                role: Reachable,
                metadata: None,
                is_entrypoint: true,
                is_detached: false,
            },
        ],
        hard_limit_hit: false,
        options: Options {
            collect_tags: true,
            commits_limit_hint: None,
            commits_limit_recharge_location: [],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
        },
        project_meta: ProjectMeta {
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        },
        symbolic_remote_names: [],
    }
    "#);

    assert!(
        graph.entrypoint()?.commit().map(|c| c.id).is_some(),
        "there is an entrypoint commit, detached or not"
    );
    assert!(
        graph.managed_entrypoint_commit(&repo)?.is_none(),
        "but it's not managed"
    );
    let root_sidx = graph
        .base_segments()
        .find(|sidx| {
            graph[*sidx]
                .commits
                .last()
                .is_some_and(|commit| commit.parent_ids.is_empty())
        })
        .expect("root segment is present");
    assert_eq!(
        graph.stop_condition(root_sidx),
        Some(StopCondition::FirstCommit)
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
fn shallow_clone_stops_at_shallow_boundary() -> anyhow::Result<()> {
    let (repo, meta) =
        utils::named_read_only_in_memory_scenario("special-conditions", "shallow-clone-depth-2")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 71a64f3 (HEAD -> main, origin/main, origin/HEAD) commit 4
    * 62d65ed (grafted) commit 3
    ");

    let shallow_commits = repo.shallow_commits()?.expect("clone is shallow");
    let shallow_boundary_id = shallow_commits.head;
    assert!(
        shallow_commits.tail.is_empty(),
        "the linear depth-2 clone should have exactly one shallow boundary"
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:1[0]:origin/main →:0:
        └── 👉►:0[1]:main[🌳] <> origin/main →:1:
            ├── ·71a64f3 (⌂|1)
            └── ⛰·62d65ed (⌂|⛰|1)
    ");
    let (boundary_sidx, boundary_cidx) = graph
        .segments()
        .find_map(|sidx| {
            graph[sidx]
                .commits
                .iter()
                .position(|commit| commit.id == shallow_boundary_id)
                .map(|cidx| (sidx, cidx))
        })
        .expect("boundary commit is included in the graph");
    let boundary_commit = &graph[boundary_sidx].commits[boundary_cidx];
    assert!(
        boundary_commit.flags.contains(CommitFlags::ShallowBoundary),
        "the boundary commit is explicitly flagged"
    );
    let missing_parent = boundary_commit
        .parent_ids
        .first()
        .copied()
        .expect("shallow boundary commit still records its grafted parent");
    assert!(
        graph.segments().all(|sidx| graph[sidx]
            .commits
            .iter()
            .all(|commit| commit.id != missing_parent)),
        "the grafted parent is not traversed"
    );

    let condition = graph
        .stop_condition(boundary_sidx)
        .expect("boundary segment has a cutoff condition");
    assert!(condition.contains(StopCondition::ShallowBoundary));
    assert!(!condition.contains(StopCondition::Limit));
    assert!(!condition.contains(StopCondition::FirstCommit));

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main on 71a64f3
    └── ≡:0:main[🌳] <> origin/main →:1: {1}
        └── :0:main[🌳] <> origin/main →:1:
    ");
    Ok(())
}

#[test]
fn merge_first_parent_older_non_workspace_maintains_graph_order() -> anyhow::Result<()> {
    let (repo, meta) = utils::named_read_only_in_memory_scenario(
        "special-conditions",
        "merge-first-parent-older",
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 738ea18 (HEAD -> first-parent) commit on top of merge
    *   408ca26 merge second-parent into first-parent
    |\  
    | * 75369b0 (second-parent) new commit 3 on second-parent
    | * 553bbf7 new commit 2 on second-parent
    | * 72614bb new commit 1 on second-parent
    * | 2854fa2 old commit on first-parent
    |/  
    * 793a434 (tag: base, main) base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:first-parent[🌳]
        └── ·738ea18 (⌂|1)
            └── ►:1[1]:anon:
                └── ·408ca26 (⌂|1)
                    ├── ►:3[2]:anon:
                    │   └── ·2854fa2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── 🏁·793a434 (⌂|1) ►tags/base
                    └── ►:2[2]:second-parent
                        ├── ·75369b0 (⌂|1)
                        ├── ·553bbf7 (⌂|1)
                        └── ·72614bb (⌂|1)
                            └── →:4: (main)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), "we see only first-parent with two commits, not the 'second-parent' ref because it *seems* to be traversed first", @"
    ⌂:0:first-parent[🌳] <> ✓!
    └── ≡:0:first-parent[🌳] {1}
        ├── :0:first-parent[🌳]
        │   ├── ·738ea18
        │   ├── ·408ca26
        │   └── ·2854fa2
        └── :4:main
            └── ·793a434 ►tags/base
    ");
    Ok(())
}

#[test]
fn main_advanced_remote_advanced() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("main-advanced-remote-advanced-two-shared")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 971953d (HEAD -> main) M2
    | * 5d29d62 (origin/main) RM1
    |/  
    * ce09734 M1
    * fafd9d0 init
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉►:0[0]:main[🌳] <> origin/main →:1:
    │   └── ·971953d (⌂|01)
    │       └── ►:2[1]:anon:
    │           ├── ·ce09734 (⌂|11)
    │           └── 🏁·fafd9d0 (⌂|11)
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 085535d (origin/main) RM2
    * dd9f8d9 (origin/split-segment) RM1
    * 971953d (HEAD -> main) M2
    * ce09734 M1
    * fafd9d0 init
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:1[0]:origin/main →:0:
        └── 🟣085535d (0x0|10)
            └── ►:2[1]:origin/split-segment
                └── 🟣dd9f8d9 (0x0|10)
                    └── 👉►:0[2]:main[🌳] <> origin/main →:1:
                        ├── ·971953d (⌂|11)
                        ├── ·ce09734 (⌂|11)
                        └── 🏁·fafd9d0 (⌂|11)
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 085535d (origin/main) RM2
    * dd9f8d9 (origin/split-segment) RM1
    * 971953d (HEAD -> main) M2
    * ce09734 (gitbutler/target) M1
    * fafd9d0 init
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:1[0]:origin/main →:0:
        └── 🟣085535d (0x0|10)
            └── ►:3[1]:origin/split-segment
                └── 🟣dd9f8d9 (0x0|10)
                    └── 👉►:0[2]:main[🌳] <> origin/main →:1:
                        └── ·971953d (⌂|11)
                            └── ►:2[3]:gitbutler/target
                                ├── ·ce09734 (⌂|11)
                                └── 🏁·fafd9d0 (⌂|11)
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:main[🌳]
        └── ·c6c8c05 (⌂|1)
            ├── ►:1[1]:anon:
            │   └── ·76fc5c4 (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── 🏁·e5d0542 (⌂|1)
            │       └── ►:4[2]:B
            │           └── 🏁·366d496 (⌂|1)
            └── ►:2[1]:C
                └── ·8631946 (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── 🏁·00fab2a (⌂|1)
                    └── ►:6[2]:D
                        └── 🏁·f4955b6 (⌂|1)
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂|1)
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
fn explicit_traversal_tips_reject_duplicate_traversal_seeds() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(merged_id, None),
            Tip::reachable(a_id, None),
            Tip::reachable(a_id, Some(a_ref)),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("duplicate traversal seeds must be rejected");

    assert!(
        err.to_string()
            .starts_with("explicit traversal tips contain duplicate traversal seed Tip"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn explicit_traversal_tips_allow_overlapping_commit_ids() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    let main_id = id_by_rev(&repo, "main").detach();
    let main_ref = ref_name("refs/heads/main");
    let release_tag = ref_name("refs/tags/release/v1");

    let graph = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(main_id, Some(main_ref)),
            Tip::reachable(main_id, Some(release_tag)),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:0[0]:tags/release/v1
        └── 👉►:1[1]:main
            └── ·541396b (⌂|1) ►tags/annotated, ►tags/release/v1
                └── ►:2[2]:other
                    └── 🏁·fafd9d0 (⌂|1)
    ");
    Ok(())
}

#[test]
fn explicit_traversal_tips_allow_named_and_anonymous_integrated_targets_on_same_commit()
-> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let main_id = id_by_rev(&repo, "main").detach();

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

    let graph = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(merged_id, Some(ref_name("refs/heads/merged"))),
            Tip::integrated(main_id, Some(ref_name("refs/heads/main"))),
            Tip::integrated(main_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    insta::assert_snapshot!(graph_tree(&graph), "anonymous target context with the same commit collapses into the named target ref", @"

    └── 👉►:1[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:2[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:4[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:0[3]:main
            │       │           └── 🏁·965998b (⌂|✓|1)
            │       └── ►:5[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:0: (main)
            └── ►:3[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:6[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:0: (main)
                    └── ►:7[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:0: (main)
    ");
    Ok(())
}

#[test]
fn explicit_traversal_tips_reject_multiple_entrypoints() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();

    let err = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(merged_id, None),
            Tip::entrypoint(a_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("multiple entrypoints must be rejected");

    assert_eq!(
        err.to_string(),
        "explicit traversal tips require exactly one entrypoint"
    );
    Ok(())
}

#[test]
fn explicit_traversal_tips_reject_duplicate_ref_names() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let a_id = id_by_rev(&repo, "A").detach();
    let c_id = id_by_rev(&repo, "C").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(a_id, Some(a_ref.clone())),
            Tip::reachable(c_id, Some(a_ref.clone())),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("duplicate ref names must be rejected");

    assert_eq!(
        err.to_string(),
        format!("explicit traversal tips contain duplicate ref name {a_ref}")
    );
    Ok(())
}

#[test]
fn explicit_traversal_tips_reject_detached_entrypoint_with_ref_name() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();

    let err = Graph::from_commit_traversal_tips(
        &repo,
        [Tip::new(merged_id)
            .with_ref_name(Some(ref_name("refs/heads/merged")))
            .with_entrypoint()
            .with_is_detached(true)],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("detached entrypoints must not be named");

    assert_eq!(
        err.to_string(),
        "explicit detached entrypoint tip cannot have a ref name"
    );
    Ok(())
}

#[test]
fn explicit_traversal_tips_reject_ref_names_that_point_elsewhere() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Graph::from_commit_traversal_tips(
        &repo,
        [Tip::entrypoint(merged_id, Some(a_ref.clone()))],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("ref names must resolve to their tip id");

    assert_eq!(
        err.to_string(),
        format!("explicit traversal tip ref {a_ref} points to {a_id}, not {merged_id}")
    );
    Ok(())
}

#[test]
fn traversal_entrypoint_ref_override_must_point_to_entrypoint() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Graph::from_commit_traversal(
        id_by_rev(&repo, "merged"),
        Some(a_ref.clone()),
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("entrypoint ref override must resolve to the entrypoint id");

    assert_eq!(
        err.to_string(),
        format!("explicit traversal entrypoint ref {a_ref} points to {a_id}, not {merged_id}")
    );
    Ok(())
}

#[test]
fn explicit_traversal_tips_use_integrated_tip_as_workspace_target_commit() -> anyhow::Result<()> {
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

    let merged_id = id_by_rev(&repo, "merged").detach();
    let target_ref_name = ref_name("refs/heads/A");
    let target_ref_id = id_by_rev(&repo, "A").detach();
    let target_commit_id = id_by_rev(&repo, "main").detach();
    let graph = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(merged_id, Some(ref_name("refs/heads/merged"))),
            Tip::integrated(target_ref_id, Some(target_ref_name.clone())),
            Tip::integrated(target_commit_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:2[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:0[1]:A
            │   └── ·62b409a (⌂|✓|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|✓|1)
            │       │       └── ►:1[3]:main
            │       │           └── 🏁·965998b (⌂|✓|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|✓|1)
            │               └── →:1: (main)
            └── ►:5[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:6[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:1: (main)
                    └── ►:7[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:1: (main)
    ");

    let target_segment = graph.segment_by_commit_id(target_commit_id)?;
    assert_eq!(
        target_segment.commits.first().map(|commit| commit.id),
        Some(target_commit_id),
        "integrated tip is also split into its own segment"
    );

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:2:merged[🌳] <> ✓refs/heads/A⇣3 on 965998b
    └── ≡:2:merged[🌳] on 965998b {1}
        ├── :2:merged[🌳]
        │   └── ·8a6c109
        └── :0:A
            ├── ·62b409a (✓)
            └── ·592abec (✓)
    ");
    assert_eq!(
        ws.target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref()),
        Some(target_ref_name.as_ref()),
        "workspace projection uses named integrated tips as target refs if no metadata is available"
    );
    assert_eq!(
        ws.target_commit.as_ref().map(|target| target.commit_id),
        Some(target_commit_id),
        "workspace projection falls back to using integrated refs"
    );
    Ok(())
}

#[test]
fn stacked_rebased_remotes() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("remote-includes-another-remote")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 682be32 (origin/B) B
    * e29c23d (origin/A) A
    | * 312f819 (HEAD -> B) B
    | * e255adc (A) A
    |/  
    * fafd9d0 (main) init
    ");

    // A remote will always be able to find their non-remotes so they don't seem cut-off.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉►:0[0]:B[🌳] <> origin/B →:1:
    │   └── ·312f819 (⌂|0001)
    │       └── ►:2[1]:A <> origin/A →:3:
    │           └── ·e255adc (⌂|0101)
    │               └── ►:4[2]:main
    │                   └── 🏁·fafd9d0 (⌂|1111)
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

    // The hard limit stops queueing deeper commits, but queued commits are still processed
    // so existing work can complete its graph connections.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_hard_limit(5),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉►:0[0]:B[🌳] <> origin/B →:1:
    │   └── ·312f819 (⌂|001)
    │       └── ►:2[1]:A <> origin/A →:5:
    │           └── ❌·e255adc (⌂|101)
    ├── ►:1[0]:origin/B →:0:
    │   └── 🟣682be32 (0x0|010)
    │       └── ►:5[1]:origin/A →:2:
    │           └── 🟣e29c23d (0x0|010)
    │               └── ►:4[2]:main
    │                   └── 🏁🟣fafd9d0 (0x0|010)
    └── ►:3[0]:origin/A
    ");
    assert!(
        graph.hard_limit_hit(),
        "graph should record that traversal stopped queueing after hitting the hard limit"
    );
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:B[🌳] <> ✓refs/remotes/origin/B⇣1 on 312f819
    └── ≡:0:B[🌳] <> origin/B →:1:⇣1 on e255adc {1}
        └── :0:B[🌳] <> origin/B →:1:⇣1
            └── 🟣682be32
    ");

    // Everything we encounter is checked for remotes (no limit)
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉►:0[0]:B[🌳] <> origin/B →:1:
    │   └── ·312f819 (⌂|0001)
    │       └── ►:2[1]:A <> origin/A →:3:
    │           └── ·e255adc (⌂|0101)
    │               └── ►:4[2]:main
    │                   └── 🏁·fafd9d0 (⌂|1111)
    └── ►:1[0]:origin/B →:0:
        └── 🟣682be32 (0x0|0010)
            └── ►:3[1]:origin/A →:2:
                └── 🟣e29c23d (0x0|1010)
                    └── →:4: (main)
    ");

    // With a lower entrypoint, we don't see part of the graph.
    let (id, name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        name,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉►:0[0]:A <> origin/A →:1:
    │   └── ·e255adc (⌂|01)
    │       └── ►:2[1]:main
    │           └── 🏁·fafd9d0 (⌂|11)
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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

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
            │           └── 🏁·eb5f731 (⌂|1)
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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(0),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

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

    // Hitting the hard limit while queueing merge parents still queues the
    // complete parent set. The hard limit only prevents traversal beyond them.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_hard_limit(2),
    )?
    .validated()?;
    assert!(
        graph.hard_limit_hit(),
        "graph should record that traversal stopped queueing after hitting the hard limit"
    );
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:C[🌳]
        └── ·2a95729 (⌂|1)
            ├── ►:1[1]:anon:
            │   └── ❌·6861158 (⌂|1)
            ├── ►:2[1]:A
            │   └── ❌·20a823c (⌂|1)
            └── ►:3[1]:B
                └── ❌·9908c99 (⌂|1)
    ");

    // The merge commit, then we witness lane-duplication of the limit so we get more than requested.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(2),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

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
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options()
            .with_limit_hint(2)
            .with_limit_extension_at(Some(id_by_rev(&repo, ":/A3").detach())),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

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
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options()
            .with_limit_hint(2)
            .with_limit_extension_at([id(":/A3"), id(":/A1"), id(":/B3"), id(":/C3")]),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

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
                        NotInRemote | 0x10,
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
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;

    // This limits the reach of the stack naturally.
    insta::assert_snapshot!(graph_tree(&graph), @"

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
            │           └── 🏁·eb5f731 (⌂|✓|1)
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 3686017 (HEAD -> main) top
    * 9725482 (gitbutler/edit) middle
    * fafd9d0 (gitbutler/target) init
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    // Standard handling after travrsal and post-processing.
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:main[🌳]
        └── ·3686017 (⌂|1)
            └── ►:1[1]:gitbutler/edit
                └── ·9725482 (⌂|1)
                    └── ►:2[2]:gitbutler/target
                        └── 🏁·fafd9d0 (⌂|1)
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:main[🌳@repo]
        └── 🏁·85efbe4 (⌂|1) ►wt-inside-ambiguous-worktree[📁], ►wt-outside-ambiguous-worktree[📁]
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳@repo] <> ✓!
    └── ≡:0:main[🌳@repo] {1}
        └── :0:main[🌳@repo]
            └── ·85efbe4 ►wt-inside-ambiguous-worktree[📁], ►wt-outside-ambiguous-worktree[📁]
    ");

    let linked_repo = gix::open_opts(
        repo.path()
            .parent()
            .expect("repository git dir is inside the worktree")
            .join("wt-inside-ambiguous-worktree"),
        gix::open::Options::isolated(),
    )?
    .with_object_memory();
    let graph = Graph::from_head(
        &linked_repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), "when the graph is built from the linked worktree repository, it can't see anything else without metadata", @"

    └── 👉►:0[0]:wt-inside-ambiguous-worktree[📁@repo]
        └── 🏁·85efbe4 (⌂|1) ►main[🌳], ►wt-outside-ambiguous-worktree[📁]
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), "workspace debug output should preserve that the linked worktree, not the main worktree, is owned by the repository used to build the graph", @"
    ⌂:0:wt-inside-ambiguous-worktree[📁@repo] <> ✓!
    └── ≡:0:wt-inside-ambiguous-worktree[📁@repo] {1}
        └── :0:wt-inside-ambiguous-worktree[📁@repo]
            └── ·85efbe4 ►main[🌳], ►wt-outside-ambiguous-worktree[📁]
    ");
    Ok(())
}

#[test]
fn commit_with_two_parents() -> anyhow::Result<()> {
    let (tmp, repo) = rust_fixture_writable("empty", 2, Creation::Execute, |fixture| {
        let open_opts = but_testsupport::open_repo_config()?;
        Ok(match fixture {
            FixtureState::Uninitialized(path) => gix::ThreadSafeRepository::init_opts(
                path,
                gix::create::Kind::WithWorktree,
                gix::create::Options::default(),
                open_opts,
            )?
            .to_thread_local(),
            FixtureState::Fresh(path) => gix::open_opts(path, open_opts)?,
        })
    })
    .map_err(anyhow::Error::from_boxed)?;

    let first_commit = repo.commit(
        "HEAD",
        "base",
        repo.object_hash().empty_tree(),
        None::<gix::ObjectId>,
    )?;
    let same_parent_twice = [first_commit.detach(), first_commit.into()];
    repo.commit(
        "HEAD",
        "commit with the same parent ('base') duplicated",
        repo.object_hash().empty_tree(),
        same_parent_twice,
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 06470d7 (HEAD -> main) commit with the same parent ('base') duplicated
    |\
    * 86719d5 base
    ");

    let meta = in_memory_meta(tmp.path())?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    // Duplicate parent commits are kept verbatim.
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:main[🌳]
        └── ·06470d7 (⌂|1)
            ├── ►:1[1]:anon:
            │   └── 🏁·86719d5 (⌂|1)
            └── →:1:
    ");
    Ok(())
}

mod overlay;
mod with_workspace;

pub(crate) mod utils;
use gix_testtools::FixtureState;
pub use utils::{
    StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev,
    read_only_in_memory_scenario, standard_options,
};

use crate::init::utils::{in_memory_meta, standard_options_with_extra_target};

fn ref_name(name: &str) -> gix::refs::FullName {
    name.try_into().expect("valid full ref name")
}
