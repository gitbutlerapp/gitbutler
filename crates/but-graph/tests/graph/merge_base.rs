use anyhow::Context;
use but_graph::{CommitFlags, FirstParent, Graph, Segment, SegmentIndex, init::Tip};
use but_testsupport::{graph_tree, visualize_commit_graph_all};

use crate::init::{read_only_in_memory_scenario, standard_options};

#[test]
fn find_git_merge_base_handles_duplicate_queue_entries_and_redundant_bases() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;
    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;

    // merged -> (A,C) -> ... -> main causes the walk from merged to queue shared ancestors repeatedly.
    assert_eq!(graph.find_merge_base(merged, main), Some(main));

    // For (merged, A), both A and main are common in ancestry, but A is the nearest one.
    assert_eq!(graph.find_merge_base(merged, a), Some(a));
    assert_ne!(graph.find_merge_base(merged, a), Some(main));

    // Independent branches under the same merge should converge at main.
    assert_eq!(graph.find_merge_base(a, c), Some(main));
    assert_eq!(graph.find_merge_base_octopus([a, c, merged]), Some(main));

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂)
            │       ├── ►:4[2]:anon:
            │       │   └── ·592abec (⌂)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂)
            │       └── ►:6[2]:B
            │           └── ·f16dddf (⌂)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂)
                    ├── ►:3[2]:anon:
                    │   └── ·35ee481 (⌂)
                    │       └── →:7: (main)
                    └── ►:5[2]:D
                        └── ·ecb1877 (⌂)
                            └── →:7: (main)
    ");

    Ok(())
}

#[test]
fn merge_base_in_redundant_ancestor_case() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;

    // `a` is an ancestor of `merged` (either order), while `a` and `c` diverge: they share
    // history without one being the base of the other.
    assert_eq!(graph.find_merge_base(a, merged), Some(a));
    assert_eq!(graph.find_merge_base(merged, a), Some(a));
    let base = graph.find_merge_base(a, c);
    assert!(base.is_some_and(|base| base != a && base != c));
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂)
            │       ├── ►:4[2]:anon:
            │       │   └── ·592abec (⌂)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂)
            │       └── ►:6[2]:B
            │           └── ·f16dddf (⌂)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂)
                    ├── ►:3[2]:anon:
                    │   └── ·35ee481 (⌂)
                    │       └── →:7: (main)
                    └── ►:5[2]:D
                        └── ·ecb1877 (⌂)
                            └── →:7: (main)
    ");

    Ok(())
}

#[test]
fn reachable_difference_returns_commits_in_traversal_order() -> anyhow::Result<()> {
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
    )?
    .validated()?;

    let merged_id = repo.rev_parse_single("merged")?.detach();
    let a_id = repo.rev_parse_single("A")?.detach();

    let ids = graph.find_commit_ids_reachable_from_a_not_b(merged_id, a_id, FirstParent::No)?;
    assert_eq!(ids, ids_by_revs(&repo, &["merged", "C", "C^1", "C^2"])?);
    let first_parent_ids =
        graph.find_commit_ids_reachable_from_a_not_b(merged_id, a_id, FirstParent::Yes)?;
    assert_eq!(first_parent_ids, ids_by_revs(&repo, &["merged"])?);

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;

    let commits = graph.find_commits_reachable_from_a_not_b(merged, a, FirstParent::No);
    assert_eq!(
        commits.iter().map(|commit| commit.id).collect::<Vec<_>>(),
        ids
    );
    let first_parent_commits =
        graph.find_commits_reachable_from_a_not_b(merged, a, FirstParent::Yes);
    assert_eq!(
        first_parent_commits
            .iter()
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        first_parent_ids
    );
    assert!(
        graph
            .find_commit_ids_reachable_from_a_not_b(a_id, a_id, FirstParent::No)?
            .is_empty(),
        "self-exclusion means nothing is returned"
    );

    Ok(())
}

#[test]
fn explicit_traversal_tips_include_unnamed_revisions() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = repo.rev_parse_single("merged")?.detach();
    let a_id = repo.rev_parse_single("A")?.detach();
    let c_id = repo.rev_parse_single("C")?.detach();
    let main_id = repo.rev_parse_single("main")?.detach();

    let graph = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(merged_id, None),
            Tip::reachable(a_id, None),
            Tip::reachable(c_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂)
            │       ├── ►:4[2]:anon:
            │       │   └── ·592abec (⌂)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂)
            │       └── ►:6[2]:B
            │           └── ·f16dddf (⌂)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂)
                    ├── ►:3[2]:anon:
                    │   └── ·35ee481 (⌂)
                    │       └── →:7: (main)
                    └── ►:5[2]:D
                        └── ·ecb1877 (⌂)
                            └── →:7: (main)
    ");

    assert_eq!(
        graph.find_commit_ids_reachable_from_a_not_b(merged_id, a_id, FirstParent::No)?,
        ids_by_revs(&repo, &["merged", "C", "C^1", "C^2"])?
    );
    assert_eq!(
        graph.find_merge_base_octopus([
            graph.segment_id_by_commit_id(a_id)?,
            graph.segment_id_by_commit_id(c_id)?,
            graph.segment_id_by_commit_id(merged_id)?,
        ]),
        Some(graph.segment_id_by_commit_id(main_id)?)
    );

    Ok(())
}

#[test]
fn explicit_traversal_prioritizes_integrated_tips_independent_of_input_order() -> anyhow::Result<()>
{
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let merged_id = repo.rev_parse_single("merged")?.detach();
    let a_id = repo.rev_parse_single("A")?.detach();
    let main_id = repo.rev_parse_single("main")?.detach();

    let graph = Graph::from_commit_traversal_tips(
        &repo,
        [
            Tip::entrypoint(merged_id, None),
            Tip::reachable(a_id, None),
            Tip::integrated(main_id, None),
        ],
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:0[0]:merged[🌳]
        └── ·8a6c109 (⌂)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂)
            │       ├── ►:4[2]:anon:
            │       │   └── ·592abec (⌂)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂|✓)
            │       └── ►:6[2]:B
            │           └── ·f16dddf (⌂)
            │               └── →:7: (main)
            └── ►:2[1]:C
                └── ·7ed512a (⌂)
                    ├── ►:3[2]:anon:
                    │   └── ·35ee481 (⌂)
                    │       └── →:7: (main)
                    └── ►:5[2]:D
                        └── ·ecb1877 (⌂)
                            └── →:7: (main)
    ");

    let (_main_seg, main) = graph
        .segment_and_commit_by_ref_name(ref_name("refs/heads/main")?.as_ref())
        .expect("main segment");
    // Segment ids are builder-assigned (the snapshot above pins them); what matters is that
    // integrated tips are queued before reachable tips so their flags propagate.
    assert!(
        main.flags.contains(CommitFlags::Integrated),
        "integrated tips should be queued before reachable tips even if the caller provides them last"
    );

    Ok(())
}

#[test]
fn merge_base_handles_identity_and_disjoint_segments() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let mut graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    assert_eq!(graph.find_merge_base(main, main), Some(main));

    let orphan = graph.insert_segment(Segment {
        ..Default::default()
    });
    assert_eq!(graph.find_merge_base(main, orphan), None);
    assert_eq!(graph.find_merge_base_octopus([main, orphan]), None);
    assert_eq!(graph.find_merge_base_octopus([main, orphan, a]), None);

    Ok(())
}

#[test]
fn merge_base_apis_can_resolve_segments_by_first_commit_id() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;
    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;

    assert_eq!(graph.find_merge_base(merged, a), Some(a));
    assert_eq!(graph.find_merge_base_octopus([a, c, merged]), Some(main));

    assert!(
        graph
            .segment_id_by_commit_id(repo.object_hash().null())
            .is_err()
    );

    Ok(())
}

fn segment_id_by_ref_name(graph: &Graph, name: &str) -> anyhow::Result<SegmentIndex> {
    let full_name = ref_name(name)?;
    graph
        .segment_by_ref_name(full_name.as_ref())
        .map(|s| s.id)
        .ok_or_else(|| anyhow::anyhow!("missing segment for {name}"))
}

fn ref_name(name: &str) -> anyhow::Result<gix::refs::FullName> {
    name.try_into()
        .with_context(|| format!("invalid ref name {name}"))
}

fn ids_by_revs(repo: &gix::Repository, revs: &[&str]) -> anyhow::Result<Vec<gix::ObjectId>> {
    revs.iter()
        .map(|rev| Ok(repo.rev_parse_single(*rev)?.detach()))
        .collect()
}
