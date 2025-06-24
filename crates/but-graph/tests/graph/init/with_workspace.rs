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
        └── ·20de6ee (⌂|🏘️|1)❱"GitButler Workspace Commit"
            └── ►:2:B
                ├── ·70e9a36 (⌂|🏘️|1)❱"with-ref"
                ├── ·320e105 (⌂|🏘️|1)❱"segment-B" ►tags/without-ref
                ├── ·2a31450 (⌂|🏘️|1)❱"segment-B~1" ►B-empty, ►ambiguous-01
                └── ·70bde6b (⌂|🏘️|1)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                    └── ►:1:origin/main
                        └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main, ►new-A, ►new-B
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
                        ├── ·320e105 (⌂|🏘️|1)❱"segment-B"
                        ├── ·2a31450 (⌂|🏘️|1)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️|1)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main, ►new-A, ►new-B
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
                        ├── 👉·320e105 (⌂|🏘️|1)❱"segment-B" ►tags/without-ref
                        ├── ·2a31450 (⌂|🏘️|1)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️|1)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main, ►new-A, ►new-B
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
                        ├── 👉·2a31450 (⌂|🏘️|1)❱"segment-B~1" ►B-empty, ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️|1)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main, ►new-A, ►new-B
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
                        ├── ·2a31450 (⌂|🏘️|1)❱"segment-B~1" ►ambiguous-01
                        └── ·70bde6b (⌂|🏘️|1)❱"segment-A" ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
                            └── ►:2:origin/main
                                └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main, ►new-A, ►new-B
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
        └── ·20de6ee (⌂|🏘️|1)❱"GitButler Workspace Commit"
            └── ►:2:B
                ├── ·70e9a36 (⌂|🏘️|1)❱"with-ref"
                └── ·320e105 (⌂|🏘️|1)❱"segment-B" ►tags/without-ref
                    └── ►:3:B-empty
                        └── ·2a31450 (⌂|🏘️|1)❱"segment-B~1" ►ambiguous-01
                            └── ►:4:A-empty-03
                                └── ►:5:A-empty-01
                                    └── ►:6:A
                                        └── ·70bde6b (⌂|🏘️|1)❱"segment-A" ►A-empty-02
                                            └── ►:1:origin/main
                                                └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main, ►new-A, ►new-B
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
        └── ·2c12d75 (⌂|🏘️|1)❱"GitButler Workspace Commit"
            └── ►:2:B
                └── ·320e105 (⌂|🏘️|1)❱"segment-B"
                    └── ►:3:B-sub
                        └── ·2a31450 (⌂|🏘️|1)❱"segment-B~1"
                            └── ►:4:A
                                └── ·70bde6b (⌂|🏘️|1)❱"segment-A"
                                    └── ►:1:origin/main
                                        └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main, ►new-A
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
        └── ·2c12d75 (⌂|🏘️|1)❱"GitButler Workspace Commit"
            └── ►:2:B
                └── ·320e105 (⌂|🏘️|1)❱"segment-B"
                    └── ►:3:B-sub
                        └── ·2a31450 (⌂|🏘️|1)❱"segment-B~1"
                            └── ►:4:A
                                └── ·70bde6b (⌂|🏘️|1)❱"segment-A"
                                    └── ►:1:origin/main
                                        └── ►:5:new-A
                                            └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main
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
        └── ·47e1cf1 (⌂|1)❱"GitButler Workspace Commit"
            └── ►:1:anon:
                └── ·f40fb16 (⌂|1)❱"Merge branch \'C\' into merge-2"
                    ├── ►:3:anon:
                    │   └── ·c6d714c (⌂|1)❱"C"
                    │       └── ►:4:anon:
                    │           └── ·0cc5a6f (⌂|1)❱"Merge branch \'A\' into merge"
                    │               ├── ►:6:anon:
                    │               │   └── ·e255adc (⌂|1)❱"A"
                    │               │       └── ►:7:anon:
                    │               │           └── ·fafd9d0 (⌂|1)❱"init"
                    │               └── ►:5:anon:
                    │                   └── ·7fdb58d (⌂|1)❱"B"
                    │                       └── →:7:
                    └── ►:2:anon:
                        └── ·450c58a (⌂|1)❱"D"
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
    │   ├── ·98c5aba (⌂|1)❱"C"
    │   ├── ·807b6ce (⌂|1)❱"B"
    │   └── ·6d05486 (⌂|1)❱"A"
    │       └── ►:3:anon:
    │           ├── ·b688f2d (⌂|🏘️|1)❱"other-1"
    │           └── ·fafd9d0 (⌂|🏘️|1)❱"init"
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
    │   └── ·47e1cf1 (⌂|1)❱"GitButler Workspace Commit"
    │       └── ►:1:merge-2
    │           └── ·f40fb16 (⌂|1)❱"Merge branch \'C\' into merge-2"
    │               ├── ►:3:C
    │               │   └── ·c6d714c (⌂|1)❱"C"
    │               │       └── ►:4:anon:
    │               │           └── ·0cc5a6f (⌂|1)❱"Merge branch \'A\' into merge" ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    │               │               ├── ►:6:A
    │               │               │   └── ·e255adc (⌂|1)❱"A"
    │               │               │       └── ►:7:main <> origin/main
    │               │               │           └── ·fafd9d0 (⌂|11)❱"init"
    │               │               └── ►:5:B
    │               │                   └── ·7fdb58d (⌂|1)❱"B"
    │               │                       └── →:7: (main)
    │               └── ►:2:D
    │                   └── ·450c58a (⌂|1)❱"D"
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
        └── ·47e1cf1 (⌂|🏘️|1)❱"GitButler Workspace Commit"
            └── ►:2:merge-2
                └── ·f40fb16 (⌂|🏘️|1)❱"Merge branch \'C\' into merge-2"
                    ├── ►:4:C
                    │   └── ·c6d714c (⌂|🏘️|1)❱"C"
                    │       └── ►:8:empty-2-on-merge
                    │           └── ►:9:empty-1-on-merge
                    │               └── ►:10:merge
                    │                   └── ·0cc5a6f (⌂|🏘️|1)❱"Merge branch \'A\' into merge"
                    │                       ├── ►:6:B
                    │                       │   └── ·7fdb58d (⌂|🏘️|1)❱"B"
                    │                       │       └── ►:1:origin/main
                    │                       │           └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►main
                    │                       └── ►:7:A
                    │                           └── ·e255adc (⌂|🏘️|1)❱"A"
                    │                               └── →:1: (origin/main)
                    └── ►:3:D
                        └── ·450c58a (⌂|🏘️|1)❱"D"
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
    ├── 👉►:0:main <> origin/main
    │   └── ►:2:origin/main
    │       └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►A, ►B, ►C, ►D, ►E, ►F, ►main
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
    ├── 👉►:0:main <> origin/main
    │   └── ►:2:origin/main
    │       └── ►:3:C
    │           └── ►:4:B
    │               └── ►:5:A
    │                   └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init" ►D, ►E, ►F, ►main
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
    │   └── ·9bcd3af (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:main <> origin/main
    │           ├── ·998eae6 (⌂|🏘️|✓|1)❱"shared"
    │           └── ·fafd9d0 (⌂|🏘️|✓|1)❱"init"
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
    │   └── ·8b39ce4 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:1:A <> origin/A
    │           ├── ·9d34471 (⌂|🏘️|11)❱"A2"
    │           └── ·5b89c71 (⌂|🏘️|11)❱"A1"
    │               └── ►:5:anon:
    │                   └── ·998eae6 (⌂|🏘️|11)❱"shared"
    │                       └── ►:3:main
    │                           └── ·fafd9d0 (⌂|🏘️|11)❱"init"
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
    │       └── ►:2:A <> origin/A
    │           ├── ·9d34471 (⌂|🏘️|10)❱"A2"
    │           └── ·5b89c71 (⌂|🏘️|10)❱"A1"
    │               └── ►:5:anon:
    │                   └── ·998eae6 (⌂|🏘️|10)❱"shared"
    │                       └── 👉►:0:main
    │                           └── ·fafd9d0 (⌂|🏘️|11)❱"init"
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
    │   └── ·7786959 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:B <> origin/B
    │           └── ·312f819 (⌂|🏘️|11)❱"B"
    │               └── ►:4:A <> origin/A
    │                   └── ·e255adc (⌂|🏘️|111)❱"A"
    │                       └── ►:1:origin/main
    │                           └── ·fafd9d0 (⌂|🏘️|✓|111)❱"init" ►main
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
    │       └── ►:4:B <> origin/B
    │           └── ·312f819 (⌂|🏘️|10)❱"B"
    │               └── 👉►:0:A <> origin/A
    │                   └── ·e255adc (⌂|🏘️|11)❱"A"
    │                       └── ►:2:origin/main
    │                           └── ·fafd9d0 (⌂|🏘️|✓|11)❱"init" ►main
    └── ►:5:origin/B
        └── 🟣682be32❱"B"
            └── ►:3:origin/A
                └── 🟣e29c23d❱"A"
                    └── →:2: (origin/main)
    "#);
    insta::assert_debug_snapshot!(graph.statistics(), @r#"
    Statistics {
        segments: 6,
        segments_integrated: 1,
        segments_remote: 2,
        segments_with_remote_tracking_branch: 2,
        segments_empty: 0,
        segments_unnamed: 0,
        segments_in_workspace: 4,
        segments_in_workspace_and_integrated: 1,
        segments_with_workspace_metadata: 1,
        segments_with_branch_metadata: 0,
        entrypoint_in_workspace: Some(
            true,
        ),
        segments_behind_of_entrypoint: 1,
        segments_ahead_of_entrypoint: 2,
        entrypoint: (
            NodeIndex(0),
            Some(
                0,
            ),
        ),
        segment_entrypoint_incoming: 1,
        segment_entrypoint_outgoing: 1,
        top_segments: [
            (
                Some(
                    FullName(
                        "refs/heads/gitbutler/workspace",
                    ),
                ),
                NodeIndex(1),
                Some(
                    CommitFlags(
                        NotInRemote | InWorkspace,
                    ),
                ),
            ),
            (
                Some(
                    FullName(
                        "refs/remotes/origin/B",
                    ),
                ),
                NodeIndex(5),
                None,
            ),
        ],
        segments_at_bottom: 1,
        connections: 5,
        commits: 6,
        commit_references: 1,
        commits_at_cutoff: 0,
    }
    "#);
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
    │   └── ·e30f90c (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:5:anon:
    │           └── ·2173153 (⌂|🏘️|11)❱"C" ►C, ►ambiguous-C
    │               └── ►:8:B <> origin/B
    │                   └── ·312f819 (⌂|🏘️|111)❱"B" ►ambiguous-B
    │                       └── ►:7:A <> origin/A
    │                           └── ·e255adc (⌂|🏘️|1111)❱"A" ►ambiguous-A
    │                               └── ►:1:origin/main
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|1111)❱"init" ►main
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

    // If 'C' is in the workspace, it's naturally disambiguated.
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "C",
        StackState::InWorkspace,
        &[],
    );
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·e30f90c (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:C <> origin/C
    │           └── ·2173153 (⌂|🏘️|11)❱"C" ►ambiguous-C
    │               └── ►:8:B <> origin/B
    │                   └── ·312f819 (⌂|🏘️|111)❱"B" ►ambiguous-B
    │                       └── ►:7:A <> origin/A
    │                           └── ·e255adc (⌂|🏘️|1111)❱"A" ►ambiguous-A
    │                               └── ►:1:origin/main
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|1111)❱"init" ►main
    ├── ►:3:origin/C
    │   └── →:2: (C)
    ├── ►:4:origin/ambiguous-C
    │   └── →:2: (C)
    ├── ►:5:origin/B
    │   └── 🟣ac24e74❱"remote-of-B"
    │       └── →:8: (B)
    └── ►:6:origin/A
        └── →:7: (A)
    "#);
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
    │   └── ·4077353 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️|1)❱"B2"
    │           └── ·03ad472 (⌂|🏘️|1)❱"B1"
    │               └── ►:5:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)❱"8"
    │                   └── ✂️·fc98174 (⌂|🏘️|✓|1)❱"7"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        ├── ·34d0715 (⌂|✓)❱"2"
                        └── ·eb5f731 (⌂|✓)❱"1"
    "#);

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B",
        StackState::InWorkspace,
        &["A"],
    );
    // ~~Now that `A` is part of the workspace, it's not cut off anymore.~~
    // This special handling was removed for now, relying on limits and extensions.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️|1)❱"B2"
    │           └── ·03ad472 (⌂|🏘️|1)❱"B1"
    │               └── ►:5:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)❱"8"
    │                   └── ✂️·fc98174 (⌂|🏘️|✓|1)❱"7"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        ├── ·34d0715 (⌂|✓)❱"2"
                        └── ·eb5f731 (⌂|✓)❱"1"
    "#);

    // The limit is effective for integrated workspaces branches, but the traversal proceeds until
    // the integration branch finds its goal.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️|1)❱"B2"
    │           └── ·03ad472 (⌂|🏘️|1)❱"B1"
    │               └── ►:5:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)❱"8"
    │                   └── ✂️·fc98174 (⌂|🏘️|✓|1)❱"7"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        ├── ·34d0715 (⌂|✓)❱"2"
                        └── ·eb5f731 (⌂|✓)❱"1"
    "#);

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    // When looking from an integrated branch within the workspace, but without limit,
    // the limit is respected.
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
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)❱"8"
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)❱"7"
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)❱"6"
    │                   └── ·777b552 (⌂|🏘️|✓|1)❱"5"
    │                       └── ►:6:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)❱"Merge branch \'A-feat\' into A"
    │                               ├── ►:8:A-feat
    │                               │   ├── ·fea59b5 (⌂|🏘️|✓|1)❱"A-feat-2"
    │                               │   └── ·4deea74 (⌂|🏘️|✓|1)❱"A-feat-1"
    │                               │       └── ►:7:anon:
    │                               │           └── ·01d0e1e (⌂|🏘️|✓|1)❱"4"
    │                               │               └── ►:5:main
    │                               │                   ├── ·4b3e5a8 (⌂|🏘️|✓|1)❱"3"
    │                               │                   ├── ·34d0715 (⌂|🏘️|✓|1)❱"2"
    │                               │                   └── ·eb5f731 (⌂|🏘️|✓|1)❱"1"
    │                               └── →:7:
    └── ►:2:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:4:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:0: (A)
                    └── →:5: (main)
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
        └── ✂️·4077353 (⌂|🏘️|1)❱"GitButler Workspace Commit"
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
    │   └── ·4077353 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:B
    │           ├── ·6b1a13b (⌂|🏘️|1)❱"B2"
    │           └── ·03ad472 (⌂|🏘️|1)❱"B1"
    │               └── ►:5:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)❱"8"
    │                   └── ✂️·fc98174 (⌂|🏘️|✓|1)❱"7"
    └── ►:1:origin/main
        ├── 🟣d0df794 (✓)❱"remote-2"
        └── 🟣09c6e08 (✓)❱"remote-1"
            └── ►:3:anon:
                └── 🟣7b9f260 (✓)❱"Merge branch \'A\' into soon-origin-main"
                    ├── →:5: (A)
                    └── ►:4:main
                        ├── ·4b3e5a8 (⌂|✓)❱"3"
                        ├── ·34d0715 (⌂|✓)❱"2"
                        └── ·eb5f731 (⌂|✓)❱"1"
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
            ├── ·2cde30a (⌂|🏘️|✓|1)❱"5" ►A, ►B, ►C, ►D, ►E, ►F
            └── ✂️·1c938f4 (⌂|🏘️|✓|1)❱"4"
    "#);

    // TODO: setup two stacks
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
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:gitbutler/workspace
        └── ►:1:origin/main
            └── ►:2:C
                └── ►:3:B
                    └── ►:4:A
                        ├── ·2cde30a (⌂|🏘️|✓|1)❱"5" ►D, ►E, ►F
                        └── ✂️·1c938f4 (⌂|🏘️|✓|1)❱"4"
    "#);
    Ok(())
}

#[test]
fn partitions_with_long_and_short_connections_to_each_other() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/gitlab-case")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 41ed0e4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | *   232ed06 (origin/main) target
    | |\  
    | | * 9e2a79e (long-workspace-to-target) Tl7
    | | * fdeaa43 Tl6
    | | * 30565ee Tl5
    | | * 0c1c23a Tl4
    | | * 56d152c Tl3
    | | * e6e1360 Tl2
    | | * 1a22a39 Tl1
    | |/  
    |/|   
    | * abcfd9a (workspace-to-target) Ts3
    | * bc86eba Ts2
    | * c7ae303 Ts1
    |/  
    *   9730cbf (workspace) W1-merge
    |\  
    | * 77f31a0 (long-main-to-workspace) Wl4
    | * eb17e31 Wl3
    | * fe2046b Wl2
    | * 5532ef5 Wl1
    | * 2438292 (main) M2
    * | dc7ab57 (main-to-workspace) Ws1
    |/  
    * c056b75 M10
    * f49c977 M9
    * 7b7ebb2 M8
    * dca4960 M7
    * 11c29b8 M6
    * c32dd03 M5
    * b625665 M4
    * a821094 M3
    * bce0c5e M2
    * 3183e43 M1
    ");

    add_workspace(&mut meta);
    let (id, ref_name) = id_at(&repo, "main");
    // Validate that we will perform long searches to connect connectable segments, without interfering
    // with other searches that may take even longer.
    // Also, without limit, we should be able to see all of 'main' without cut-off
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── ►►►:1:gitbutler/workspace
    │   └── ·41ed0e4 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:5:workspace
    │           └── ·9730cbf (⌂|🏘️|✓)❱"W1-merge"
    │               ├── ►:7:long-main-to-workspace
    │               │   ├── ·77f31a0 (⌂|🏘️|✓)❱"Wl4"
    │               │   ├── ·eb17e31 (⌂|🏘️|✓)❱"Wl3"
    │               │   ├── ·fe2046b (⌂|🏘️|✓)❱"Wl2"
    │               │   └── ·5532ef5 (⌂|🏘️|✓)❱"Wl1"
    │               │       └── 👉►:0:main
    │               │           └── ·2438292 (⌂|🏘️|✓|1)❱"M2"
    │               │               └── ►:8:anon:
    │               │                   ├── ·c056b75 (⌂|🏘️|✓|1)❱"M10"
    │               │                   ├── ·f49c977 (⌂|🏘️|✓|1)❱"M9"
    │               │                   ├── ·7b7ebb2 (⌂|🏘️|✓|1)❱"M8"
    │               │                   ├── ·dca4960 (⌂|🏘️|✓|1)❱"M7"
    │               │                   ├── ·11c29b8 (⌂|🏘️|✓|1)❱"M6"
    │               │                   ├── ·c32dd03 (⌂|🏘️|✓|1)❱"M5"
    │               │                   ├── ·b625665 (⌂|🏘️|✓|1)❱"M4"
    │               │                   ├── ·a821094 (⌂|🏘️|✓|1)❱"M3"
    │               │                   ├── ·bce0c5e (⌂|🏘️|✓|1)❱"M2"
    │               │                   └── ·3183e43 (⌂|🏘️|✓|1)❱"M1"
    │               └── ►:6:main-to-workspace
    │                   └── ·dc7ab57 (⌂|🏘️|✓)❱"Ws1"
    │                       └── →:8:
    └── ►:2:origin/main
        └── 🟣232ed06 (✓)❱"target"
            ├── ►:4:long-workspace-to-target
            │   ├── 🟣9e2a79e (✓)❱"Tl7"
            │   ├── 🟣fdeaa43 (✓)❱"Tl6"
            │   ├── 🟣30565ee (✓)❱"Tl5"
            │   ├── 🟣0c1c23a (✓)❱"Tl4"
            │   ├── 🟣56d152c (✓)❱"Tl3"
            │   ├── 🟣e6e1360 (✓)❱"Tl2"
            │   └── 🟣1a22a39 (✓)❱"Tl1"
            │       └── →:5: (workspace)
            └── ►:3:workspace-to-target
                ├── 🟣abcfd9a (✓)❱"Ts3"
                ├── 🟣bc86eba (✓)❱"Ts2"
                └── 🟣c7ae303 (✓)❱"Ts1"
                    └── →:5: (workspace)
    "#);

    // When setting a limit when traversing 'main', it is respected.
    // We still want it to be found and connected though, and it's notable that the limit kicks in
    // once everything reconciled.
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options().with_limit_hint(1))?
            .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── ►►►:1:gitbutler/workspace
    │   └── ·41ed0e4 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:5:workspace
    │           └── ·9730cbf (⌂|🏘️|✓)❱"W1-merge"
    │               ├── ►:7:long-main-to-workspace
    │               │   ├── ·77f31a0 (⌂|🏘️|✓)❱"Wl4"
    │               │   ├── ·eb17e31 (⌂|🏘️|✓)❱"Wl3"
    │               │   ├── ·fe2046b (⌂|🏘️|✓)❱"Wl2"
    │               │   └── ·5532ef5 (⌂|🏘️|✓)❱"Wl1"
    │               │       └── 👉►:0:main
    │               │           └── ·2438292 (⌂|🏘️|✓|1)❱"M2"
    │               │               └── ►:8:anon:
    │               │                   ├── ·c056b75 (⌂|🏘️|✓|1)❱"M10"
    │               │                   ├── ·f49c977 (⌂|🏘️|✓|1)❱"M9"
    │               │                   ├── ·7b7ebb2 (⌂|🏘️|✓|1)❱"M8"
    │               │                   ├── ·dca4960 (⌂|🏘️|✓|1)❱"M7"
    │               │                   ├── ·11c29b8 (⌂|🏘️|✓|1)❱"M6"
    │               │                   ├── ·c32dd03 (⌂|🏘️|✓|1)❱"M5"
    │               │                   ├── ·b625665 (⌂|🏘️|✓|1)❱"M4"
    │               │                   ├── ·a821094 (⌂|🏘️|✓|1)❱"M3"
    │               │                   └── ✂️·bce0c5e (⌂|🏘️|✓|1)❱"M2"
    │               └── ►:6:main-to-workspace
    │                   └── ·dc7ab57 (⌂|🏘️|✓)❱"Ws1"
    │                       └── →:8:
    └── ►:2:origin/main
        └── 🟣232ed06 (✓)❱"target"
            ├── ►:4:long-workspace-to-target
            │   ├── 🟣9e2a79e (✓)❱"Tl7"
            │   ├── 🟣fdeaa43 (✓)❱"Tl6"
            │   ├── 🟣30565ee (✓)❱"Tl5"
            │   ├── 🟣0c1c23a (✓)❱"Tl4"
            │   ├── 🟣56d152c (✓)❱"Tl3"
            │   ├── 🟣e6e1360 (✓)❱"Tl2"
            │   └── 🟣1a22a39 (✓)❱"Tl1"
            │       └── →:5: (workspace)
            └── ►:3:workspace-to-target
                ├── 🟣abcfd9a (✓)❱"Ts3"
                ├── 🟣bc86eba (✓)❱"Ts2"
                └── 🟣c7ae303 (✓)❱"Ts1"
                    └── →:5: (workspace)
    "#);

    // From the workspace, even without limit, we don't traverse all of 'main' as it's uninteresting.
    // However, we wait for the target to be fully reconciled to get the proper workspace configuration.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·41ed0e4 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:4:workspace
    │           └── ·9730cbf (⌂|🏘️|✓|1)❱"W1-merge"
    │               ├── ►:6:long-main-to-workspace
    │               │   ├── ·77f31a0 (⌂|🏘️|✓|1)❱"Wl4"
    │               │   ├── ·eb17e31 (⌂|🏘️|✓|1)❱"Wl3"
    │               │   ├── ·fe2046b (⌂|🏘️|✓|1)❱"Wl2"
    │               │   └── ·5532ef5 (⌂|🏘️|✓|1)❱"Wl1"
    │               │       └── ►:7:main
    │               │           └── ·2438292 (⌂|🏘️|✓|1)❱"M2"
    │               │               └── ►:8:anon:
    │               │                   ├── ·c056b75 (⌂|🏘️|✓|1)❱"M10"
    │               │                   ├── ·f49c977 (⌂|🏘️|✓|1)❱"M9"
    │               │                   ├── ·7b7ebb2 (⌂|🏘️|✓|1)❱"M8"
    │               │                   ├── ·dca4960 (⌂|🏘️|✓|1)❱"M7"
    │               │                   ├── ·11c29b8 (⌂|🏘️|✓|1)❱"M6"
    │               │                   └── ✂️·c32dd03 (⌂|🏘️|✓|1)❱"M5"
    │               └── ►:5:main-to-workspace
    │                   └── ·dc7ab57 (⌂|🏘️|✓|1)❱"Ws1"
    │                       └── →:8:
    └── ►:1:origin/main
        └── 🟣232ed06 (✓)❱"target"
            ├── ►:3:long-workspace-to-target
            │   ├── 🟣9e2a79e (✓)❱"Tl7"
            │   ├── 🟣fdeaa43 (✓)❱"Tl6"
            │   ├── 🟣30565ee (✓)❱"Tl5"
            │   ├── 🟣0c1c23a (✓)❱"Tl4"
            │   ├── 🟣56d152c (✓)❱"Tl3"
            │   ├── 🟣e6e1360 (✓)❱"Tl2"
            │   └── 🟣1a22a39 (✓)❱"Tl1"
            │       └── →:4: (workspace)
            └── ►:2:workspace-to-target
                ├── 🟣abcfd9a (✓)❱"Ts3"
                ├── 🟣bc86eba (✓)❱"Ts2"
                └── 🟣c7ae303 (✓)❱"Ts1"
                    └── →:4: (workspace)
    "#);
    Ok(())
}

#[test]
fn partitions_with_long_and_short_connections_to_each_other_part_2() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/gitlab-case2")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f514495 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 024f837 (origin/main, long-workspace-to-target) Tl10
    | * 64a8284 Tl9
    | * b72938c Tl8
    | * 9ccbf6f Tl7
    | * 5fa4905 Tl6
    | * 43074d3 Tl5
    | * 800d4a9 Tl4
    | * 742c068 Tl3
    | * fe06afd Tl2
    | *   3027746 Tl-merge
    | |\  
    | | * edf041f (longer-workspace-to-target) Tll6
    | | * d9f03f6 Tll5
    | | * 8d1d264 Tll4
    | | * fa7ceae Tll3
    | | * 95bdbf1 Tll2
    | | * 5bac978 Tll1
    | * | f0d2a35 Tl1
    |/ /  
    * |   c9120f1 (workspace) W1-merge
    |\ \  
    | |/  
    |/|   
    | * b39c7ec (long-main-to-workspace) Wl4
    | * 2983a97 Wl3
    | * 144ea85 Wl2
    | * 5aecfd2 Wl1
    | * bce0c5e (main) M2
    * | 1126587 (main-to-workspace) Ws1
    |/  
    * 3183e43 M1
    ");

    add_workspace(&mut meta);
    let (id, ref_name) = id_at(&repo, "main");
    // Here the target shouldn't be cut off from finding its workspace
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── ►►►:1:gitbutler/workspace
    │   └── ·f514495 (⌂|🏘️)❱"GitButler Workspace Commit"
    │       └── ►:3:workspace
    │           └── ·c9120f1 (⌂|🏘️|✓)❱"W1-merge"
    │               ├── ►:5:long-main-to-workspace
    │               │   ├── ·b39c7ec (⌂|🏘️|✓)❱"Wl4"
    │               │   ├── ·2983a97 (⌂|🏘️|✓)❱"Wl3"
    │               │   ├── ·144ea85 (⌂|🏘️|✓)❱"Wl2"
    │               │   └── ·5aecfd2 (⌂|🏘️|✓)❱"Wl1"
    │               │       └── 👉►:0:main
    │               │           └── ·bce0c5e (⌂|🏘️|✓|1)❱"M2"
    │               │               └── ►:6:anon:
    │               │                   └── ·3183e43 (⌂|🏘️|✓|1)❱"M1"
    │               └── ►:4:main-to-workspace
    │                   └── ·1126587 (⌂|🏘️|✓)❱"Ws1"
    │                       └── →:6:
    └── ►:2:origin/main
        ├── 🟣024f837 (✓)❱"Tl10" ►long-workspace-to-target
        ├── 🟣64a8284 (✓)❱"Tl9"
        ├── 🟣b72938c (✓)❱"Tl8"
        ├── 🟣9ccbf6f (✓)❱"Tl7"
        ├── 🟣5fa4905 (✓)❱"Tl6"
        ├── 🟣43074d3 (✓)❱"Tl5"
        ├── 🟣800d4a9 (✓)❱"Tl4"
        ├── 🟣742c068 (✓)❱"Tl3"
        └── 🟣fe06afd (✓)❱"Tl2"
            └── ►:7:anon:
                └── 🟣3027746 (✓)❱"Tl-merge"
                    ├── ►:9:longer-workspace-to-target
                    │   ├── 🟣edf041f (✓)❱"Tll6"
                    │   ├── 🟣d9f03f6 (✓)❱"Tll5"
                    │   ├── 🟣8d1d264 (✓)❱"Tll4"
                    │   ├── 🟣fa7ceae (✓)❱"Tll3"
                    │   ├── 🟣95bdbf1 (✓)❱"Tll2"
                    │   └── 🟣5bac978 (✓)❱"Tll1"
                    │       └── →:4: (main-to-workspace)
                    └── ►:8:anon:
                        └── 🟣f0d2a35 (✓)❱"Tl1"
                            └── →:3: (workspace)
    "#);

    // Now the target looks for the entrypoint, which is the workspace, something it can do more easily.
    // We wait for targets to fully reconcile as well.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    ├── 👉►►►:0:gitbutler/workspace
    │   └── ·f514495 (⌂|🏘️|1)❱"GitButler Workspace Commit"
    │       └── ►:2:workspace
    │           └── ·c9120f1 (⌂|🏘️|✓|1)❱"W1-merge"
    │               ├── ►:4:long-main-to-workspace
    │               │   ├── ·b39c7ec (⌂|🏘️|✓|1)❱"Wl4"
    │               │   ├── ·2983a97 (⌂|🏘️|✓|1)❱"Wl3"
    │               │   ├── ·144ea85 (⌂|🏘️|✓|1)❱"Wl2"
    │               │   └── ·5aecfd2 (⌂|🏘️|✓|1)❱"Wl1"
    │               │       └── ►:5:main
    │               │           └── ·bce0c5e (⌂|🏘️|✓|1)❱"M2"
    │               │               └── ►:6:anon:
    │               │                   └── ·3183e43 (⌂|🏘️|✓|1)❱"M1"
    │               └── ►:3:main-to-workspace
    │                   └── ·1126587 (⌂|🏘️|✓|1)❱"Ws1"
    │                       └── →:6:
    └── ►:1:origin/main
        ├── 🟣024f837 (✓)❱"Tl10" ►long-workspace-to-target
        ├── 🟣64a8284 (✓)❱"Tl9"
        ├── 🟣b72938c (✓)❱"Tl8"
        ├── 🟣9ccbf6f (✓)❱"Tl7"
        ├── 🟣5fa4905 (✓)❱"Tl6"
        ├── 🟣43074d3 (✓)❱"Tl5"
        ├── 🟣800d4a9 (✓)❱"Tl4"
        ├── 🟣742c068 (✓)❱"Tl3"
        └── 🟣fe06afd (✓)❱"Tl2"
            └── ►:7:anon:
                └── 🟣3027746 (✓)❱"Tl-merge"
                    ├── ►:9:longer-workspace-to-target
                    │   ├── 🟣edf041f (✓)❱"Tll6"
                    │   ├── 🟣d9f03f6 (✓)❱"Tll5"
                    │   ├── 🟣8d1d264 (✓)❱"Tll4"
                    │   ├── 🟣fa7ceae (✓)❱"Tll3"
                    │   ├── 🟣95bdbf1 (✓)❱"Tll2"
                    │   └── 🟣5bac978 (✓)❱"Tll1"
                    │       └── →:3: (main-to-workspace)
                    └── ►:8:anon:
                        └── 🟣f0d2a35 (✓)❱"Tl1"
                            └── →:2: (workspace)
    "#);
    Ok(())
}
