use crate::graph_tree;
use crate::init::utils::add_workspace_without_target;
use crate::init::{StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev};
use crate::init::{read_only_in_memory_scenario, standard_options};
use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;
use gitbutler_stack::StackId;

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
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ·20de6ee (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:2:B
                ├── ·70e9a36 (⌂|🏘️)❱"with-ref"
                ├── ·320e105 (⌂|🏘️)❱"segment-B" ►tags/without-ref
                ├── ·2a31450 (⌂|🏘️)❱"segment-B~1" ►B-empty, ►ambiguous-01
                └── ·70bde6b (⌂|🏘️)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                    └── ►:1:origin/main
                        └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main, ►new-A, ►new-B
    "#);

    // There is always a segment for the entrypoint, and code working with the graph
    // deals with that naturally.
    let (without_ref_id, ref_name) = id_at(&repo, "without-ref");
    let graph = Graph::from_commit_traversal(without_ref_id, ref_name, &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:gitbutler/workspace
        └── ·20de6ee (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:3:B
                └── ·70e9a36 (⌂|🏘️)❱"with-ref"
                    └── 👉►:0:tags/without-ref
                        ├── ·320e105 (⌂|🏘️)❱"segment-B"
                        ├── ·2a31450 (⌂|🏘️)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main, ►new-A, ►new-B
    "#);

    // We don't have to give it a ref-name
    let graph = Graph::from_commit_traversal(without_ref_id, None, &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:gitbutler/workspace
        └── ·20de6ee (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:3:B
                └── ·70e9a36 (⌂|🏘️)❱"with-ref"
                    └── ►:0:anon:
                        ├── 👉·320e105 (⌂|🏘️)❱"segment-B" ►tags/without-ref
                        ├── ·2a31450 (⌂|🏘️)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main, ►new-A, ►new-B
    "#);

    // Putting the entrypoint onto a commit in an anonymous segment makes no difference.
    let (b_id_1, tag_ref_name) = id_at(&repo, "B-empty");
    let graph =
        Graph::from_commit_traversal(b_id_1, None, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:gitbutler/workspace
        └── ·20de6ee (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:3:B
                ├── ·70e9a36 (⌂|🏘️)❱"with-ref"
                └── ·320e105 (⌂|🏘️)❱"segment-B" ►tags/without-ref
                    └── ►:0:anon:
                        ├── 👉·2a31450 (⌂|🏘️)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main, ►new-A, ►new-B
    "#);

    // If we pass an entrypoint ref name, it will be used as segment name (despite ambiguous without it)
    let graph = Graph::from_commit_traversal(b_id_1, tag_ref_name, &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►►►:1:gitbutler/workspace
        └── ·20de6ee (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:3:B
                ├── ·70e9a36 (⌂|🏘️)❱"with-ref"
                └── ·320e105 (⌂|🏘️)❱"segment-B" ►tags/without-ref
                    └── 👉►:0:B-empty
                        ├── ·2a31450 (⌂|🏘️)❱"segment-B~1" ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main, ►new-A, ►new-B
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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ·20de6ee (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:2:B
                ├── ·70e9a36 (⌂|🏘️)❱"with-ref"
                └── ·320e105 (⌂|🏘️)❱"segment-B" ►tags/without-ref
                    └── ►:3:B-empty
                        └── ·2a31450 (⌂|🏘️)❱"segment-B~1" ►ambiguous-01
                            └── ►:4:A-empty-03
                                └── ►:5:A-empty-01
                                    └── ►:6:A
                                        └── ·70bde6b (⌂|🏘️)❱"segment-A" ►A-empty-02
                                            └── ►:1:origin/main
                                                └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main, ►new-A, ►new-B
    "#);

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
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ·2c12d75 (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:2:B
                └── ·320e105 (⌂|🏘️)❱"segment-B"
                    └── ►:3:B-sub
                        └── ·2a31450 (⌂|🏘️)❱"segment-B~1"
                            └── ►:4:A
                                └── ·70bde6b (⌂|🏘️)❱"segment-A"
                                    └── ►:1:origin/main
                                        └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main, ►new-A
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

    // TODO: We shouldn't create the empty stack on top rather than below,
    //       but even then it would be hard to know where to reasonably put it in
    //       as remote tracking branches should keep pointing to their original targets,
    //       maybe?
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ·2c12d75 (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:2:B
                └── ·320e105 (⌂|🏘️)❱"segment-B"
                    └── ►:3:B-sub
                        └── ·2a31450 (⌂|🏘️)❱"segment-B~1"
                            └── ►:4:A
                                └── ·70bde6b (⌂|🏘️)❱"segment-A"
                                    └── ►:1:origin/main
                                        └── ►:5:new-A
                                            └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main
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
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:gitbutler/workspace
        └── ·47e1cf1 (⌂)❱"GitButler Workspace Commit"
            └── ►:1:anon:
                └── ·f40fb16 (⌂)❱"Merge branch \'C\' into merge-2"
                    ├── ►:3:anon:
                    │   └── ·c6d714c (⌂)❱"C"
                    │       └── ►:4:anon:
                    │           └── ·0cc5a6f (⌂)❱"Merge branch \'A\' into merge"
                    │               ├── ►:6:anon:
                    │               │   └── ·e255adc (⌂)❱"A"
                    │               │       └── ►:7:anon:
                    │               │           └── ·fafd9d0 (⌂)❱"init"
                    │               └── ►:5:anon:
                    │                   └── ·7fdb58d (⌂)❱"B"
                    │                       └── →:7:
                    └── ►:2:anon:
                        └── ·450c58a (⌂)❱"D"
                            └── →:4:
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
    let graph = Graph::from_commit_traversal(id, name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:entrypoint
    │   ├── ·98c5aba (⌂)❱"C"
    │   ├── ·807b6ce (⌂)❱"B"
    │   └── ·6d05486 (⌂)❱"A"
    │       └── ►:3:anon:
    │           ├── ·b688f2d (⌂|🏘️)❱"other-1"
    │           └── ·fafd9d0 (⌂|🏘️)❱"init"
    └── ►►►:1:gitbutler/workspace
        └── ·b6917c7 (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:2:main
                └── ·f7fe830 (⌂|🏘️)❱"other-2"
                    └── →:3:
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
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:gitbutler/workspace
    │   └── ·47e1cf1 (⌂)❱"GitButler Workspace Commit"
    │       └── ►:1:merge-2
    │           └── ·f40fb16 (⌂)❱"Merge branch \'C\' into merge-2"
    │               ├── ►:3:C
    │               │   └── ·c6d714c (⌂)❱"C"
    │               │       └── ►:4:anon:
    │               │           └── ·0cc5a6f (⌂)❱"Merge branch \'A\' into merge" ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    │               │               ├── ►:6:A
    │               │               │   └── ·e255adc (⌂)❱"A"
    │               │               │       └── ►:7:main
    │               │               │           └── ·fafd9d0 (⌂)❱"init"
    │               │               └── ►:5:B
    │               │                   └── ·7fdb58d (⌂)❱"B"
    │               │                       └── →:7: (main)
    │               └── ►:2:D
    │                   └── ·450c58a (⌂)❱"D"
    │                       └── →:4:
    └── ►:8:origin/main
        └── →:7: (main)
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
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ·47e1cf1 (⌂|🏘️)❱"GitButler Workspace Commit"
            └── ►:2:merge-2
                └── ·f40fb16 (⌂|🏘️)❱"Merge branch \'C\' into merge-2"
                    ├── ►:4:C
                    │   └── ·c6d714c (⌂|🏘️)❱"C"
                    │       └── ►:8:empty-2-on-merge
                    │           └── ►:9:empty-1-on-merge
                    │               └── ►:10:merge
                    │                   └── ·0cc5a6f (⌂|🏘️)❱"Merge branch \'A\' into merge"
                    │                       ├── ►:6:B
                    │                       │   └── ·7fdb58d (⌂|🏘️)❱"B"
                    │                       │       └── ►:1:origin/main
                    │                       │           └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main
                    │                       └── ►:7:A
                    │                           └── ·e255adc (⌂|🏘️)❱"A"
                    │                               └── →:1: (origin/main)
                    └── ►:3:D
                        └── ·450c58a (⌂|🏘️)❱"D"
                            └── →:8: (empty-2-on-merge)
    "#);
    Ok(())
}

#[test]
fn just_init_with_branches() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    // Note the dedicated workspace branch without a workspace commit.
    // All is fair game, and we use it to validate 'empty parent branch handling after new children took the commit'.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init");

    // Without hints - `main` is picked up as it's the entrypoint.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:main
    │   └── ►:2:origin/main
    │       └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►A, ►B, ►C, ►D, ►E, ►F, ►main
    └── ►►►:1:gitbutler/workspace
        └── →:2: (origin/main)
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
    //       also: order is wrong now due to target branch handling
    //       - needs insertion of multi-segment above 'fixed' references like the target branch.
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►:0:main
    │   └── ►:2:origin/main
    │       └── ►:3:C
    │           └── ►:4:B
    │               └── ►:5:A
    │                   └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►D, ►E, ►F, ►main
    └── ►►►:1:gitbutler/workspace
        └── →:2: (origin/main)
    "#);
    Ok(())
}

#[test]
fn proper_remote_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/proper-remote-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9bcd3af (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * ca7baa7 (origin/main) only-remote-02
    | * 7ea1468 only-remote-01
    |/  
    * 998eae6 (main) shared
    * fafd9d0 init
    ");

    // Remote segments are picked up automatically and traversed - they never take ownership of already assigned commits.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·9bcd3af (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:2:main
    │           ├── ·998eae6 (⌂|🏘️|✓)❱"shared"
    │           └── ·fafd9d0 (⌂|🏘️|✓)❱"init"
    └── ►:1:origin/main
        ├── 🟣ca7baa7 (✓)❱"only-remote-02"
        └── 🟣7ea1468 (✓)❱"only-remote-01"
            └── →:2: (main)
    "#);
    Ok(())
}

#[test]
fn deduced_remote_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/deduced-remote-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8b39ce4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9d34471 (A) A2
    * 5b89c71 A1
    | * 3ea1a8f (origin/A) only-remote-02
    | * 9c50f71 only-remote-01
    | * 2cfbb79 merge
    |/| 
    | * e898cd0 feat-on-remote
    |/  
    * 998eae6 shared
    * fafd9d0 (main) init
    ");

    // Remote segments are picked up automatically and traversed - they never take ownership of already assigned commits.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·8b39ce4 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:1:A
    │           ├── ·9d34471 (⌂|🏘️)❱"A2"
    │           └── ·5b89c71 (⌂|🏘️)❱"A1"
    │               └── ►:5:anon:
    │                   └── ·998eae6 (⌂|🏘️)❱"shared"
    │                       └── ►:3:main
    │                           └── ·fafd9d0 (⌂|🏘️)❱"init"
    └── ►:2:origin/A
        ├── 🟣3ea1a8f❱"only-remote-02"
        └── 🟣9c50f71❱"only-remote-01"
            └── ►:4:anon:
                └── 🟣2cfbb79❱"merge"
                    ├── ►:6:anon:
                    │   └── 🟣e898cd0❱"feat-on-remote"
                    │       └── →:5:
                    └── →:5:
    "#);

    let id = id_by_rev(&repo, ":/init");
    let graph = Graph::from_commit_traversal(id, None, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── ►►►:1:gitbutler/workspace
    │   └── ·8b39ce4 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:2:A
    │           ├── ·9d34471 (⌂|🏘️)❱"A2"
    │           └── ·5b89c71 (⌂|🏘️)❱"A1"
    │               └── ►:5:anon:
    │                   └── ·998eae6 (⌂|🏘️)❱"shared"
    │                       └── 👉►:0:main
    │                           └── ·fafd9d0 (⌂|🏘️)❱"init"
    └── ►:3:origin/A
        ├── 🟣3ea1a8f❱"only-remote-02"
        └── 🟣9c50f71❱"only-remote-01"
            └── ►:4:anon:
                └── 🟣2cfbb79❱"merge"
                    ├── ►:6:anon:
                    │   └── 🟣e898cd0❱"feat-on-remote"
                    │       └── →:5:
                    └── →:5:
    "#);
    Ok(())
}

#[test]
fn stacked_rebased_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-includes-another-remote")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 682be32 (origin/B) B
    * e29c23d (origin/A) A
    | * 7786959 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 312f819 (B) B
    | * e255adc (A) A
    |/  
    * fafd9d0 (origin/main, main) init
    ");

    // This is like remotes have been stacked and are completely rebased so they differ from their local
    // commits. This also means they include each other.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·7786959 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           └── ·312f819 (⌂|🏘️)❱"B"
    │               └── ►:4:A
    │                   └── ·e255adc (⌂|🏘️)❱"A"
    │                       └── ►:1:origin/main
    │                           └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main
    └── ►:3:origin/B
        └── 🟣682be32❱"B"
            └── ►:5:origin/A
                └── 🟣e29c23d❱"A"
                    └── →:1: (origin/main)
    "#);

    // The result is the same when changing the entrypoint.
    let (id, name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── ►►►:1:gitbutler/workspace
    │   └── ·7786959 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:4:B
    │           └── ·312f819 (⌂|🏘️)❱"B"
    │               └── 👉►:0:A
    │                   └── ·e255adc (⌂|🏘️)❱"A"
    │                       └── ►:2:origin/main
    │                           └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main
    └── ►:5:origin/B
        └── 🟣682be32❱"B"
            └── ►:3:origin/A
                └── 🟣e29c23d❱"A"
                    └── →:2: (origin/main)
    "#);
    assert_eq!(graph.num_remote_segments(), 2);
    Ok(())
}

#[test]
fn disambiguate_by_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/disambiguate-by-remote")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e30f90c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2173153 (origin/ambiguous-C, origin/C, ambiguous-C, C) C
    | * ac24e74 (origin/B) remote-of-B
    |/  
    * 312f819 (ambiguous-B, B) B
    * e255adc (origin/A, ambiguous-A, A) A
    * fafd9d0 (origin/main, main) init
    ");

    add_workspace(&mut meta);
    // As remote connections point at segments, if these stream back into their local tracking
    // branch, and the segment is unnamed, and the first commit is ambiguous name-wise, we
    // use the remote tracking branch to disambiguate the segment. After all, it's beneficial
    // to have properly wired segments.
    // Note that this is more complicated if the local tracking branch is also advanced, but
    // this is something to improve when workspace-less operation becomes a thing *and* we
    // need to get better as disambiguation.
    // The target branch is actually counted as remote, but it doesn't come through here as
    // it steals the commit from `main`. This should be fine.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·e30f90c (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:5:anon:
    │           └── ·2173153 (⌂|🏘️)❱"C" ►C, ►ambiguous-C
    │               └── ►:8:B
    │                   └── ·312f819 (⌂|🏘️)❱"B" ►ambiguous-B
    │                       └── ►:7:A
    │                           └── ·e255adc (⌂|🏘️)❱"A" ►ambiguous-A
    │                               └── ►:1:origin/main
    │                                   └── ·fafd9d0 (⌂|🏘️|✓)❱"init" ►main
    ├── ►:2:origin/C
    │   └── →:5:
    ├── ►:3:origin/ambiguous-C
    │   └── →:5:
    ├── ►:4:origin/B
    │   └── 🟣ac24e74❱"remote-of-B"
    │       └── →:8: (B)
    └── ►:6:origin/A
        └── →:7: (A)
    "#);

    assert_eq!(
        graph.partial_segments().count(),
        0,
        "a fully realized graph"
    );
    Ok(())
}

#[test]
fn integrated_tips_stop_early() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-segments-one-integrated")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * d0df794 (origin/main) remote-2
    * 09c6e08 remote-1
    *   7b9f260 Merge branch 'A' into soon-origin-main
    |\  
    | | * 4077353 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | | * 6b1a13b (B) B2
    | | * 03ad472 B1
    | |/  
    | * 79bbb29 (A) 8
    | * fc98174 7
    | * a381df5 6
    | * 777b552 5
    | *   ce4a760 Merge branch 'A-feat' into A
    | |\  
    | | * fea59b5 (A-feat) A-feat-2
    | | * 4deea74 A-feat-1
    | |/  
    | * 01d0e1e 4
    |/  
    * 4b3e5a8 (main) 3
    * 34d0715 2
    * eb5f731 1
    ");

    add_workspace(&mut meta);
    // We can abort early if there is only integrated commits left.
    // We also abort integrated named segments early, unless these are named as being part of the
    // workspace - here `A` is cut off.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️)❱"B2"
    │           └── ·03ad472 (⌂|🏘️)❱"B1"
    │               └── ►:5:A
    │                   └── ✂️·79bbb29 (⌂|🏘️|✓)❱"8"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        └── ✂️·34d0715 (⌂|✓)❱"2"
    "#);

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B",
        StackState::InWorkspace,
        &["A"],
    );
    // Now that `A` is part of the workspace, it's not cut off anymore.
    // Instead, we get to keep `A` in full, and it aborts only one later as the
    // segment definitely isn't in the workspace.
    // As we start at a workspace, even a limit of 0 has no effect - we get to see the whole workspace.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️)❱"B2"
    │           └── ·03ad472 (⌂|🏘️)❱"B1"
    │               └── ►:5:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓)❱"8"
    │                   ├── ·fc98174 (⌂|🏘️|✓)❱"7"
    │                   ├── ·a381df5 (⌂|🏘️|✓)❱"6"
    │                   └── ·777b552 (⌂|🏘️|✓)❱"5"
    │                       └── ►:6:anon:
    │                           └── ✂️·ce4a760 (⌂|🏘️|✓)❱"Merge branch \'A-feat\' into A"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        └── ✂️·34d0715 (⌂|✓)❱"2"
    "#);

    // The limit is effective for integrated workspaces branches though to prevent runaways.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️)❱"B2"
    │           └── ·03ad472 (⌂|🏘️)❱"B1"
    │               └── ►:5:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓)❱"8"
    │                   ├── ·fc98174 (⌂|🏘️|✓)❱"7"
    │                   └── ✂️·a381df5 (⌂|🏘️|✓)❱"6"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        └── ✂️·34d0715 (⌂|✓)❱"2"
    "#);

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    // When looking from an integrated branch, we get a bit further until we know we can stop as
    // the target branch first has to catch up with us.
    let (id, ref_name) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── ►►►:1:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:3:B
    │           ├── ·6b1a13b (⌂|🏘️)❱"B2"
    │           └── ·03ad472 (⌂|🏘️)❱"B1"
    │               └── 👉►:0:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓)❱"8"
    │                   ├── ·fc98174 (⌂|🏘️|✓)❱"7"
    │                   ├── ·a381df5 (⌂|🏘️|✓)❱"6"
    │                   └── ✂️·777b552 (⌂|🏘️|✓)❱"5"
    └── ►:2:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:4:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:0: (A)
                    └── ►:5:main
                        └── ✂️·4b3e5a8 (⌂|✓)❱"3"
    "#);
    Ok(())
}

#[test]
fn workspace_obeys_limit_when_target_branch_is_missing() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-segments-one-integrated")?;
    add_workspace_without_target(&mut meta);
    assert!(
        meta.data_mut().default_target.is_none(),
        "without target, limits affect workspaces too"
    );
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(0))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ✂️·4077353 (⌂|🏘️)❱"GitButler Workspace Commit"
    "#);

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    assert!(
        meta.data_mut().default_target.is_some(),
        "But with workspace and target, we see everything"
    );
    // It's notable that there is no way to bypass the early abort when everything is integrated.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(0))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️)❱"B2"
    │           └── ·03ad472 (⌂|🏘️)❱"B1"
    │               └── ►:5:A
    │                   └── ✂️·79bbb29 (⌂|🏘️|✓)❱"8"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        └── ✂️·34d0715 (⌂|✓)❱"2"
    "#);
    Ok(())
}

#[test]
fn on_top_of_target_with_history() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/on-top-of-target-with-history")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 2cde30a (HEAD -> gitbutler/workspace, origin/main, F, E, D, C, B, A) 5
    * 1c938f4 4
    * b82769f 3
    * 988032f 2
    * cd5b655 1
    * 2be54cd (main) outdated-main
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ►:1:origin/main
            ├── ·2cde30a (⌂|🏘️|✓)❱"5" ►A, ►B, ►C, ►D, ►E, ►F
            └── ✂️·1c938f4 (⌂|🏘️|✓)❱"4"
    "#);

    // TODO: fix this - it builds a wrong graph.
    // add_stack_with_segments(
    //     &mut meta,
    //     StackId::from_number_for_testing(0),
    //     "C",
    //     StackState::InWorkspace,
    //     &["B", "A"],
    // );
    // add_stack_with_segments(
    //     &mut meta,
    //     StackId::from_number_for_testing(1),
    //     "D",
    //     StackState::InWorkspace,
    //     &["E", "F"],
    // );
    // let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated_or_open_as_svg()?;
    // insta::assert_snapshot!(graph_tree(&graph), @r#""#);
    Ok(())
}
