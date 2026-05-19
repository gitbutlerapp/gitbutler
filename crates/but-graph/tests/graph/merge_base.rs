use anyhow::Context;
use but_graph::{
    CommitFlags, FirstParent, Graph, Segment, SegmentIndex, SegmentRelation, init::Tip,
};
use but_testsupport::{graph_tree, visualize_commit_graph_all};

use crate::init::{read_only_in_memory_scenario, standard_options};

#[test]
fn find_git_merge_base_handles_duplicate_queue_entries_and_redundant_bases() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

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

    Ok(())
}

#[test]
fn relation_between_matches_merge_base_in_redundant_ancestor_case() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;

    assert_eq!(graph.relation_between(a, merged), SegmentRelation::Ancestor);
    assert_eq!(
        graph.relation_between(merged, a),
        SegmentRelation::Descendant
    );
    assert_eq!(graph.relation_between(a, c), SegmentRelation::Diverged);
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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

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
        standard_options(),
    )?
    .validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:2[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:0[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:7[3]:main
            │       │           └── 🏁·965998b (⌂|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:7: (main)
            └── ►:1[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:5[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:7: (main)
                    └── ►:6[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:7: (main)
    ");

    assert_eq!(
        graph.find_commit_ids_reachable_from_a_not_b(merged_id, a_id, FirstParent::No)?,
        ids_by_revs(&repo, &["merged", "C", "C^1", "C^2"])?
    );
    assert_eq!(
        graph.find_merge_base_octopus_by_commit_id([a_id, c_id, merged_id])?,
        Some(main_id)
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
        standard_options(),
    )?
    .validated()?;

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉►:2[0]:merged[🌳]
        └── ·8a6c109 (⌂|1)
            ├── ►:1[1]:A
            │   └── ·62b409a (⌂|1)
            │       ├── ►:3[2]:anon:
            │       │   └── ·592abec (⌂|1)
            │       │       └── ►:0[3]:main
            │       │           └── 🏁·965998b (⌂|✓|1)
            │       └── ►:4[2]:B
            │           └── ·f16dddf (⌂|1)
            │               └── →:0: (main)
            └── ►:5[1]:C
                └── ·7ed512a (⌂|1)
                    ├── ►:6[2]:anon:
                    │   └── ·35ee481 (⌂|1)
                    │       └── →:0: (main)
                    └── ►:7[2]:D
                        └── ·ecb1877 (⌂|1)
                            └── →:0: (main)
    ");

    let (main_seg, main) = graph
        .segment_and_commit_by_ref_name(ref_name("refs/heads/main")?.as_ref())
        .expect("main segment");
    assert!(
        main.flags.contains(CommitFlags::Integrated),
        "integrated tips should be queued before reachable tips even if the caller provides them last"
    );
    assert_eq!(
        main_seg.id.index(),
        0,
        "schedule first, hence the first node"
    );

    Ok(())
}

#[test]
fn relation_between_handles_identity_and_disjoint_segments() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let mut graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    assert_eq!(
        graph.relation_between(main, main),
        SegmentRelation::Identity
    );

    let orphan = graph.insert_segment(Segment {
        generation: 0,
        ..Default::default()
    });
    assert_eq!(
        graph.relation_between(main, orphan),
        SegmentRelation::Disjoint
    );
    assert_eq!(graph.find_merge_base_octopus([main, orphan]), None);
    assert_eq!(graph.find_merge_base_octopus([main, orphan, a]), None);

    Ok(())
}

#[test]
fn merge_base_apis_can_resolve_segments_by_first_commit_id() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let merged = segment_id_by_ref_name(&graph, "refs/heads/merged")?;
    let a = segment_id_by_ref_name(&graph, "refs/heads/A")?;
    let c = segment_id_by_ref_name(&graph, "refs/heads/C")?;
    let main = segment_id_by_ref_name(&graph, "refs/heads/main")?;

    let merged_id = graph[merged].tip().expect("commit");
    let a_id = graph[a].tip().expect("commit");
    let c_id = graph[c].tip().expect("commit");
    let main_id = graph[main].tip().expect("commit");

    assert_eq!(
        graph.relation_between_by_commit_id(a_id, merged_id)?,
        SegmentRelation::Ancestor
    );
    assert_eq!(
        graph.find_merge_base_by_commit_id(merged_id, a_id)?,
        Some(a_id)
    );
    assert_eq!(
        graph.find_merge_base_octopus_by_commit_id([a_id, c_id, merged_id])?,
        Some(main_id)
    );

    assert!(
        graph
            .find_merge_base_by_commit_id(repo.object_hash().null(), main_id)
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
