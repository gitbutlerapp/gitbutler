use but_graph::{
    CommitFlags, Workspace,
    walk::{Overlay, Seed},
};
use but_testsupport::{
    gix_testtools::{self, Creation, rust_fixture_writable},
    graph_dag, graph_workspace, visualize_commit_graph_all,
};
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

#[test]
fn unborn() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(graph_dag(&ws), snapbox::str!["<UNBORN> 👉►main"]);

    assert!(
        ws.managed_entrypoint_commit_id(&repo)?.is_none(),
        "there is no commit it could return"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]

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
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·541396b (⌂) ►main, ►tags/annotated, ►tags/release/v1
*  🏁·fafd9d0 (⌂) ►other
"#]]
    );

    assert!(
        ws.entrypoint_commit_id()?.is_some(),
        "there is an entrypoint commit, detached or not"
    );
    assert!(
        ws.managed_entrypoint_commit_id(&repo)?.is_none(),
        "but it's not managed"
    );
    let cg = ws.commit_graph();
    let root = cg
        .commit_ids()
        .find(|id| cg.node(*id).is_some_and(|n| n.parent_ids.is_empty()))
        .expect("root commit is present");
    assert_eq!(
        cg.connected_parents(root).count(),
        0,
        "traversal naturally ended at the first commit"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:DETACHED <> ✓!
└── ≡:anon: {1}
    ├── :anon:
    │   └── ·541396b ►tags/annotated, ►tags/release/v1, ►main
    └── :other
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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·71a64f3 (⌂) ►main[🌳], ►origin/main <> origin/main
*  ⛰·62d65ed (⌂|⛰)
"#]]
    );
    let cg = ws.commit_graph();
    let boundary_commit = cg
        .node(shallow_boundary_id)
        .expect("boundary commit is included in the graph");
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
        cg.node(missing_parent).is_none(),
        "the grafted parent is not traversed"
    );
    assert_eq!(
        cg.connected_parents(shallow_boundary_id).count(),
        0,
        "the walk cut at the shallow boundary, not at a limit"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main on 71a64f3
└── ≡:main[🌳] <> origin/main {1}
    └── :main[🌳] <> origin/main

"#]]
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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·738ea18 (⌂) ►first-parent[🌳]
*    ·408ca26 (⌂)
├─╮
* │  ·2854fa2 (⌂)
│ *  ·75369b0 (⌂) ►second-parent
│ *  ·553bbf7 (⌂)
│ *  ·72614bb (⌂)
├─╯
*  🏁·793a434 (⌂) ►main, ►tags/base
"#]]
    );

    // we see only first-parent with two commits, not the 'second-parent' ref because it *seems* to be traversed first
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:first-parent[🌳] <> ✓!
└── ≡:first-parent[🌳] {1}
    ├── :first-parent[🌳]
    │   ├── ·738ea18
    │   ├── ·408ca26
    │   └── ·2854fa2
    └── :main
        └── ·793a434 ►tags/base

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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·971953d (⌂) ►main[🌳] <> origin/main
│ *  🟣5d29d62 ►origin/main
├─╯
*  ·ce09734 (⌂)
*  🏁·fafd9d0 (⌂)
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main⇣1 on ce09734
└── ≡:main[🌳] <> origin/main⇡1⇣1 on ce09734 {1}
    └── :main[🌳] <> origin/main⇡1⇣1
        ├── 🟣5d29d62
        └── ·971953d

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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🟣085535d ►origin/main
*  🟣dd9f8d9 ►origin/split-segment
*  👉·971953d (⌂) ►main[🌳] <> origin/main
*  ·ce09734 (⌂)
*  🏁·fafd9d0 (⌂)
"#]]
    );

    // TODO: it should detect that `main` has no own commits as it's fully integrated.
    //       This also affects the base which would have to be 085535d, the first commit.
    //       which is strange but maybe can work?
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main⇣2 on 971953d
└── ≡:main[🌳] <> origin/main⇣1 {1}
    └── :main[🌳] <> origin/main⇣1
        └── 🟣085535d

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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🟣085535d ►origin/main
*  🟣dd9f8d9 ►origin/split-segment
*  👉·971953d (⌂) ►main[🌳] <> origin/main
*  ·ce09734 (⌂) ►gitbutler/target
*  🏁·fafd9d0 (⌂)
"#]]
    );

    // TODO: We'd actually have to recognise that the `origin/split-segment` branch
    //       isn't related to our stack and count its commits to `origin/main`.
    //       Right now we are missing dd9f8d9.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main⇣2 on 971953d
└── ≡:main[🌳] <> origin/main⇣1 {1}
    └── :main[🌳] <> origin/main⇣1
        └── 🟣085535d

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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·c6c8c05 (⌂) ►main[🌳]
├─╮
* │    ·76fc5c4 (⌂)
├───╮
* │ │  🏁·e5d0542 (⌂)
  │ *  🏁·366d496 (⌂) ►B
  *  ·8631946 (⌂) ►C
╭─┤
│ *  🏁·00fab2a (⌂)
*  🏁·f4955b6 (⌂) ►D
"#]]
    );
    assert_eq!(
        ws.statistics().commits_at_tip,
        1,
        "all leads to a single merge-commit"
    );
    assert_eq!(
        ws.statistics().commits_at_bottom,
        4,
        "there are 4 orphaned bases"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );

    let stats = ws.statistics();
    assert_eq!(stats.commits, 8, "one commit per node");
    assert_eq!(
        stats.edges_connected, 10,
        "however, we see only a portion of the edges as the tree can only show simple stacks"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:merged[🌳] <> ✓!
└── ≡:merged[🌳] {1}
    ├── :merged[🌳]
    │   └── ·8a6c109
    ├── :A
    │   ├── ·62b409a
    │   └── ·592abec
    └── :main
        └── ·965998b

"#]]
    );
    Ok(())
}

#[test]
fn explicit_seeds_reject_duplicate_traversal_seeds() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(merged_id, None),
            Seed::reachable(a_id, None),
            Seed::reachable(a_id, Some(a_ref)),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("duplicate traversal seeds must be rejected");

    assert!(
        err.to_string()
            .starts_with("explicit traversal seeds contain duplicate traversal seed Seed"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn explicit_seeds_allow_overlapping_commit_ids() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    let main_id = id_by_rev(&repo, "main").detach();
    let main_ref = ref_name("refs/heads/main");
    let release_tag = ref_name("refs/tags/release/v1");

    let graph = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(main_id, Some(main_ref)),
            Seed::reachable(main_id, Some(release_tag)),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*  👉·541396b (⌂) ►main, ►tags/annotated, ►tags/release/v1
*  🏁·fafd9d0 (⌂) ►other
"#]]
    );
    Ok(())
}

#[test]
fn explicit_seeds_include_unnamed_revisions() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let c_id = id_by_rev(&repo, "C").detach();

    let ws = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(merged_id, None),
            Seed::reachable(a_id, None),
            Seed::reachable(c_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂) ►main
"#]]
    );
    Ok(())
}

#[test]
fn explicit_traversal_prioritizes_integrated_tips_independent_of_input_order() -> anyhow::Result<()>
{
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let main_id = id_by_rev(&repo, "main").detach();

    // The integrated tip comes LAST from the caller; it must still be queued first
    // so its flag propagates (main shows ✓).
    let ws = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(merged_id, None),
            Seed::reachable(a_id, None),
            Seed::integrated(main_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂|✓) ►main
"#]]
    );
    Ok(())
}

#[test]
fn explicit_seeds_allow_named_and_anonymous_integrated_targets_on_same_commit() -> anyhow::Result<()>
{
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

    let graph = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(merged_id, Some(ref_name("refs/heads/merged"))),
            Seed::integrated(main_id, Some(ref_name("refs/heads/main"))),
            Seed::integrated(main_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    // anonymous target context with the same commit collapses into the named target ref
    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂) ►A
├───╮
* │ │  ·592abec (⌂)
│ │ *  ·f16dddf (⌂) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂|✓) ►main
"#]]
    );
    Ok(())
}

#[test]
fn explicit_seeds_reject_multiple_entrypoints() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();

    let err = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(merged_id, None),
            Seed::entrypoint(a_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("multiple entrypoints must be rejected");

    assert_eq!(
        err.to_string(),
        "explicit traversal seeds require exactly one entrypoint"
    );
    Ok(())
}

#[test]
fn explicit_seeds_reject_duplicate_ref_names() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let a_id = id_by_rev(&repo, "A").detach();
    let c_id = id_by_rev(&repo, "C").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(a_id, Some(a_ref.clone())),
            Seed::reachable(c_id, Some(a_ref.clone())),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("duplicate ref names must be rejected");

    assert_eq!(
        err.to_string(),
        format!("explicit traversal seeds contain duplicate ref name {a_ref}")
    );
    Ok(())
}

#[test]
fn explicit_seeds_reject_detached_entrypoint_with_ref_name() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();

    let err = Workspace::from_seeds(
        &repo,
        [Seed::new(merged_id)
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
        "explicit detached entrypoint seed cannot have a ref name"
    );
    Ok(())
}

#[test]
fn explicit_seeds_reject_ref_names_that_point_elsewhere() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Workspace::from_seeds(
        &repo,
        [Seed::entrypoint(merged_id, Some(a_ref.clone()))],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )
    .expect_err("ref names must resolve to their tip id");

    assert_eq!(
        err.to_string(),
        format!("explicit traversal seed ref {a_ref} points to {a_id}, not {merged_id}")
    );
    Ok(())
}

#[test]
fn traversal_entrypoint_ref_override_must_point_to_entrypoint() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = id_by_rev(&repo, "merged").detach();
    let a_id = id_by_rev(&repo, "A").detach();
    let a_ref = ref_name("refs/heads/A");

    let err = Workspace::from_tip(
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
fn explicit_seeds_use_integrated_tip_as_workspace_target_commit() -> anyhow::Result<()> {
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
    let ws = Workspace::from_seeds(
        &repo,
        [
            Seed::entrypoint(merged_id, Some(ref_name("refs/heads/merged"))),
            Seed::integrated(target_ref_id, Some(target_ref_name.clone())),
            Seed::integrated(target_commit_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·8a6c109 (⌂) ►merged[🌳]
├─╮
* │    ·62b409a (⌂|✓) ►A
├───╮
* │ │  ·592abec (⌂|✓)
│ │ *  ·f16dddf (⌂|✓) ►B
├───╯
│ *    ·7ed512a (⌂) ►C
│ ├─╮
│ * │  ·35ee481 (⌂)
├─╯ │
│   *  ·ecb1877 (⌂) ►D
├───╯
*  🏁·965998b (⌂|✓) ►main
"#]]
    );

    assert!(
        ws.commit_graph().node(target_commit_id).is_some(),
        "the integrated tip made it into the graph; the DAG above shows it as its own run"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:merged[🌳] <> ✓refs/heads/A⇣3 on 965998b
└── ≡:merged[🌳] on 965998b {1}
    ├── :merged[🌳]
    │   └── ·8a6c109
    └── :A
        ├── ·62b409a (✓)
        └── ·592abec (✓)

"#]]
    );
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
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·312f819 (⌂) ►B[🌳] <> origin/B
*  ·e255adc (⌂) ►A <> origin/A
│ *  🟣682be32 ►origin/B
│ *  🟣e29c23d ►origin/A
├─╯
*  🏁·fafd9d0 (⌂) ►main
"#]]
    );

    // 'main' is frozen because it connects to a 'foreign' remote, the commit was pushed.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:B[🌳] <> ✓refs/remotes/origin/B⇣2 on fafd9d0
└── ≡:B[🌳] <> origin/B⇡1⇣1 on fafd9d0 {1}
    ├── :B[🌳] <> origin/B⇡1⇣1
    │   ├── 🟣682be32
    │   └── ·312f819
    └── :A <> origin/A⇡1⇣1
        ├── 🟣e29c23d
        └── ·e255adc

"#]]
    );

    // The hard limit stops queueing deeper commits, but queued commits are still processed
    // so existing work can complete its graph connections.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_hard_limit(5),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·312f819 (⌂) ►B[🌳] <> origin/B
*  ❌·e255adc (⌂) ►A <> origin/A
*  🟣682be32 ►origin/B
*  🟣e29c23d ►origin/A
*  🏁🟣fafd9d0 ►main
"#]]
    );
    assert!(
        ws.hard_limit_hit(),
        "graph should record that traversal stopped queueing after hitting the hard limit"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:B[🌳] <> ✓refs/remotes/origin/B⇣2 on fafd9d0
└── ≡:B[🌳] <> origin/B⇡1⇣1 {1}
    ├── :B[🌳] <> origin/B⇡1⇣1
    │   ├── 🟣682be32
    │   └── ·312f819
    └── :A <> origin/A⇡1⇣2
        ├── 🟣e29c23d
        ├── 🟣fafd9d0
        └── ❌·e255adc

"#]]
    );

    // Everything we encounter is checked for remotes (no limit)
    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*  👉·312f819 (⌂) ►B[🌳] <> origin/B
*  ·e255adc (⌂) ►A <> origin/A
│ *  🟣682be32 ►origin/B
│ *  🟣e29c23d ►origin/A
├─╯
*  🏁·fafd9d0 (⌂) ►main
"#]]
    );

    // With a lower entrypoint, we don't see part of the graph.
    let (id, name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        name,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·e255adc (⌂) ►A <> origin/A
│ *  🟣e29c23d ►origin/A
├─╯
*  🏁·fafd9d0 (⌂) ►main
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A <> ✓refs/remotes/origin/A⇣1 on fafd9d0
└── ≡:A <> origin/A⇡1⇣1 on fafd9d0 {1}
    └── :A <> origin/A⇡1⇣1
        ├── 🟣e29c23d
        └── ·e255adc

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
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·2a95729 (⌂) ►C[🌳]
├─┬─╮
* │ │  ·6861158 (⌂)
* │ │  ·4f1f248 (⌂)
* │ │  ·487ffce (⌂)
│ * │  ·20a823c (⌂) ►A
│ * │  ·442a12f (⌂)
│ * │  ·686706b (⌂)
├─╯ │
│   *  ·9908c99 (⌂) ►B
│   *  ·60d9a56 (⌂)
│   *  ·9d171ff (⌂)
├───╯
*  ·edc4dee (⌂) ►main
*  ·01d0e1e (⌂)
*  ·4b3e5a8 (⌂)
*  ·34d0715 (⌂)
*  🏁·eb5f731 (⌂)
"#]]
    );
    // No limits list the first parent everywhere.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓!
└── ≡:C[🌳] {1}
    ├── :C[🌳]
    │   ├── ·2a95729
    │   ├── ·6861158
    │   ├── ·4f1f248
    │   └── ·487ffce
    └── :main
        ├── ·edc4dee
        ├── ·01d0e1e
        ├── ·4b3e5a8
        ├── ·34d0715
        └── ·eb5f731

"#]]
    );

    // There is no empty starting points, we always traverse the first commit as we really want
    // to get to remote processing there.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(0),
    )?
    .validated()?;
    snapbox::assert_data_eq!(graph_dag(&ws), snapbox::str!["*  👉✂·2a95729 (⌂) ►C[🌳]"]);
    // The cut by limit is also represented here.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓!
└── ≡:C[🌳] {1}
    └── :C[🌳]
        └── ✂️·2a95729

"#]]
    );

    // A single commit, the merge commit.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·2a95729 (⌂) ►C[🌳]
├─┬─╮
* │ │  ✂·6861158 (⌂)
  * │  ✂·20a823c (⌂) ►A
    *  ✂·9908c99 (⌂) ►B
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓!
└── ≡:C[🌳] {1}
    └── :C[🌳]
        ├── ·2a95729
        └── ✂️·6861158

"#]]
    );

    // Hitting the hard limit while queueing merge parents still queues the
    // complete parent set. The hard limit only prevents traversal beyond them.
    let graph = Workspace::from_head(
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
        graph_dag(&graph),
        snapbox::str![[r#"
*      👉·2a95729 (⌂) ►C[🌳]
├─┬─╮
* │ │  ❌·6861158 (⌂)
  * │  ❌·20a823c (⌂) ►A
    *  ❌·9908c99 (⌂) ►B
"#]]
    );

    // The merge commit, then we witness lane-duplication of the limit so we get more than requested.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_limit_hint(2),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·2a95729 (⌂) ►C[🌳]
├─┬─╮
* │ │  ·6861158 (⌂)
* │ │  ✂·4f1f248 (⌂)
  * │  ·20a823c (⌂) ►A
  * │  ✂·442a12f (⌂)
    *  ·9908c99 (⌂) ►B
    *  ✂·60d9a56 (⌂)
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓!
└── ≡:C[🌳] {1}
    └── :C[🌳]
        ├── ·2a95729
        ├── ·6861158
        └── ✂️·4f1f248

"#]]
    );

    // Allow to see more commits just in the middle lane, the limit is reset,
    // and we see two more.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options()
            .with_limit_hint(2)
            .with_limit_extension_at(Some(id_by_rev(&repo, ":/A3").detach())),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·2a95729 (⌂) ►C[🌳]
├─┬─╮
* │ │  ·6861158 (⌂)
* │ │  ✂·4f1f248 (⌂)
  * │  ·20a823c (⌂) ►A
  * │  ·442a12f (⌂)
  * │  ✂·686706b (⌂)
    *  ·9908c99 (⌂) ►B
    *  ✂·60d9a56 (⌂)
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓!
└── ≡:C[🌳] {1}
    └── :C[🌳]
        ├── ·2a95729
        ├── ·6861158
        └── ✂️·4f1f248

"#]]
    );

    // Multiple extensions are fine as well.
    let id = |rev| id_by_rev(&repo, rev).detach();
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options()
            .with_limit_hint(2)
            .with_limit_extension_at([id(":/A3"), id(":/A1"), id(":/B3"), id(":/C3")]),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·2a95729 (⌂) ►C[🌳]
├─┬─╮
* │ │  ·6861158 (⌂)
* │ │  ·4f1f248 (⌂)
* │ │  ✂·487ffce (⌂)
  * │  ·20a823c (⌂) ►A
  * │  ·442a12f (⌂)
  * │  ·686706b (⌂)
  * │  ·edc4dee (⌂) ►main
  * │  ✂·01d0e1e (⌂)
    *  ·9908c99 (⌂) ►B
    *  ·60d9a56 (⌂)
    *  ✂·9d171ff (⌂)
"#]]
    );
    snapbox::assert_data_eq!(
        format!("{:#?}", ws.statistics()).as_str(),
        snapbox::str![[r#"
CommitGraphStatistics {
    commits: 12,
    edges_connected: 11,
    edges_cut: 0,
    refs: 4,
    commits_at_tip: 1,
    commits_at_bottom: 3,
    commits_in_workspace: 0,
    commits_integrated: 0,
    commits_not_in_remote: 12,
    layout_refs: Some(
        4,
    ),
    hard_limit_hit: false,
    entrypoint: Some(
        Sha1(2a957298aaca646cc5e1d0bfebbc9840e7568c78),
    ),
}
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓!
└── ≡:C[🌳] {1}
    └── :C[🌳]
        ├── ·2a95729
        ├── ·6861158
        ├── ·4f1f248
        └── ✂️·487ffce

"#]]
    );

    // We can specify any target, despite not having a workspace setup.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;

    // This limits the reach of the stack naturally.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·2a95729 (⌂) ►C[🌳]
├─┬─╮
* │ │  ·6861158 (⌂)
* │ │  ·4f1f248 (⌂)
* │ │  ·487ffce (⌂)
│ * │  ·20a823c (⌂) ►A
│ * │  ·442a12f (⌂)
│ * │  ·686706b (⌂)
├─╯ │
│   *  ·9908c99 (⌂) ►B
│   *  ·60d9a56 (⌂)
│   *  ·9d171ff (⌂)
├───╯
*  ·edc4dee (⌂|✓) ►main
*  ·01d0e1e (⌂|✓)
*  ·4b3e5a8 (⌂|✓)
*  ·34d0715 (⌂|✓)
*  🏁·eb5f731 (⌂|✓)
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:C[🌳] <> ✓! on edc4dee
└── ≡:C[🌳] on edc4dee {1}
    └── :C[🌳]
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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    // Standard handling after travrsal and post-processing.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·3686017 (⌂) ►main[🌳]
*  ·9725482 (⌂) ►gitbutler/edit
*  🏁·fafd9d0 (⌂) ►gitbutler/target
"#]]
    );

    // But special handling for workspace views.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
        ├── ·3686017
        ├── ·9725482
        └── ·fafd9d0

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

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![
            "*  👉🏁·85efbe4 (⌂) ►main[🌳@repo], ►wt-inside-ambiguous-worktree[📁], ►wt-outside-ambiguous-worktree[📁]"
        ]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳@repo] <> ✓!
└── ≡:main[🌳@repo] {1}
    └── :main[🌳@repo]
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
    let ws = Workspace::from_head(
        &linked_repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    // when the graph is built from the linked worktree repository, it can't see anything else without metadata
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![
            "*  👉🏁·85efbe4 (⌂) ►main[🌳], ►wt-inside-ambiguous-worktree[📁@repo], ►wt-outside-ambiguous-worktree[📁]"
        ]
    );

    // workspace debug output should preserve that the linked worktree, not the main worktree, is owned by the repository used to build the graph
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:wt-inside-ambiguous-worktree[📁@repo] <> ✓!
└── ≡:wt-inside-ambiguous-worktree[📁@repo] {1}
    └── :wt-inside-ambiguous-worktree[📁@repo]
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
    let graph = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    // Duplicate parent commits are collapsed at read time: lanes come from workspace metadata, not
    // repeated parent entries, so `[base, base]` becomes a single edge and the history stays linear.
    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
*  👉·06470d7 (⌂) ►main[🌳]
*  🏁·86719d5 (⌂)
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

    let ws = workspace_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·960152d (⌂) ►bottom, ►main[🌳], ►top
layout:
  empty chain anchors: 960152d^
"#]]
    );
    assert_eq!(
        ws.entrypoint_commit_id()?,
        Some(tip),
        "a checked-out empty ordered branch still points at the bottom commit"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:top <> ✓!
└── ≡:top {1}
    ├── :top
    └── :bottom
        └── ·960152d ►main[🌳]

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

    let ws = workspace_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/bottom",
        ["refs/heads/top", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·960152d (⌂) ►bottom, ►main[🌳], ►top
layout:
  empty chain anchors: 960152d^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:bottom <> ✓!
└── ≡:bottom {1}
    └── :bottom
        └── ·960152d ►main[🌳]

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

    let ws = workspace_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/middle", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·960152d (⌂) ►bottom, ►main[🌳], ►middle, ►top
layout:
  empty chain anchors: 960152d^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:top <> ✓!
└── ≡:top {1}
    ├── :top
    ├── :middle
    └── :bottom
        └── ·960152d ►main[🌳]

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

    let ws = workspace_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/missing", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·960152d (⌂) ►bottom, ►main[🌳], ►top
layout:
  empty chain anchors: 960152d^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:top <> ✓!
└── ≡:top {1}
    ├── :top
    └── :bottom
        └── ·960152d ►main[🌳]

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

    let ws = workspace_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/top",
        ["refs/heads/top", "refs/heads/bottom"],
    )?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·5cd63e5 (⌂) ►top
*  🏁·fa91c94 (⌂) ►bottom, ►main[🌳]
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:top <> ✓!
└── ≡:top {1}
    └── :top
        ├── ·5cd63e5
        └── ·fa91c94 ►bottom, ►main[🌳]

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

    let ws = workspace_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/empty-top",
        [
            "refs/heads/empty-top",
            "refs/heads/commit-branch",
            "refs/heads/bottom",
        ],
    )?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:empty-top <> ✓!
└── ≡:empty-top {1}
    ├── :empty-top
    ├── :commit-branch
    │   └── ·4782705
    ├── :bottom
    │   └── ·dbc3a4c
    └── :main[🌳]
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

    let ws = workspace_with_branch_order(
        &repo,
        &*meta,
        "refs/heads/commit-branch",
        [
            "refs/heads/commit-branch",
            "refs/heads/empty-top",
            "refs/heads/empty-low",
            "refs/heads/base",
        ],
    )?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:commit-branch <> ✓!
└── ≡:commit-branch {1}
    ├── :commit-branch
    │   └── ·5380c0a
    ├── :empty-top
    ├── :empty-low
    ├── :base
    │   └── ·a5cd64d
    └── :main[🌳]
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

    let ws = workspace_with_branch_orders(
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
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·960152d (⌂) ►bottom, ►main[🌳], ►other-bottom, ►other-top, ►top
layout:
  empty chain anchors: 960152d^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:top <> ✓!
└── ≡:top {1}
    ├── :top
    └── :bottom
        └── ·960152d ►main[🌳], ►other-bottom, ►other-top

"#]]
    );
    Ok(())
}

/// The single-branch-mode e2e shape: `refs/heads/gitbutler/workspace` still exists (left at
/// the target base by `but setup`), so the build takes the managed path — yet HEAD is on a
/// plain branch `top` that points at the same commit as `bottom`, with a persisted
/// `top` → `bottom` ordering. The ordering must apply on the managed path too, rendering
/// `top` as an empty segment above `bottom` instead of one combined branch.
#[test]
fn ad_hoc_same_tip_pair_above_commit_run_with_target() -> anyhow::Result<()> {
    let (tmp, repo) = empty_repo()?;
    let base = commit(&repo, "base")?;
    let c1 = commit_with_parent(&repo, "first", base)?;
    let c2 = commit_with_parent(&repo, "second", c1)?;
    let seed = commit_with_parent(&repo, "third", c2)?;
    repo.commit(
        "refs/heads/gitbutler/workspace",
        "GitButler Workspace Commit",
        repo.object_hash().empty_tree(),
        Some(base),
    )?;
    create_branches(
        &repo,
        base,
        [
            "refs/heads/master",
            "refs/remotes/origin/master",
            "refs/heads/gitbutler/target",
        ],
    )?;
    create_branches(&repo, seed, ["refs/heads/top", "refs/heads/bottom"])?;
    let meta = in_memory_meta(tmp.as_ref())?;

    let entrypoint_ref = ref_name("refs/heads/top");
    let overlay = Overlay::default()
        .with_branch_stack_order_override(["refs/heads/top", "refs/heads/bottom"].map(ref_name));
    let ws = Workspace::from_tip(
        seed.attach(&repo),
        Some(entrypoint_ref),
        &*meta,
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some(ref_name("refs/remotes/origin/master")),
            target_commit_id: Some(base),
            push_remote: Some("origin".into()),
        },
        standard_options(),
    )?
    .redo(&repo, &*meta, overlay)?
    .validated()?;

    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·96fee4c (⌂) ►bottom, ►main[🌳], ►top
*  ·a6448ba (⌂)
*  ·ff07efd (⌂)
*  🏁·86719d5 (⌂) ►gitbutler/target, ►master, ►origin/master <> origin/master
layout:
  empty chain anchors: 96fee4c^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:top <> ✓refs/remotes/origin/master on 86719d5
└── ≡:top on 86719d5 {1}
    ├── :top
    └── :bottom
        ├── ·96fee4c ►main[🌳]
        ├── ·a6448ba
        └── ·ff07efd

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

use crate::walk::utils::{in_memory_meta, standard_options_with_extra_target};

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

fn workspace_with_branch_order<const N: usize>(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    entrypoint_ref: &str,
    order: [&str; N],
) -> anyhow::Result<Workspace> {
    workspace_with_branch_orders(repo, meta, entrypoint_ref, &[&order])
}

fn workspace_with_branch_orders(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    entrypoint_ref: &str,
    orders: &[&[&str]],
) -> anyhow::Result<Workspace> {
    let entrypoint_ref = ref_name(entrypoint_ref);
    let tip = repo
        .find_reference(entrypoint_ref.as_ref())?
        .peel_to_id()?
        .detach();
    let mut overlay = Overlay::default();
    for order in orders {
        overlay = overlay.with_branch_stack_order_override(order.iter().copied().map(ref_name));
    }
    Workspace::from_tip(
        tip.attach(repo),
        Some(entrypoint_ref),
        meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .redo(repo, meta, overlay)
}
