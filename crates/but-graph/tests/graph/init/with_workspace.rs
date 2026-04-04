use but_core::{
    RefMetadata,
    ref_metadata::{StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch},
};
use but_graph::{Graph, init::Overlay};
use but_testsupport::{
    InMemoryRefMetadata, graph_tree, graph_workspace, visualize_commit_graph_all,
};

use crate::init::{
    StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev,
    read_only_in_memory_scenario, standard_options,
    utils::{
        add_stack, add_workspace_with_target, add_workspace_without_target, remove_target,
        standard_options_with_extra_target,
    },
};

#[test]
fn workspace_with_stack_and_local_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   59a427f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * a62b0de (A) A2
    | * 120a217 A1
    * | 0a415d8 (main) M3
    | | * 1f5c47b (origin/main) RM1
    | |/  
    |/|   
    * | 73ba99d M2
    |/  
    * fafd9d0 init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·59a427f (⌂|🏘|01)
    │       ├── ►:2[1]:main <> origin/main →:1:
    │       │   └── ·0a415d8 (⌂|🏘|11)
    │       │       └── ►:4[2]:anon:
    │       │           └── ·73ba99d (⌂|🏘|✓|11)
    │       │               └── ►:5[3]:anon:
    │       │                   └── ·fafd9d0 (⌂|🏘|✓|11)
    │       └── ►:3[1]:A
    │           ├── ·a62b0de (⌂|🏘|01)
    │           └── ·120a217 (⌂|🏘|01)
    │               └── →:5:
    └── ►:1[0]:origin/main →:2:
        └── 🟣1f5c47b (✓)
            └── →:4:
    ");

    insta::assert_debug_snapshot!(graph.managed_entrypoint_commit(&repo)?.expect("this is managed workspace commit"), @"Commit(59a427f, ⌂|🏘|1)");

    // It's perfectly valid to have the local tracking branch of our target in the workspace,
    // and the low-bound computation works as well.
    let ws = &graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡:3:A on fafd9d0
    │   └── :3:A
    │       ├── ·a62b0de (🏘️)
    │       └── ·120a217 (🏘️)
    └── ≡:2:main <> origin/main →:1:⇡1⇣1 on fafd9d0
        └── :2:main <> origin/main →:1:⇡1⇣1
            ├── 🟣1f5c47b (✓)
            ├── ·0a415d8 (🏘️)
            └── ❄️73ba99d (🏘️|✓)
    ");

    Ok(())
}

#[test]
fn workspace_with_only_local_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-contained-and-target-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e5e2623 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * cb54dca (origin/main) RM1
    |/  
    * 0a415d8 (main) M3
    * 73ba99d M2
    * fafd9d0 init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·e5e2623 (⌂|🏘|01)
    │       └── ►:2[1]:main <> origin/main →:1:
    │           ├── ·0a415d8 (⌂|🏘|✓|11)
    │           ├── ·73ba99d (⌂|🏘|✓|11)
    │           └── ·fafd9d0 (⌂|🏘|✓|11)
    └── ►:1[0]:origin/main →:2:
        └── 🟣cb54dca (✓)
            └── →:2: (main →:1:)
    ");

    let ws = &graph.into_workspace()?;
    // It's notable how the local tracking branch of our target (origin/main) is ignored, it's not part of our workspace,
    // but acts as base.
    insta::assert_snapshot!(graph_workspace(ws), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 0a415d8");

    Ok(())
}

#[test]
fn reproduce_11483() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/reproduce-11483")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   3562fcd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 7236012 (A) A
    * | 68c8a9d (B) B
    |/  
    * 3183e43 (origin/main, main, below) M1
    ");

    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &["below"]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let ws = &graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:4:B on 3183e43 {2}
    │   ├── 📙:4:B
    │   │   └── ·68c8a9d (🏘️)
    │   └── 📙:5:below
    └── ≡📙:3:A on 3183e43 {1}
        └── 📙:3:A
            └── ·7236012 (🏘️)
    ");

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["below"]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:4:B on 3183e43 {2}
    │   └── 📙:4:B
    │       └── ·68c8a9d (🏘️)
    └── ≡📙:3:A on 3183e43 {1}
        ├── 📙:3:A
        │   └── ·7236012 (🏘️)
        └── 📙:5:below
    ");

    Ok(())
}

#[test]
fn workspace_projection_with_advanced_stack_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/advanced-stack-tip-outside-workspace")?;
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * cc0bf57 (B) B-outside
    | * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |/  
    * d69fe94 B
    * 09d8e52 (A) A
    * 85efbe4 (origin/main, main) M
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    
    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·2076060 (⌂|🏘|01)
    │       └── ►:5[1]:anon: →:3:
    │           └── ·d69fe94 (⌂|🏘|01)
    │               └── 📙►:4[2]:A
    │                   └── ·09d8e52 (⌂|🏘|01)
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·85efbe4 (⌂|🏘|✓|11)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── 📙►:3[0]:B
        └── ·cc0bf57 (⌂)
            └── →:5:
    ");
    let ws = &graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:5:B →:3: on 85efbe4 {1}
        ├── 📙:5:B →:3:
        │   ├── ·cc0bf57*
        │   └── ·d69fe94 (🏘️)
        └── 📙:4:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn no_overzealous_stacks_due_to_workspace_metadata() -> anyhow::Result<()> {
    // NOTE: Was supposed to reproduce #11459, but it found another issue instead.
    let (repo, mut meta) = read_only_in_memory_scenario("ws/reproduce-11459")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   12102a6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 0b203b5 (X) X2
    | * 4840f3b (origin/X) X1
    * | 835086d (three, four) W2
    * | ff310d3 W1
    | | * 5e9d772 (origin/two) T1
    | |/  
    |/|   
    * | a821094 (origin/main, two, remote, one, main, feat-2) M3
    * | bce0c5e M2
    |/  
    * 3183e43 (A) M1
    ");

    add_stack_with_segments(&mut meta, 1, "X", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "feat-2", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:3:X <> origin/X →:5:⇡1 on 3183e43 {1}
    │   └── 📙:3:X <> origin/X →:5:⇡1
    │       ├── ·0b203b5 (🏘️)
    │       └── ❄️4840f3b (🏘️)
    └── ≡:7:anon: on 3183e43 {2}
        ├── :7:anon:
        │   ├── ·835086d (🏘️) ►four, ►three
        │   └── ·ff310d3 (🏘️)
        └── 📙:2:feat-2
            ├── ·a821094 (🏘️|✓) ►main, ►one, ►remote, ►two
            └── ·bce0c5e (🏘️|✓)
    ");

    Ok(())
}

#[test]
fn single_stack_ambiguous() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack-ambiguous")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 20de6ee (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 70e9a36 (B) with-ref
    * 320e105 (tag: without-ref) segment-B
    * 2a31450 (ambiguous-01, B-empty) segment-B~1
    * 70bde6b (origin/B, A-empty-03, A-empty-02, A-empty-01, A) segment-A
    * fafd9d0 (origin/main, new-B, new-A, main) init
    ");

    // Just a workspace, no additional ref information.
    // As the segments are ambiguous, there are many unnamed segments.
    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘|0001)
    │       └── ►:3[1]:B <> origin/B →:4:
    │           ├── ·70e9a36 (⌂|🏘|0101)
    │           ├── ·320e105 (⌂|🏘|0101) ►tags/without-ref
    │           └── ·2a31450 (⌂|🏘|0101) ►B-empty, ►ambiguous-01
    │               └── ►:4[2]:origin/B →:3:
    │                   └── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // All non-integrated segments are visible.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:B <> origin/B →:4:⇡3 on fafd9d0
        └── :3:B <> origin/B →:4:⇡3
            ├── ·70e9a36 (🏘️)
            ├── ·320e105 (🏘️) ►tags/without-ref
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄️70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // There is always a segment for the entrypoint, and code working with the graph
    // deals with that naturally.
    let (without_ref_id, ref_name) = id_at(&repo, "without-ref");
    let graph = Graph::from_commit_traversal(without_ref_id, ref_name, &*meta, standard_options())?
        .validated()?;
    // See how tags ARE allowed to name a segment, at least when used as entrypoint.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           └── ·70e9a36 (⌂|🏘|0100)
    │               └── 👉►:0[2]:tags/without-ref
    │                   ├── ·320e105 (⌂|🏘|0101)
    │                   └── ·2a31450 (⌂|🏘|0101) ►B-empty, ►ambiguous-01
    │                       └── ►:6[3]:anon:
    │                           └── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:5[0]:origin/B →:4:
        └── →:6:
    ");
    // Now `HEAD` is outside a workspace, which goes to single-branch mode. But it knows it's in a workspace
    // and shows the surrounding parts, while marking the segment as entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:B <> origin/B →:5:⇡1 on fafd9d0
        ├── :4:B <> origin/B →:5:⇡1
        │   └── ·70e9a36 (🏘️)
        └── 👉:0:tags/without-ref
            ├── ·320e105 (🏘️)
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // We don't have to give it a ref-name
    let graph = Graph::from_commit_traversal(without_ref_id, None, &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           └── ·70e9a36 (⌂|🏘|0100)
    │               └── ►:0[2]:anon:
    │                   ├── 👉·320e105 (⌂|🏘|0101) ►tags/without-ref
    │                   └── ·2a31450 (⌂|🏘|0101) ►B-empty, ►ambiguous-01
    │                       └── ►:6[3]:anon:
    │                           └── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:5[0]:origin/B →:4:
        └── →:6:
    ");

    // Entrypoint is now unnamed (as no ref-name was provided for traversal)
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:B <> origin/B →:5:⇡1 on fafd9d0
        ├── :4:B <> origin/B →:5:⇡1
        │   └── ·70e9a36 (🏘️)
        └── 👉:0:anon:
            ├── ·320e105 (🏘️) ►tags/without-ref
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // Putting the entrypoint onto a commit in an anonymous segment with ambiguous refs makes no difference.
    let (b_id_1, tag_ref_name) = id_at(&repo, "B-empty");
    let graph =
        Graph::from_commit_traversal(b_id_1, None, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           ├── ·70e9a36 (⌂|🏘|0100)
    │           └── ·320e105 (⌂|🏘|0100) ►tags/without-ref
    │               └── ►:0[2]:anon:
    │                   └── 👉·2a31450 (⌂|🏘|0101) ►B-empty, ►ambiguous-01
    │                       └── ►:6[3]:anon:
    │                           └── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:5[0]:origin/B →:4:
        └── →:6:
    ");

    // Doing this is very much like edit mode, and there is always a segment starting at the entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:B <> origin/B →:5:⇡2 on fafd9d0
        ├── :4:B <> origin/B →:5:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        └── 👉:0:anon:
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // If we pass an entrypoint ref name, it will be used as segment name (despite being ambiguous without it)
    let graph = Graph::from_commit_traversal(b_id_1, tag_ref_name, &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           ├── ·70e9a36 (⌂|🏘|0100)
    │           └── ·320e105 (⌂|🏘|0100) ►tags/without-ref
    │               └── 👉►:0[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘|0101) ►ambiguous-01
    │                       └── ►:6[3]:anon:
    │                           └── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:5[0]:origin/B →:4:
        └── →:6:
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:B <> origin/B →:5:⇡2 on fafd9d0
        ├── :4:B <> origin/B →:5:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        └── 👉:0:B-empty
            ├── ·2a31450 (🏘️) ►ambiguous-01
            └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");
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
    * 70bde6b (origin/B, A-empty-03, A-empty-02, A-empty-01, A) segment-A
    * fafd9d0 (origin/main, new-B, new-A, main) init
    ");
    // Fully defined workspace with multiple empty segments on top of each other.
    // Notably the order doesn't match, 'B-empty' is after 'B', but we use it anyway for segment definition.
    // On single commits, the desired order fully defines where stacks go.
    // Note that this does match the single-stack (one big segment) configuration we actually have.
    add_stack_with_segments(
        &mut meta,
        0,
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘|0001)
    │       └── 📙►:4[1]:B <> origin/B →:6:
    │           ├── ·70e9a36 (⌂|🏘|0101)
    │           └── ·320e105 (⌂|🏘|0101) ►tags/without-ref
    │               └── 📙►:3[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘|0101) ►ambiguous-01
    │                       └── 📙►:7[3]:A-empty-03
    │                           └── 📙►:8[4]:A-empty-01
    │                               └── 📙►:9[5]:A
    │                                   └── ·70bde6b (⌂|🏘|1101) ►A-empty-02
    │                                       └── ►:2[6]:main <> origin/main →:1:
    │                                           └── ·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:6[0]:origin/B →:4:
        └── →:7: (A-empty-03)
    ");

    // We pickup empty segments.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:4:B <> origin/B →:6:⇡2 on fafd9d0 {0}
        ├── 📙:4:B <> origin/B →:6:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:3:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        ├── 📙:7:A-empty-03
        ├── 📙:8:A-empty-01
        └── 📙:9:A
            └── ❄70bde6b (🏘️) ►A-empty-02
    ");

    // Now something similar but with two stacks.
    // As the actual topology is different, we can't really comply with that's desired.
    // Instead, we reuse as many of the named segments as possible, even if they are from multiple branches.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 0, "B-empty", StackState::InWorkspace, &["B"]);
    add_stack_with_segments(
        &mut meta,
        1,
        "A-empty-03",
        StackState::InWorkspace,
        &["A-empty-02", "A-empty-01", "A"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘|0001)
    │       └── 📙►:4[1]:B <> origin/B →:6:
    │           ├── ·70e9a36 (⌂|🏘|0101)
    │           └── ·320e105 (⌂|🏘|0101) ►tags/without-ref
    │               └── 📙►:3[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘|0101) ►ambiguous-01
    │                       └── 📙►:7[3]:A-empty-03
    │                           └── 📙►:8[4]:A-empty-02
    │                               └── 📙►:9[5]:A-empty-01
    │                                   └── 📙►:10[6]:A
    │                                       └── ·70bde6b (⌂|🏘|1101)
    │                                           └── ►:2[7]:main <> origin/main →:1:
    │                                               └── ·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:6[0]:origin/B →:4:
        └── →:7: (A-empty-03)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:4:B <> origin/B →:6:⇡2 on fafd9d0 {0}
        ├── 📙:4:B <> origin/B →:6:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:3:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        ├── 📙:7:A-empty-03
        ├── 📙:8:A-empty-02
        ├── 📙:9:A-empty-01
        └── 📙:10:A
            └── ❄70bde6b (🏘️)
    ");

    // Define only some of the branches, it should figure that out.
    // It respects the order of the mention in the stack, `A` before `A-empty-01`.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-empty-01"]);
    add_stack_with_segments(&mut meta, 1, "B-empty", StackState::InWorkspace, &["B"]);

    let (id, ref_name) = id_at(&repo, "A-empty-01");
    let graph = Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·20de6ee (⌂|🏘)
    │       └── 📙►:5[1]:B <> origin/B →:6:
    │           ├── ·70e9a36 (⌂|🏘|100)
    │           └── ·320e105 (⌂|🏘|100) ►tags/without-ref
    │               └── 📙►:4[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘|100) ►ambiguous-01
    │                       └── 📙►:7[3]:A
    │                           └── 👉📙►:8[4]:A-empty-01
    │                               └── ·70bde6b (⌂|🏘|101) ►A-empty-02, ►A-empty-03
    │                                   └── ►:3[5]:main <> origin/main →:2:
    │                                       └── ·fafd9d0 (⌂|🏘|✓|111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:6[0]:origin/B →:5:
        └── →:7: (A)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:B <> origin/B →:6:⇡2 on fafd9d0 {1}
        ├── 📙:5:B <> origin/B →:6:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:4:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        ├── 📙:7:A
        └── 👉📙:8:A-empty-01
            └── ❄70bde6b (🏘️) ►A-empty-02, ►A-empty-03
    ");

    add_stack_with_segments(&mut meta, 2, "new-A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "new-B", StackState::InWorkspace, &[]);

    let (id, ref_name) = id_at(&repo, "new-A");
    let graph = Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?;

    // We can also summon new empty stacks from branches resting on the base, and set them
    // as entrypoint, to have two more stacks.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:8:new-B on fafd9d0 {3}
    │   └── 📙:8:new-B
    ├── ≡👉📙:7:new-A on fafd9d0 {2}
    │   └── 👉📙:7:new-A
    └── ≡📙:5:B <> origin/B →:6:⇡2 on fafd9d0 {1}
        ├── 📙:5:B <> origin/B →:6:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:4:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        ├── 📙:9:A
        └── 📙:10:A-empty-01
            └── ❄70bde6b (🏘️) ►A-empty-02, ►A-empty-03
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·2c12d75 (⌂|🏘|01)
    │       └── ►:3[1]:B
    │           └── ·320e105 (⌂|🏘|01)
    │               └── ►:4[2]:B-sub
    │                   └── ·2a31450 (⌂|🏘|01)
    │                       └── ►:5[3]:A
    │                           └── ·70bde6b (⌂|🏘|01)
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|11) ►new-A
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:B on fafd9d0
        ├── :3:B
        │   └── ·320e105 (🏘️)
        ├── :4:B-sub
        │   └── ·2a31450 (🏘️)
        └── :5:A
            └── ·70bde6b (🏘️)
    ");

    meta.data_mut().branches.clear();
    // Just repeat the existing segment verbatim, but also add a new unborn stack
    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["B-sub", "A"]);
    add_stack_with_segments(
        &mut meta,
        1,
        "new-A",
        StackState::InWorkspace,
        &["below-new-A"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·2c12d75 (⌂|🏘|01)
    │       ├── 📙►:3[1]:B
    │       │   └── ·320e105 (⌂|🏘|01)
    │       │       └── 📙►:4[2]:B-sub
    │       │           └── ·2a31450 (⌂|🏘|01)
    │       │               └── 📙►:5[3]:A
    │       │                   └── ·70bde6b (⌂|🏘|01)
    │       │                       └── ►:2[4]:main <> origin/main →:1:
    │       │                           └── ·fafd9d0 (⌂|🏘|✓|11)
    │       └── 📙►:6[1]:new-A
    │           └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:6:new-A on fafd9d0 {1}
    │   └── 📙:6:new-A
    └── ≡📙:3:B on fafd9d0 {0}
        ├── 📙:3:B
        │   └── ·320e105 (🏘️)
        ├── 📙:4:B-sub
        │   └── ·2a31450 (🏘️)
        └── 📙:5:A
            └── ·70bde6b (🏘️)
    ");

    Ok(())
}

#[test]
fn single_merge_into_main_base_archived() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-merge-into-main")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 866c905 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c6d714c (C) C
    *   0cc5a6f (origin/main, merge, main) Merge branch 'A' into merge
    |\  
    | * e255adc (A) A
    * | 7fdb58d (B) B
    |/  
    * fafd9d0 init
    ");

    let stack_id = add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["merge"]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    // By default, everything with metadata on the branch will show up, even if on the base.
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    └── ≡📙:3:C on 0cc5a6f {0}
        ├── 📙:3:C
        │   └── ·c6d714c (🏘️)
        └── 📙:7:merge
    ");

    // But even if everything is marked as archived, only the ones that matter are hidden.
    for head in &mut meta
        .data_mut()
        .branches
        .get_mut(&stack_id)
        .expect("just added")
        .heads
    {
        head.archived = true;
    }

    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Default::default())?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    └── ≡📙:3:C {0}
        └── 📙:3:C
            └── ·c6d714c (🏘️)
    ");

    // Finally, when the 'merge' branch is independent, it still works as it should.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "merge", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    ├── ≡📙:7:merge on 0cc5a6f {1}
    │   └── 📙:7:merge
    └── ≡📙:3:C on 0cc5a6f {0}
        └── 📙:3:C
            └── ·c6d714c (🏘️)
    ");

    // The order is respected.
    add_stack_with_segments(&mut meta, 1, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 0, "merge", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    ├── ≡📙:3:C on 0cc5a6f {1}
    │   └── 📙:3:C
    │       └── ·c6d714c (🏘️)
    └── ≡📙:7:merge on 0cc5a6f {0}
        └── 📙:7:merge
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉►:0[0]:gitbutler/workspace[🌳]
        └── ·47e1cf1 (⌂|1)
            └── ►:1[1]:anon:
                └── ·f40fb16 (⌂|1)
                    ├── ►:2[2]:anon:
                    │   └── ·450c58a (⌂|1)
                    │       └── ►:4[3]:anon:
                    │           └── ·0cc5a6f (⌂|1)
                    │               ├── ►:5[4]:anon:
                    │               │   └── ·7fdb58d (⌂|1)
                    │               │       └── ►:7[5]:anon:
                    │               │           └── ·fafd9d0 (⌂|1)
                    │               └── ►:6[4]:anon:
                    │                   └── ·e255adc (⌂|1)
                    │                       └── →:7:
                    └── ►:3[2]:anon:
                        └── ·c6d714c (⌂|1)
                            └── →:4:
    ");

    // This a very untypical setup, but it's not forbidden. Code might want to check
    // if the workspace commit is actually managed before proceeding.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:0:gitbutler/workspace[🌳] {1}
        └── :0:gitbutler/workspace[🌳]
            ├── ·47e1cf1
            ├── ·f40fb16
            ├── ·450c58a
            ├── ·0cc5a6f
            ├── ·7fdb58d
            └── ·fafd9d0
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉►:0[0]:entrypoint
    │   ├── ·98c5aba (⌂|1)
    │   ├── ·807b6ce (⌂|1)
    │   └── ·6d05486 (⌂|1)
    │       └── ►:3[2]:anon:
    │           ├── ·b688f2d (⌂|🏘|1)
    │           └── ·fafd9d0 (⌂|🏘|1)
    └── 📕►►►:1[0]:gitbutler/workspace[🌳]
        └── ·b6917c7 (⌂|🏘)
            └── ►:2[1]:main
                └── ·f7fe830 (⌂|🏘)
                    └── →:3:
    ");
    // This is an unmanaged workspace, even though commits from a workspace flow into it.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:entrypoint <> ✓!
    └── ≡:0:entrypoint {1}
        └── :0:entrypoint
            ├── ·98c5aba
            ├── ·807b6ce
            ├── ·6d05486
            ├── ·b688f2d (🏘️)
            └── ·fafd9d0 (🏘️)
    ");
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

    // Without hints, and no workspace data, the branch is normal!
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉►:0[0]:gitbutler/workspace[🌳]
    │   └── ·47e1cf1 (⌂|01)
    │       └── ►:1[1]:merge-2
    │           └── ·f40fb16 (⌂|01)
    │               ├── ►:2[2]:D
    │               │   └── ·450c58a (⌂|01)
    │               │       └── ►:4[3]:anon:
    │               │           └── ·0cc5a6f (⌂|01) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    │               │               ├── ►:5[4]:B
    │               │               │   └── ·7fdb58d (⌂|01)
    │               │               │       └── ►:7[5]:main <> origin/main →:8:
    │               │               │           └── ·fafd9d0 (⌂|11)
    │               │               └── ►:6[4]:A
    │               │                   └── ·e255adc (⌂|01)
    │               │                       └── →:7: (main →:8:)
    │               └── ►:3[2]:C
    │                   └── ·c6d714c (⌂|01)
    │                       └── →:4:
    └── ►:8[0]:origin/main →:7:
        └── →:7: (main →:8:)
    ");

    // Without workspace data this becomes a single-branch workspace, with `main` as normal segment.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:0:gitbutler/workspace[🌳] {1}
        ├── :0:gitbutler/workspace[🌳]
        │   └── ·47e1cf1
        ├── :1:merge-2
        │   └── ·f40fb16
        ├── :2:D
        │   ├── ·450c58a
        │   └── ·0cc5a6f ►empty-1-on-merge, ►empty-2-on-merge, ►merge
        ├── :5:B
        │   └── ·7fdb58d
        └── :7:main <> origin/main →:8:
            └── ❄️fafd9d0
    ");

    // There is empty stacks on top of `merge`, and they need to be connected to the incoming segments and the outgoing ones.
    // This also would leave the original segment empty unless we managed to just put empty stacks on top.
    add_stack_with_segments(
        &mut meta,
        0,
        "empty-2-on-merge",
        StackState::InWorkspace,
        &["empty-1-on-merge", "merge"],
    );
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·47e1cf1 (⌂|🏘|01)
    │       └── ►:6[1]:merge-2
    │           └── ·f40fb16 (⌂|🏘|01)
    │               ├── ►:7[2]:D
    │               │   └── ·450c58a (⌂|🏘|01)
    │               │       └── 📙►:9[3]:empty-2-on-merge
    │               │           └── 📙►:10[4]:empty-1-on-merge
    │               │               └── 📙►:11[5]:merge
    │               │                   └── ·0cc5a6f (⌂|🏘|01)
    │               │                       ├── ►:4[6]:B
    │               │                       │   └── ·7fdb58d (⌂|🏘|01)
    │               │                       │       └── ►:2[7]:main <> origin/main →:1:
    │               │                       │           └── ·fafd9d0 (⌂|🏘|✓|11)
    │               │                       └── ►:5[6]:A
    │               │                           └── ·e255adc (⌂|🏘|01)
    │               │                               └── →:2: (main →:1:)
    │               └── ►:8[2]:C
    │                   └── ·c6d714c (⌂|🏘|01)
    │                       └── →:9: (empty-2-on-merge)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:6:merge-2 on fafd9d0 {0}
        ├── :6:merge-2
        │   └── ·f40fb16 (🏘️)
        ├── :7:D
        │   └── ·450c58a (🏘️)
        ├── 📙:9:empty-2-on-merge
        ├── 📙:10:empty-1-on-merge
        ├── 📙:11:merge
        │   └── ·0cc5a6f (🏘️)
        └── :4:B
            └── ·7fdb58d (🏘️)
    ");
    Ok(())
}

#[test]
fn stack_configuration_is_respected_if_one_of_them_is_an_entrypoint() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-two-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> gitbutler/workspace, main, B, A) init");

    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let extra_target_options = standard_options_with_extra_target(&repo, "main");
    let graph = Graph::from_head(&repo, &*meta, extra_target_options.clone())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        ├── 📙►:2[1]:A
        │   └── ►:1[2]:anon:
        │       └── ·fafd9d0 (⌂|🏘|1) ►main
        └── 📙►:3[1]:B
            └── →:1:
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on fafd9d0
    ├── ≡📙:3:B on fafd9d0 {2}
    │   └── 📙:3:B
    └── ≡📙:2:A on fafd9d0 {1}
        └── 📙:2:A
    ");

    let (id, ref_name) = id_at(&repo, "B");
    let graph =
        Graph::from_commit_traversal(id, ref_name.clone(), &*meta, extra_target_options.clone())?
            .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 📕►►►:1[0]:gitbutler/workspace[🌳]
        ├── 📙►:2[1]:A
        │   └── ►:0[2]:anon:
        │       └── ·fafd9d0 (⌂|🏘|1) ►main
        └── 👉📙►:3[1]:B
            └── →:0:
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on fafd9d0
    ├── ≡👉📙:3:B on fafd9d0 {2}
    │   └── 👉📙:3:B
    └── ≡📙:2:A on fafd9d0 {1}
        └── 📙:2:A
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, extra_target_options)?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 📕►►►:1[0]:gitbutler/workspace[🌳]
        ├── 👉📙►:2[1]:A
        │   └── ►:0[2]:anon:
        │       └── ·fafd9d0 (⌂|🏘|1) ►main
        └── 📙►:3[1]:B
            └── →:0:
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on fafd9d0
    ├── ≡📙:3:B on fafd9d0 {2}
    │   └── 📙:3:B
    └── ≡👉📙:2:A on fafd9d0 {1}
        └── 👉📙:2:A
    ");

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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── 👉►:0[1]:main[🌳] <> origin/main →:2:
    │       └── ·fafd9d0 (⌂|🏘|✓|1) ►A, ►B, ►C, ►D, ►E, ►F
    └── ►:2[0]:origin/main →:0:
        └── →:0: (main[🌳] →:2:)
    ");

    // There is no workspace as `main` is the base of the workspace, so it's shown directly,
    // outside the workspace.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] <> origin/main →:2: {1}
        └── :0:main[🌳] <> origin/main →:2:
            └── ❄️fafd9d0 (🏘️|✓) ►A, ►B, ►C, ►D, ►E, ►F
    ");

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let graph = Graph::from_commit_traversal(id, ws_ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ►:2[1]:main[🌳] <> origin/main →:1:
    │       └── ·fafd9d0 (⌂|🏘|✓|1) ►A, ►B, ►C, ►D, ►E, ►F
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main[🌳] →:1:)
    ");

    // However, when the workspace is checked out, it's at least empty.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0");

    // The simplest possible setup where we can define how the workspace should look like,
    // in terms of dependent and independent virtual segments.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace
    │   ├── 📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── 📙►:5[3]:A
    │   │           └── 👉►:0[4]:main[🌳] <> origin/main →:2:
    │   │               └── ·fafd9d0 (⌂|🏘|✓|1)
    │   └── 📙►:6[1]:D
    │       └── 📙►:7[2]:E
    │           └── 📙►:8[3]:F
    │               └── →:0: (main[🌳] →:2:)
    └── ►:2[0]:origin/main →:0:
        └── →:0: (main[🌳] →:2:)
    ");

    // ~~There is no segmentation outside the workspace.~~ workspace segmentation always happens so the view is consistent.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] <> origin/main →:2: {1}
        └── :0:main[🌳] <> origin/main →:2:
            └── ❄️fafd9d0 (🏘️|✓)
    ");

    let graph = Graph::from_commit_traversal(id, ws_ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    // Now the dependent segments are applied, and so is the separate stack.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   ├── 📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── 📙►:5[3]:A
    │   │           └── ►:2[4]:main[🌳] <> origin/main →:1:
    │   │               └── ·fafd9d0 (⌂|🏘|✓|1)
    │   └── 📙►:6[1]:D
    │       └── 📙►:7[2]:E
    │           └── 📙►:8[3]:F
    │               └── →:2: (main[🌳] →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main[🌳] →:1:)
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:6:D on fafd9d0 {1}
    │   ├── 📙:6:D
    │   ├── 📙:7:E
    │   └── 📙:8:F
    └── ≡📙:3:C on fafd9d0 {0}
        ├── 📙:3:C
        ├── 📙:4:B
        └── 📙:5:A
    ");

    ws.graph.anonymize(&repo.remote_names())?;
    insta::assert_snapshot!(graph_workspace(&ws.graph.into_workspace()?), @r"
    📕🏘️⚠️:0:A <> ✓refs/remotes/remote-0/A on fafd9d0
    ├── ≡📙:6:E on fafd9d0 {1}
    │   ├── 📙:6:E
    │   ├── 📙:7:F
    │   └── 📙:8:G
    └── ≡📙:3:B on fafd9d0 {0}
        ├── 📙:3:B
        ├── 📙:4:C
        └── 📙:5:D
    ");

    let graph = Graph::from_commit_traversal(
        id,
        ws_ref_name,
        &*meta,
        but_graph::init::Options {
            dangerously_skip_postprocessing_for_debugging: true,
            ..standard_options()
        },
    )?
    .validated()?;
    // Show how the lack of post-processing affects the graph - remotes are also not connected.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ►:2[0]:anon:
    │       └── ·fafd9d0 (⌂|🏘|✓|1) ►A, ►B, ►C, ►D, ►E, ►F, ►main[🌳], ►origin/main
    └── ►:1[0]:origin/main
        └── →:2:
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0");

    Ok(())
}

#[test]
fn just_init_with_archived_branches() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    // Note the dedicated workspace branch without a workspace commit.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init");

    let stack_id = add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let graph = Graph::from_commit_traversal(id, ws_ref_name.clone(), &*meta, standard_options())?
        .validated()?;

    // By default, we see both stacks as they are configured, which disambiguates them.
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:C on fafd9d0 {0}
        ├── 📙:3:C
        ├── 📙:4:B
        └── 📙:5:A
    ");

    meta.data_mut()
        .branches
        .get_mut(&stack_id)
        .expect("just added")
        .heads[1]
        .archived = true;

    // The first archived segment causes everything else to be hidden.
    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Default::default())?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:C {0}
        └── 📙:3:C
    ");

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = false;

    // Now only the first one is archived.
    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Default::default())?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:C {0}
        ├── 📙:3:C
        └── 📙:4:B
    ");

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = true;
    heads[2].archived = true;

    // Archiving everything removes the stack entirely.
    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Default::default())?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0");
    Ok(())
}

#[test]
fn two_stacks_many_refs() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/one-stacks-many-refs")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 298d938 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 16f132b (S1, G, F) 2
    * 917b9da (E, D) 1
    * fafd9d0 (origin/main, main, C, B, A) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    // Without any information it looks quite barren.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·298d938 (⌂|🏘|01)
    │       └── ►:3[1]:anon:
    │           ├── ·16f132b (⌂|🏘|01) ►F, ►G, ►S1
    │           └── ·917b9da (⌂|🏘|01) ►D, ►E
    │               └── ►:2[2]:main <> origin/main →:1:
    │                   └── ·fafd9d0 (⌂|🏘|✓|11) ►A, ►B, ►C
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // With no workspace at all as the workspace segment isn't split.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:anon: on fafd9d0
        └── :3:anon:
            ├── ·16f132b (🏘️) ►F, ►G, ►S1
            └── ·917b9da (🏘️) ►D, ►E
    ");

    let (id, ref_name) = id_at(&repo, "S1");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    // The S1 starting position is a split, so there is more.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·298d938 (⌂|🏘)
    │       └── 👉►:0[1]:S1
    │           ├── ·16f132b (⌂|🏘|01) ►F, ►G
    │           └── ·917b9da (⌂|🏘|01) ►D, ►E
    │               └── ►:3[2]:main <> origin/main →:2:
    │                   └── ·fafd9d0 (⌂|🏘|✓|11) ►A, ►B, ►C
    └── ►:2[0]:origin/main →:3:
        └── →:3: (main →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡👉:0:S1 on fafd9d0
        └── 👉:0:S1
            ├── ·16f132b (🏘️) ►F, ►G
            └── ·917b9da (🏘️) ►D, ►E
    ");

    // Define the workspace.
    add_stack_with_segments(&mut meta, 1, "C", StackState::InWorkspace, &["B"]);
    add_stack_with_segments(&mut meta, 2, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "S1", StackState::InWorkspace, &["G", "F"]);
    add_stack_with_segments(&mut meta, 4, "D", StackState::InWorkspace, &["E"]);

    // We see that all segments are used: S1 C B A E D G F
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·298d938 (⌂|🏘|01)
    │       ├── 📙►:5[1]:C
    │       │   └── 📙►:6[2]:B
    │       │       └── ►:2[6]:main <> origin/main →:1:
    │       │           └── ·fafd9d0 (⌂|🏘|✓|11)
    │       ├── 📙►:7[1]:A
    │       │   └── →:2: (main →:1:)
    │       └── 📙►:8[1]:S1
    │           └── 📙►:9[2]:G
    │               └── 📙►:10[3]:F
    │                   └── ·16f132b (⌂|🏘|01)
    │                       └── 📙►:11[4]:D
    │                           └── 📙►:12[5]:E
    │                               └── ·917b9da (⌂|🏘|01)
    │                                   └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:8:S1 on fafd9d0 {3}
    │   ├── 📙:8:S1
    │   ├── 📙:9:G
    │   ├── 📙:10:F
    │   │   └── ·16f132b (🏘️)
    │   ├── 📙:11:D
    │   └── 📙:12:E
    │       └── ·917b9da (🏘️)
    ├── ≡📙:7:A on fafd9d0 {2}
    │   └── 📙:7:A
    └── ≡📙:5:C on fafd9d0 {1}
        ├── 📙:5:C
        └── 📙:6:B
    ");

    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    // This should look the same as before, despite the starting position.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·298d938 (⌂|🏘)
    │       ├── 📙►:5[1]:C
    │       │   └── 📙►:6[2]:B
    │       │       └── ►:3[6]:main <> origin/main →:2:
    │       │           └── ·fafd9d0 (⌂|🏘|✓|11)
    │       ├── 📙►:7[1]:A
    │       │   └── →:3: (main →:2:)
    │       └── 👉📙►:8[1]:S1
    │           └── 📙►:9[2]:G
    │               └── 📙►:10[3]:F
    │                   └── ·16f132b (⌂|🏘|01)
    │                       └── 📙►:11[4]:D
    │                           └── 📙►:12[5]:E
    │                               └── ·917b9da (⌂|🏘|01)
    │                                   └── →:3: (main →:2:)
    └── ►:2[0]:origin/main →:3:
        └── →:3: (main →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡👉📙:8:S1 on fafd9d0 {3}
    │   ├── 👉📙:8:S1
    │   ├── 📙:9:G
    │   ├── 📙:10:F
    │   │   └── ·16f132b (🏘️)
    │   ├── 📙:11:D
    │   └── 📙:12:E
    │       └── ·917b9da (🏘️)
    ├── ≡📙:7:A on fafd9d0 {2}
    │   └── 📙:7:A
    └── ≡📙:5:C on fafd9d0 {1}
        ├── 📙:5:C
        └── 📙:6:B
    ");
    Ok(())
}

#[test]
fn just_init_with_branches_complex() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;

    // A combination of dependent and independent stacks.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B"]);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "D", StackState::InWorkspace, &["E"]);
    add_stack_with_segments(&mut meta, 3, "F", StackState::InWorkspace, &[]);

    let (id, ref_name) = id_at(&repo, "gitbutler/workspace");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   ├── 📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── ►:2[3]:main[🌳] <> origin/main →:1:
    │   │           └── ·fafd9d0 (⌂|🏘|✓|1)
    │   ├── 📙►:5[1]:A
    │   │   └── →:2: (main[🌳] →:1:)
    │   ├── 📙►:6[1]:D
    │   │   └── 📙►:7[2]:E
    │   │       └── →:2: (main[🌳] →:1:)
    │   └── 📙►:8[1]:F
    │       └── →:2: (main[🌳] →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main[🌳] →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:8:F on fafd9d0 {3}
    │   └── 📙:8:F
    ├── ≡📙:6:D on fafd9d0 {2}
    │   ├── 📙:6:D
    │   └── 📙:7:E
    ├── ≡📙:5:A on fafd9d0 {1}
    │   └── 📙:5:A
    └── ≡📙:3:C on fafd9d0 {0}
        ├── 📙:3:C
        └── 📙:4:B
    ");

    let (id, ref_name) = id_at(&repo, "C");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    // The entrypoint shouldn't affect the outcome (even though it changes the initial segmentation).
    // However, as the segment it's on is integrated, it's not considered to be part of the workspace.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace
    │   ├── 👉📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── ►:0[3]:main[🌳] <> origin/main →:2:
    │   │           └── ·fafd9d0 (⌂|🏘|✓|1)
    │   ├── 📙►:5[1]:A
    │   │   └── →:0: (main[🌳] →:2:)
    │   ├── 📙►:6[1]:D
    │   │   └── 📙►:7[2]:E
    │   │       └── →:0: (main[🌳] →:2:)
    │   └── 📙►:8[1]:F
    │       └── →:0: (main[🌳] →:2:)
    └── ►:2[0]:origin/main →:0:
        └── →:0: (main[🌳] →:2:)
    ");

    // We should see the same stacks as we did before, just with a different entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:8:F on fafd9d0 {3}
    │   └── 📙:8:F
    ├── ≡📙:6:D on fafd9d0 {2}
    │   ├── 📙:6:D
    │   └── 📙:7:E
    ├── ≡📙:5:A on fafd9d0 {1}
    │   └── 📙:5:A
    └── ≡👉📙:3:C on fafd9d0 {0}
        ├── 👉📙:3:C
        └── 📙:4:B
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·9bcd3af (⌂|🏘|01)
    │       └── ►:2[1]:main <> origin/main →:1:
    │           ├── ·998eae6 (⌂|🏘|✓|11)
    │           └── ·fafd9d0 (⌂|🏘|✓|11)
    └── ►:1[0]:origin/main →:2:
        ├── 🟣ca7baa7 (✓)
        └── 🟣7ea1468 (✓)
            └── →:2: (main →:1:)
    ");

    // Everything in the workspace is integrated, thus it's empty.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 998eae6");

    let (id, ref_name) = id_at(&repo, "main");
    // The integration branch can be in the workspace and be checked out.
    let graph = Graph::from_commit_traversal(id, Some(ref_name), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·9bcd3af (⌂|🏘)
    │       └── 👉►:0[1]:main <> origin/main →:2:
    │           ├── ·998eae6 (⌂|🏘|✓|1)
    │           └── ·fafd9d0 (⌂|🏘|✓|1)
    └── ►:2[0]:origin/main →:0:
        ├── 🟣ca7baa7 (✓)
        └── 🟣7ea1468 (✓)
            └── →:0: (main →:2:)
    ");

    // If it's checked out, we must show it, but it's not part of the workspace.
    // This is special as other segments still are.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:⇣2 {1}
        └── :0:main <> origin/main →:2:⇣2
            ├── 🟣ca7baa7 (✓)
            ├── 🟣7ea1468 (✓)
            ├── ❄️998eae6 (🏘️|✓)
            └── ❄️fafd9d0 (🏘️|✓)
    ");
    Ok(())
}

#[test]
fn deduced_remote_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/deduced-remote-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8b39ce4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9d34471 (A) A2
    * 5b89c71 A1
    | * 3ea1a8f (push-remote/A, origin/A) only-remote-02
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·8b39ce4 (⌂|🏘|001)
    │       └── ►:1[1]:A <> origin/A →:2:
    │           ├── ·9d34471 (⌂|🏘|011)
    │           └── ·5b89c71 (⌂|🏘|011)
    │               └── ►:5[3]:anon:
    │                   └── ·998eae6 (⌂|🏘|111)
    │                       └── ►:3[4]:main
    │                           └── ·fafd9d0 (⌂|🏘|111)
    └── ►:2[0]:origin/A →:1:
        ├── 🟣3ea1a8f (0x0|100)
        └── 🟣9c50f71 (0x0|100)
            └── ►:4[1]:anon:
                └── 🟣2cfbb79 (0x0|100)
                    ├── →:5:
                    └── ►:6[2]:anon:
                        └── 🟣e898cd0 (0x0|100)
                            └── →:5:
    ");
    // There is no target branch, so nothing is integrated, and `main` shows up.
    // It's not special.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> origin/A →:2:⇡2⇣4
        ├── :1:A <> origin/A →:2:⇡2⇣4
        │   ├── 🟣3ea1a8f
        │   ├── 🟣9c50f71
        │   ├── 🟣2cfbb79
        │   ├── 🟣e898cd0
        │   ├── ·9d34471 (🏘️)
        │   ├── ·5b89c71 (🏘️)
        │   └── ❄️998eae6 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    let id = id_by_rev(&repo, ":/init");
    let graph = Graph::from_commit_traversal(id, None, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·8b39ce4 (⌂|🏘)
    │       └── ►:2[1]:A <> origin/A →:3:
    │           ├── ·9d34471 (⌂|🏘|010)
    │           └── ·5b89c71 (⌂|🏘|010)
    │               └── ►:5[3]:anon:
    │                   └── ·998eae6 (⌂|🏘|110)
    │                       └── 👉►:0[4]:main
    │                           └── ·fafd9d0 (⌂|🏘|111)
    └── ►:3[0]:origin/A →:2:
        ├── 🟣3ea1a8f (0x0|100)
        └── 🟣9c50f71 (0x0|100)
            └── ►:4[1]:anon:
                └── 🟣2cfbb79 (0x0|100)
                    ├── →:5:
                    └── ►:6[2]:anon:
                        └── 🟣e898cd0 (0x0|100)
                            └── →:5:
    ");
    // The whole workspace is visible, but it's clear where the entrypoint is.
    // As there is no target ref, `main` shows up.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡:2:A <> origin/A →:3:⇡2⇣4
        ├── :2:A <> origin/A →:3:⇡2⇣4
        │   ├── 🟣3ea1a8f
        │   ├── 🟣9c50f71
        │   ├── 🟣2cfbb79
        │   ├── 🟣e898cd0
        │   ├── ·9d34471 (🏘️)
        │   ├── ·5b89c71 (🏘️)
        │   └── ❄️998eae6 (🏘️)
        └── 👉:0:main
            └── ❄fafd9d0 (🏘️)
    ");

    // When the push-remote is configured, it overrides the remote we use for listing, even if a fetch remote is available.
    meta.data_mut()
        .default_target
        .as_mut()
        .expect("set by default")
        .push_remote_name = Some("push-remote".into());
    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·8b39ce4 (⌂|🏘|001)
    │       └── ►:1[1]:A <> push-remote/A →:2:
    │           ├── ·9d34471 (⌂|🏘|011)
    │           └── ·5b89c71 (⌂|🏘|011)
    │               └── ►:5[3]:anon:
    │                   └── ·998eae6 (⌂|🏘|111)
    │                       └── ►:3[4]:main
    │                           └── ·fafd9d0 (⌂|🏘|111)
    └── ►:2[0]:push-remote/A →:1:
        ├── 🟣3ea1a8f (0x0|100)
        └── 🟣9c50f71 (0x0|100)
            └── ►:4[1]:anon:
                └── 🟣2cfbb79 (0x0|100)
                    ├── →:5:
                    └── ►:6[2]:anon:
                        └── 🟣e898cd0 (0x0|100)
                            └── →:5:
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> push-remote/A →:2:⇡2⇣4
        ├── :1:A <> push-remote/A →:2:⇡2⇣4
        │   ├── 🟣3ea1a8f
        │   ├── 🟣9c50f71
        │   ├── 🟣2cfbb79
        │   ├── 🟣e898cd0
        │   ├── ·9d34471 (🏘️)
        │   ├── ·5b89c71 (🏘️)
        │   └── ❄️998eae6 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·7786959 (⌂|🏘|000001)
    │       └── ►:3[1]:B <> origin/B →:4:
    │           └── ·312f819 (⌂|🏘|000101)
    │               └── ►:5[2]:A <> origin/A →:6:
    │                   └── ·e255adc (⌂|🏘|010101)
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·fafd9d0 (⌂|🏘|✓|111111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/B →:3:
        └── 🟣682be32 (0x0|001000)
            └── ►:6[1]:origin/A →:5:
                └── 🟣e29c23d (0x0|101000)
                    └── →:2: (main →:1:)
    ");
    // It's worth noting that we avoid double-listing remote commits that are also
    // directly owned by another remote segment.
    // they have to be considered as something relevant to the branch history.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:B <> origin/B →:4:⇡1⇣1 on fafd9d0
        ├── :3:B <> origin/B →:4:⇡1⇣1
        │   ├── 🟣682be32
        │   └── ·312f819 (🏘️)
        └── :5:A <> origin/A →:6:⇡1⇣1
            ├── 🟣e29c23d
            └── ·e255adc (🏘️)
    ");

    // The result is the same when changing the entrypoint.
    let (id, name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·7786959 (⌂|🏘)
    │       └── ►:5[1]:B <> origin/B →:6:
    │           └── ·312f819 (⌂|🏘|01000)
    │               └── 👉►:0[2]:A <> origin/A →:4:
    │                   └── ·e255adc (⌂|🏘|01001)
    │                       └── ►:3[3]:main <> origin/main →:2:
    │                           └── ·fafd9d0 (⌂|🏘|✓|11111)
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:6[0]:origin/B →:5:
        └── 🟣682be32 (0x0|10000)
            └── ►:4[1]:origin/A →:0:
                └── 🟣e29c23d (0x0|10100)
                    └── →:3: (main →:2:)
    ");
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:5:B <> origin/B →:6:⇡1⇣1 on fafd9d0
        ├── :5:B <> origin/B →:6:⇡1⇣1
        │   ├── 🟣682be32
        │   └── ·312f819 (🏘️)
        └── 👉:0:A <> origin/A →:4:⇡1⇣1
            ├── 🟣e29c23d
            └── ·e255adc (🏘️)
    ");
    insta::assert_debug_snapshot!(ws.graph.statistics(), @r#"
    Statistics {
        segments: 7,
        segments_integrated: 1,
        segments_remote: 2,
        segments_with_remote_tracking_branch: 3,
        segments_empty: 1,
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
                        "refs/remotes/origin/main",
                    ),
                ),
                NodeIndex(2),
                None,
            ),
            (
                Some(
                    FullName(
                        "refs/remotes/origin/B",
                    ),
                ),
                NodeIndex(6),
                Some(
                    CommitFlags(
                        0x80,
                    ),
                ),
            ),
        ],
        segments_at_bottom: 1,
        connections: 6,
        commits: 6,
        commit_references: 0,
        commits_at_cutoff: 0,
    }
    "#);
    Ok(())
}

#[test]
fn target_with_remote_on_stack_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-ahead-and-on-stack-tip")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * dd0cca8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e255adc (main, A) A
    * fafd9d0 (origin/main) init
    ");
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        └── ·dd0cca8 (⌂|🏘|01)
            └── 📙►:2[1]:A
                └── ·e255adc (⌂|🏘|11) ►main
                    └── ►:1[2]:origin/main
                        └── ·fafd9d0 (⌂|🏘|✓|11)
    ");

    // The main branch is not present, as it's the target.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:2:A on fafd9d0 {1}
        └── 📙:2:A
            └── ·e255adc (🏘️) ►main
    ");

    // But mention it if it's in the workspace. It should retain order.
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["main"]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        ├── 📙:3:A
        └── 📙:4:main <> origin/main →:1:⇡1
            └── ·e255adc (🏘️)
    ");

    // But mention it if it's in the workspace. It should retain order - inverting the order is fine.
    add_stack_with_segments(&mut meta, 1, "main", StackState::InWorkspace, &["A"]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:main <> origin/main →:1: on fafd9d0 {1}
        ├── 📙:3:main <> origin/main →:1:
        └── 📙:4:A
            └── ·e255adc (🏘️)
    ");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·e30f90c (⌂|🏘|000001)
    │       └── ►:6[1]:anon:
    │           └── ·2173153 (⌂|🏘|000101) ►C, ►ambiguous-C
    │               └── ►:9[2]:B <> origin/B →:5:
    │                   └── ·312f819 (⌂|🏘|011101) ►ambiguous-B
    │                       └── ►:8[3]:A <> origin/A →:7:
    │                           └── ·e255adc (⌂|🏘|111101) ►ambiguous-A
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|111111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    ├── ►:3[0]:origin/C
    │   └── →:6:
    ├── ►:4[0]:origin/ambiguous-C
    │   └── →:6:
    ├── ►:5[0]:origin/B →:9:
    │   └── 🟣ac24e74 (0x0|010000)
    │       └── →:9: (B →:5:)
    └── ►:7[0]:origin/A →:8:
        └── →:8: (A →:7:)
    ");

    assert_eq!(
        graph.partial_segments().count(),
        0,
        "a fully realized graph"
    );
    // An anonymous segment to start with is alright, and can always happen for other situations as well.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:6:anon: on fafd9d0
        ├── :6:anon:
        │   └── ·2173153 (🏘️) ►C, ►ambiguous-C
        ├── :9:B <> origin/B →:5:⇣1
        │   ├── 🟣ac24e74
        │   └── ❄️312f819 (🏘️) ►ambiguous-B
        └── :8:A <> origin/A →:7:
            └── ❄️e255adc (🏘️) ►ambiguous-A
    ");

    // If 'C' is in the workspace, it's naturally disambiguated.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·e30f90c (⌂|🏘|000001)
    │       └── 📙►:3[1]:C <> origin/C →:4:
    │           └── ·2173153 (⌂|🏘|000101) ►ambiguous-C
    │               └── ►:9[2]:B <> origin/B →:6:
    │                   └── ·312f819 (⌂|🏘|011101) ►ambiguous-B
    │                       └── ►:8[3]:A <> origin/A →:7:
    │                           └── ·e255adc (⌂|🏘|111101) ►ambiguous-A
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|111111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    ├── ►:4[0]:origin/C →:3:
    │   └── →:3: (C →:4:)
    ├── ►:5[0]:origin/ambiguous-C
    │   └── →:3: (C →:4:)
    ├── ►:6[0]:origin/B →:9:
    │   └── 🟣ac24e74 (0x0|010000)
    │       └── →:9: (B →:6:)
    └── ►:7[0]:origin/A →:8:
        └── →:8: (A →:7:)
    ");
    // And because `C` is in the workspace data, its data is denoted.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:C <> origin/C →:4: on fafd9d0 {0}
        ├── 📙:3:C <> origin/C →:4:
        │   └── ❄️2173153 (🏘️) ►ambiguous-C
        ├── :9:B <> origin/B →:6:⇣1
        │   ├── 🟣ac24e74
        │   └── ❄️312f819 (🏘️) ►ambiguous-B
        └── :8:A <> origin/A →:7:
            └── ❄️e255adc (🏘️) ►ambiguous-A
    ");
    Ok(())
}

#[test]
fn integrated_tips_stop_early_if_remote_is_not_configured() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-segments-one-integrated-without-remote")?;
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
    // We can abort early if there is only integrated commits left, but also if there is *no remote setup*.
    // We also abort integrated named segments early, unless these are named as being part of the
    // workspace - here `A` is cut off.
    // Without remote, the traversal can't setup `main` as target for the workspace entrypoint to find.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    assert_eq!(graph.partial_segments().count(), 1);
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘|1)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘|1)
    │           └── ·03ad472 (⌂|🏘|1)
    │               └── ►:5[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   └── ✂·a381df5 (⌂|🏘|✓|1)
    └── ►:1[0]:origin/main →:4:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:2[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:4[2]:main <> origin/main →:1:
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:5: (A)
    ");
    // It's true that `A` is fully integrated so it isn't displayed. so from a workspace-perspective
    // it's the right answer.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡:3:B on 79bbb29
        └── :3:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["A"]);
    // ~~Now that `A` is part of the workspace, it's not cut off anymore.~~
    // This special handling was removed for now, relying on limits and extensions.
    // And since it's integrated, traversal is stopped without convergence.
    // We see more though as we add workspace segments immediately.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘|1)
    │       └── 📙►:2[1]:B
    │           ├── ·6b1a13b (⌂|🏘|1)
    │           └── ·03ad472 (⌂|🏘|1)
    │               └── 📙►:3[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   ├── ·a381df5 (⌂|🏘|✓|1)
    │                   └── ✂·777b552 (⌂|🏘|✓|1)
    └── ►:1[0]:origin/main →:5:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:5[2]:main <> origin/main →:1:
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:3: (A)
    ");
    // `A` is integrated, hence it's not shown.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡📙:2:B on 79bbb29 {0}
        └── 📙:2:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    // The limit is effective for integrated workspaces branches, and it doesn't unnecessarily
    // prolong the traversal once the all tips are known to be integrated.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘|1)
    │       └── 📙►:2[1]:B
    │           ├── ·6b1a13b (⌂|🏘|1)
    │           └── ·03ad472 (⌂|🏘|1)
    │               └── 📙►:3[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   ├── ·a381df5 (⌂|🏘|✓|1)
    │                   └── ✂·777b552 (⌂|🏘|✓|1)
    └── ►:1[0]:origin/main →:5:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:5[2]:main <> origin/main →:1:
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:3: (A)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡📙:2:B on 79bbb29 {0}
        └── 📙:2:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    // When looking from an integrated branch within the workspace, but without limit,
    // the (lack of) limit is respected.
    // When the entrypoint starts on an integrated commit, the 'all-tips-are-integrated' condition doesn't
    // kick in anymore.
    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘)
    │       └── ►:4[1]:B
    │           ├── ·6b1a13b (⌂|🏘)
    │           └── ·03ad472 (⌂|🏘)
    │               └── 👉►:0[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   ├── ·a381df5 (⌂|🏘|✓|1)
    │                   └── ·777b552 (⌂|🏘|✓|1)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘|✓|1)
    │                               ├── ►:7[5]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘|✓|1)
    │                               │       └── ►:5[6]:main <> origin/main →:2:
    │                               │           ├── ·4b3e5a8 (⌂|🏘|✓|1)
    │                               │           ├── ·34d0715 (⌂|🏘|✓|1)
    │                               │           └── ·eb5f731 (⌂|🏘|✓|1)
    │                               └── ►:8[4]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                                   └── ·4deea74 (⌂|🏘|✓|1)
    │                                       └── →:7:
    └── ►:2[0]:origin/main →:5:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:3[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── →:5: (main →:2:)
                    └── →:0: (A)
    ");
    // It looks like some commits are missing, but it's a first-parent traversal.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:A <> ✓!
    └── ≡:0:A {1}
        ├── :0:A
        │   ├── ❄79bbb29 (🏘️|✓)
        │   ├── ❄fc98174 (🏘️|✓)
        │   ├── ❄a381df5 (🏘️|✓)
        │   ├── ❄777b552 (🏘️|✓)
        │   ├── ❄ce4a760 (🏘️|✓)
        │   └── ❄01d0e1e (🏘️|✓)
        └── :5:main <> origin/main →:2:⇣3
            ├── 🟣d0df794 (✓)
            ├── 🟣09c6e08 (✓)
            ├── 🟣7b9f260 (✓)
            ├── ❄️4b3e5a8 (🏘️|✓)
            ├── ❄️34d0715 (🏘️|✓)
            └── ❄️eb5f731 (🏘️|✓)
    ");

    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options().with_limit_hint(1))?
            .validated()?;
    // It's still getting quite far despite the limit due to other heads searching for their goals,
    // but also ends traversal early.
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘)
    │       └── ►:4[1]:B
    │           ├── ·6b1a13b (⌂|🏘)
    │           └── ·03ad472 (⌂|🏘)
    │               └── 👉►:0[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   ├── ·a381df5 (⌂|🏘|✓|1)
    │                   └── ✂·777b552 (⌂|🏘|✓|1)
    └── ►:2[0]:origin/main →:5:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:3[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:5[2]:main <> origin/main →:2:
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:0: (A)
    ");
    // Because the branch is integrated, the surrounding workspace isn't shown.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:A <> ✓!
    └── ≡:0:A {1}
        └── :0:A
            ├── ·79bbb29 (🏘️|✓)
            ├── ·fc98174 (🏘️|✓)
            ├── ·a381df5 (🏘️|✓)
            └── ✂️·777b552 (🏘️|✓)
    ");

    // See what happens with an out-of-workspace HEAD and an arbitrary extra target.
    let (id, _ref_name) = id_at(&repo, "origin/main");
    let graph = Graph::from_commit_traversal(
        id,
        None,
        &*meta,
        standard_options_with_extra_target(&repo, "gitbutler/workspace"),
    )?
    .validated()?;
    // It keeps the tip-settings of the workspace it setup by itself, and doesn't override this
    // with the extra-target settings.
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘)
    │       └── ►:4[1]:B
    │           ├── ·6b1a13b (⌂|🏘)
    │           └── ·03ad472 (⌂|🏘)
    │               └── ►:6[3]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   ├── ·a381df5 (⌂|🏘|✓|1)
    │                   └── ·777b552 (⌂|🏘|✓|1)
    │                       └── ►:7[4]:anon:
    │                           └── ·ce4a760 (⌂|🏘|✓|1)
    │                               ├── ►:8[6]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘|✓|1)
    │                               │       └── ►:5[7]:main <> origin/main →:2:
    │                               │           ├── ·4b3e5a8 (⌂|🏘|✓|1)
    │                               │           ├── ·34d0715 (⌂|🏘|✓|1)
    │                               │           └── ·eb5f731 (⌂|🏘|✓|1)
    │                               └── ►:9[5]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                                   └── ·4deea74 (⌂|🏘|✓|1)
    │                                       └── →:8:
    └── ►:2[0]:origin/main →:5:
        └── ►:0[1]:anon:
            ├── 👉·d0df794 (⌂|✓|1)
            └── ·09c6e08 (⌂|✓|1)
                └── ►:3[2]:anon:
                    └── ·7b9f260 (⌂|✓|1)
                        ├── →:5: (main →:2:)
                        └── →:6: (A)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:DETACHED <> ✓! on 79bbb29
    └── ≡:0:anon: {1}
        ├── :0:anon:
        │   ├── ❄d0df794 (✓)
        │   ├── ❄09c6e08 (✓)
        │   └── ❄7b9f260 (✓)
        └── :5:main <> origin/main →:2:
            ├── ❄️4b3e5a8 (🏘️|✓)
            ├── ❄️34d0715 (🏘️|✓)
            └── ❄️eb5f731 (🏘️|✓)
    ");

    // However, when choosing an initially unknown branch, it will get the extra target tip settings.
    let graph = Graph::from_commit_traversal(
        id,
        None,
        &*meta,
        standard_options_with_extra_target(&repo, "B"),
    )?
    .validated()?;
    // For now we don't do anything to limit the each in single-branch mode using extra-targets.
    // Thanks to the limit-transplant we get to discover more of the workspace.
    // TODO(extra-target): make it work so they limit single branches even, but it's a special case
    //                     as we can't have remotes here.
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘|✓)
    │           └── ·03ad472 (⌂|🏘|✓)
    │               └── ►:6[3]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   ├── ·a381df5 (⌂|🏘|✓|1)
    │                   └── ·777b552 (⌂|🏘|✓|1)
    │                       └── ►:7[4]:anon:
    │                           └── ·ce4a760 (⌂|🏘|✓|1)
    │                               ├── ►:8[6]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘|✓|1)
    │                               │       └── ►:5[7]:main <> origin/main →:2:
    │                               │           ├── ·4b3e5a8 (⌂|🏘|✓|1)
    │                               │           ├── ·34d0715 (⌂|🏘|✓|1)
    │                               │           └── ·eb5f731 (⌂|🏘|✓|1)
    │                               └── ►:9[5]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                                   └── ·4deea74 (⌂|🏘|✓|1)
    │                                       └── →:8:
    └── ►:2[0]:origin/main →:5:
        └── ►:0[1]:anon:
            ├── 👉·d0df794 (⌂|✓|1)
            └── ·09c6e08 (⌂|✓|1)
                └── ►:4[2]:anon:
                    └── ·7b9f260 (⌂|✓|1)
                        ├── →:5: (main →:2:)
                        └── →:6: (A)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:DETACHED <> ✓! on 79bbb29
    └── ≡:0:anon: {1}
        ├── :0:anon:
        │   ├── ❄d0df794 (✓)
        │   ├── ❄09c6e08 (✓)
        │   └── ❄7b9f260 (✓)
        └── :5:main <> origin/main →:2:
            ├── ❄️4b3e5a8 (🏘️|✓)
            ├── ❄️34d0715 (🏘️|✓)
            └── ❄️eb5f731 (🏘️|✓)
    ");

    Ok(())
}

#[test]
fn integrated_tips_do_not_stop_early() -> anyhow::Result<()> {
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
    // Thanks to the remote `main` is searched for by the entrypoint.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘|01)
    │       └── ►:4[1]:B
    │           ├── ·6b1a13b (⌂|🏘|01)
    │           └── ·03ad472 (⌂|🏘|01)
    │               └── ►:5[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|01)
    │                   ├── ·fc98174 (⌂|🏘|✓|01)
    │                   ├── ·a381df5 (⌂|🏘|✓|01)
    │                   └── ·777b552 (⌂|🏘|✓|01)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘|✓|01)
    │                               ├── ►:7[5]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘|✓|01)
    │                               │       └── ►:2[6]:main <> origin/main →:1:
    │                               │           ├── ·4b3e5a8 (⌂|🏘|✓|11)
    │                               │           ├── ·34d0715 (⌂|🏘|✓|11)
    │                               │           └── ·eb5f731 (⌂|🏘|✓|11)
    │                               └── ►:8[4]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘|✓|01)
    │                                   └── ·4deea74 (⌂|🏘|✓|01)
    │                                       └── →:7:
    └── ►:1[0]:origin/main →:2:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:3[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── →:2: (main →:1:)
                    └── →:5: (A)
    ");

    // This search discovers the whole workspace, without the integrated one.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
    └── ≡:4:B on 79bbb29
        └── :4:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    // However, we can specify an additional/old target segment to show integrated portions as well.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 4b3e5a8
    └── ≡:4:B on 4b3e5a8
        ├── :4:B
        │   ├── ·6b1a13b (🏘️)
        │   └── ·03ad472 (🏘️)
        └── :5:A
            ├── ·79bbb29 (🏘️|✓)
            ├── ·fc98174 (🏘️|✓)
            ├── ·a381df5 (🏘️|✓)
            ├── ·777b552 (🏘️|✓)
            ├── ·ce4a760 (🏘️|✓)
            └── ·01d0e1e (🏘️|✓)
    ");

    // When looking from an integrated branch within the workspace, and without limit
    // the limit isn't respected, and we still know the whole workspace.
    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘)
    │       └── ►:5[1]:B
    │           ├── ·6b1a13b (⌂|🏘)
    │           └── ·03ad472 (⌂|🏘)
    │               └── 👉►:0[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|01)
    │                   ├── ·fc98174 (⌂|🏘|✓|01)
    │                   ├── ·a381df5 (⌂|🏘|✓|01)
    │                   └── ·777b552 (⌂|🏘|✓|01)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘|✓|01)
    │                               ├── ►:7[5]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘|✓|01)
    │                               │       └── ►:3[6]:main <> origin/main →:2:
    │                               │           ├── ·4b3e5a8 (⌂|🏘|✓|11)
    │                               │           ├── ·34d0715 (⌂|🏘|✓|11)
    │                               │           └── ·eb5f731 (⌂|🏘|✓|11)
    │                               └── ►:8[4]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘|✓|01)
    │                                   └── ·4deea74 (⌂|🏘|✓|01)
    │                                       └── →:7:
    └── ►:2[0]:origin/main →:3:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── →:3: (main →:2:)
                    └── →:0: (A)
    ");

    // The entrypoint isn't contained in the workspace anymore, so it's standalone.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:A <> ✓!
    └── ≡:0:A {1}
        ├── :0:A
        │   ├── ❄79bbb29 (🏘️|✓)
        │   ├── ❄fc98174 (🏘️|✓)
        │   ├── ❄a381df5 (🏘️|✓)
        │   ├── ❄777b552 (🏘️|✓)
        │   ├── ❄ce4a760 (🏘️|✓)
        │   └── ❄01d0e1e (🏘️|✓)
        └── :3:main <> origin/main →:2:⇣3
            ├── 🟣d0df794 (✓)
            ├── 🟣09c6e08 (✓)
            ├── 🟣7b9f260 (✓)
            ├── ❄️4b3e5a8 (🏘️|✓)
            ├── ❄️34d0715 (🏘️|✓)
            └── ❄️eb5f731 (🏘️|✓)
    ");

    // When converting to a workspace, we are still aware of the workspace membership as long as
    // the lower bound of the workspace includes it.
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 4b3e5a8
    └── ≡:5:B on 4b3e5a8
        ├── :5:B
        │   ├── ·6b1a13b (🏘️)
        │   └── ·03ad472 (🏘️)
        └── 👉:0:A
            ├── ·79bbb29 (🏘️|✓)
            ├── ·fc98174 (🏘️|✓)
            ├── ·a381df5 (🏘️|✓)
            ├── ·777b552 (🏘️|✓)
            ├── ·ce4a760 (🏘️|✓)
            └── ·01d0e1e (🏘️|✓)
    ");

    let (id, ref_name) = id_at(&repo, "main");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    // When the branch is below the forkpoint, the workspace also isn't shown anymore.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:⇣3 {1}
        └── :0:main <> origin/main →:2:⇣3
            ├── 🟣d0df794 (✓)
            ├── 🟣09c6e08 (✓)
            ├── 🟣7b9f260 (✓)
            ├── ❄️4b3e5a8 (🏘️|✓)
            ├── ❄️34d0715 (🏘️|✓)
            └── ❄️eb5f731 (🏘️|✓)
    ");

    let id = id_by_rev(&repo, "main~1");
    let graph = Graph::from_commit_traversal(id, None, &*meta, standard_options())?.validated()?;
    // Detached states are also possible.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:DETACHED <> ✓!
    └── ≡:0:anon: {1}
        └── :0:anon:
            ├── ·34d0715 (🏘️|✓)
            └── ·eb5f731 (🏘️|✓)
    ");
    Ok(())
}

#[test]
fn workspace_without_target_can_see_remote() -> anyhow::Result<()> {
    let (mut repo, _) = read_only_in_memory_scenario("ws/main-with-remote-and-workspace-ref")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 956a3de (origin/main) on-remote-only
    * 3183e43 (HEAD -> main, gitbutler/workspace) M1
    ");

    // Use an in-memory version directly as vb.toml can't bring in remote branches.
    let mut meta = InMemoryRefMetadata::default();
    let ws_ref = "refs/heads/gitbutler/workspace".try_into()?;
    let mut ws = meta.workspace(ws_ref)?;
    for (idx, ref_name) in ["refs/heads/main", "refs/remotes/origin/main"]
        .into_iter()
        .enumerate()
    {
        ws.stacks.push(WorkspaceStack {
            id: StackId::from_number_for_testing(idx as u128),
            branches: vec![WorkspaceStackBranch {
                ref_name: ref_name.try_into()?,
                head_commit_id: None,
                archived: false,
            }],
            workspacecommit_relation: WorkspaceCommitRelation::Merged,
        });
        meta.branches.push((
            ref_name.try_into()?,
            but_core::ref_metadata::Branch::default(),
        ))
    }
    meta.set_workspace(&ws)?;

    let graph = Graph::from_head(&repo, &meta, standard_options())?.validated()?;
    // Main is a normal branch, and its remote is known.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── 👉📙►:0[1]:main[🌳] <> origin/main →:2:
    │       └── ·3183e43 (⌂|🏘|1)
    └── 📙►:2[0]:origin/main →:0:
        └── ·956a3de (⌂)
            └── →:0: (main[🌳] →:2:)
    ");

    let ws = graph.into_workspace()?;
    // The workspace shows the remote commit, there is nothing special about the target.
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓!
    └── ≡👉📙:0:main[🌳] <> origin/main →:2:⇡1 {0}
        └── 👉📙:0:main[🌳] <> origin/main →:2:⇡1
            └── ·3183e43 (🏘️)
    ");

    // If the remote isn't setup officially, deduction still works as we find
    // symbolic remote names for deduction in workspace ref names as well.
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));
    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &meta, Overlay::default())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── 👉📙►:0[1]:main[🌳] <> origin/main →:2:
    │       └── ·3183e43 (⌂|🏘|1)
    └── 📙►:2[0]:origin/main →:0:
        └── ·956a3de (⌂)
            └── →:0: (main[🌳] →:2:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓!
    └── ≡👉📙:0:main[🌳] <> origin/main →:2:⇡1 {0}
        └── 👉📙:0:main[🌳] <> origin/main →:2:⇡1
            └── ·3183e43 (🏘️)
    ");
    Ok(())
}

#[test]
fn workspace_obeys_limit_when_target_branch_is_missing() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-segments-one-integrated-without-remote")?;
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
    add_workspace_without_target(&mut meta);
    assert!(
        meta.data_mut().default_target.is_none(),
        "without target, limits affect workspaces too"
    );
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(0))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        └── ✂·4077353 (⌂|🏘|1)
    ");
    // The commit in the workspace branch is always ignored and is expected to be the workspace merge commit.
    // So nothing to show here.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓!");

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    assert!(
        meta.data_mut().default_target.is_some(),
        "But with workspace and target, we see everything"
    );
    // It's notable that there is no way to bypass the early abort when everything is integrated.
    // and there is no deductible remote relationship between origin/main and main (no remote not configured).
    // Then the traversal ends on integrated branches as `main` isn't a target.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(0))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·4077353 (⌂|🏘|1)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘|1)
    │           └── ·03ad472 (⌂|🏘|1)
    │               └── ►:5[2]:A
    │                   ├── ·79bbb29 (⌂|🏘|✓|1)
    │                   ├── ·fc98174 (⌂|🏘|✓|1)
    │                   └── ✂·a381df5 (⌂|🏘|✓|1)
    └── ►:1[0]:origin/main →:4:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:2[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:4[2]:main <> origin/main →:1:
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:5: (A)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡:3:B on 79bbb29
        └── :3:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    Ok(())
}

#[test]
fn three_branches_one_advanced_ws_commit_advanced_fully_pushed_empty_dependent()
-> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependent",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cbc6713 (origin/advanced-lane, on-top-of-dependent, dependent, advanced-lane) change
    * fafd9d0 (origin/main, main, lane) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·f8f33a7 (⌂|🏘|001)
    │       └── ►:4[1]:advanced-lane <> origin/advanced-lane →:3:
    │           └── ·cbc6713 (⌂|🏘|101) ►dependent, ►on-top-of-dependent
    │               └── ►:2[2]:main <> origin/main →:1:
    │                   └── ·fafd9d0 (⌂|🏘|✓|111) ►lane
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:3[0]:origin/advanced-lane →:4:
        └── →:4: (advanced-lane →:3:)
    ");

    // By default, the advanced lane is simply frozen as its remote contains the commit.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:advanced-lane <> origin/advanced-lane →:3: on fafd9d0
        └── :4:advanced-lane <> origin/advanced-lane →:3:
            └── ❄️cbc6713 (🏘️) ►dependent, ►on-top-of-dependent
    ");

    add_stack_with_segments(
        &mut meta,
        1,
        "dependent",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

    // Lanes are properly ordered
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·f8f33a7 (⌂|🏘|001)
    │       └── 📙►:5[1]:dependent
    │           └── 📙►:6[2]:advanced-lane <> origin/advanced-lane →:4:
    │               └── ·cbc6713 (⌂|🏘|101) ►on-top-of-dependent
    │                   └── ►:2[3]:main <> origin/main →:1:
    │                       └── ·fafd9d0 (⌂|🏘|✓|111) ►lane
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/advanced-lane →:6:
        └── →:6: (advanced-lane →:4:)
    ");

    // When putting the dependent branch on top as empty segment, the frozen state is retained.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:dependent on fafd9d0 {1}
        ├── 📙:5:dependent
        └── 📙:6:advanced-lane <> origin/advanced-lane →:4:
            └── ❄️cbc6713 (🏘️) ►on-top-of-dependent
    ");
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
    // It sees the entire history as it had to find `main`.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        └── ►:1[1]:origin/main →:2:
            ├── ·2cde30a (⌂|🏘|✓|01) ►A, ►B, ►C, ►D, ►E, ►F
            ├── ·1c938f4 (⌂|🏘|✓|01)
            ├── ·b82769f (⌂|🏘|✓|01)
            ├── ·988032f (⌂|🏘|✓|01)
            └── ·cd5b655 (⌂|🏘|✓|01)
                └── ►:2[2]:main <> origin/main →:1:
                    └── ·2be54cd (⌂|🏘|✓|11)
    ");
    // Workspace is empty as everything is integrated.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a");

    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        ├── 📙►:3[1]:C
        │   └── 📙►:4[2]:B
        │       └── 📙►:5[3]:A
        │           └── ►:1[4]:origin/main →:2:
        │               ├── ·2cde30a (⌂|🏘|✓|01)
        │               ├── ·1c938f4 (⌂|🏘|✓|01)
        │               ├── ·b82769f (⌂|🏘|✓|01)
        │               ├── ·988032f (⌂|🏘|✓|01)
        │               └── ·cd5b655 (⌂|🏘|✓|01)
        │                   └── ►:2[5]:main <> origin/main →:1:
        │                       └── ·2be54cd (⌂|🏘|✓|11)
        └── 📙►:6[1]:D
            └── 📙►:7[2]:E
                └── 📙►:8[3]:F
                    └── →:1: (origin/main →:2:)
    ");

    // Empty stack segments on top of integrated portions will show, and nothing integrated shows.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a
    ├── ≡📙:6:D on 2cde30a {1}
    │   ├── 📙:6:D
    │   ├── 📙:7:E
    │   └── 📙:8:F
    └── ≡📙:3:C on 2cde30a {0}
        ├── 📙:3:C
        ├── 📙:4:B
        └── 📙:5:A
    ");

    // However, when passing an additional old position of the target, we can show the now-integrated parts.
    // The stacks will always be created on top of the integrated segments as that's where their references are
    // (these segments are never conjured up out of thin air).
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2be54cd
    ├── ≡📙:6:D on 2be54cd {1}
    │   ├── 📙:6:D
    │   ├── 📙:7:E
    │   └── 📙:8:F
    │       ├── ·2cde30a (🏘️|✓)
    │       ├── ·1c938f4 (🏘️|✓)
    │       ├── ·b82769f (🏘️|✓)
    │       ├── ·988032f (🏘️|✓)
    │       └── ·cd5b655 (🏘️|✓)
    └── ≡📙:3:C on 2be54cd {0}
        ├── 📙:3:C
        ├── 📙:4:B
        └── 📙:5:A
            ├── ·2cde30a (🏘️|✓)
            ├── ·1c938f4 (🏘️|✓)
            ├── ·b82769f (🏘️|✓)
            ├── ·988032f (🏘️|✓)
            └── ·cd5b655 (🏘️|✓)
    ");
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
    let (main_id, main_ref_name) = id_at(&repo, "main");
    // Validate that we will perform long searches to connect connectable segments, without interfering
    // with other searches that may take even longer.
    // Also, without limit, we should be able to see all of 'main' without cut-off
    let graph =
        Graph::from_commit_traversal(main_id, main_ref_name.clone(), &*meta, standard_options())?
            .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·41ed0e4 (⌂|🏘)
    │       └── ►:3[2]:workspace
    │           └── ·9730cbf (⌂|🏘|✓)
    │               ├── ►:6[3]:main-to-workspace
    │               │   └── ·dc7ab57 (⌂|🏘|✓)
    │               │       └── ►:8[5]:anon:
    │               │           ├── ·c056b75 (⌂|🏘|✓|1)
    │               │           ├── ·f49c977 (⌂|🏘|✓|1)
    │               │           ├── ·7b7ebb2 (⌂|🏘|✓|1)
    │               │           ├── ·dca4960 (⌂|🏘|✓|1)
    │               │           ├── ·11c29b8 (⌂|🏘|✓|1)
    │               │           ├── ·c32dd03 (⌂|🏘|✓|1)
    │               │           ├── ·b625665 (⌂|🏘|✓|1)
    │               │           ├── ·a821094 (⌂|🏘|✓|1)
    │               │           ├── ·bce0c5e (⌂|🏘|✓|1)
    │               │           └── ·3183e43 (⌂|🏘|✓|1)
    │               └── ►:7[3]:long-main-to-workspace
    │                   ├── ·77f31a0 (⌂|🏘|✓)
    │                   ├── ·eb17e31 (⌂|🏘|✓)
    │                   ├── ·fe2046b (⌂|🏘|✓)
    │                   └── ·5532ef5 (⌂|🏘|✓)
    │                       └── 👉►:0[4]:main <> origin/main →:2:
    │                           └── ·2438292 (⌂|🏘|✓|1)
    │                               └── →:8:
    └── ►:2[0]:origin/main →:0:
        └── 🟣232ed06 (✓)
            ├── ►:4[1]:workspace-to-target
            │   ├── 🟣abcfd9a (✓)
            │   ├── 🟣bc86eba (✓)
            │   └── 🟣c7ae303 (✓)
            │       └── →:3: (workspace)
            └── ►:5[1]:long-workspace-to-target
                ├── 🟣9e2a79e (✓)
                ├── 🟣fdeaa43 (✓)
                ├── 🟣30565ee (✓)
                ├── 🟣0c1c23a (✓)
                ├── 🟣56d152c (✓)
                ├── 🟣e6e1360 (✓)
                └── 🟣1a22a39 (✓)
                    └── →:3: (workspace)
    ");
    // Entrypoint is outside of workspace.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:⇣11 {1}
        └── :0:main <> origin/main →:2:⇣11
            ├── 🟣232ed06 (✓)
            ├── 🟣9e2a79e (✓)
            ├── 🟣fdeaa43 (✓)
            ├── 🟣30565ee (✓)
            ├── 🟣0c1c23a (✓)
            ├── 🟣56d152c (✓)
            ├── 🟣e6e1360 (✓)
            ├── 🟣1a22a39 (✓)
            ├── 🟣abcfd9a (✓)
            ├── 🟣bc86eba (✓)
            ├── 🟣c7ae303 (✓)
            ├── ·2438292 (🏘️|✓)
            ├── ·c056b75 (🏘️|✓)
            ├── ·f49c977 (🏘️|✓)
            ├── ·7b7ebb2 (🏘️|✓)
            ├── ·dca4960 (🏘️|✓)
            ├── ·11c29b8 (🏘️|✓)
            ├── ·c32dd03 (🏘️|✓)
            ├── ·b625665 (🏘️|✓)
            ├── ·a821094 (🏘️|✓)
            ├── ·bce0c5e (🏘️|✓)
            └── ·3183e43 (🏘️|✓)
    ");

    // When setting a limit when traversing 'main', it is respected.
    // We still want it to be found and connected though, and it's notable that the limit kicks in
    // once everything reconciled.
    let graph = Graph::from_commit_traversal(
        main_id,
        main_ref_name,
        &*meta,
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·41ed0e4 (⌂|🏘)
    │       └── ►:3[2]:workspace
    │           └── ·9730cbf (⌂|🏘|✓)
    │               ├── ►:6[3]:main-to-workspace
    │               │   └── ·dc7ab57 (⌂|🏘|✓)
    │               │       └── ►:8[5]:anon:
    │               │           ├── ·c056b75 (⌂|🏘|✓|1)
    │               │           ├── ·f49c977 (⌂|🏘|✓|1)
    │               │           ├── ·7b7ebb2 (⌂|🏘|✓|1)
    │               │           ├── ·dca4960 (⌂|🏘|✓|1)
    │               │           └── ✂·11c29b8 (⌂|🏘|✓|1)
    │               └── ►:7[3]:long-main-to-workspace
    │                   ├── ·77f31a0 (⌂|🏘|✓)
    │                   ├── ·eb17e31 (⌂|🏘|✓)
    │                   ├── ·fe2046b (⌂|🏘|✓)
    │                   └── ·5532ef5 (⌂|🏘|✓)
    │                       └── 👉►:0[4]:main <> origin/main →:2:
    │                           └── ·2438292 (⌂|🏘|✓|1)
    │                               └── →:8:
    └── ►:2[0]:origin/main →:0:
        └── 🟣232ed06 (✓)
            ├── ►:4[1]:workspace-to-target
            │   ├── 🟣abcfd9a (✓)
            │   ├── 🟣bc86eba (✓)
            │   └── 🟣c7ae303 (✓)
            │       └── →:3: (workspace)
            └── ►:5[1]:long-workspace-to-target
                ├── 🟣9e2a79e (✓)
                ├── 🟣fdeaa43 (✓)
                ├── 🟣30565ee (✓)
                ├── 🟣0c1c23a (✓)
                ├── 🟣56d152c (✓)
                ├── 🟣e6e1360 (✓)
                └── 🟣1a22a39 (✓)
                    └── →:3: (workspace)
    ");
    // The limit is visible as well.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:⇣11 {1}
        └── :0:main <> origin/main →:2:⇣11
            ├── 🟣232ed06 (✓)
            ├── 🟣9e2a79e (✓)
            ├── 🟣fdeaa43 (✓)
            ├── 🟣30565ee (✓)
            ├── 🟣0c1c23a (✓)
            ├── 🟣56d152c (✓)
            ├── 🟣e6e1360 (✓)
            ├── 🟣1a22a39 (✓)
            ├── 🟣abcfd9a (✓)
            ├── 🟣bc86eba (✓)
            ├── 🟣c7ae303 (✓)
            ├── ·2438292 (🏘️|✓)
            ├── ·c056b75 (🏘️|✓)
            ├── ·f49c977 (🏘️|✓)
            ├── ·7b7ebb2 (🏘️|✓)
            ├── ·dca4960 (🏘️|✓)
            └── ✂️·11c29b8 (🏘️|✓)
    ");

    // From the workspace, even without limit, we don't traverse all of 'main' as it's uninteresting.
    // However, we wait for the target to be fully reconciled to get the proper workspace configuration.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·41ed0e4 (⌂|🏘|1)
    │       └── ►:2[2]:workspace
    │           └── ·9730cbf (⌂|🏘|✓|1)
    │               ├── ►:5[3]:main-to-workspace
    │               │   └── ·dc7ab57 (⌂|🏘|✓|1)
    │               │       └── ►:8[5]:anon:
    │               │           ├── ·c056b75 (⌂|🏘|✓|1)
    │               │           ├── ·f49c977 (⌂|🏘|✓|1)
    │               │           ├── ·7b7ebb2 (⌂|🏘|✓|1)
    │               │           ├── ·dca4960 (⌂|🏘|✓|1)
    │               │           ├── ·11c29b8 (⌂|🏘|✓|1)
    │               │           ├── ·c32dd03 (⌂|🏘|✓|1)
    │               │           └── ✂·b625665 (⌂|🏘|✓|1)
    │               └── ►:6[3]:long-main-to-workspace
    │                   ├── ·77f31a0 (⌂|🏘|✓|1)
    │                   ├── ·eb17e31 (⌂|🏘|✓|1)
    │                   ├── ·fe2046b (⌂|🏘|✓|1)
    │                   └── ·5532ef5 (⌂|🏘|✓|1)
    │                       └── ►:7[4]:main <> origin/main →:1:
    │                           └── ·2438292 (⌂|🏘|✓|1)
    │                               └── →:8:
    └── ►:1[0]:origin/main →:7:
        └── 🟣232ed06 (✓)
            ├── ►:3[1]:workspace-to-target
            │   ├── 🟣abcfd9a (✓)
            │   ├── 🟣bc86eba (✓)
            │   └── 🟣c7ae303 (✓)
            │       └── →:2: (workspace)
            └── ►:4[1]:long-workspace-to-target
                ├── 🟣9e2a79e (✓)
                ├── 🟣fdeaa43 (✓)
                ├── 🟣30565ee (✓)
                ├── 🟣0c1c23a (✓)
                ├── 🟣56d152c (✓)
                ├── 🟣e6e1360 (✓)
                └── 🟣1a22a39 (✓)
                    └── →:2: (workspace)
    ");

    // Everything is integrated, nothing to see here.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣11 on 9730cbf");
    Ok(())
}

#[test]
fn remote_far_in_ancestry() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-far-in-ancestry")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9412ebd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8407093 (A) A3
    * 7dfaa0c A2
    * 544e458 A1
    * 685d644 (origin/main, main) M12
    * cafdb27 M11
    * c056b75 M10
    * f49c977 M9
    * 7b7ebb2 M8
    * dca4960 M7
    * 11c29b8 M6
    * c32dd03 M5
    * b625665 M4
    * a821094 M3
    * bce0c5e M2
    | * 975754f (origin/A) R3
    | * f48ff69 R2
    |/  
    * 3183e43 M1
    ");

    add_workspace(&mut meta);
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    // It's critical that the main branch isn't cut off and the local and remote part find each other,
    // or else the remote part will go on forever create a lot of issues for those who want to display
    // all these incorrectly labeled commits.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·9412ebd (⌂|🏘|0001)
    │       └── ►:3[1]:A <> origin/A →:4:
    │           ├── ·8407093 (⌂|🏘|0101)
    │           ├── ·7dfaa0c (⌂|🏘|0101)
    │           └── ·544e458 (⌂|🏘|0101)
    │               └── ►:2[2]:main <> origin/main →:1:
    │                   ├── ·685d644 (⌂|🏘|✓|0111)
    │                   ├── ·cafdb27 (⌂|🏘|✓|0111)
    │                   ├── ·c056b75 (⌂|🏘|✓|0111)
    │                   ├── ·f49c977 (⌂|🏘|✓|0111)
    │                   ├── ·7b7ebb2 (⌂|🏘|✓|0111)
    │                   ├── ·dca4960 (⌂|🏘|✓|0111)
    │                   ├── ·11c29b8 (⌂|🏘|✓|0111)
    │                   ├── ·c32dd03 (⌂|🏘|✓|0111)
    │                   ├── ·b625665 (⌂|🏘|✓|0111)
    │                   ├── ·a821094 (⌂|🏘|✓|0111)
    │                   └── ·bce0c5e (⌂|🏘|✓|0111)
    │                       └── ►:5[3]:anon:
    │                           └── ·3183e43 (⌂|🏘|✓|1111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/A →:3:
        ├── 🟣975754f (0x0|1000)
        └── 🟣f48ff69 (0x0|1000)
            └── →:5:
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 685d644
    └── ≡:3:A <> origin/A →:4:⇡3⇣2 on 685d644
        └── :3:A <> origin/A →:4:⇡3⇣2
            ├── 🟣975754f
            ├── 🟣f48ff69
            ├── ·8407093 (🏘️)
            ├── ·7dfaa0c (🏘️)
            └── ·544e458 (🏘️)
    ");
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
    * 3183e43 (B, A) M1
    ");

    add_workspace(&mut meta);
    let (id, ref_name) = id_at(&repo, "main");
    // Here the target shouldn't be cut off from finding its workspace
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·f514495 (⌂|🏘)
    │       └── ►:3[3]:workspace
    │           └── ·c9120f1 (⌂|🏘|✓)
    │               ├── ►:4[4]:main-to-workspace
    │               │   └── ·1126587 (⌂|🏘|✓)
    │               │       └── ►:6[6]:anon:
    │               │           └── ·3183e43 (⌂|🏘|✓|1) ►A, ►B
    │               └── ►:5[4]:long-main-to-workspace
    │                   ├── ·b39c7ec (⌂|🏘|✓)
    │                   ├── ·2983a97 (⌂|🏘|✓)
    │                   ├── ·144ea85 (⌂|🏘|✓)
    │                   └── ·5aecfd2 (⌂|🏘|✓)
    │                       └── 👉►:0[5]:main <> origin/main →:2:
    │                           └── ·bce0c5e (⌂|🏘|✓|1)
    │                               └── →:6:
    └── ►:2[0]:origin/main →:0:
        ├── 🟣024f837 (✓) ►long-workspace-to-target
        ├── 🟣64a8284 (✓)
        ├── 🟣b72938c (✓)
        ├── 🟣9ccbf6f (✓)
        ├── 🟣5fa4905 (✓)
        ├── 🟣43074d3 (✓)
        ├── 🟣800d4a9 (✓)
        ├── 🟣742c068 (✓)
        └── 🟣fe06afd (✓)
            └── ►:7[1]:anon:
                └── 🟣3027746 (✓)
                    ├── ►:8[2]:anon:
                    │   └── 🟣f0d2a35 (✓)
                    │       └── →:3: (workspace)
                    └── ►:9[2]:longer-workspace-to-target
                        ├── 🟣edf041f (✓)
                        ├── 🟣d9f03f6 (✓)
                        ├── 🟣8d1d264 (✓)
                        ├── 🟣fa7ceae (✓)
                        ├── 🟣95bdbf1 (✓)
                        └── 🟣5bac978 (✓)
                            └── →:4: (main-to-workspace)
    ");
    // `main` is integrated, but the entrypoint so it's shown.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:⇣17 {1}
        └── :0:main <> origin/main →:2:⇣17
            ├── 🟣024f837 (✓) ►long-workspace-to-target
            ├── 🟣64a8284 (✓)
            ├── 🟣b72938c (✓)
            ├── 🟣9ccbf6f (✓)
            ├── 🟣5fa4905 (✓)
            ├── 🟣43074d3 (✓)
            ├── 🟣800d4a9 (✓)
            ├── 🟣742c068 (✓)
            ├── 🟣fe06afd (✓)
            ├── 🟣3027746 (✓)
            ├── 🟣edf041f (✓)
            ├── 🟣d9f03f6 (✓)
            ├── 🟣8d1d264 (✓)
            ├── 🟣fa7ceae (✓)
            ├── 🟣95bdbf1 (✓)
            ├── 🟣5bac978 (✓)
            ├── 🟣f0d2a35 (✓)
            ├── ·bce0c5e (🏘️|✓)
            └── ·3183e43 (🏘️|✓) ►A, ►B
    ");

    // Now the target looks for the entrypoint, which is the workspace, something it can do more easily.
    // We wait for targets to fully reconcile as well.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·f514495 (⌂|🏘|1)
    │       └── ►:2[3]:workspace
    │           └── ·c9120f1 (⌂|🏘|✓|1)
    │               ├── ►:3[4]:main-to-workspace
    │               │   └── ·1126587 (⌂|🏘|✓|1)
    │               │       └── ►:6[6]:anon:
    │               │           └── ·3183e43 (⌂|🏘|✓|1) ►A, ►B
    │               └── ►:4[4]:long-main-to-workspace
    │                   ├── ·b39c7ec (⌂|🏘|✓|1)
    │                   ├── ·2983a97 (⌂|🏘|✓|1)
    │                   ├── ·144ea85 (⌂|🏘|✓|1)
    │                   └── ·5aecfd2 (⌂|🏘|✓|1)
    │                       └── ►:5[5]:main <> origin/main →:1:
    │                           └── ·bce0c5e (⌂|🏘|✓|1)
    │                               └── →:6:
    └── ►:1[0]:origin/main →:5:
        ├── 🟣024f837 (✓) ►long-workspace-to-target
        ├── 🟣64a8284 (✓)
        ├── 🟣b72938c (✓)
        ├── 🟣9ccbf6f (✓)
        ├── 🟣5fa4905 (✓)
        ├── 🟣43074d3 (✓)
        ├── 🟣800d4a9 (✓)
        ├── 🟣742c068 (✓)
        └── 🟣fe06afd (✓)
            └── ►:7[1]:anon:
                └── 🟣3027746 (✓)
                    ├── ►:8[2]:anon:
                    │   └── 🟣f0d2a35 (✓)
                    │       └── →:2: (workspace)
                    └── ►:9[2]:longer-workspace-to-target
                        ├── 🟣edf041f (✓)
                        ├── 🟣d9f03f6 (✓)
                        ├── 🟣8d1d264 (✓)
                        ├── 🟣fa7ceae (✓)
                        ├── 🟣95bdbf1 (✓)
                        └── 🟣5bac978 (✓)
                            └── →:3: (main-to-workspace)
    ");

    let ws = graph.into_workspace()?;
    // Everything is integrated.
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1");

    // With a lower base for the target, we see more.
    let target_commit_id = repo.rev_parse_single("3183e43")?.detach();
    add_workspace_with_target(&mut meta, target_commit_id);

    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on 3183e43
    └── ≡:3:workspace on 3183e43
        ├── :3:workspace
        │   └── ·c9120f1 (🏘️|✓)
        └── :4:main-to-workspace
            └── ·1126587 (🏘️|✓)
    ");

    // We can also add independent virtual branches to that new base.
    add_stack(&mut meta, 3, "A", StackState::InWorkspace);
    add_stack(&mut meta, 4, "B", StackState::InWorkspace);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on 3183e43
    ├── ≡📙:11:B on 3183e43 {4}
    │   └── 📙:11:B
    ├── ≡📙:10:A on 3183e43 {3}
    │   └── 📙:10:A
    └── ≡:3:workspace on 3183e43
        ├── :3:workspace
        │   └── ·c9120f1 (🏘️|✓)
        └── :4:main-to-workspace
            └── ·1126587 (🏘️|✓)
    ");

    // We can also add stacked virtual branches to that new base.
    meta.data_mut().branches.clear();
    add_workspace_with_target(&mut meta, target_commit_id);
    add_stack_with_segments(&mut meta, 3, "A", StackState::InWorkspace, &["B"]);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on 3183e43
    ├── ≡📙:10:A on 3183e43 {3}
    │   ├── 📙:10:A
    │   └── 📙:11:B
    └── ≡:3:workspace on 3183e43
        ├── :3:workspace
        │   └── ·c9120f1 (🏘️|✓)
        └── :4:main-to-workspace
            └── ·1126587 (🏘️|✓)
    ");
    Ok(())
}

#[test]
fn multi_lane_with_shared_segment_one_integrated() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multi-lane-with-shared-segment-one-integrated")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   2b30d94 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * acdc49a (B) B2
    | | * f0117e0 B1
    * | | 9895054 (D) D1
    * | | de625cc (C) C3
    * | | 23419f8 C2
    * | | 5dc4389 C1
    | |/  
    |/|   
    | | *   c08dc6b (origin/main) Merge branch 'A' into soon-remote-main
    | | |\  
    | | |/  
    | |/|   
    | * | 0bad3af (A) A1
    |/ /  
    * | d4f537e (shared) S3
    * | b448757 S2
    * | e9a378d S1
    |/  
    * 3183e43 (main) M1
    ");

    add_workspace(&mut meta);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·2b30d94 (⌂|🏘|01)
    │       ├── ►:3[1]:D
    │       │   └── ·9895054 (⌂|🏘|01)
    │       │       └── ►:6[2]:C
    │       │           ├── ·de625cc (⌂|🏘|01)
    │       │           ├── ·23419f8 (⌂|🏘|01)
    │       │           └── ·5dc4389 (⌂|🏘|01)
    │       │               └── ►:7[3]:shared
    │       │                   ├── ·d4f537e (⌂|🏘|✓|01)
    │       │                   ├── ·b448757 (⌂|🏘|✓|01)
    │       │                   └── ·e9a378d (⌂|🏘|✓|01)
    │       │                       └── ►:2[4]:main <> origin/main →:1:
    │       │                           └── ·3183e43 (⌂|🏘|✓|11)
    │       ├── ►:4[1]:A
    │       │   └── ·0bad3af (⌂|🏘|✓|01)
    │       │       └── →:7: (shared)
    │       └── ►:5[1]:B
    │           ├── ·acdc49a (⌂|🏘|01)
    │           └── ·f0117e0 (⌂|🏘|01)
    │               └── →:7: (shared)
    └── ►:1[0]:origin/main →:2:
        └── 🟣c08dc6b (✓)
            ├── →:2: (main →:1:)
            └── →:4: (A)
    ");

    // A is still shown despite it being fully integrated, as it's still enclosed by the
    // workspace tip and the fork-point, at least when we provide the previous known location of the target.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    ├── ≡:5:B on 3183e43
    │   ├── :5:B
    │   │   ├── ·acdc49a (🏘️)
    │   │   └── ·f0117e0 (🏘️)
    │   └── :7:shared
    │       ├── ·d4f537e (🏘️|✓)
    │       ├── ·b448757 (🏘️|✓)
    │       └── ·e9a378d (🏘️|✓)
    ├── ≡:4:A on 3183e43
    │   ├── :4:A
    │   │   └── ·0bad3af (🏘️|✓)
    │   └── :7:shared
    │       ├── ·d4f537e (🏘️|✓)
    │       ├── ·b448757 (🏘️|✓)
    │       └── ·e9a378d (🏘️|✓)
    └── ≡:3:D on 3183e43
        ├── :3:D
        │   └── ·9895054 (🏘️)
        ├── :6:C
        │   ├── ·de625cc (🏘️)
        │   ├── ·23419f8 (🏘️)
        │   └── ·5dc4389 (🏘️)
        └── :7:shared
            ├── ·d4f537e (🏘️|✓)
            ├── ·b448757 (🏘️|✓)
            └── ·e9a378d (🏘️|✓)
    ");

    // If we do not, integrated portions are removed.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on d4f537e
    ├── ≡:5:B on d4f537e
    │   └── :5:B
    │       ├── ·acdc49a (🏘️)
    │       └── ·f0117e0 (🏘️)
    ├── ≡:4:A on d4f537e
    │   └── :4:A
    │       └── ·0bad3af (🏘️|✓)
    └── ≡:3:D on d4f537e
        ├── :3:D
        │   └── ·9895054 (🏘️)
        └── :6:C
            ├── ·de625cc (🏘️)
            ├── ·23419f8 (🏘️)
            └── ·5dc4389 (🏘️)
    ");
    Ok(())
}

#[test]
fn multi_lane_with_shared_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/multi-lane-with-shared-segment")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   2b30d94 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * acdc49a (B) B2
    | | * f0117e0 B1
    | * | 0bad3af (A) A1
    | |/  
    * | 9895054 (D) D1
    * | de625cc (C) C3
    * | 23419f8 C2
    * | 5dc4389 C1
    |/  
    * d4f537e (shared) S3
    * b448757 S2
    * e9a378d S1
    | * bce0c5e (origin/main) M2
    |/  
    * 3183e43 (main) M1
    ");

    add_workspace(&mut meta);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·2b30d94 (⌂|🏘|1)
    │       ├── ►:2[1]:D
    │       │   └── ·9895054 (⌂|🏘|1)
    │       │       └── ►:6[2]:C
    │       │           ├── ·de625cc (⌂|🏘|1)
    │       │           ├── ·23419f8 (⌂|🏘|1)
    │       │           └── ·5dc4389 (⌂|🏘|1)
    │       │               └── ►:7[3]:shared
    │       │                   ├── ·d4f537e (⌂|🏘|1)
    │       │                   ├── ·b448757 (⌂|🏘|1)
    │       │                   └── ·e9a378d (⌂|🏘|1)
    │       │                       └── ►:5[4]:main <> origin/main →:1:
    │       │                           └── ·3183e43 (⌂|🏘|✓|1)
    │       ├── ►:3[1]:A
    │       │   └── ·0bad3af (⌂|🏘|1)
    │       │       └── →:7: (shared)
    │       └── ►:4[1]:B
    │           ├── ·acdc49a (⌂|🏘|1)
    │           └── ·f0117e0 (⌂|🏘|1)
    │               └── →:7: (shared)
    └── ►:1[0]:origin/main →:5:
        └── 🟣bce0c5e (✓)
            └── →:5: (main →:1:)
    ");

    // Segments can definitely repeat
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    ├── ≡:4:B on 3183e43
    │   ├── :4:B
    │   │   ├── ·acdc49a (🏘️)
    │   │   └── ·f0117e0 (🏘️)
    │   └── :7:shared
    │       ├── ·d4f537e (🏘️)
    │       ├── ·b448757 (🏘️)
    │       └── ·e9a378d (🏘️)
    ├── ≡:3:A on 3183e43
    │   ├── :3:A
    │   │   └── ·0bad3af (🏘️)
    │   └── :7:shared
    │       ├── ·d4f537e (🏘️)
    │       ├── ·b448757 (🏘️)
    │       └── ·e9a378d (🏘️)
    └── ≡:2:D on 3183e43
        ├── :2:D
        │   └── ·9895054 (🏘️)
        ├── :6:C
        │   ├── ·de625cc (🏘️)
        │   ├── ·23419f8 (🏘️)
        │   └── ·5dc4389 (🏘️)
        └── :7:shared
            ├── ·d4f537e (🏘️)
            ├── ·b448757 (🏘️)
            └── ·e9a378d (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, Some(ref_name), &*meta, standard_options())?
        .validated()?;
    // Checking out anything inside the workspace yields the same result.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    ├── ≡:5:B on 3183e43
    │   ├── :5:B
    │   │   ├── ·acdc49a (🏘️)
    │   │   └── ·f0117e0 (🏘️)
    │   └── :3:shared
    │       ├── ·d4f537e (🏘️)
    │       ├── ·b448757 (🏘️)
    │       └── ·e9a378d (🏘️)
    ├── ≡👉:0:A on 3183e43
    │   ├── 👉:0:A
    │   │   └── ·0bad3af (🏘️)
    │   └── :3:shared
    │       ├── ·d4f537e (🏘️)
    │       ├── ·b448757 (🏘️)
    │       └── ·e9a378d (🏘️)
    └── ≡:4:D on 3183e43
        ├── :4:D
        │   └── ·9895054 (🏘️)
        ├── :7:C
        │   ├── ·de625cc (🏘️)
        │   ├── ·23419f8 (🏘️)
        │   └── ·5dc4389 (🏘️)
        └── :3:shared
            ├── ·d4f537e (🏘️)
            ├── ·b448757 (🏘️)
            └── ·e9a378d (🏘️)
    ");
    Ok(())
}

#[test]
fn dependent_branch_insertion() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependent",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   335d6f2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * cbc6713 (origin/advanced-lane, dependent, advanced-lane) change
    |/  
    * fafd9d0 (origin/main, main, lane) init
    ");

    add_stack_with_segments(
        &mut meta,
        1,
        "dependent",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·335d6f2 (⌂|🏘|001)
    │       ├── ►:2[3]:main <> origin/main →:1:
    │       │   └── ·fafd9d0 (⌂|🏘|✓|111) ►lane
    │       └── 📙►:5[1]:dependent
    │           └── 📙►:6[2]:advanced-lane <> origin/advanced-lane →:4:
    │               └── ·cbc6713 (⌂|🏘|101)
    │                   └── →:2: (main →:1:)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/advanced-lane →:6:
        └── →:6: (advanced-lane →:4:)
    ");

    // The dependent branch is empty and on top of the one with the remote
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:dependent on fafd9d0 {1}
        ├── 📙:5:dependent
        └── 📙:6:advanced-lane <> origin/advanced-lane →:4:
            └── ❄️cbc6713 (🏘️)
    ");

    // Create the dependent branch below.
    add_stack_with_segments(
        &mut meta,
        1,
        "advanced-lane",
        StackState::InWorkspace,
        &["dependent"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·335d6f2 (⌂|🏘|001)
    │       ├── ►:2[3]:main <> origin/main →:1:
    │       │   └── ·fafd9d0 (⌂|🏘|✓|111) ►lane
    │       └── 📙►:5[1]:advanced-lane <> origin/advanced-lane →:4:
    │           └── 📙►:6[2]:dependent
    │               └── ·cbc6713 (⌂|🏘|101)
    │                   └── →:2: (main →:1:)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/advanced-lane →:5:
        └── →:5: (advanced-lane →:4:)
    ");

    // Having done something unusual, which is to put the dependent branch
    // underneath the other already pushed, it creates a different view of ownership.
    // It's probably OK to leave it like this for now, and instead allow users to reorder
    // these more easily.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:advanced-lane <> origin/advanced-lane →:4: on fafd9d0 {1}
        ├── 📙:5:advanced-lane <> origin/advanced-lane →:4:
        └── 📙:6:dependent
            └── ❄cbc6713 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "advanced-lane");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡👉📙:5:advanced-lane <> origin/advanced-lane →:4: on fafd9d0 {1}
        ├── 👉📙:5:advanced-lane <> origin/advanced-lane →:4:
        └── 📙:6:dependent
            └── ❄cbc6713 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "dependent");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:advanced-lane <> origin/advanced-lane →:4: on fafd9d0 {1}
        ├── 📙:5:advanced-lane <> origin/advanced-lane →:4:
        └── 👉📙:6:dependent
            └── ❄cbc6713 (🏘️)
    ");
    Ok(())
}

#[test]
fn multiple_stacks_with_shared_parent_and_remote() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multiple-stacks-with-shared-segment-and-remote")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   e982e8a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * aff8449 (B-on-A) B-on-A
    * | 4f1bb32 (C-on-A) C-on-A
    |/  
    | * b627ca7 (origin/A) A-on-remote
    |/  
    * e255adc (A) A
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(&mut meta, 1, "C-on-A", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·e982e8a (⌂|🏘|0001)
    │       ├── 📙►:3[1]:C-on-A
    │       │   └── ·4f1bb32 (⌂|🏘|0001)
    │       │       └── ►:4[2]:A <> origin/A →:5:
    │       │           └── ·e255adc (⌂|🏘|1101)
    │       │               └── ►:2[3]:main <> origin/main →:1:
    │       │                   └── ·fafd9d0 (⌂|🏘|✓|1111)
    │       └── ►:6[1]:B-on-A
    │           └── ·aff8449 (⌂|🏘|0001)
    │               └── →:4: (A →:5:)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:5[0]:origin/A →:4:
        └── 🟣b627ca7 (0x0|1000)
            └── →:4: (A →:5:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡:6:B-on-A on fafd9d0
    │   ├── :6:B-on-A
    │   │   └── ·aff8449 (🏘️)
    │   └── :4:A <> origin/A →:5:⇣1
    │       ├── 🟣b627ca7
    │       └── ❄️e255adc (🏘️)
    └── ≡📙:3:C-on-A on fafd9d0 {1}
        ├── 📙:3:C-on-A
        │   └── ·4f1bb32 (🏘️)
        └── :4:A <> origin/A →:5:⇣1
            ├── 🟣b627ca7
            └── ❄️e255adc (🏘️)
    ");
    Ok(())
}

#[test]
fn a_stack_segment_can_be_a_segment_elsewhere_and_stack_order() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-branches-one-advanced-two-parent-ws-commit-diverged-ttb",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   873d056 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | cbc6713 (advanced-lane) change
    |/  
    * fafd9d0 (main, lane) init
    * da83717 (origin/main) disjoint remote target
    ");

    let lanes = ["advanced-lane", "lane"];
    for (idx, name) in lanes.into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·873d056 (⌂|🏘|1)
    │       ├── 📙►:2[1]:advanced-lane
    │       │   └── ·cbc6713 (⌂|🏘|1)
    │       │       └── ►:3[2]:anon:
    │       │           └── ·fafd9d0 (⌂|🏘|1) ►main
    │       └── 📙►:4[1]:lane
    │           └── →:3:
    └── ►:1[0]:origin/main
        └── 🟣da83717 (✓)
    ");

    // Since `lane` is connected directly, no segment has to be created.
    // However, as nothing is integrated, it really is another name for `main` now,
    // `main` is nothing special.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:4:lane on fafd9d0 {1}
    │   └── 📙:4:lane
    └── ≡📙:2:advanced-lane on fafd9d0 {0}
        └── 📙:2:advanced-lane
            └── ·cbc6713 (🏘️)
    ");

    // Reverse the order of stacks in the worktree data.
    for (idx, name) in lanes.into_iter().rev().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·873d056 (⌂|🏘|1)
    │       ├── 📙►:4[1]:lane
    │       │   └── ►:2[2]:anon:
    │       │       └── ·fafd9d0 (⌂|🏘|1) ►main
    │       └── 📙►:3[1]:advanced-lane
    │           └── ·cbc6713 (⌂|🏘|1)
    │               └── →:2:
    └── ►:1[0]:origin/main
        └── 🟣da83717 (✓)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:advanced-lane on fafd9d0 {1}
    │   └── 📙:3:advanced-lane
    │       └── ·cbc6713 (🏘️)
    └── ≡📙:4:lane on fafd9d0 {0}
        └── 📙:4:lane
    ");
    Ok(())
}

#[test]
fn two_dependent_branches_with_embedded_remote() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-dependent-branches-with-interesting-remote-setup")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a221221 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * aadad9d (A) shared by name
    * 96a2408 (origin/main) another unrelated
    | * 2b1808c (origin/A) shared by name
    |/  
    * f15ca75 (integrated) other integrated
    * 9456d79 integrated in target
    * fafd9d0 (main) init
    ");

    // Just a single explicit reference we want to know of.
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    // Note how the target remote tracking branch is integrated into the stack
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·a221221 (⌂|🏘|0001)
    │       └── 📙►:3[1]:A <> origin/A →:4:
    │           └── ·aadad9d (⌂|🏘|0101)
    │               └── ►:1[2]:origin/main →:2:
    │                   └── ·96a2408 (⌂|🏘|✓|0101)
    │                       └── ►:5[3]:integrated
    │                           ├── ·f15ca75 (⌂|🏘|✓|1101)
    │                           └── ·9456d79 (⌂|🏘|✓|1101)
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘|✓|1111)
    └── ►:4[0]:origin/A →:3:
        └── 🟣2b1808c (0x0|1000)
            └── →:5: (integrated)
    ");

    // Remote tracking branches we just want to aggregate, just like anonymous segments,
    // but only when another target is provided (the old position, `main`).
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A <> origin/A →:4:⇡1⇣1 on fafd9d0 {1}
        ├── 📙:3:A <> origin/A →:4:⇡1⇣1
        │   ├── 🟣2b1808c
        │   ├── ·aadad9d (🏘️)
        │   └── ·96a2408 (🏘️|✓)
        └── :5:integrated
            ├── ❄f15ca75 (🏘️|✓)
            └── ❄9456d79 (🏘️|✓)
    ");

    // Otherwise, nothing that's integrated is shown. Note how 96a2408 seems missing,
    // but it's skipped because it's actually part of an integrated otherwise ignored segment.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 96a2408
    └── ≡📙:3:A <> origin/A →:4:⇡1⇣1 on 96a2408 {1}
        └── 📙:3:A <> origin/A →:4:⇡1⇣1
            ├── 🟣2b1808c
            └── ·aadad9d (🏘️)
    ");
    Ok(())
}

#[test]
fn two_dependent_branches_rebased_with_remotes_merge_local() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-dependent-branches-rebased-with-remotes-merge-one-local",
    )?;
    // Each of the stacked branches has a remote, and the local branch was merged into main.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e0bd0a7 (origin/B) B
    * 0b6b861 (origin/A) A
    | * b694668 (origin/main) Merge branch 'A' into soon-origin-main
    |/| 
    | | * 4f08b8d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | | * da597e8 (B) B
    | |/  
    | * 1818c17 (A) A
    |/  
    * 281456a (main) init
    ");

    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["A"]);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·4f08b8d (⌂|🏘|000001)
    │       └── 📙►:3[1]:B <> origin/B →:5:
    │           └── ·da597e8 (⌂|🏘|000101)
    │               └── 📙►:4[2]:A <> origin/A →:6:
    │                   └── ·1818c17 (⌂|🏘|✓|010101)
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·281456a (⌂|🏘|✓|111111)
    ├── ►:1[0]:origin/main →:2:
    │   └── 🟣b694668 (✓)
    │       ├── →:2: (main →:1:)
    │       └── →:4: (A →:6:)
    └── ►:5[0]:origin/B →:3:
        └── 🟣e0bd0a7 (0x0|001000)
            └── ►:6[1]:origin/A →:4:
                └── 🟣0b6b861 (0x0|101000)
                    └── →:2: (main →:1:)
    ");

    // This is the default as it includes both the integrated and non-integrated segment.
    // Note how there is no expensive computation to see if remote commits are the same,
    // it's all ID-based.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 281456a
    └── ≡📙:3:B <> origin/B →:5:⇡1⇣1 on 281456a {0}
        ├── 📙:3:B <> origin/B →:5:⇡1⇣1
        │   ├── 🟣e0bd0a7
        │   └── ·da597e8 (🏘️)
        └── 📙:4:A <> origin/A →:6:⇣1
            ├── 🟣0b6b861
            └── ·1818c17 (🏘️|✓)
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "A"),
    )?
    .validated()?;
    // Pretending we are rebased onto A still shows the same remote commits.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1818c17
    └── ≡📙:4:B <> origin/B →:6:⇡1⇣1 on 1818c17 {0}
        └── 📙:4:B <> origin/B →:6:⇡1⇣1
            ├── 🟣e0bd0a7
            └── ·da597e8 (🏘️)
    ");
    Ok(())
}

#[test]
fn two_dependent_branches_rebased_with_remotes_squash_merge_remote_ambiguous() -> anyhow::Result<()>
{
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-dependent-branches-rebased-with-remotes-squash-merge-one-remote-ambiguous",
    )?;
    // Each of the stacked branches has a remote, the remote branch was merged into main,
    // and the remaining branch B was rebased onto the merge, simulating a workspace update.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 1109eb2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 624e118 (D) D
    * 0b6b861 (origin/main, main) A
    | * 3045ea6 (origin/D) D
    | * 1818c17 (origin/C, origin/B, origin/A) A
    |/  
    * 281456a init
    ");

    // The branch A, B, C are not in the workspace anymore, and we *could* signal it by removing metadata.
    // But even with metadata, it still works fine.
    add_stack_with_segments(&mut meta, 0, "D", StackState::InWorkspace, &["C", "B", "A"]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·1109eb2 (⌂|🏘|0001)
    │       └── 📙►:3[1]:D <> origin/D →:4:
    │           └── ·624e118 (⌂|🏘|0101)
    │               └── ►:2[2]:main <> origin/main →:1:
    │                   └── ·0b6b861 (⌂|🏘|✓|0111)
    │                       └── ►:5[3]:anon:
    │                           └── ·281456a (⌂|🏘|✓|1111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/D →:3:
        └── 🟣3045ea6 (0x0|1000)
            └── ►:6[1]:origin/A
                └── 🟣1818c17 (0x0|1000)
                    └── →:5:
    ");

    // We want to let each remote on the path down own a commit, even if ownership would be ambiguous
    // as we are in this situation because these ambiguous remotes don't actually matter as their
    // local tracking branches aren't present anymore.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
    └── ≡📙:3:D <> origin/D →:4:⇡1⇣1 on 0b6b861 {0}
        └── 📙:3:D <> origin/D →:4:⇡1⇣1
            ├── 🟣3045ea6
            └── ·624e118 (🏘️)
    ");
    Ok(())
}

#[test]
fn two_dependent_branches_rebased_with_remotes_squash_merge_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-dependent-branches-rebased-with-remotes-squash-merge-one-remote",
    )?;
    // Each of the stacked branches has a remote, the remote branch was merged into main,
    // and the remaining branch B was rebased onto the merge, simulating a workspace update.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * deeae50 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 353471f (D) D
    * 8a4b945 C
    * e0bd0a7 B
    * 0b6b861 (origin/main, main) A
    | * bbd4ff6 (origin/D) D
    | * e5f5a87 (origin/C) C
    | * da597e8 (origin/B) B
    | * 1818c17 (origin/A) A
    |/  
    * 281456a init
    ");

    // The branch A, B, C are not in the workspace anymore, and we *could* signal it by removing metadata.
    // But even with metadata, it still works fine.
    add_stack_with_segments(&mut meta, 0, "D", StackState::InWorkspace, &["C", "B", "A"]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·deeae50 (⌂|🏘|0001)
    │       └── 📙►:3[1]:D <> origin/D →:4:
    │           ├── ·353471f (⌂|🏘|0101)
    │           ├── ·8a4b945 (⌂|🏘|0101)
    │           └── ·e0bd0a7 (⌂|🏘|0101)
    │               └── ►:2[2]:main <> origin/main →:1:
    │                   └── ·0b6b861 (⌂|🏘|✓|0111)
    │                       └── ►:5[4]:anon:
    │                           └── ·281456a (⌂|🏘|✓|1111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/D →:3:
        └── 🟣bbd4ff6 (0x0|1000)
            └── ►:8[1]:origin/C
                └── 🟣e5f5a87 (0x0|1000)
                    └── ►:7[2]:origin/B
                        └── 🟣da597e8 (0x0|1000)
                            └── ►:6[3]:origin/A
                                └── 🟣1818c17 (0x0|1000)
                                    └── →:5:
    ");

    // We let each remote on the path down own a commit so we only see one remote commit here,
    // the one belonging to the last remaining associated remote tracking branch of D.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
    └── ≡📙:3:D <> origin/D →:4:⇡3⇣1 on 0b6b861 {0}
        └── 📙:3:D <> origin/D →:4:⇡3⇣1
            ├── 🟣bbd4ff6
            ├── ·353471f (🏘️)
            ├── ·8a4b945 (🏘️)
            └── ·e0bd0a7 (🏘️)
    ");
    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4fe5a6f (origin/A) A-remote
    * a62b0de (HEAD -> gitbutler/workspace, A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ►:1[1]:A <> origin/A →:2:
    │       ├── ·a62b0de (⌂|🏘|11)
    │       └── ·120a217 (⌂|🏘|11)
    │           └── ►:3[2]:main
    │               └── ·fafd9d0 (⌂|🏘|11)
    └── ►:2[0]:origin/A →:1:
        └── 🟣4fe5a6f (0x0|10)
            └── →:1: (A →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> origin/A →:2:⇣1
        ├── :1:A <> origin/A →:2:⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── 👉►:0[1]:A <> origin/A →:2:
    │       ├── ·a62b0de (⌂|🏘|11)
    │       └── ·120a217 (⌂|🏘|11)
    │           └── ►:3[2]:main
    │               └── ·fafd9d0 (⌂|🏘|11)
    └── ►:2[0]:origin/A →:0:
        └── 🟣4fe5a6f (0x0|10)
            └── →:0: (A →:2:)
    ");

    // Main can be a normal segment if there is no target ref.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:A <> origin/A →:2:⇣1
        ├── 👉:0:A <> origin/A →:2:⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");
    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit_ambiguous() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-target-without-ws-commit-ambiguous")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4fe5a6f (origin/A) A-remote
    * a62b0de (HEAD -> gitbutler/workspace, B, A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    // Without disambiguation, there is no segment name.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ►:1[1]:A <> origin/A →:2:
    │       ├── ·a62b0de (⌂|🏘|11) ►B
    │       └── ·120a217 (⌂|🏘|11)
    │           └── ►:3[2]:main
    │               └── ·fafd9d0 (⌂|🏘|11)
    └── ►:2[0]:origin/A →:1:
        └── 🟣4fe5a6f (0x0|10)
            └── →:1: (A →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> origin/A →:2:⇣1
        ├── :1:A <> origin/A →:2:⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️) ►B
        │   └── ❄️120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    // We can help it by adding metadata.
    // Note how the selection still manages to hold on to the `A` which now gets its very own
    // empty segment.
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let (id, a_ref) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, a_ref.clone(), &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── 👉►:4[1]:A <> origin/A →:2:
    │       └── 📙►:0[2]:B
    │           ├── ·a62b0de (⌂|🏘|11)
    │           └── ·120a217 (⌂|🏘|11)
    │               └── ►:3[3]:main
    │                   └── ·fafd9d0 (⌂|🏘|11)
    └── ►:2[0]:origin/A →:4:
        └── 🟣4fe5a6f (0x0|10)
            └── →:4: (A →:2:)
    ");

    // Main can be a normal segment if there is no target ref.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:4:A <> origin/A →:2:⇣1 {1}
        ├── 👉:4:A <> origin/A →:2:⇣1
        │   └── 🟣4fe5a6f
        ├── 📙:0:B
        │   ├── ❄a62b0de (🏘️)
        │   └── ❄120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    // Finally, show the normal version with just disambiguated 'B".
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── 📙►:1[1]:B
    │       ├── ·a62b0de (⌂|🏘|11) ►A
    │       └── ·120a217 (⌂|🏘|11)
    │           └── ►:3[2]:main
    │               └── ·fafd9d0 (⌂|🏘|11)
    └── ►:2[0]:origin/A
        └── 🟣4fe5a6f (0x0|10)
            └── →:1: (B)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:1:B {1}
        ├── 📙:1:B
        │   ├── ·a62b0de (🏘️) ►A
        │   └── ·120a217 (🏘️)
        └── :3:main
            └── ·fafd9d0 (🏘️)
    ");

    // Order is respected
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);
    let graph =
        Graph::from_commit_traversal(id, a_ref.clone(), &*meta, standard_options())?.validated()?;
    // The remote tracking branch must remain linked.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:4:B {1}
        ├── 📙:4:B
        ├── 👉📙:5:A <> origin/A →:2:⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    // Order is respected, vice-versa
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let graph = Graph::from_commit_traversal(id, a_ref, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉📙:4:A <> origin/A →:2:⇣1 {1}
        ├── 👉📙:4:A <> origin/A →:2:⇣1
        │   └── 🟣4fe5a6f
        ├── 📙:5:B
        │   ├── ❄a62b0de (🏘️)
        │   └── ❄120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit_ambiguous_with_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-target-without-ws-commit-ambiguous-with-remotes")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a62b0de (HEAD -> gitbutler/workspace, origin/B, origin/A, B, A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    // Without disambiguation, there is no segment name.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ►:1[1]:anon:
    │       ├── ·a62b0de (⌂|🏘|1) ►A, ►B
    │       └── ·120a217 (⌂|🏘|1)
    │           └── ►:4[2]:main <> origin/main
    │               └── ·fafd9d0 (⌂|🏘|1)
    ├── ►:2[0]:origin/A
    │   └── →:1:
    └── ►:3[0]:origin/B
        └── →:1:
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:anon:
        ├── :1:anon:
        │   ├── ·a62b0de (🏘️) ►A, ►B
        │   └── ·120a217 (🏘️)
        └── :4:main <> origin/main⇡1
            └── ·fafd9d0 (🏘️)
    ");

    // Remote handling is still happening when A is disambiguated by entrypoint.
    let (id, a_ref) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, a_ref.clone(), &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── 👉►:0[1]:A <> origin/A →:2:
    │       ├── ·a62b0de (⌂|🏘|1) ►B
    │       └── ·120a217 (⌂|🏘|1)
    │           └── ►:4[2]:main <> origin/main
    │               └── ·fafd9d0 (⌂|🏘|1)
    ├── ►:2[0]:origin/A →:0:
    │   └── →:0: (A →:2:)
    └── ►:3[0]:origin/B
        └── →:0: (A →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:A <> origin/A →:2:
        ├── 👉:0:A <> origin/A →:2:
        │   ├── ❄️a62b0de (🏘️) ►B
        │   └── ❄️120a217 (🏘️)
        └── :4:main <> origin/main
            └── ❄fafd9d0 (🏘️)
    ");

    // The same is true when starting at a different ref.
    let (id, b_ref) = id_at(&repo, "B");
    let graph = Graph::from_commit_traversal(id, b_ref, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:B <> origin/B →:3:
        ├── 👉:0:B <> origin/B →:3:
        │   ├── ❄️a62b0de (🏘️) ►A
        │   └── ❄️120a217 (🏘️)
        └── :4:main <> origin/main
            └── ❄fafd9d0 (🏘️)
    ");

    // If disambiguation happens through the workspace, 'A' still shows the right remote, and 'B' as well
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let graph =
        Graph::from_commit_traversal(id, a_ref.clone(), &*meta, standard_options())?.validated()?;
    // NOTE: origin/A points to :5, but origin/B now also points to :5 even though it should point to :0,
    //       a relationship still preserved though the sibling ID.
    //       There is no easy way of fixing this as we'd have to know that this one connection, which can
    //       indirectly reach the remote tracking segment, should remain on the local tracking segment when
    //       reconnecting them during the segment insertion.
    //       This is acceptable as graph connections aren't used for this, and ultimately they still
    //       reach the right segment, just through one more indirection. Empty segments are 'looked through'
    //       as well by all algorithms for exactly that reason.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── 👉►:5[1]:A <> origin/A →:2:
    │       └── 📙►:0[2]:B <> origin/B →:3:
    │           ├── ·a62b0de (⌂|🏘|1)
    │           └── ·120a217 (⌂|🏘|1)
    │               └── ►:4[3]:main <> origin/main
    │                   └── ·fafd9d0 (⌂|🏘|1)
    ├── ►:2[0]:origin/A →:5:
    │   └── →:5: (A →:2:)
    └── ►:3[0]:origin/B →:0:
        └── →:0: (B →:3:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:5:A <> origin/A →:2: {1}
        ├── 👉:5:A <> origin/A →:2:
        ├── 📙:0:B <> origin/B →:3:
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :4:main <> origin/main
            └── ❄fafd9d0 (🏘️)
    ");
    Ok(())
}

#[test]
fn without_target_ref_with_managed_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-with-ws-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 4fe5a6f (origin/A) A-remote
    |/  
    * a62b0de (A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    // The commit is ambiguous, so there is just the entrypoint to split the segment.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·3ea2742 (⌂|🏘|001)
    │       └── ►:1[1]:A <> origin/A →:2:
    │           ├── ·a62b0de (⌂|🏘|111)
    │           └── ·120a217 (⌂|🏘|111)
    │               └── ►:3[2]:main
    │                   └── ·fafd9d0 (⌂|🏘|111)
    └── ►:2[0]:origin/A →:1:
        └── 🟣4fe5a6f (0x0|100)
            └── →:1: (A →:2:)
    ");
    // TODO: add more stacks.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> origin/A →:2:⇣1
        ├── :1:A <> origin/A →:2:⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·3ea2742 (⌂|🏘)
    │       └── 👉►:0[1]:A <> origin/A →:2:
    │           ├── ·a62b0de (⌂|🏘|11)
    │           └── ·120a217 (⌂|🏘|11)
    │               └── ►:3[2]:main
    │                   └── ·fafd9d0 (⌂|🏘|11)
    └── ►:2[0]:origin/A →:0:
        └── 🟣4fe5a6f (0x0|10)
            └── →:0: (A →:2:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:A <> origin/A →:2:⇣1
        ├── 👉:0:A <> origin/A →:2:⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    Ok(())
}

#[test]
fn workspace_commit_pushed_to_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/ws-commit-pushed-to-target")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8ee08de (HEAD -> gitbutler/workspace, origin/main) GitButler Workspace Commit
    * 120a217 (A) A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:1[0]:origin/main →:3:
        └── 👉📕►►►:0[1]:gitbutler/workspace[🌳]
            └── ·8ee08de (⌂|🏘|✓|1)
                └── ►:2[2]:A
                    └── ·120a217 (⌂|🏘|✓|1)
                        └── ►:3[3]:main <> origin/main →:1:
                            └── ·fafd9d0 (⌂|🏘|✓|1)
    ");
    // Everything is integrated, so nothing is shown.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 120a217");
    Ok(())
}

#[test]
fn no_workspace_no_target_commit_under_managed_ref() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-ws-no-target-commit-with-managed-ref")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * dca94a4 (HEAD -> gitbutler/workspace) unmanaged
    * 120a217 (A) A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        └── ►:1[1]:anon:
            └── ·dca94a4 (⌂|🏘|1)
                └── ►:2[2]:A
                    └── ·120a217 (⌂|🏘|1)
                        └── ►:3[3]:main
                            └── ·fafd9d0 (⌂|🏘|1)
    ");

    // It's notable how hard the workspace ref tries to not own the commit
    // it's under unless it's a managed commit.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:anon:
        ├── :1:anon:
        │   └── ·dca94a4 (🏘️)
        ├── :2:A
        │   └── ·120a217 (🏘️)
        └── :3:main
            └── ·fafd9d0 (🏘️)
    ");
    Ok(())
}

#[test]
fn no_workspace_commit() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multiple-dependent-branches-per-stack-without-ws-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * cbc6713 (HEAD -> gitbutler/workspace, lane) change
    * fafd9d0 (origin/main, main, lane-segment-02, lane-segment-01, lane-2-segment-02, lane-2-segment-01, lane-2) init
    ");

    // Follow the natural order, lane first.
    add_stack_with_segments(
        &mut meta,
        0,
        "lane",
        StackState::InWorkspace,
        &["lane-segment-01", "lane-segment-02"],
    );
    add_stack_with_segments(
        &mut meta,
        1,
        "lane-2",
        StackState::InWorkspace,
        &["lane-2-segment-01", "lane-2-segment-02"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    // Notably we also pick up 'lane' which sits on the base.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   ├── 📙►:3[1]:lane
    │   │   └── ·cbc6713 (⌂|🏘|01)
    │   │       └── 📙►:7[2]:lane-segment-01
    │   │           └── 📙►:8[3]:lane-segment-02
    │   │               └── ►:2[4]:main <> origin/main →:1:
    │   │                   └── ·fafd9d0 (⌂|🏘|✓|11)
    │   └── 📙►:4[1]:lane-2
    │       └── 📙►:5[2]:lane-2-segment-01
    │           └── 📙►:6[3]:lane-2-segment-02
    │               └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:4:lane-2 on fafd9d0 {1}
    │   ├── 📙:4:lane-2
    │   ├── 📙:5:lane-2-segment-01
    │   └── 📙:6:lane-2-segment-02
    └── ≡📙:3:lane on fafd9d0 {0}
        ├── 📙:3:lane
        │   └── ·cbc6713 (🏘️)
        ├── 📙:7:lane-segment-01
        └── 📙:8:lane-segment-02
    ");

    // Natural order here is `lane` first, but we say we want `lane-2` first
    meta.data_mut().branches.clear();
    add_stack_with_segments(
        &mut meta,
        0,
        "lane-2",
        StackState::InWorkspace,
        &["lane-2-segment-01", "lane-2-segment-02"],
    );
    add_stack_with_segments(
        &mut meta,
        1,
        "lane",
        StackState::InWorkspace,
        &["lane-segment-01", "lane-segment-02"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    // the order is maintained as provided in the workspace.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   ├── 📙►:4[1]:lane-2
    │   │   └── 📙►:5[2]:lane-2-segment-01
    │   │       └── 📙►:6[3]:lane-2-segment-02
    │   │           └── ►:2[4]:main <> origin/main →:1:
    │   │               └── ·fafd9d0 (⌂|🏘|✓|11)
    │   └── 📙►:3[1]:lane
    │       └── ·cbc6713 (⌂|🏘|01)
    │           └── 📙►:7[2]:lane-segment-01
    │               └── 📙►:8[3]:lane-segment-02
    │                   └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:lane on fafd9d0 {1}
    │   ├── 📙:3:lane
    │   │   └── ·cbc6713 (🏘️)
    │   ├── 📙:7:lane-segment-01
    │   └── 📙:8:lane-segment-02
    └── ≡📙:4:lane-2 on fafd9d0 {0}
        ├── 📙:4:lane-2
        ├── 📙:5:lane-2-segment-01
        └── 📙:6:lane-2-segment-02
    ");
    Ok(())
}

#[test]
fn two_dependent_branches_first_merged_by_rebase() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-dependent-branches-first-rebased-and-merged")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0b6b861 (origin/main, origin/A) A
    | * 4f08b8d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * da597e8 (B) B
    | * 1818c17 (A) A
    |/  
    * 281456a (main) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·4f08b8d (⌂|🏘|0001)
    │       └── ►:3[1]:B
    │           └── ·da597e8 (⌂|🏘|0001)
    │               └── ►:4[2]:A <> origin/A →:5:
    │                   └── ·1818c17 (⌂|🏘|0101)
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·281456a (⌂|🏘|✓|1111)
    └── ►:5[0]:origin/A →:4:
        └── ►:1[1]:origin/main →:2:
            └── 🟣0b6b861 (✓|1000)
                └── →:2: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 281456a
    └── ≡:3:B on 281456a
        ├── :3:B
        │   └── ·da597e8 (🏘️)
        └── :4:A <> origin/A →:5:⇡1⇣1
            ├── 🟣0b6b861 (✓)
            └── ·1818c17 (🏘️)
    ");
    Ok(())
}

#[test]
fn special_branch_names_do_not_end_up_in_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/special-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8926b15 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 3686017 (main) top
    * 9725482 (gitbutler/edit) middle
    * fafd9d0 (gitbutler/target) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    // Standard handling after traversal and post-processing.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        └── ·8926b15 (⌂|🏘|1)
            └── ►:1[1]:main
                └── ·3686017 (⌂|🏘|1)
                    └── ►:2[2]:gitbutler/edit
                        └── ·9725482 (⌂|🏘|1)
                            └── ►:3[3]:gitbutler/target
                                └── ·fafd9d0 (⌂|🏘|1)
    ");

    // But special handling for workspace views.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:main
        └── :1:main
            ├── ·3686017 (🏘️)
            ├── ·9725482 (🏘️)
            └── ·fafd9d0 (🏘️)
    ");
    Ok(())
}

#[test]
fn special_branch_do_not_allow_overly_long_segments() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/special-branches-edgecase")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 270738b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c59457b (A) top
    * e146f13 (gitbutler/edit) middle
    * 971953d (origin/main, main) M2
    * ce09734 (origin/gitbutler/target, gitbutler/target) M1
    * fafd9d0 init
    ");

    add_workspace(&mut meta);
    let mut md = meta.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    md.target_ref = Some("refs/remotes/origin/gitbutler/target".try_into()?);
    meta.set_workspace(&md)?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        // standard_options_with_extra_target(&repo, "gitbutler/target"),
        standard_options(),
    )?
    .validated()?;
    // Standard handling after traversal and post-processing.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·270738b (⌂|🏘|001)
    │       └── ►:3[1]:A
    │           └── ·c59457b (⌂|🏘|001)
    │               └── ►:4[2]:gitbutler/edit
    │                   └── ·e146f13 (⌂|🏘|001)
    │                       └── ►:5[3]:main <> origin/main →:6:
    │                           └── ·971953d (⌂|🏘|101)
    │                               └── ►:2[4]:gitbutler/target <> origin/gitbutler/target →:1:
    │                                   ├── ·ce09734 (⌂|🏘|✓|111)
    │                                   └── ·fafd9d0 (⌂|🏘|✓|111)
    ├── ►:1[0]:origin/gitbutler/target →:2:
    │   └── →:2: (gitbutler/target →:1:)
    └── ►:6[0]:origin/main →:5:
        └── →:5: (main →:6:)
    ");

    // But special handling for workspace views. Note how we don't overshoot
    // and stop exactly where we have to, magically even.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/gitbutler/target on ce09734
    └── ≡:3:A on ce09734
        ├── :3:A
        │   ├── ·c59457b (🏘️)
        │   └── ·e146f13 (🏘️)
        └── :5:main <> origin/main →:6:
            └── ❄️971953d (🏘️)
    ");
    Ok(())
}

#[test]
fn branch_ahead_of_workspace() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/branches-ahead-of-workspace")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   790a17d (C-bottom) C2 merge commit
    |\  
    | * 631be19 (tmp) C1-outside2
    * | 969aaec C1-outside
    |/  
    | * 71dad1a (D) D2-outside
    | | * c83f258 (A) A2-outside
    | | | * 27c2545 (origin/A-middle, A-middle) A1-outside
    | | | | * c8f73c7 (B-middle) B3-outside
    | | | | * ff75b80 (intermediate-branch) B2-outside
    | | | | | *-.   fe6ba62 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | | | |_|/|\ \  
    | | |/| | | |/  
    | | |_|_|_|/|   
    | |/| | | | |   
    | * | | | | | ed36e3b (new-name-for-D) D1
    | | | | | | * 3f7c4e6 (C) C2
    | |_|_|_|_|/  
    |/| | | | |   
    * | | | | | b6895d7 C1
    |/ / / / /  
    | | | | * 2f8f06d (B) B3
    | | | |/  
    | | | | *   867927f (origin/main, main) Merge branch 'B-middle'
    | | | | |\  
    | | | | |/  
    | | | |/|   
    | | | * | 91bc3fc (origin/B-middle) B2
    | | | * | cf9330f B1
    | |_|/ /  
    |/| | |   
    | | | * 6e03461 Merge branch 'A'
    | |_|/| 
    |/| |/  
    | |/|   
    | * | a62b0de A2
    | |/  
    | * 120a217 A1
    |/  
    * fafd9d0 init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·fe6ba62 (⌂|🏘|01)
    │       ├── ►:5[3]:anon:
    │       │   ├── ·a62b0de (⌂|🏘|✓|11)
    │       │   └── ·120a217 (⌂|🏘|✓|11)
    │       │       └── ►:9[4]:anon:
    │       │           └── ·fafd9d0 (⌂|🏘|✓|11)
    │       ├── ►:6[1]:B
    │       │   └── ·2f8f06d (⌂|🏘|01)
    │       │       └── ►:4[2]:anon:
    │       │           ├── ·91bc3fc (⌂|🏘|✓|11)
    │       │           └── ·cf9330f (⌂|🏘|✓|11)
    │       │               └── →:9:
    │       ├── ►:7[1]:C
    │       │   ├── ·3f7c4e6 (⌂|🏘|01)
    │       │   └── ·b6895d7 (⌂|🏘|01)
    │       │       └── →:9:
    │       └── ►:8[1]:new-name-for-D
    │           └── ·ed36e3b (⌂|🏘|01)
    │               └── →:9:
    └── ►:1[0]:origin/main →:2:
        └── ►:2[1]:main <> origin/main →:1:
            └── ·867927f (⌂|✓|10)
                ├── ►:3[2]:anon:
                │   └── ·6e03461 (⌂|✓|10)
                │       ├── →:9:
                │       └── →:5:
                └── →:4:
    ");

    // If it doesn't know how the workspace should be looking like, i.e. which branches are contained,
    // nothing special happens.
    // The branches that are outside the workspace don't exist and segments are flattened.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on fafd9d0
    ├── ≡:8:new-name-for-D on fafd9d0
    │   └── :8:new-name-for-D
    │       └── ·ed36e3b (🏘️)
    ├── ≡:7:C on fafd9d0
    │   └── :7:C
    │       ├── ·3f7c4e6 (🏘️)
    │       └── ·b6895d7 (🏘️)
    ├── ≡:6:B on fafd9d0
    │   └── :6:B
    │       ├── ·2f8f06d (🏘️)
    │       ├── ·91bc3fc (🏘️|✓)
    │       └── ·cf9330f (🏘️|✓)
    └── ≡:5:anon: on fafd9d0
        └── :5:anon:
            ├── ·a62b0de (🏘️|✓)
            └── ·120a217 (🏘️|✓)
    ");

    // However, when the desired workspace is set up, the traversal will include these extra tips.
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-middle"]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["B-middle"]);
    add_stack_with_segments(&mut meta, 2, "C", StackState::InWorkspace, &["C-bottom"]);
    add_stack_with_segments(&mut meta, 3, "D", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, ":/init"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·fe6ba62 (⌂|🏘|00001)
    │       ├── ►:19[3]:anon: →:4:
    │       │   └── ·a62b0de (⌂|🏘|✓|00011)
    │       │       └── ►:21[4]:anon: →:5:
    │       │           └── ·120a217 (⌂|🏘|✓|00111)
    │       │               └── ►:3[5]:anon:
    │       │                   └── ·fafd9d0 (⌂|🏘|✓|11111)
    │       ├── 📙►:6[1]:B
    │       │   └── ·2f8f06d (⌂|🏘|00001)
    │       │       └── ►:15[2]:anon: →:7:
    │       │           ├── ·91bc3fc (⌂|🏘|✓|11011)
    │       │           └── ·cf9330f (⌂|🏘|✓|11011)
    │       │               └── →:3:
    │       ├── 📙►:8[1]:C
    │       │   └── ·3f7c4e6 (⌂|🏘|00001)
    │       │       └── ►:20[2]:anon: →:9:
    │       │           └── ·b6895d7 (⌂|🏘|00001)
    │       │               └── →:3:
    │       └── ►:18[1]:new-name-for-D
    │           └── ·ed36e3b (⌂|🏘|00001)
    │               └── →:3:
    ├── ►:1[0]:origin/main →:2:
    │   └── ►:2[1]:main <> origin/main →:1:
    │       └── ·867927f (⌂|✓|00010)
    │           ├── ►:13[2]:anon:
    │           │   └── ·6e03461 (⌂|✓|00010)
    │           │       ├── →:3:
    │           │       └── →:19:
    │           └── →:15:
    ├── 📙►:4[0]:A
    │   └── ·c83f258 (⌂)
    │       └── →:19:
    ├── 📙►:7[0]:B-middle <> origin/B-middle →:12:
    │   └── ·c8f73c7 (⌂|01000)
    │       └── ►:16[1]:intermediate-branch
    │           └── ·ff75b80 (⌂|01000)
    │               └── →:15:
    ├── 📙►:9[0]:C-bottom
    │   └── ·790a17d (⌂)
    │       ├── ►:14[1]:tmp
    │       │   └── ·631be19 (⌂)
    │       │       └── →:20:
    │       └── ►:17[1]:anon:
    │           └── ·969aaec (⌂)
    │               └── →:20:
    ├── 📙►:10[0]:D
    │   └── ·71dad1a (⌂)
    │       └── →:18: (new-name-for-D)
    ├── ►:11[0]:origin/A-middle →:5:
    │   └── 📙►:5[1]:A-middle <> origin/A-middle →:11:
    │       └── ·27c2545 (⌂|00100)
    │           └── →:21:
    └── ►:12[0]:origin/B-middle →:7:
        └── →:15:
    ");

    // The workspace itself contains information about the outside tips.
    // We collect it no matter the location of the tip, e.g.
    // - anon segment directly below the workspace commit
    // - middle anon segment leading to the named branch over intermediate branches
    // - middle anon segment leading to the named branch over two outgoing connections
    // - except: if the segment with a known named segment in its future has a (new) name,
    //   we leave it and don't attempt to reconstruct the original (out-of-workspace) reference
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on fafd9d0
    ├── ≡:18:new-name-for-D on fafd9d0
    │   └── :18:new-name-for-D
    │       └── ·ed36e3b (🏘️)
    ├── ≡📙:8:C on fafd9d0 {2}
    │   ├── 📙:8:C
    │   │   └── ·3f7c4e6 (🏘️)
    │   └── 📙:20:C-bottom →:9:
    │       ├── ·790a17d*
    │       ├── ·969aaec*
    │       ├── ·631be19*
    │       └── ·b6895d7 (🏘️)
    ├── ≡📙:6:B on fafd9d0 {1}
    │   ├── 📙:6:B
    │   │   └── ·2f8f06d (🏘️)
    │   └── 📙:15:B-middle <> origin/B-middle →:7:
    │       ├── ·c8f73c7*
    │       ├── ·ff75b80*
    │       ├── ·91bc3fc (🏘️|✓)
    │       └── ·cf9330f (🏘️|✓)
    └── ≡📙:19:A →:4: on fafd9d0 {0}
        ├── 📙:19:A →:4:
        │   ├── ·c83f258*
        │   └── ·a62b0de (🏘️|✓)
        └── 📙:21:A-middle <> origin/A-middle →:5:
            ├── ·27c2545*
            └── ·120a217 (🏘️|✓)
    ");
    Ok(())
}

#[test]
fn two_branches_one_advanced_two_parent_ws_commit_diverged_ttb() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-branches-one-advanced-two-parent-ws-commit-diverged-ttb",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   873d056 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | cbc6713 (advanced-lane) change
    |/  
    * fafd9d0 (main, lane) init
    * da83717 (origin/main) disjoint remote target
    ");

    for (idx, name) in ["lane", "advanced-lane"].into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }

    let (id, ref_name) = id_at(&repo, "lane");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
    │   └── ·873d056 (⌂|🏘)
    │       ├── 👉📙►:4[1]:lane
    │       │   └── ►:0[2]:anon:
    │       │       └── ·fafd9d0 (⌂|🏘|1) ►main
    │       └── 📙►:3[1]:advanced-lane
    │           └── ·cbc6713 (⌂|🏘)
    │               └── →:0:
    └── ►:2[0]:origin/main
        └── 🟣da83717 (✓)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:advanced-lane on fafd9d0 {1}
    │   └── 📙:3:advanced-lane
    │       └── ·cbc6713 (🏘️)
    └── ≡👉📙:4:lane on fafd9d0 {0}
        └── 👉📙:4:lane
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·873d056 (⌂|🏘|1)
    │       ├── 📙►:4[1]:lane
    │       │   └── ►:2[2]:anon:
    │       │       └── ·fafd9d0 (⌂|🏘|✓|1) ►main
    │       └── 📙►:3[1]:advanced-lane
    │           └── ·cbc6713 (⌂|🏘|1)
    │               └── →:2:
    └── ►:1[0]:origin/main
        └── 🟣da83717 (✓)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:advanced-lane on fafd9d0 {1}
    │   └── 📙:3:advanced-lane
    │       └── ·cbc6713 (🏘️)
    └── ≡📙:4:lane on fafd9d0 {0}
        └── 📙:4:lane
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·873d056 (⌂|🏘|1)
    │       ├── 📙►:4[1]:lane
    │       │   └── ►:2[2]:anon:
    │       │       └── ·fafd9d0 (⌂|🏘|1) ►main
    │       └── 📙►:3[1]:advanced-lane
    │           └── ·cbc6713 (⌂|🏘|1)
    │               └── →:2:
    └── ►:1[0]:origin/main
        └── 🟣da83717 (✓)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:advanced-lane on fafd9d0 {1}
    │   └── 📙:3:advanced-lane
    │       └── ·cbc6713 (🏘️)
    └── ≡📙:4:lane on fafd9d0 {0}
        └── 📙:4:lane
    ");
    Ok(())
}

#[test]
fn advanced_workspace_ref() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/advanced-workspace-ref")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a7131b1 (HEAD -> gitbutler/workspace) on-top4
    * 4d3831e (intermediate-ref) on-top3
    *   468357f on-top2-merge
    |\  
    | * d3166f7 (branch-on-top) on-top-sibling
    |/  
    * 118ddbb on-top1
    *   619d548 GitButler Workspace Commit
    |\  
    | * 6fdab32 (A) A1
    * | 8a352d5 (B) B1
    |/  
    * bce0c5e (origin/main, main) M2
    * 3183e43 M1
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ►:5[1]:anon:
    │       └── ·a7131b1 (⌂|🏘|01)
    │           └── ►:6[2]:intermediate-ref
    │               └── ·4d3831e (⌂|🏘|01)
    │                   └── ►:7[3]:anon:
    │                       └── ·468357f (⌂|🏘|01)
    │                           ├── ►:8[5]:anon:
    │                           │   └── ·118ddbb (⌂|🏘|01)
    │                           │       └── ►:10[6]:anon:
    │                           │           └── ·619d548 (⌂|🏘|01)
    │                           │               ├── 📙►:4[7]:B
    │                           │               │   └── ·8a352d5 (⌂|🏘|01)
    │                           │               │       └── ►:2[8]:main <> origin/main →:1:
    │                           │               │           ├── ·bce0c5e (⌂|🏘|✓|11)
    │                           │               │           └── ·3183e43 (⌂|🏘|✓|11)
    │                           │               └── 📙►:3[7]:A
    │                           │                   └── ·6fdab32 (⌂|🏘|01)
    │                           │                       └── →:2: (main →:1:)
    │                           └── ►:9[4]:branch-on-top
    │                               └── ·d3166f7 (⌂|🏘|01)
    │                                   └── →:8:
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // We show the original 'native' configuration without pruning anything, even though
    // it contains the workspace commit 619d548.
    // It's up to the caller to deal with this situation as the workspace now is marked differently.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡:5:anon: on bce0c5e {1}
        ├── :5:anon:
        │   └── ·a7131b1 (🏘️)
        ├── :6:intermediate-ref
        │   ├── ·4d3831e (🏘️)
        │   ├── ·468357f (🏘️)
        │   ├── ·118ddbb (🏘️)
        │   └── ·619d548 (🏘️)
        └── 📙:4:B
            └── ·8a352d5 (🏘️)
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    // The extra-target as would happen in the typical case would change nothing though.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ►:5[1]:anon:
    │       └── ·a7131b1 (⌂|🏘|01)
    │           └── ►:6[2]:intermediate-ref
    │               └── ·4d3831e (⌂|🏘|01)
    │                   └── ►:7[3]:anon:
    │                       └── ·468357f (⌂|🏘|01)
    │                           ├── ►:8[5]:anon:
    │                           │   └── ·118ddbb (⌂|🏘|01)
    │                           │       └── ►:10[6]:anon:
    │                           │           └── ·619d548 (⌂|🏘|01)
    │                           │               ├── 📙►:4[7]:B
    │                           │               │   └── ·8a352d5 (⌂|🏘|01)
    │                           │               │       └── ►:2[8]:main <> origin/main →:1:
    │                           │               │           ├── ·bce0c5e (⌂|🏘|✓|11)
    │                           │               │           └── ·3183e43 (⌂|🏘|✓|11)
    │                           │               └── 📙►:3[7]:A
    │                           │                   └── ·6fdab32 (⌂|🏘|01)
    │                           │                       └── →:2: (main →:1:)
    │                           └── ►:9[4]:branch-on-top
    │                               └── ·d3166f7 (⌂|🏘|01)
    │                                   └── →:8:
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡:5:anon: on bce0c5e {1}
        ├── :5:anon:
        │   └── ·a7131b1 (🏘️)
        ├── :6:intermediate-ref
        │   ├── ·4d3831e (🏘️)
        │   ├── ·468357f (🏘️)
        │   ├── ·118ddbb (🏘️)
        │   └── ·619d548 (🏘️)
        └── 📙:4:B
            └── ·8a352d5 (🏘️)
    ");
    Ok(())
}

#[test]
fn advanced_workspace_ref_single_stack() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/advanced-workspace-ref-and-single-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * da912a8 (HEAD -> gitbutler/workspace) on-top4
    * 198eaf8 (intermediate-ref) on-top3
    *   3147997 on-top2-merge
    |\  
    | * dd7bb9a (branch-on-top) on-top-sibling
    |/  
    * 9785229 on-top1
    * c58f157 GitButler Workspace Commit
    * 6fdab32 (A) A1
    * bce0c5e (origin/main, main) M2
    * 3183e43 M1
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ►:4[1]:anon:
    │       └── ·da912a8 (⌂|🏘|01)
    │           └── ►:5[2]:intermediate-ref
    │               └── ·198eaf8 (⌂|🏘|01)
    │                   └── ►:6[3]:anon:
    │                       └── ·3147997 (⌂|🏘|01)
    │                           ├── ►:7[5]:anon:
    │                           │   ├── ·9785229 (⌂|🏘|01)
    │                           │   └── ·c58f157 (⌂|🏘|01)
    │                           │       └── 📙►:3[6]:A
    │                           │           └── ·6fdab32 (⌂|🏘|01)
    │                           │               └── ►:2[7]:main <> origin/main →:1:
    │                           │                   ├── ·bce0c5e (⌂|🏘|✓|11)
    │                           │                   └── ·3183e43 (⌂|🏘|✓|11)
    │                           └── ►:8[4]:branch-on-top
    │                               └── ·dd7bb9a (⌂|🏘|01)
    │                                   └── →:7:
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // Here we'd show what happens if the workspace commit is somewhere in the middle
    // of the segment. This is relevant for code trying to find it, which isn't done here.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡:4:anon: on bce0c5e {0}
        ├── :4:anon:
        │   └── ·da912a8 (🏘️)
        ├── :5:intermediate-ref
        │   ├── ·198eaf8 (🏘️)
        │   ├── ·3147997 (🏘️)
        │   ├── ·9785229 (🏘️)
        │   └── ·c58f157 (🏘️)
        └── 📙:3:A
            └── ·6fdab32 (🏘️)
    ");
    Ok(())
}

#[test]
fn applied_stack_below_explicit_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-branches-one-below-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   e82dfab (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 6fdab32 (A) A1
    * | 78b1b59 (B) B1
    | | * 938e6f2 (origin/main, main) M4
    | |/  
    |/|   
    * | f52fcec M3
    |/  
    * bce0c5e M2
    * 3183e43 M1
    ");

    add_workspace(&mut meta);
    meta.data_mut().default_target = None;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        └── ·e82dfab (⌂|🏘|1)
            ├── ►:1[1]:B
            │   ├── ·78b1b59 (⌂|🏘|1)
            │   └── ·f52fcec (⌂|🏘|1)
            │       └── ►:3[2]:anon:
            │           ├── ·bce0c5e (⌂|🏘|1)
            │           └── ·3183e43 (⌂|🏘|1)
            └── ►:2[1]:A
                └── ·6fdab32 (⌂|🏘|1)
                    └── →:3:
    ");

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on bce0c5e
    ├── ≡:2:A on bce0c5e
    │   └── :2:A
    │       └── ·6fdab32 (🏘️)
    └── ≡:1:B on bce0c5e
        └── :1:B
            ├── ·78b1b59 (🏘️)
            └── ·f52fcec (🏘️)
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    // The same is true if stacks are known in workspace metadata.
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·e82dfab (⌂|🏘|01)
    │       ├── 📙►:3[1]:A
    │       │   └── ·6fdab32 (⌂|🏘|01)
    │       │       └── ►:6[3]:anon:
    │       │           ├── ·bce0c5e (⌂|🏘|✓|11)
    │       │           └── ·3183e43 (⌂|🏘|✓|11)
    │       └── 📙►:4[1]:B
    │           └── ·78b1b59 (⌂|🏘|01)
    │               └── ►:5[2]:anon:
    │                   └── ·f52fcec (⌂|🏘|✓|11)
    │                       └── →:6:
    └── ►:1[0]:origin/main →:2:
        └── ►:2[1]:main <> origin/main →:1:
            └── ·938e6f2 (⌂|✓|10)
                └── →:5:
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e
    ├── ≡📙:4:B on bce0c5e {1}
    │   └── 📙:4:B
    │       ├── ·78b1b59 (🏘️)
    │       └── ·f52fcec (🏘️|✓)
    └── ≡📙:3:A on bce0c5e {0}
        └── 📙:3:A
            └── ·6fdab32 (🏘️)
    ");

    // Finally, if the extra-target, indicating an old stored base that isn't valid anymore.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, ":/M3"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·e82dfab (⌂|🏘|01)
    │       ├── 📙►:4[1]:A
    │       │   └── ·6fdab32 (⌂|🏘|01)
    │       │       └── ►:6[3]:anon:
    │       │           ├── ·bce0c5e (⌂|🏘|✓|11)
    │       │           └── ·3183e43 (⌂|🏘|✓|11)
    │       └── 📙►:5[1]:B
    │           └── ·78b1b59 (⌂|🏘|01)
    │               └── ►:3[2]:anon:
    │                   └── ·f52fcec (⌂|🏘|✓|11)
    │                       └── →:6:
    └── ►:1[0]:origin/main →:2:
        └── ►:2[1]:main <> origin/main →:1:
            └── ·938e6f2 (⌂|✓|10)
                └── →:3:
    ");

    // The base is still adjusted so it matches the actual stacks.
    // Note how it shows more of the base of `B` due to `A` having a lower base with the target branch.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e
    ├── ≡📙:5:B on bce0c5e {1}
    │   └── 📙:5:B
    │       ├── ·78b1b59 (🏘️)
    │       └── ·f52fcec (🏘️|✓)
    └── ≡📙:4:A on bce0c5e {0}
        └── 📙:4:A
            └── ·6fdab32 (🏘️)
    ");

    Ok(())
}

#[test]
fn applied_stack_above_explicit_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-branches-one-above-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c5587c9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * de6d39c (A) A1
    | * a821094 (origin/main, main) M3
    * | ce25240 (B) B1
    |/  
    * bce0c5e M2
    * 3183e43 M1
    ");

    add_workspace(&mut meta);
    meta.data_mut().default_target = None;
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·c5587c9 (⌂|🏘|01)
    │       ├── ►:1[1]:B
    │       │   └── ·ce25240 (⌂|🏘|01)
    │       │       └── ►:5[3]:anon:
    │       │           ├── ·bce0c5e (⌂|🏘|11)
    │       │           └── ·3183e43 (⌂|🏘|11)
    │       └── ►:2[1]:A
    │           └── ·de6d39c (⌂|🏘|01)
    │               └── ►:3[2]:main <> origin/main →:4:
    │                   └── ·a821094 (⌂|🏘|11)
    │                       └── →:5:
    └── ►:4[0]:origin/main →:3:
        └── →:3: (main →:4:)
    ");

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    // Interestingly, A now gets to see integrated parts of the target branch.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on bce0c5e
    ├── ≡:2:A on bce0c5e
    │   ├── :2:A
    │   │   └── ·de6d39c (🏘️)
    │   └── :3:main <> origin/main →:4:
    │       └── ❄️a821094 (🏘️)
    └── ≡:1:B on bce0c5e
        └── :1:B
            └── ·ce25240 (🏘️)
    ");
    Ok(())
}

#[test]
fn dependent_branch_on_base() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/dependent-branch-on-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   a0385a8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * 49d4b34 (A) A1
    | |/  
    |/|   
    | * f9e2cb7 (C2-3, C2-2, C2-1, C) C2
    | * aaa195b (C1-3, C1-2, C1-1) C1
    |/  
    * 3183e43 (origin/main, main, below-below-C, below-below-B, below-below-A, below-C, below-B, below-A, B) M1
    ");

    add_stack_with_segments(
        &mut meta,
        1,
        "A",
        StackState::InWorkspace,
        &["below-A", "below-below-A"],
    );
    add_stack_with_segments(
        &mut meta,
        2,
        "B",
        StackState::InWorkspace,
        &["below-B", "below-below-B"],
    );
    add_stack_with_segments(
        &mut meta,
        3,
        "C",
        StackState::InWorkspace,
        &[
            "C2-1",
            "C2-2",
            "C2-3",
            "C1-3",
            "C1-2",
            "C1-1",
            "below-C",
            "below-below-C",
        ],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·a0385a8 (⌂|🏘|01)
    │       ├── 📙►:3[1]:A
    │       │   └── ·49d4b34 (⌂|🏘|01)
    │       │       └── 📙►:9[2]:below-A
    │       │           └── 📙►:10[3]:below-below-A
    │       │               └── ►:2[10]:main <> origin/main →:1:
    │       │                   └── ·3183e43 (⌂|🏘|✓|11)
    │       ├── 📙►:6[1]:B
    │       │   └── 📙►:7[2]:below-B
    │       │       └── 📙►:8[3]:below-below-B
    │       │           └── →:2: (main →:1:)
    │       └── 📙►:11[1]:C
    │           └── 📙►:12[2]:C2-1
    │               └── 📙►:13[3]:C2-2
    │                   └── 📙►:14[4]:C2-3
    │                       └── ·f9e2cb7 (⌂|🏘|01)
    │                           └── 📙►:15[5]:C1-3
    │                               └── 📙►:16[6]:C1-2
    │                                   └── 📙►:17[7]:C1-1
    │                                       └── ·aaa195b (⌂|🏘|01)
    │                                           └── 📙►:18[8]:below-C
    │                                               └── 📙►:19[9]:below-below-C
    │                                                   └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // Both stacks will look the same, with the dependent branch inserted at the very bottom.
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:11:C on 3183e43 {3}
    │   ├── 📙:11:C
    │   ├── 📙:12:C2-1
    │   ├── 📙:13:C2-2
    │   ├── 📙:14:C2-3
    │   │   └── ·f9e2cb7 (🏘️)
    │   ├── 📙:15:C1-3
    │   ├── 📙:16:C1-2
    │   ├── 📙:17:C1-1
    │   │   └── ·aaa195b (🏘️)
    │   ├── 📙:18:below-C
    │   └── 📙:19:below-below-C
    ├── ≡📙:6:B on 3183e43 {2}
    │   ├── 📙:6:B
    │   ├── 📙:7:below-B
    │   └── 📙:8:below-below-B
    └── ≡📙:3:A on 3183e43 {1}
        ├── 📙:3:A
        │   └── ·49d4b34 (🏘️)
        ├── 📙:9:below-A
        └── 📙:10:below-below-A
    ");

    let wrongly_inactive = StackState::Inactive;
    add_stack_with_segments(
        &mut meta,
        1,
        "A",
        wrongly_inactive,
        &["below-A", "below-below-A"],
    );
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    // The stack-id could still be found, even though `A` is wrongly marked as outside the workspace.
    // Below A doesn't apply as it's marked inactive.
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:9:C on 3183e43 {3}
    │   ├── 📙:9:C
    │   ├── 📙:10:C2-1
    │   ├── 📙:11:C2-2
    │   ├── 📙:12:C2-3
    │   │   └── ·f9e2cb7 (🏘️)
    │   ├── 📙:13:C1-3
    │   ├── 📙:14:C1-2
    │   ├── 📙:15:C1-1
    │   │   └── ·aaa195b (🏘️)
    │   ├── 📙:16:below-C
    │   └── 📙:17:below-below-C
    ├── ≡📙:6:B on 3183e43 {2}
    │   ├── 📙:6:B
    │   ├── 📙:7:below-B
    │   └── 📙:8:below-below-B
    └── ≡📙:5:A on 3183e43 {1}
        └── 📙:5:A
            └── ·49d4b34 (🏘️)
    ");
    Ok(())
}

#[test]
fn remote_and_integrated_tracking_branch_on_merge() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-and-integrated-tracking")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * d018f71 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * c1e26b0 (origin/main, main) M-advanced
    |/  
    | * 2181501 (origin/A) A-remote
    |/  
    *   1ee1e34 (A) M-base
    |\  
    | * efc3b77 (tmp1) X
    * | c822d66 Y
    |/  
    * bce0c5e M2
    * 3183e43 M1
    ");
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
    └── ≡📙:8:A <> origin/A →:4:⇣1 on 1ee1e34 {1}
        └── 📙:8:A <> origin/A →:4:⇣1
            └── 🟣2181501
    ");

    Ok(())
}

#[test]
fn remote_and_integrated_tracking_branch_on_linear_segment() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/remote-and-integrated-tracking-linear")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 21e584f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 8dc508f (origin/main, main) M-advanced
    |/  
    | * 197ddce (origin/A) A-remote
    |/  
    * 081bae9 (A) M-base
    * 3183e43 M1
    ");
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
    └── ≡📙:5:A <> origin/A →:4:⇣1 on 081bae9 {1}
        └── 📙:5:A <> origin/A →:4:⇣1
            └── 🟣197ddce
    ");

    Ok(())
}

#[test]
fn remote_and_integrated_tracking_branch_on_merge_extra_target() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/remote-and-integrated-tracking-extra-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 5f2810f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9f47a25 (A) A-local
    | * c1e26b0 (origin/main, main) M-advanced
    |/  
    | * 2181501 (origin/A) A-remote
    |/  
    *   1ee1e34 M-base
    |\  
    | * efc3b77 (tmp1) X
    * | c822d66 Y
    |/  
    * bce0c5e M2
    * 3183e43 M1
    ");
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
    └── ≡📙:3:A <> origin/A →:4:⇡1⇣1 on 1ee1e34 {1}
        └── 📙:3:A <> origin/A →:4:⇡1⇣1
            ├── 🟣2181501
            └── ·9f47a25 (🏘️)
    ");

    Ok(())
}

#[test]
fn unapplied_branch_on_base() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/unapplied-branch-on-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * fafd9d0 (origin/main, unapplied, main) init
    ");
    add_workspace(&mut meta);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·a26ae77 (⌂|🏘|01)
    │       └── ►:2[1]:main <> origin/main →:1:
    │           └── ·fafd9d0 (⌂|🏘|✓|11) ►unapplied
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // if the branch was never seen, it's not visible as one would expect.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0");

    // An applied branch would be present, but has no commit.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:unapplied on fafd9d0 {1}
        └── 📙:3:unapplied
    ");

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    // This will be an empty workspace.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0");

    Ok(())
}

#[test]
fn unapplied_branch_on_base_no_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/unapplied-branch-on-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * fafd9d0 (origin/main, unapplied, main) init
    ");
    add_workspace(&mut meta);
    remove_target(&mut meta);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·a26ae77 (⌂|🏘|01)
    │       └── ►:2[1]:main <> origin/main →:1:
    │           └── ·fafd9d0 (⌂|🏘|11) ►unapplied
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // the main branch is disambiguated by its remote reference.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:2:main <> origin/main →:1:
        └── :2:main <> origin/main →:1:
            └── ❄️fafd9d0 (🏘️) ►unapplied
    ");

    // The 'unapplied' branch can be added on top of that, and we make clear we want `main` as well.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "main", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·a26ae77 (⌂|🏘|01)
    │       ├── 📙►:3[1]:unapplied
    │       │   └── ►:2[2]:anon:
    │       │       └── ·fafd9d0 (⌂|🏘|✓|11)
    │       └── 📙►:4[1]:main <> origin/main →:1:
    │           └── →:2:
    └── ►:1[0]:origin/main →:4:
        └── →:4: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:4:main <> origin/main →:1: on fafd9d0 {2}
    │   └── 📙:4:main <> origin/main →:1:
    └── ≡📙:3:unapplied on fafd9d0 {1}
        └── 📙:3:unapplied
    ");

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    // Now only `main` shows up.
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:main <> origin/main →:1: on fafd9d0 {2}
        └── 📙:3:main <> origin/main →:1:
    ");

    Ok(())
}

#[test]
fn no_ws_commit_two_branches_no_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-ws-ref-no-ws-commit-two-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * bce0c5e (HEAD -> gitbutler/workspace, main, B, A) M2
    * 3183e43 M1
    ");
    remove_target(&mut meta);
    add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        ├── 📙►:2[1]:main
        │   └── ►:1[2]:anon:
        │       ├── ·bce0c5e (⌂|🏘|1) ►B
        │       └── ·3183e43 (⌂|🏘|1)
        └── 📙►:3[1]:A
            └── →:1:
    ");
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on bce0c5e
    ├── ≡📙:3:A on bce0c5e {1}
    │   └── 📙:3:A
    └── ≡📙:2:main on bce0c5e {0}
        └── 📙:2:main
    ");
    Ok(())
}

#[test]
fn ambiguous_worktrees() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/ambiguous-worktrees")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   a5f94a2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 3e01e28 (B) B
    |/  
    | * 8dc508f (origin/main, main) M-advanced
    |/  
    | * 197ddce (origin/A) A-remote
    |/  
    * 081bae9 (A-outside, A-inside, A) M-base
    * 3183e43 M1
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·a5f94a2 (⌂|🏘|0001)
    │       ├── ►:5[1]:B[📁wt-B-inside]
    │       │   └── ·3e01e28 (⌂|🏘|0001)
    │       │       └── ►:3[2]:anon:
    │       │           ├── ·081bae9 (⌂|🏘|✓|1111) ►A-inside[📁wt-A-inside], ►A-outside[📁wt-A-outside]
    │       │           └── ·3183e43 (⌂|🏘|✓|1111)
    │       └── 📙►:6[1]:A <> origin/A →:4:
    │           └── →:3:
    ├── ►:1[0]:origin/main →:2:
    │   └── ►:2[1]:main <> origin/main →:1:
    │       └── ·8dc508f (⌂|✓|0010)
    │           └── →:3:
    └── ►:4[0]:origin/A →:6:
        └── 🟣197ddce (0x0|1000)
            └── →:3:
    ");

    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
    ├── ≡📙:6:A <> origin/A →:4:⇣1 on 081bae9 {0}
    │   └── 📙:6:A <> origin/A →:4:⇣1
    │       └── 🟣197ddce
    └── ≡:5:B[📁wt-B-inside] on 081bae9
        └── :5:B[📁wt-B-inside]
            └── ·3e01e28 (🏘️)
    ");
    Ok(())
}

#[test]
fn duplicate_parent_connection_from_ws_commit_to_ambiguous_branch_no_advanced_target()
-> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/duplicate-workspace-connection-no-target")?;
    // Note that HEAD isn't actually pointing at origin/main, but twice at main
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f18d244 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\
    * fafd9d0 (origin/main, main, B, A) init
    ");

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    // Our graph is incapable of showing these two connections due to traversal
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·f18d244 (⌂|🏘|01)
    │       └── 📙►:3[1]:A
    │           └── ►:2[2]:main <> origin/main →:1:
    │               └── ·fafd9d0 (⌂|🏘|✓|11) ►B
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // Branch should be visible in workspace once.
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        └── 📙:3:A
    ");

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:4:B on fafd9d0 {2}
    │   └── 📙:4:B
    └── ≡📙:3:A on fafd9d0 {1}
        └── 📙:3:A
    ");

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        ├── 📙:3:A
        └── 📙:4:B
    ");

    Ok(())
}

#[test]
fn duplicate_parent_connection_from_ws_commit_to_ambiguous_branch() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/duplicate-workspace-connection")?;
    // Note that HEAD isn't actually pointing at origin/main, but twice at main
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f18d244 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\
    | * 12b42b0 (origin/main) RM
    |/  
    * fafd9d0 (main, B, A) init
    ");

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·f18d244 (⌂|🏘|01)
    │       └── 📙►:3[1]:A
    │           └── ►:2[2]:main <> origin/main →:1:
    │               └── ·fafd9d0 (⌂|🏘|✓|11) ►B
    └── ►:1[0]:origin/main →:2:
        └── 🟣12b42b0 (✓)
            └── →:2: (main →:1:)
    ");

    // Branch should be visible in workspace once.
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        └── 📙:3:A
    ");

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:4:B on fafd9d0 {2}
    │   └── 📙:4:B
    └── ≡📙:3:A on fafd9d0 {1}
        └── 📙:3:A
    ");

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        ├── 📙:3:A
        └── 📙:4:B
    ");

    // With extra-target these cases work as well
    meta.data_mut().branches.clear();
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:4:B on fafd9d0 {2}
    │   └── 📙:4:B
    └── ≡📙:3:A on fafd9d0 {1}
        └── 📙:3:A
    ");

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        ├── 📙:3:A
        └── 📙:4:B
    ");

    Ok(())
}

mod edit_commit {
    use but_graph::Graph;
    use but_testsupport::{graph_tree, graph_workspace, visualize_commit_graph_all};

    use crate::init::{add_workspace, id_at, read_only_in_memory_scenario, standard_options};

    #[test]
    fn applied_stack_below_explicit_lower_bound() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("ws/edit-commit/simple")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        * a62b0de (A) A2
        * 120a217 (gitbutler/edit) A1
        * fafd9d0 (origin/main, main) init
        ");

        add_workspace(&mut meta);
        let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
        insta::assert_snapshot!(graph_tree(&graph), @r"

        ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
        │   └── ·3ea2742 (⌂|🏘|01)
        │       └── ►:3[1]:A
        │           └── ·a62b0de (⌂|🏘|01)
        │               └── ►:4[2]:gitbutler/edit
        │                   └── ·120a217 (⌂|🏘|01)
        │                       └── ►:2[3]:main <> origin/main →:1:
        │                           └── ·fafd9d0 (⌂|🏘|✓|11)
        └── ►:1[0]:origin/main →:2:
            └── →:2: (main →:1:)
        ");

        // special branch names are skipped by default and entirely invisible.
        insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
        └── ≡:3:A on fafd9d0
            └── :3:A
                ├── ·a62b0de (🏘️)
                └── ·120a217 (🏘️)
        ");

        // However, if the HEAD points to that reference…
        let (id, ref_name) = id_at(&repo, "gitbutler/edit");
        let graph =
            Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
        insta::assert_snapshot!(graph_tree(&graph), @r"

        ├── 📕►►►:1[0]:gitbutler/workspace[🌳]
        │   └── ·3ea2742 (⌂|🏘)
        │       └── ►:4[1]:A
        │           └── ·a62b0de (⌂|🏘)
        │               └── 👉►:0[2]:gitbutler/edit
        │                   └── ·120a217 (⌂|🏘|01)
        │                       └── ►:3[3]:main <> origin/main →:2:
        │                           └── ·fafd9d0 (⌂|🏘|✓|11)
        └── ►:2[0]:origin/main →:3:
            └── →:3: (main →:2:)
        ");
        // …then the segment becomes visible.
        insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
        📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
        └── ≡:4:A on fafd9d0
            ├── :4:A
            │   └── ·a62b0de (🏘️)
            └── 👉:0:gitbutler/edit
                └── ·120a217 (🏘️)
        ");
        Ok(())
    }
}

/// Complex merge history with origin/main as the target branch.
/// This simulates a real-world scenario where:
/// - origin/main has multiple merged PRs with complex merge history
/// - A local workspace branch exists with uncommitted work
/// - The local stack branches off from an earlier point in history (nightly/0.5.1754)
#[test]
fn complex_merge_history_with_origin_main_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/complex-merge-origin-main")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4d53bb1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 4eaff93 (reimplement-insert-blank-commit, reconstructed-insert-blank-commit-branch, local-stack) composability improvements
    * d19db1d rename reword_commit to commit_reword
    * fb0a67e Reimplement insert blank commit
    | *   e7e93d6 (origin/main, main) Merge pull request #11567 from gitbutlerapp/jt/uhunk2
    | |\  
    | | * eadc96a (jt-uhunk2) Address Copilot review
    | | * 8db8b43 refactor
    | | * 0aa7094 rub: uncommitted hunk to unassigned area
    | | * 28a0336 id: ensure that branch IDs work
    | |/  
    |/|   
    | * 49b28a4 (tag: nightly/0.5.1755) refactor-remove-unused-css-variables (#11576)
    | *   d627ca0 Merge pull request #11571
    | |\  
    | | * d62ab55 (pr-11571) Restrict visibility of some functions
    | |/  
    | * 4ad4354 Merge pull request #11574 from Byron/fix
    |/| 
    | * 5de9f4e (byron-fix) Adjust type of ui.check_for_updates_interval_in_seconds
    * |   68e62aa (tag: nightly/0.5.1754) Merge pull request #11573
    |\ \  
    | |/  
    |/|   
    | * 2d02c78 (pr-11573) fix kiril reword example
    |/  
    * 322cb14 base
    * fafd9d0 init
    ");

    // Add workspace with origin/main as target (not origin/main)
    add_workspace(&mut meta);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣10 on 68e62aa
    └── ≡:12:anon: on 68e62aa
        └── :12:anon:
            ├── ·4eaff93 (🏘️) ►local-stack, ►reconstructed-insert-blank-commit-branch, ►reimplement-insert-blank-commit
            ├── ·d19db1d (🏘️)
            └── ·fb0a67e (🏘️)
    ");

    // Also add the local stack as a workspace stack
    add_stack_with_segments(
        &mut meta,
        0,
        "reimplement-insert-blank-commit",
        StackState::InWorkspace,
        &["reconstructed-insert-blank-commit-branch"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.into_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣10 on 68e62aa
    └── ≡📙:13:reimplement-insert-blank-commit on 68e62aa {0}
        ├── 📙:13:reimplement-insert-blank-commit
        └── 📙:14:reconstructed-insert-blank-commit-branch
            ├── ·4eaff93 (🏘️) ►local-stack
            ├── ·d19db1d (🏘️)
            └── ·fb0a67e (🏘️)
    ");

    Ok(())
}

#[test]
fn reproduce_12146() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/reproduce-12146")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   d77ecda (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 7163661 (B) New commit on branch B
    |/  
    * 81d4e38 (A) add A
    * e32cf47 (origin/main, main) add M
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @"

    ├── 👉📕►►►:0[0]:gitbutler/workspace[🌳]
    │   └── ·d77ecda (⌂|🏘|01)
    │       ├── 📙►:5[1]:A
    │       │   └── ►:3[2]:anon:
    │       │       └── ·81d4e38 (⌂|🏘|01)
    │       │           └── ►:2[3]:main <> origin/main →:1:
    │       │               └── ·e32cf47 (⌂|🏘|✓|11)
    │       └── 📙►:4[1]:B
    │           └── ·7163661 (⌂|🏘|01)
    │               └── →:3:
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // The sibling ID is not set, and we see only two stacks: B owns 7163661,
    // and both A and B include the shared base commit 81d4e38 (A only has 81d4e38).
    let ws = &graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e32cf47
    ├── ≡📙:4:B on e32cf47 {1}
    │   └── 📙:4:B
    │       ├── ·7163661 (🏘️)
    │       └── ·81d4e38 (🏘️)
    └── ≡📙:5:A on e32cf47 {0}
        └── 📙:5:A
            └── ·81d4e38 (🏘️)
    ");

    Ok(())
}
