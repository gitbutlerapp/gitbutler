use but_graph::{
    CommitFlags, Graph, NodeGraphEntrypoint, NodeKind, StopCondition,
    init::{Overlay, Tip},
};
use but_testsupport::{
    gix_testtools::{self, Creation, rust_fixture_writable},
    graph_workspace, visualize_commit_graph_all,
};
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

use but_testsupport::graph_tree;

#[test]
fn unborn() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉main[🌳]

"#]]
    );

    assert!(
        graph.managed_workspace_commit_id().is_none(),
        "there is no commit it could return"
    );
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace.id, None,
        "an unborn workspace has no backing graph node"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
⌂:-:main <> ✓!

"#]]
    );

    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 541396b (HEAD, tag: release/v1, tag: annotated, main) first
* fafd9d0 (other) init

"#]]
    );

    // Detached branches are forcefully made anonymous, and it's something
    // we only know by examining `HEAD`.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main
│ ◎  tags/annotated
├─╯
│ ◎  tags/release/v1
├─╯
●  👉·541396b (⌂)
◎  other
●  🏁·fafd9d0 (⌂)

"#]]
    );

    assert!(
        matches!(
            graph.entrypoint(),
            NodeGraphEntrypoint::Node(index)
                if matches!(graph.nodes()[*index].kind(), NodeKind::Commit { .. })
        ),
        "there is an entrypoint commit, detached or not"
    );
    assert!(
        graph.managed_workspace_commit_id().is_none(),
        "but it's not managed"
    );
    assert!(
        graph.nodes().iter().any(|node| matches!(
            node.kind(),
            NodeKind::Commit { .. } if node.parents().is_empty()
        )),
        "root commit node is present"
    );

    let workspace = graph.into_workspace()?;
    let entrypoint_segment = workspace
        .stacks
        .first()
        .and_then(|stack| stack.segments.first())
        .expect("detached HEAD projects to an anonymous stack segment");
    assert!(
        entrypoint_segment.is_entrypoint && entrypoint_segment.ref_info.is_none(),
        "a detached commit entrypoint must stay anonymous"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
⌂:0:DETACHED <> ✓!
└── ≡👉:0:anon {1}
    ├── 👉:0:anon
    │   └── ·541396b ►main, ►annotated, ►release/v1
    └── :5:other
        └── ·fafd9d0

"#]]
    );
    Ok(())
}

#[test]
fn shallow_clone_stops_at_shallow_boundary() -> anyhow::Result<()> {
    let (repo, meta) =
        utils::named_read_only_in_memory_scenario("special-conditions", "shallow-clone-depth-2")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 71a64f3 (HEAD -> main, origin/main, origin/HEAD) commit 4
* 62d65ed (grafted) commit 3

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  origin/main
◎  👉main[🌳] <> origin/main
●  ·71a64f3 (⌂)
●  ⛰·62d65ed (⌂|⛰)

"#]]
    );
    let (boundary_index, boundary_node) = graph
        .node_by_commit_id(shallow_boundary_id)
        .expect("boundary commit is included in the graph");
    assert!(
        graph.annotations()[boundary_index].contains(CommitFlags::ShallowBoundary),
        "the boundary commit is explicitly flagged"
    );
    let (missing_parent, condition) = boundary_node
        .parents()
        .iter()
        .find_map(|parent| match graph.nodes()[*parent].kind() {
            NodeKind::ShallowPoint { id, reason } => Some((*id, *reason)),
            _ => None,
        })
        .expect("shallow boundary commit has an omitted-parent sentinel");
    assert!(
        graph.node_by_commit_id(missing_parent).is_none(),
        "the grafted parent is not traversed"
    );
    assert!(condition.contains(StopCondition::ShallowBoundary));
    assert!(!condition.contains(StopCondition::Limit));
    assert!(!condition.contains(StopCondition::FirstCommit));

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:3:main <> ✓!
└── ≡👉:3:main[🌳] <> origin/main →:4: on 0847f69 {1}
    └── 👉:3:main[🌳] <> origin/main →:4:
        ├── ❄71a64f3 ►origin/main
        └── ✂️❄62d65ed (⛰)

"#]]
    );
    Ok(())
}

#[test]
fn skipping_postprocessing_keeps_commit_graph_and_traversal_boundaries() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    let normal = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    assert!(
        normal
            .nodes()
            .iter()
            .any(|node| matches!(node.kind(), NodeKind::Reference(_))),
        "normal postprocessing discovers repository references"
    );

    let skipped = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options {
            dangerously_skip_postprocessing_for_debugging: true,
            ..standard_options()
        },
    )?
    .validated()?;
    assert!(
        skipped
            .nodes()
            .iter()
            .all(|node| !matches!(node.kind(), NodeKind::Reference(_))),
        "skipping postprocessing adapts only commit nodes"
    );

    let (repo, meta) = read_only_in_memory_scenario("triple-merge")?;
    let limited = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options {
            dangerously_skip_postprocessing_for_debugging: true,
            ..standard_options().with_hard_limit(2)
        },
    )?
    .validated()?;
    assert!(
        limited.hard_limit_hit(),
        "skipping postprocessing preserves the hard-limit result"
    );
    assert!(
        limited.nodes().iter().any(|node| matches!(
            node.kind(),
            NodeKind::ShallowPoint { reason, .. } if reason.contains(StopCondition::Limit)
        )),
        "skipping postprocessing preserves limit boundaries"
    );

    let (repo, meta) =
        utils::named_read_only_in_memory_scenario("special-conditions", "shallow-clone-depth-2")?;
    let shallow = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options {
            dangerously_skip_postprocessing_for_debugging: true,
            ..standard_options()
        },
    )?
    .validated()?;
    assert!(
        shallow
            .annotations()
            .iter()
            .any(|flags| flags.contains(CommitFlags::ShallowBoundary)),
        "skipping postprocessing preserves shallow commit annotations"
    );
    assert!(
        shallow.nodes().iter().any(|node| matches!(
            node.kind(),
            NodeKind::ShallowPoint { reason, .. }
                if reason.contains(StopCondition::ShallowBoundary)
        )),
        "skipping postprocessing preserves shallow boundaries"
    );
    Ok(())
}

#[test]
fn merge_first_parent_older_non_workspace_maintains_graph_order() -> anyhow::Result<()> {
    let (repo, meta) = utils::named_read_only_in_memory_scenario(
        "special-conditions",
        "merge-first-parent-older",
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 738ea18 (HEAD -> first-parent) commit on top of merge
*   408ca26 merge second-parent into first-parent
|\  
| * 75369b0 (second-parent) new commit 3 on second-parent
| * 553bbf7 new commit 2 on second-parent
| * 72614bb new commit 1 on second-parent
* | 2854fa2 old commit on first-parent
|/  
* 793a434 (tag: base, main) base

"#]]
        .raw()
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉first-parent[🌳]
●  ·738ea18 (⌂)
●    ·408ca26 (⌂)
├─╮
● │  ·2854fa2 (⌂)
│ ◎  second-parent
│ ●  ·75369b0 (⌂)
│ ●  ·553bbf7 (⌂)
│ ●  ·72614bb (⌂)
├─╯
│ ◎  main
├─╯
│ ◎  tags/base
├─╯
●  🏁·793a434 (⌂)

"#]]
    );

    // we see only first-parent with two commits, not the 'second-parent' ref because it *seems* to be traversed first
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:7:first-parent <> ✓!
└── ≡👉:7:first-parent[🌳] {1}
    └── 👉:7:first-parent[🌳]
        ├── ·738ea18
        ├── ·408ca26
        ├── ·2854fa2
        └── ·793a434 ►main, ►base

"#]]
    );
    Ok(())
}

#[test]
fn main_advanced_remote_advanced() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("main-advanced-remote-advanced-two-shared")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 971953d (HEAD -> main) M2
| * 5d29d62 (origin/main) RM1
|/  
* ce09734 M1
* fafd9d0 init

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉main[🌳] <> origin/main
●  ·971953d (⌂)
│ ◎  origin/main
│ ●  🟣5d29d62
├─╯
●  ·ce09734 (⌂)
●  🏁·fafd9d0 (⌂)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:4:main <> ✓!
└── ≡👉:4:main[🌳] <> origin/main →:5:⇡1⇣1 {1}
    └── 👉:4:main[🌳] <> origin/main →:5:⇡1⇣1
        ├── 🟣5d29d62 ►origin/main
        ├── ·971953d
        ├── ❄ce09734
        └── ❄fafd9d0

"#]]
    );

    Ok(())
}

#[test]
fn only_remote_advanced() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("only-remote-advanced")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 085535d (origin/main) RM2
* dd9f8d9 (origin/split-segment) RM1
* 971953d (HEAD -> main) M2
* ce09734 M1
* fafd9d0 init

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  origin/main
●  🟣085535d
◎  origin/split-segment
●  🟣dd9f8d9
◎  👉main[🌳] <> origin/main
●  ·971953d (⌂)
●  ·ce09734 (⌂)
●  🏁·fafd9d0 (⌂)

"#]]
    );

    // TODO: it should detect that `main` has no own commits as it's fully integrated.
    //       This also affects the base which would have to be 085535d, the first commit.
    //       which is strange but maybe can work?
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:5:main <> ✓!
└── ≡👉:5:main[🌳] <> origin/main →:6:⇣2 {1}
    └── 👉:5:main[🌳] <> origin/main →:6:⇣2
        ├── 🟣085535d ►origin/main
        ├── 🟣dd9f8d9 ►origin/split-segment
        ├── ❄971953d
        ├── ❄ce09734
        └── ❄fafd9d0

"#]]
    );

    Ok(())
}

#[test]
fn only_remote_advanced_with_special_branch_name() -> anyhow::Result<()> {
    let (repo, meta) =
        read_only_in_memory_scenario("only-remote-advanced-with-special-branch-name")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 085535d (origin/main) RM2
* dd9f8d9 (origin/split-segment) RM1
* 971953d (HEAD -> main) M2
* ce09734 (gitbutler/target) M1
* fafd9d0 init

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  origin/main
●  🟣085535d
◎  origin/split-segment
●  🟣dd9f8d9
◎  👉main[🌳] <> origin/main
●  ·971953d (⌂)
◎  gitbutler/target
●  ·ce09734 (⌂)
●  🏁·fafd9d0 (⌂)

"#]]
    );

    // TODO: We'd actually have to recognise that the `origin/split-segment` branch
    //       isn't related to our stack and count its commits to `origin/main`.
    //       Right now we are missing dd9f8d9.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:5:main <> ✓!
└── ≡👉:5:main[🌳] <> origin/main →:7:⇣2 {1}
    └── 👉:5:main[🌳] <> origin/main →:7:⇣2
        ├── 🟣085535d ►origin/main
        ├── 🟣dd9f8d9 ►origin/split-segment
        ├── ❄971953d
        ├── ❄ce09734 ►gitbutler/target
        └── ❄fafd9d0

"#]]
    );

    Ok(())
}

#[test]
fn multi_root() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("multi-root")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉main[🌳]
●    ·c6c8c05 (⌂)
├─╮
● │    ·76fc5c4 (⌂)
├───╮
● │ │  🏁·e5d0542 (⌂)
  │ ◎  B
  │ ●  🏁·366d496 (⌂)
  ◎  C
  ●  ·8631946 (⌂)
╭─┤
│ ●  🏁·00fab2a (⌂)
◎  D
●  🏁·f4955b6 (⌂)

"#]]
    );
    assert_eq!(
        graph
            .nodes()
            .iter()
            .filter(|node| matches!(
                node.kind(),
                NodeKind::Commit { .. } if node.parents().is_empty()
            ))
            .count(),
        4,
        "there are 4 orphaned bases"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:7:main <> ✓!
└── ≡👉:7:main[🌳] {1}
    └── 👉:7:main[🌳]
        ├── ·c6c8c05
        ├── ·76fc5c4
        └── ·e5d0542

"#]]
    );
    Ok(())
}

#[test]
fn four_diamond() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main
│ ◎  👉merged[🌳]
│ ●    ·8a6c109 (⌂)
│ ├─╮
│ ◎ │  A
│ ● │    ·62b409a (⌂)
│ ├───╮
│ ● │ │  ·592abec (⌂)
├─╯ │ │
│   │ ◎  B
│   │ ●  ·f16dddf (⌂)
├─────╯
│   ◎  C
│   ●  ·7ed512a (⌂)
│ ╭─┤
│ │ ●  ·35ee481 (⌂)
├───╯
│ ◎  D
│ ●  ·ecb1877 (⌂)
├─╯
●  🏁·965998b (⌂)

"#]]
    );

    assert_eq!(
        graph
            .nodes()
            .iter()
            .filter(|node| matches!(node.kind(), NodeKind::Commit { .. }))
            .count(),
        8,
        "all commits are represented as canonical nodes"
    );
    assert_eq!(graph.annotations().len(), graph.nodes().len());

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:8:merged <> ✓!
└── ≡👉:8:merged[🌳] {1}
    ├── 👉:8:merged[🌳]
    │   └── ·8a6c109
    └── :9:A
        ├── ·62b409a
        ├── ·592abec
        └── ·965998b ►main

"#]]
    );
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

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉main
│ ◎  tags/annotated
├─╯
│ ◎  tags/release/v1
├─╯
●  ·541396b (⌂)
◎  other
●  🏁·fafd9d0 (⌂)

"#]]
    );
    Ok(())
}

#[test]
fn explicit_traversal_tips_allow_named_and_anonymous_integrated_targets_on_same_commit()
-> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let main_id = id_by_rev(&repo, "main").detach();

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

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

    // anonymous target context with the same commit collapses into the named target ref
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main
│ ◎  👉merged[🌳]
│ ●    ·8a6c109 (⌂)
│ ├─╮
│ ◎ │  A
│ ● │    ·62b409a (⌂)
│ ├───╮
│ ● │ │  ·592abec (⌂)
├─╯ │ │
│   │ ◎  B
│   │ ●  ·f16dddf (⌂)
├─────╯
│   ◎  C
│   ●  ·7ed512a (⌂)
│ ╭─┤
│ │ ●  ·35ee481 (⌂)
├───╯
│ ◎  D
│ ●  ·ecb1877 (⌂)
├─╯
●  🏁·965998b (⌂|✓)

"#]]
    );
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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

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
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main
│ ◎  👉merged[🌳]
│ ●    ·8a6c109 (⌂)
│ ├─╮
│ ◎ │  A
│ ● │    ·62b409a (⌂|✓)
│ ├───╮
│ ● │ │  ·592abec (⌂|✓)
├─╯ │ │
│   │ ◎  B
│   │ ●  ·f16dddf (⌂|✓)
├─────╯
│   ◎  C
│   ●  ·7ed512a (⌂)
│ ╭─┤
│ │ ●  ·35ee481 (⌂)
├───╯
│ ◎  D
│ ●  ·ecb1877 (⌂)
├─╯
●  🏁·965998b (⌂|✓)

"#]]
    );

    let (target_index, _) = graph
        .node_by_commit_id(target_commit_id)
        .expect("integrated target is present");
    assert!(
        graph.annotations()[target_index].contains(CommitFlags::Integrated),
        "the integrated target keeps its traversal annotation"
    );

    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref()),
        Some(target_ref_name.as_ref()),
        "the named integrated tip becomes the target ref without workspace metadata"
    );
    assert_eq!(
        ws.target_ref_tip_commit_id(),
        Some(target_ref_id),
        "the moving target ref keeps its own tip"
    );
    assert_eq!(
        ws.stored_target_commit_id(),
        Some(target_commit_id),
        "the lower anonymous integrated tip becomes stable target context"
    );
    assert_eq!(
        ws.lower_bound,
        Some(target_commit_id),
        "the stable target commit bounds the projected workspace"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:11:merged <> ✓refs/heads/A⇣3 on 965998b
└── ≡👉:11:merged[🌳] on 965998b {1}
    ├── 👉:11:merged[🌳]
    │   └── ·8a6c109
    └── :8:A
        ├── ·62b409a (✓)
        └── ·592abec (✓)

"#]]
    );
    Ok(())
}

#[test]
fn extra_target_commit_is_workspace_context_for_head_traversal() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let target_commit_id = id_by_rev(&repo, "main").detach();
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_extra_target_commit_id(target_commit_id),
    )?
    .validated()?;

    let ws = graph.into_workspace()?;
    assert!(
        ws.target_ref.is_none(),
        "commit-only target context must not invent a moving target ref"
    );
    assert_eq!(
        ws.stored_target_commit_id(),
        Some(target_commit_id),
        "the extra target commit becomes stable workspace context"
    );
    assert_eq!(
        ws.lower_bound,
        Some(target_commit_id),
        "the extra target commit bounds HEAD projection"
    );
    Ok(())
}

#[test]
fn stacked_rebased_remotes() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("remote-includes-another-remote")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 682be32 (origin/B) B
* e29c23d (origin/A) A
| * 312f819 (HEAD -> B) B
| * e255adc (A) A
|/  
* fafd9d0 (main) init

"#]]
    );

    // A remote will always be able to find their non-remotes so they don't seem cut-off.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉B[🌳] <> origin/B
●  ·312f819 (⌂)
◎  A <> origin/A
●  ·e255adc (⌂)
│ ◎  main
├─╯
│ ◎  origin/B
│ ●  🟣682be32
│ ◎  origin/A
│ ●  🟣e29c23d
├─╯
●  🏁·fafd9d0 (⌂)

"#]]
    );

    // 'main' is frozen because it connects to a 'foreign' remote, the commit was pushed.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:5:B <> ✓!
└── ≡👉:5:B[🌳] <> origin/B →:7:⇡1⇣2 {1}
    ├── 👉:5:B[🌳] <> origin/B →:7:⇡1⇣2
    │   ├── 🟣682be32 ►origin/B
    │   ├── 🟣e29c23d ►origin/A
    │   └── ·312f819
    └── :6:A <> origin/A →:8:⇡1⇣1
        ├── 🟣e29c23d ►origin/A
        ├── ·e255adc
        └── ❄fafd9d0 ►main

"#]]
    );

    // The hard limit stops queueing deeper commits, but queued commits are still processed
    // so existing work can complete its graph connections.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_hard_limit(5),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉B[🌳] <> origin/B
●  ·312f819 (⌂)
◎  A <> origin/A
●  ❌·e255adc (⌂)
◎  origin/B
●  🟣682be32
◎  origin/A
●  🟣e29c23d
◎  main
●  🏁🟣fafd9d0

"#]]
    );
    assert!(
        graph.hard_limit_hit(),
        "graph should record that traversal stopped queueing after hitting the hard limit"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:6:B <> ✓!
└── ≡👉:6:B[🌳] <> origin/B →:8:⇡1⇣2 on fafd9d0 {1}
    ├── 👉:6:B[🌳] <> origin/B →:8:⇡1⇣2
    │   ├── 🟣682be32 ►origin/B
    │   ├── 🟣e29c23d ►origin/A
    │   └── ·312f819
    └── :7:A <> origin/A →:9:⇡1⇣1
        ├── 🟣e29c23d ►origin/A
        └── ❌·e255adc

"#]]
    );

    // Everything we encounter is checked for remotes (no limit)
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉B[🌳] <> origin/B
●  ·312f819 (⌂)
◎  A <> origin/A
●  ·e255adc (⌂)
│ ◎  main
├─╯
│ ◎  origin/B
│ ●  🟣682be32
│ ◎  origin/A
│ ●  🟣e29c23d
├─╯
●  🏁·fafd9d0 (⌂)

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉A <> origin/A
●  ·e255adc (⌂)
│ ◎  main
├─╯
│ ◎  origin/A
│ ●  🟣e29c23d
├─╯
●  🏁·fafd9d0 (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:3:A <> ✓!
└── ≡👉:3:A <> origin/A →:5:⇡1⇣1 {1}
    └── 👉:3:A <> origin/A →:5:⇡1⇣1
        ├── 🟣e29c23d ►origin/A
        ├── ·e255adc
        └── ❄fafd9d0 ►main

"#]]
    );
    Ok(())
}

#[test]
fn with_limits() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("triple-merge")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    // Without limits
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●      ·2a95729 (⌂)
├─┬─╮
● │ │  ·6861158 (⌂)
● │ │  ·4f1f248 (⌂)
● │ │  ·487ffce (⌂)
│ ◎ │  A
│ ● │  ·20a823c (⌂)
│ ● │  ·442a12f (⌂)
│ ● │  ·686706b (⌂)
├─╯ │
│   ◎  B
│   ●  ·9908c99 (⌂)
│   ●  ·60d9a56 (⌂)
│   ●  ·9d171ff (⌂)
├───╯
│ ◎  main
├─╯
●  ·edc4dee (⌂)
●  ·01d0e1e (⌂)
●  ·4b3e5a8 (⌂)
●  ·34d0715 (⌂)
●  🏁·eb5f731 (⌂)

"#]]
    );
    // No limits list the first parent everywhere.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:15:C <> ✓!
└── ≡👉:15:C[🌳] {1}
    └── 👉:15:C[🌳]
        ├── ·2a95729
        ├── ·6861158
        ├── ·4f1f248
        ├── ·487ffce
        ├── ·edc4dee ►main
        ├── ·01d0e1e
        ├── ·4b3e5a8
        ├── ·34d0715
        └── ·eb5f731

"#]]
    );

    // There is no empty starting points, we always traverse the first commit as we really want
    // to get to remote processing there.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(0),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●  ✂·2a95729 (⌂)

"#]]
    );
    // The cut by limit is also represented here.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:4:C <> ✓!
└── ≡👉:4:C[🌳] on 6861158 {1}
    └── 👉:4:C[🌳]
        └── ✂️·2a95729

"#]]
    );

    // A single commit, the merge commit.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●      ·2a95729 (⌂)
├─┬─╮
● │ │  ✂·6861158 (⌂)
  ◎ │  A
  ● │  ✂·20a823c (⌂)
    ◎  B
    ●  ✂·9908c99 (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:7:C <> ✓!
└── ≡👉:7:C[🌳] on 4f1f248 {1}
    └── 👉:7:C[🌳]
        ├── ·2a95729
        └── ✂️·6861158

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●      ·2a95729 (⌂)
├─┬─╮
● │ │  ❌·6861158 (⌂)
  ◎ │  A
  ● │  ❌·20a823c (⌂)
    ◎  B
    ●  ❌·9908c99 (⌂)

"#]]
    );

    // The merge commit, then we witness lane-duplication of the limit so we get more than requested.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(2),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●      ·2a95729 (⌂)
├─┬─╮
● │ │  ·6861158 (⌂)
● │ │  ✂·4f1f248 (⌂)
  ◎ │  A
  ● │  ·20a823c (⌂)
  ● │  ✂·442a12f (⌂)
    ◎  B
    ●  ·9908c99 (⌂)
    ●  ✂·60d9a56 (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:10:C <> ✓!
└── ≡👉:10:C[🌳] on 487ffce {1}
    └── 👉:10:C[🌳]
        ├── ·2a95729
        ├── ·6861158
        └── ✂️·4f1f248

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●      ·2a95729 (⌂)
├─┬─╮
● │ │  ·6861158 (⌂)
● │ │  ✂·4f1f248 (⌂)
  ◎ │  A
  ● │  ·20a823c (⌂)
  ● │  ·442a12f (⌂)
  ● │  ✂·686706b (⌂)
    ◎  B
    ●  ·9908c99 (⌂)
    ●  ✂·60d9a56 (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:11:C <> ✓!
└── ≡👉:11:C[🌳] on 487ffce {1}
    └── 👉:11:C[🌳]
        ├── ·2a95729
        ├── ·6861158
        └── ✂️·4f1f248

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●      ·2a95729 (⌂)
├─┬─╮
● │ │  ·6861158 (⌂)
● │ │  ·4f1f248 (⌂)
● │ │  ✂·487ffce (⌂)
  ◎ │  A
  ● │  ·20a823c (⌂)
  ● │  ·442a12f (⌂)
  ● │  ·686706b (⌂)
  ◎ │  main
  ● │  ·edc4dee (⌂)
  ● │  ✂·01d0e1e (⌂)
    ◎  B
    ●  ·9908c99 (⌂)
    ●  ·60d9a56 (⌂)
    ●  ✂·9d171ff (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:14:C <> ✓!
└── ≡👉:14:C[🌳] on edc4dee {1}
    └── 👉:14:C[🌳]
        ├── ·2a95729
        ├── ·6861158
        ├── ·4f1f248
        └── ✂️·487ffce

"#]]
    );

    // We can specify any target, despite not having a workspace setup.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;

    // This limits the reach of the stack naturally.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉C[🌳]
●      ·2a95729 (⌂)
├─┬─╮
● │ │  ·6861158 (⌂)
● │ │  ·4f1f248 (⌂)
● │ │  ·487ffce (⌂)
│ ◎ │  A
│ ● │  ·20a823c (⌂)
│ ● │  ·442a12f (⌂)
│ ● │  ·686706b (⌂)
├─╯ │
│   ◎  B
│   ●  ·9908c99 (⌂)
│   ●  ·60d9a56 (⌂)
│   ●  ·9d171ff (⌂)
├───╯
│ ◎  main
├─╯
●  ·edc4dee (⌂|✓)
●  ·01d0e1e (⌂|✓)
●  ·4b3e5a8 (⌂|✓)
●  ·34d0715 (⌂|✓)
●  🏁·eb5f731 (⌂|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:16:C <> ✓! on edc4dee
└── ≡👉:16:C[🌳] on edc4dee {1}
    └── 👉:16:C[🌳]
        ├── ·2a95729
        ├── ·6861158
        ├── ·4f1f248
        └── ·487ffce

"#]]
    );
    Ok(())
}

#[test]
fn special_branch_names_do_not_end_up_in_segment() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("special-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3686017 (HEAD -> main) top
* 9725482 (gitbutler/edit) middle
* fafd9d0 (gitbutler/target) init

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    // Standard handling after travrsal and post-processing.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉main[🌳]
●  ·3686017 (⌂)
◎  gitbutler/edit
●  ·9725482 (⌂)
◎  gitbutler/target
●  🏁·fafd9d0 (⌂)

"#]]
    );

    // But special handling for workspace views.
    let workspace = graph.into_workspace()?;
    assert_eq!(workspace.stacks.len(), 1, "there is one ad-hoc stack");
    let stack = &workspace.stacks[0];
    assert_eq!(
        stack.segments.len(),
        1,
        "internal GitButler refs must not split user-visible stacks"
    );
    let main = ref_name("refs/heads/main");
    assert_eq!(
        stack.segments[0].ref_name(),
        Some(main.as_ref()),
        "main remains the only user-visible branch segment"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
⌂:3:main <> ✓!
└── ≡👉:3:main[🌳] {1}
    └── 👉:3:main[🌳]
        ├── ·3686017
        ├── ·9725482 ►gitbutler/edit
        └── ·fafd9d0 ►gitbutler/target

"#]]
    );
    Ok(())
}

#[test]
fn ambiguous_worktrees() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("ambiguous-worktrees")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 85efbe4 (HEAD -> main, wt-outside-ambiguous-worktree, wt-inside-ambiguous-worktree) M

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉main[🌳@repo]
│ ◎  wt-inside-ambiguous-worktree[📁]
├─╯
│ ◎  wt-outside-ambiguous-worktree[📁]
├─╯
●  🏁·85efbe4 (⌂)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:1:main <> ✓!
└── ≡👉:1:main[🌳@repo] {1}
    └── 👉:1:main[🌳@repo]
        └── ·85efbe4 ►wt-inside-ambiguous-worktree[📁], ►wt-outside-ambiguous-worktree[📁]

"#]]
    );

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
    // when the graph is built from the linked worktree repository, it can't see anything else without metadata
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
│ ◎  👉wt-inside-ambiguous-worktree[📁@repo]
├─╯
│ ◎  wt-outside-ambiguous-worktree[📁]
├─╯
●  🏁·85efbe4 (⌂)

"#]]
    );

    // workspace debug output should preserve that the linked worktree, not the main worktree, is owned by the repository used to build the graph
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:2:wt-inside-ambiguous-worktree <> ✓!
└── ≡👉:2:wt-inside-ambiguous-worktree[📁@repo] {1}
    └── 👉:2:wt-inside-ambiguous-worktree[📁@repo]
        └── ·85efbe4 ►main[🌳], ►wt-outside-ambiguous-worktree[📁]

"#]]
    );
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

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 06470d7 (HEAD -> main) commit with the same parent ('base') duplicated
|\
* 86719d5 base

"#]]
        .raw()
    );

    let meta = in_memory_meta(tmp.as_ref())?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    // Duplicate parent commits are kept verbatim.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉main[🌳]
●  ·06470d7 (⌂)
●  🏁·86719d5 (⌂)

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_same_tip_order_creates_empty_branch_segments() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let tip = commit(&repo, "same tip")?;
    create_branches(&repo, tip, ["refs/heads/top", "refs/heads/bottom"])?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
│ ◎  👉top
│ ◎  bottom
├─╯
●  🏁·960152d (⌂)

"#]]
    );
    let NodeGraphEntrypoint::Node(entrypoint) = graph.entrypoint() else {
        panic!("checked-out ordered branch is born");
    };
    let NodeKind::Reference(reference) = graph.nodes()[*entrypoint].kind() else {
        panic!("symbolic entrypoint is represented by a reference node");
    };
    assert_eq!(
        reference.ref_info.commit_id,
        Some(tip),
        "a checked-out empty ordered branch still points at the bottom commit"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:3:top <> ✓!
└── ≡👉:3:top {1}
    ├── 👉:3:top
    └── :1:bottom
        └── ·960152d ►main[🌳], ►top

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_order_projects_from_entrypoint_when_top_is_above_it() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let tip = commit(&repo, "same tip")?;
    create_branches(&repo, tip, ["refs/heads/top", "refs/heads/bottom"])?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/bottom",
        ["refs/heads/top", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
│ ◎  top
│ ◎  👉bottom
├─╯
●  🏁·960152d (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:1:bottom <> ✓!
└── ≡👉:1:bottom {1}
    └── 👉:1:bottom
        └── ·960152d ►main[🌳], ►top

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_three_branch_order_preserves_middle_empty_segment() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let tip = commit(&repo, "same tip")?;
    create_branches(
        &repo,
        tip,
        ["refs/heads/top", "refs/heads/middle", "refs/heads/bottom"],
    )?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/middle", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
│ ◎  👉top
│ ◎  middle
│ ◎  bottom
├─╯
●  🏁·960152d (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:4:top <> ✓!
└── ≡👉:4:top {1}
    ├── 👉:4:top
    ├── :3:middle
    └── :1:bottom
        └── ·960152d ►main[🌳], ►middle, ►top

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_order_ignores_missing_metadata_refs_without_phantoms() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let tip = commit(&repo, "same tip")?;
    create_branches(&repo, tip, ["refs/heads/top", "refs/heads/bottom"])?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/missing", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
│ ◎  👉top
│ ◎  bottom
├─╯
●  🏁·960152d (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:3:top <> ✓!
└── ≡👉:3:top {1}
    ├── 👉:3:top
    └── :1:bottom
        └── ·960152d ►main[🌳], ►top

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_order_does_not_force_diverged_refs_into_empty_stack() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let bottom_tip = commit(&repo, "bottom")?;
    let top_tip = commit_with_parent(&repo, "top", bottom_tip)?;
    create_branches(&repo, bottom_tip, ["refs/heads/bottom", "refs/heads/main"])?;
    create_branches(&repo, top_tip, ["refs/heads/top"])?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
│ ◎  👉top
│ ●  ·5cd63e5 (⌂)
│ ◎  bottom
├─╯
●  🏁·fa91c94 (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:2:top <> ✓!
└── ≡👉:2:top {1}
    ├── 👉:2:top
    │   └── ·5cd63e5
    └── :3:bottom
        └── ·fa91c94 ►main[🌳]

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_order_preserves_empty_top_above_commit_owning_branch() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let target_tip = commit(&repo, "target")?;
    let bottom_tip = commit_with_parent(&repo, "bottom", target_tip)?;
    let commit_branch_tip = commit_with_parent(&repo, "top", bottom_tip)?;
    create_branches(
        &repo,
        commit_branch_tip,
        ["refs/heads/empty-top", "refs/heads/commit-branch"],
    )?;
    create_branches(&repo, bottom_tip, ["refs/heads/bottom"])?;
    create_branches(&repo, target_tip, ["refs/heads/main"])?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/empty-top",
        [
            "refs/heads/empty-top",
            "refs/heads/commit-branch",
            "refs/heads/bottom",
        ],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:4:empty-top <> ✓!
└── ≡👉:4:empty-top {1}
    ├── 👉:4:empty-top
    ├── :3:commit-branch
    │   └── ·4782705 ►empty-top
    ├── :5:bottom
    │   └── ·dbc3a4c
    └── :6:main[🌳]
        └── ·67b14ca

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_order_keeps_lower_empty_branches_after_non_empty_move() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let target_tip = commit(&repo, "target")?;
    let base_tip = commit_with_parent(&repo, "base", target_tip)?;
    let commit_branch_tip = commit_with_parent(&repo, "commit branch", base_tip)?;
    create_branches(&repo, commit_branch_tip, ["refs/heads/commit-branch"])?;
    create_branches(
        &repo,
        base_tip,
        [
            "refs/heads/empty-top",
            "refs/heads/empty-low",
            "refs/heads/base",
        ],
    )?;
    create_branches(&repo, target_tip, ["refs/heads/main"])?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/commit-branch",
        [
            "refs/heads/commit-branch",
            "refs/heads/empty-top",
            "refs/heads/empty-low",
            "refs/heads/base",
        ],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:3:commit-branch <> ✓!
└── ≡👉:3:commit-branch {1}
    ├── 👉:3:commit-branch
    │   └── ·5380c0a
    ├── :6:empty-top
    ├── :5:empty-low
    ├── :4:base
    │   └── ·a5cd64d ►empty-low, ►empty-top
    └── :7:main[🌳]
        └── ·67b14ca

"#]]
    );
    Ok(())
}

#[test]
fn ad_hoc_order_scopes_empty_segments_to_active_chain() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let tip = commit(&repo, "same tip")?;
    create_branches(
        &repo,
        tip,
        [
            "refs/heads/top",
            "refs/heads/bottom",
            "refs/heads/other-top",
            "refs/heads/other-bottom",
        ],
    )?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let graph = graph_with_branch_orders(
        &repo,
        &*meta,
        "refs/heads/top",
        &[
            &["refs/heads/top", "refs/heads/bottom"],
            &["refs/heads/other-top", "refs/heads/other-bottom"],
        ],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  main[🌳]
│ ◎  other-bottom
├─╯
│ ◎  other-top
├─╯
│ ◎  👉top
│ ◎  bottom
├─╯
●  🏁·960152d (⌂)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:5:top <> ✓!
└── ≡👉:5:top {1}
    ├── 👉:5:top
    └── :1:bottom
        └── ·960152d ►main[🌳], ►other-bottom, ►other-top, ►top

"#]]
    );
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

fn empty_repo() -> anyhow::Result<(impl AsRef<std::path::Path>, gix::Repository)> {
    rust_fixture_writable("empty", 1, Creation::Execute, |fixture| {
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
    .map_err(anyhow::Error::from_boxed)
}

fn commit(repo: &gix::Repository, message: &str) -> anyhow::Result<gix::ObjectId> {
    Ok(repo
        .commit(
            "HEAD",
            message,
            repo.object_hash().empty_tree(),
            None::<gix::ObjectId>,
        )?
        .detach())
}

fn commit_with_parent(
    repo: &gix::Repository,
    message: &str,
    parent: gix::ObjectId,
) -> anyhow::Result<gix::ObjectId> {
    Ok(repo
        .commit(
            "HEAD",
            message,
            repo.object_hash().empty_tree(),
            Some(parent),
        )?
        .detach())
}

fn create_branches<const N: usize>(
    repo: &gix::Repository,
    target: gix::ObjectId,
    branches: [&str; N],
) -> anyhow::Result<()> {
    for branch in branches {
        repo.reference(
            ref_name(branch),
            target,
            gix::refs::transaction::PreviousValue::Any,
            "test branch order",
        )?;
    }
    Ok(())
}

fn graph_with_branch_order<const N: usize>(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    entrypoint_ref: &str,
    order: [&str; N],
) -> anyhow::Result<Graph> {
    graph_with_branch_orders(repo, meta, entrypoint_ref, &[&order])
}

fn graph_with_branch_orders(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    entrypoint_ref: &str,
    orders: &[&[&str]],
) -> anyhow::Result<Graph> {
    let entrypoint_ref = ref_name(entrypoint_ref);
    let tip = repo
        .find_reference(entrypoint_ref.as_ref())?
        .peel_to_id()?
        .detach();
    let mut overlay = Overlay::default();
    for order in orders {
        overlay = overlay.with_branch_stack_order_override(order.iter().copied().map(ref_name));
    }
    Graph::from_commit_traversal(
        tip.attach(repo),
        Some(entrypoint_ref),
        meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .redo_traversal_with_overlay(repo, meta, overlay)
}
