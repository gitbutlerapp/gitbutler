use crate::graph_tree;
use crate::init::{StackState, add_stack_with_segments, add_workspace};
use crate::init::{read_only_in_memory_scenario, standard_options};
use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;
use gitbutler_stack::StackId;
use gix::Repository;

#[test]
fn single_stack_ambigous() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack-ambiguous")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 20de6ee (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 70e9a36 (B) with-ref
    * 320e105 (tag: without-ref) segment-B
    * 2a31450 (ambiguous-01, B-empty) segment-B~1
    * 70bde6b (A-empty-03, A-empty-02, A-empty-01, A) segment-A
    * fafd9d0 (origin/main, new-B, new-A, main) init
    ");

    // Just a workspace, no additional ref information.
    // As the segments are ambiguous, there are many unnamed segments.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:refs/heads/gitbutler/workspace
        └── 🔵20de6ee (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:1:refs/heads/B
                ├── 🔵70e9a36 (InWorkspace)❱"with-ref"
                ├── 🔵320e105 (InWorkspace)❱"segment-B" ►tags/without-ref
                ├── 🔵2a31450 (InWorkspace)❱"segment-B~1" ►B-empty, ►ambiguous-01
                ├── 🔵70bde6b (InWorkspace)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A, ►new-B
    "#);

    // There is always a segment for the entrypoint, and code working with the graph
    // deals with that naturally.
    let (without_ref_id, ref_name) = id_at(&repo, "without-ref");
    let graph = Graph::from_commit_traversal(without_ref_id, ref_name, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:refs/heads/gitbutler/workspace
        └── 🔵20de6ee (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:2:refs/heads/B
                └── 🔵70e9a36 (InWorkspace)❱"with-ref"
                    └── 👉►:0:refs/tags/without-ref
                        ├── 🔵320e105 (InWorkspace)❱"segment-B"
                        ├── 🔵2a31450 (InWorkspace)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        ├── 🔵70bde6b (InWorkspace)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                        └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A, ►new-B
    "#);

    // We don't have to give it a ref-name
    let graph = Graph::from_commit_traversal(without_ref_id, None, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:refs/heads/gitbutler/workspace
        └── 🔵20de6ee (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:2:refs/heads/B
                └── 🔵70e9a36 (InWorkspace)❱"with-ref"
                    └── ►:0:anon:
                        ├── 👉🔵320e105 (InWorkspace)❱"segment-B" ►tags/without-ref
                        ├── 🔵2a31450 (InWorkspace)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        ├── 🔵70bde6b (InWorkspace)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                        └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A, ►new-B
    "#);

    // Putting the entrypoint onto a commit in an anonymous segment makes no difference.
    let (b_id_1, tag_ref_name) = id_at(&repo, "B-empty");
    let graph = Graph::from_commit_traversal(b_id_1, None, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:refs/heads/gitbutler/workspace
        └── 🔵20de6ee (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:2:refs/heads/B
                ├── 🔵70e9a36 (InWorkspace)❱"with-ref"
                └── 🔵320e105 (InWorkspace)❱"segment-B" ►tags/without-ref
                    └── ►:0:anon:
                        ├── 👉🔵2a31450 (InWorkspace)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        ├── 🔵70bde6b (InWorkspace)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                        └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A, ►new-B
    "#);

    // If we pass an entrypoint ref name, it will be used as segment name.
    let graph = Graph::from_commit_traversal(b_id_1, tag_ref_name, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:refs/heads/gitbutler/workspace
        └── 🔵20de6ee (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:2:refs/heads/B
                ├── 🔵70e9a36 (InWorkspace)❱"with-ref"
                └── 🔵320e105 (InWorkspace)❱"segment-B" ►tags/without-ref
                    └── 👉►:0:refs/heads/B-empty
                        ├── 🔵2a31450 (InWorkspace)❱"segment-B~1" ►ambiguous-01
                        ├── 🔵70bde6b (InWorkspace)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                        └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A, ►new-B
    "#);
    Ok(())
}

#[test]
fn single_stack_ws_insertions() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack-ambiguous")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 20de6ee (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 70e9a36 (B) with-ref
    * 320e105 (tag: without-ref) segment-B
    * 2a31450 (ambiguous-01, B-empty) segment-B~1
    * 70bde6b (A-empty-03, A-empty-02, A-empty-01, A) segment-A
    * fafd9d0 (origin/main, new-B, new-A, main) init
    ");
    // Fully defined workspace with multiple empty segments on top of each other.
    // Notably the order doesn't match, 'B-empty' is after 'B', but we use it anyway for segment definition.
    // On single commits, the desired order fully defines where stacks go.
    meta.data_mut().branches.clear();
    // Note that this does match the single-stack (one big segment) configuration we actually have.
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B-empty",
        StackState::InWorkspace,
        &[
            "B",
            "A-empty-03",
            /* A-empty-02 purposefully missing */ "not-A-empty-02",
            "A-empty-01",
            "A",
        ],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:refs/heads/gitbutler/workspace
        └── 🔵20de6ee (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:1:refs/heads/B
                ├── 🔵70e9a36 (InWorkspace)❱"with-ref"
                └── 🔵320e105 (InWorkspace)❱"segment-B" ►tags/without-ref
                    └── ►:2:refs/heads/B-empty
                        ├── 🔵2a31450 (InWorkspace)❱"segment-B~1" ►ambiguous-01
                        └── ►:3:refs/heads/A-empty-03
                            └── ►:4:refs/heads/A-empty-01
                                └── ►:5:refs/heads/A
                                    ├── 🔵70bde6b (InWorkspace)❱"segment-A" ►A-empty-02
                                    └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A, ►new-B
    "#);
    graph.eprint_dot_graph();

    // TODO: do more complex new-stack segmentation
    // // Note that this doesn't match the single-stack (one big segment) configuration we actually have.
    // // Only stack B should be used here.
    // meta.data_mut().branches.clear();
    // add_stack_with_segments(
    //     &mut meta,
    //     StackId::from_number_for_testing(0),
    //     "B-empty",
    //     StackState::InWorkspace,
    //     &["B"],
    // );
    // add_stack_with_segments(
    //     &mut meta,
    //     StackId::from_number_for_testing(1),
    //     "A-empty-03",
    //     StackState::InWorkspace,
    //     &["A-empty-02", "A-empty-01", "A"],
    // );

    // let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    // insta::assert_snapshot!(graph_tree(&graph), @r#"
    // └── 👉►►►refs/heads/gitbutler/workspace
    //     ├── 🔵2c12d75 (InWorkspace)❱"GitButler Workspace Commit"
    //     ├── 🔵320e105 (InWorkspace)❱"segment-B" ►B, ►ambiguous-02
    //     ├── 🔵2a31450 (InWorkspace)❱"segment-B~1" ►B-empty, ►ambiguous-01
    //     ├── 🔵70bde6b (InWorkspace)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    //     └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A, ►new-B
    // "#);

    // // Define only some of the branches, it should figure that out.
    // meta.data_mut().branches.clear();
    // add_stack_with_segments(
    //     &mut meta,
    //     StackId::from_number_for_testing(0),
    //     "A",
    //     StackState::InWorkspace,
    //     &["A-empty-01"],
    // );
    // add_stack_with_segments(
    //     &mut meta,
    //     StackId::from_number_for_testing(1),
    //     "B-empty",
    //     StackState::InWorkspace,
    //     &["B"],
    // );
    //
    // // TODO: show how the entrypoint affects the segmentation, by design.
    // let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    // insta::assert_snapshot!(graph_tree(&graph), @r#"
    // └── 👉►►►refs/heads/gitbutler/workspace
    //     ├── 🔵2c12d75 (InWorkspace)❱"GitButler Workspace Commit"
    //     ├── 🔵320e105 (InWorkspace)❱"segment-B" ►B, ►ambiguous-02
    //     ├── 🔵2a31450 (InWorkspace)❱"segment-B~1" ►B-empty, ►ambiguous-01
    //     └── 🔵70bde6b (InWorkspace)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    //         └── ►refs/heads/main
    //             └── 🔵fafd9d0 (InWorkspace)❱"init"
    // "#);
    Ok(())
}

#[test]
fn single_stack() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 2c12d75 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 320e105 (B) segment-B
    * 2a31450 (B-sub) segment-B~1
    * 70bde6b (A) segment-A
    * fafd9d0 (origin/main, new-A, main) init
    ");

    // Just a workspace, no additional ref information.
    // It segments across the unambiguous ref names.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:refs/heads/gitbutler/workspace
        └── 🔵2c12d75 (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:1:refs/heads/B
                └── 🔵320e105 (InWorkspace)❱"segment-B"
                    └── ►:2:refs/heads/B-sub
                        └── 🔵2a31450 (InWorkspace)❱"segment-B~1"
                            └── ►:3:refs/heads/A
                                ├── 🔵70bde6b (InWorkspace)❱"segment-A"
                                └── 🔵fafd9d0 (InWorkspace)❱"init" ►main, ►new-A
    "#);

    meta.data_mut().branches.clear();
    // Just repeat the existing segment verbatim, but also add a new unborn stack
    // TODO: make this work: unborn stack
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B",
        StackState::InWorkspace,
        &["B-sub", "A"],
    );
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(1),
        "new-A",
        StackState::InWorkspace,
        &[],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:refs/heads/gitbutler/workspace
        └── 🔵2c12d75 (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:1:refs/heads/B
                └── 🔵320e105 (InWorkspace)❱"segment-B"
                    └── ►:2:refs/heads/B-sub
                        └── 🔵2a31450 (InWorkspace)❱"segment-B~1"
                            └── ►:3:refs/heads/A
                                └── 🔵70bde6b (InWorkspace)❱"segment-A"
                                    └── ►:4:refs/heads/new-A
                                        └── 🔵fafd9d0 (InWorkspace)❱"init" ►main
    "#);

    Ok(())
}

#[test]
fn minimal_merge_no_refs() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("ws/dual-merge-no-refs")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 47e1cf1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   f40fb16 Merge branch 'C' into merge-2
    |\  
    | * c6d714c C
    * | 450c58a D
    |/  
    *   0cc5a6f Merge branch 'A' into merge
    |\  
    | * e255adc A
    * | 7fdb58d B
    |/  
    * fafd9d0 init
    ");

    // Without hints.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:refs/heads/gitbutler/workspace
        └── 🔵47e1cf1❱"GitButler Workspace Commit"
            └── ►:1:anon:
                └── 🔵f40fb16❱"Merge branch \'C\' into merge-2"
                    ├── ►:3:anon:
                    │   └── 🔵c6d714c❱"C"
                    │       └── ►:4:anon:
                    │           └── 🔵0cc5a6f❱"Merge branch \'A\' into merge"
                    │               ├── ►:6:anon:
                    │               │   └── 🔵e255adc❱"A"
                    │               │       └── ►:7:anon:
                    │               │           └── 🔵fafd9d0❱"init"
                    │               └── ►:5:anon:
                    │                   └── 🔵7fdb58d❱"B"
                    │                       └── ERROR: Reached segment :7: for a second time: None
                    └── ►:2:anon:
                        └── 🔵450c58a❱"D"
                            └── ERROR: Reached segment :4: for a second time: None
    "#);
    Ok(())
}

#[test]
fn segment_on_each_incoming_connection() -> anyhow::Result<()> {
    // Validate that the graph is truly having segments whenever there is an incoming connection.
    // This is required to not need special edge-weights.
    let (repo, mut meta) = read_only_in_memory_scenario("ws/graph-splitting")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 98c5aba (entrypoint) C
    * 807b6ce B
    * 6d05486 A
    | * b6917c7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * f7fe830 (main) other-2
    |/  
    * b688f2d other-1
    * fafd9d0 init
    ");

    // Without hints - needs to split `refs/heads/main` at `b688f2d`
    let (id, name) = id_at(&repo, "entrypoint");
    add_workspace(&mut meta);
    let graph = Graph::from_commit_traversal(id, name, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:refs/heads/entrypoint
    │   ├── 🔵98c5aba❱"C"
    │   ├── 🔵807b6ce❱"B"
    │   └── 🔵6d05486❱"A"
    │       └── ►:3:anon:
    │           ├── 🔵b688f2d (InWorkspace)❱"other-1"
    │           └── 🔵fafd9d0 (InWorkspace)❱"init"
    └── ►►►:1:refs/heads/gitbutler/workspace
        └── 🔵b6917c7 (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:2:refs/heads/main
                └── 🔵f7fe830 (InWorkspace)❱"other-2"
                    └── ERROR: Reached segment :3: for a second time: None
    "#);
    Ok(())
}

#[test]
fn minimal_merge() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/dual-merge")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 47e1cf1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   f40fb16 (merge-2) Merge branch 'C' into merge-2
    |\  
    | * c6d714c (C) C
    * | 450c58a (D) D
    |/  
    *   0cc5a6f (merge, empty-2-on-merge, empty-1-on-merge) Merge branch 'A' into merge
    |\  
    | * e255adc (A) A
    * | 7fdb58d (B) B
    |/  
    * fafd9d0 (origin/main, main) init
    ");

    // Without hints.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:refs/heads/gitbutler/workspace
        └── 🔵47e1cf1❱"GitButler Workspace Commit"
            └── ►:1:refs/heads/merge-2
                └── 🔵f40fb16❱"Merge branch \'C\' into merge-2"
                    ├── ►:3:refs/heads/C
                    │   └── 🔵c6d714c❱"C"
                    │       └── ►:4:anon:
                    │           └── 🔵0cc5a6f❱"Merge branch \'A\' into merge" ►empty-1-on-merge, ►empty-2-on-merge, ►merge
                    │               ├── ►:6:refs/heads/A
                    │               │   └── 🔵e255adc❱"A"
                    │               │       └── ►:7:refs/heads/main
                    │               │           └── 🔵fafd9d0❱"init"
                    │               └── ►:5:refs/heads/B
                    │                   └── 🔵7fdb58d❱"B"
                    │                       └── ERROR: Reached segment :7: for a second time: Some("refs/heads/main")
                    └── ►:2:refs/heads/D
                        └── 🔵450c58a❱"D"
                            └── ERROR: Reached segment :4: for a second time: None
    "#);

    // There is empty stacks on top of `merge`, and they need to be connected to the incoming segments and the outgoing ones.
    // This also would leave the original segment empty unless we managed to just put empty stacks on top.
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "empty-2-on-merge",
        StackState::InWorkspace,
        &["empty-1-on-merge", "merge"],
    );
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:refs/heads/gitbutler/workspace
        └── 🔵47e1cf1 (InWorkspace)❱"GitButler Workspace Commit"
            └── ►:1:refs/heads/merge-2
                └── 🔵f40fb16 (InWorkspace)❱"Merge branch \'C\' into merge-2"
                    ├── ►:3:refs/heads/C
                    │   └── 🔵c6d714c (InWorkspace)❱"C"
                    │       └── ►:8:refs/heads/empty-2-on-merge
                    │           └── ►:9:refs/heads/empty-1-on-merge
                    │               └── ►:10:refs/heads/merge
                    │                   └── 🔵0cc5a6f (InWorkspace)❱"Merge branch \'A\' into merge"
                    │                       ├── ►:5:refs/heads/B
                    │                       │   └── 🔵7fdb58d (InWorkspace)❱"B"
                    │                       │       └── ►:7:refs/heads/main
                    │                       │           └── 🔵fafd9d0 (InWorkspace)❱"init"
                    │                       └── ►:6:refs/heads/A
                    │                           └── 🔵e255adc (InWorkspace)❱"A"
                    │                               └── ERROR: Reached segment :7: for a second time: Some("refs/heads/main")
                    └── ►:2:refs/heads/D
                        └── 🔵450c58a (InWorkspace)❱"D"
                            └── ERROR: Reached segment :8: for a second time: Some("refs/heads/empty-2-on-merge")
    "#);
    Ok(())
}

#[test]
fn just_init_with_branches() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    // Note the dedicated workspace branch without a workspace commit.
    // All is fair game, and we use it to validate 'empty parent branch handling after new children took the commit'.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init");

    // Without hints.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:refs/heads/gitbutler/workspace
        └── 👉►:0:refs/heads/main
            └── 🔵fafd9d0 (InWorkspace)❱"init" ►A, ►B, ►C, ►D, ►E, ►F, ►main
    "#);

    // The simplest possible setup where we can define how the workspace should look like,
    // in terms of dependent and independent virtual segments.
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "C",
        StackState::InWorkspace,
        &["B", "A"],
    );
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(1),
        "D",
        StackState::InWorkspace,
        &["E", "F"],
    );
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    // TODO: where is the segmentation of D E F in a separate stack?
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:refs/heads/gitbutler/workspace
        └── 👉►:0:refs/heads/main
            └── ►:2:refs/heads/C
                └── ►:3:refs/heads/B
                    └── ►:4:refs/heads/A
                        └── 🔵fafd9d0 (InWorkspace)❱"init" ►D, ►E, ►F, ►main
    "#);
    Ok(())
}

fn id_at<'repo>(repo: &'repo Repository, name: &str) -> (gix::Id<'repo>, gix::refs::FullName) {
    let mut rn = repo
        .find_reference(name)
        .expect("statically known reference exists");
    let id = rn.peel_to_id_in_place().expect("must be valid reference");
    (id, rn.inner.name)
}
