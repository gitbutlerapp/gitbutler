use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
    ref_metadata::{
        ProjectMeta, StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_graph::{
    SegmentMetadata,
    init::{Overlay, Tip, TipRole},
};
use but_testsupport::{
    InMemoryRefMetadata, branch_tree, graph_workspace, visualize_commit_graph_all,
};

use crate::init::{
    StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev,
    read_only_in_memory_scenario, standard_options,
    utils::{
        add_stack, add_workspace_with_target, add_workspace_without_target,
        named_read_only_in_memory_scenario, remove_target, standard_options_with_extra_target,
    },
};

fn project_meta(meta: &impl RefMetadata) -> ProjectMeta {
    meta.workspace(WORKSPACE_REF_NAME.try_into().expect("valid workspace ref"))
        .map(|workspace| workspace.project_meta())
        .unwrap_or_default()
}

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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·59a427f (⌂|🏘|1)
    │   ├── :1:►main
    │   │   ├── ·0a415d8 (⌂|🏘|11)
    │   │   └── :4:►anon:
    │   │       ├── ·73ba99d (⌂|🏘|111)
    │   │       └── :5:►anon:
    │   │           └── 🏁·fafd9d0 (⌂|🏘|111)
    │   └── :3:►A
    │       ├── ·a62b0de (⌂|🏘|1)
    │       ├── ·120a217 (⌂|🏘|1)
    │       └── →:5:►anon:
    └── :2:►origin/main
        ├── ·1f5c47b (0x0|100)
        └── →:4:►anon:
    ");

    // It's perfectly valid to have the local tracking branch of our target in the workspace,
    // and the low-bound computation works as well.
    let ws = &graph;
    insta::assert_snapshot!(graph_workspace(ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on fafd9d0
    ├── ≡:1:main <> origin/main⇡1⇣1 on fafd9d0
    │   └── :1:main <> origin/main⇡1⇣1
    │       ├── 🟣1f5c47b
    │       ├── ·0a415d8 (🏘️)
    │       └── ❄️73ba99d (🏘️)
    └── ≡:3:A on fafd9d0
        └── :3:A
            ├── ·a62b0de (🏘️)
            └── ·120a217 (🏘️)
    ");

    Ok(())
}

#[test]
fn workspace_with_only_local_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-contained-and-target-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * e5e2623 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * cb54dca (origin/main) RM1
    |/  
    * 0a415d8 (main) M3
    * 73ba99d M2
    * fafd9d0 init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·e5e2623 (⌂|🏘|1)
    │   └── :2:►main
    │       ├── ·0a415d8 (⌂|🏘|✓|11)
    │       ├── ·73ba99d (⌂|🏘|✓|11)
    │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    └── :1:►origin/main
        ├── ·cb54dca (✓)
        └── →:2:►main
    ");

    let ws = &graph;
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    let ws = &graph;
    insta::assert_snapshot!(graph_workspace(ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:2:A on 3183e43 {1}
    │   └── 📙:2:A
    │       └── ·7236012 (🏘️)
    └── ≡📙:3:B on 3183e43 {2}
        ├── 📙:3:B
        │   └── ·68c8a9d (🏘️)
        └── 📙:5:below
    ");

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["below"]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:2:A on 3183e43 {1}
    │   ├── 📙:2:A
    │   │   └── ·7236012 (🏘️)
    │   └── 📙:5:below
    └── ≡📙:3:B on 3183e43 {2}
        └── 📙:3:B
            └── ·68c8a9d (🏘️)
    ");

    Ok(())
}

#[test]
fn workspace_projection_with_advanced_stack_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/advanced-stack-tip-outside-workspace")?;
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * cc0bf57 (B) B-outside
    | * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |/  
    * d69fe94 B
    * 09d8e52 (A) A
    * 85efbe4 (origin/main, main) M
    ");

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·2076060 (⌂|🏘|1)
    │   └── :5:►anon:
    │       ├── ·d69fe94 (⌂|🏘|1)
    │       └── :4:►A
    │           ├── ·09d8e52 (⌂|🏘|1)
    │           └── :1:►main
    │               └── 🏁·85efbe4 (⌂|🏘|✓|11)
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :3:►B
        ├── ·cc0bf57 (⌂)
        └── →:5:►anon:
    ");
    let ws = &graph;
    insta::assert_snapshot!(graph_workspace(ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {1}
        ├── 📙:4:B
        │   ├── ·cc0bf57*
        │   └── ·d69fe94 (🏘️)
        └── 📙:3:A
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡:5:anon: on a821094 {2}
    │   └── :5:anon:
    │       ├── ·835086d (🏘️) ►four, ►three
    │       └── ·ff310d3 (🏘️)
    └── ≡📙:2:X <> origin/X⇡1 on 3183e43 {1}
        └── 📙:2:X <> origin/X⇡1
            ├── ·0b203b5 (🏘️)
            └── ❄️4840f3b (🏘️)
    ");

    Ok(())
}

#[test]
fn single_stack_ambiguous() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack-ambiguous")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘|1)
    │   └── :3:►B
    │       ├── ·70e9a36 (⌂|🏘|101)
    │       ├── ·320e105 (⌂|🏘|101) ►without-ref
    │       ├── ·2a31450 (⌂|🏘|101) ►B-empty, ►ambiguous-01
    │       └── :4:►origin/B
    │           ├── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │           └── :1:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    └── :2:►origin/main
        └── →:1:►main
    ");

    // All non-integrated segments are visible.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:2:B <> origin/B⇡3 on fafd9d0
        └── :2:B <> origin/B⇡3
            ├── ·70e9a36 (🏘️)
            ├── ·320e105 (🏘️) ►tags/without-ref
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄️70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // There is always a segment for the entrypoint, and code working with the graph
    // deals with that naturally.
    let (without_ref_id, ref_name) = id_at(&repo, "without-ref");
    let graph = but_graph::Workspace::from_commit_traversal(
        without_ref_id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // See how tags ARE allowed to name a segment, at least when used as entrypoint.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘)
    │   └── :4:►B
    │       ├── ·70e9a36 (⌂|🏘|100)
    │       └── 👉:0:►without-ref
    │           ├── ·320e105 (⌂|🏘|101)
    │           ├── ·2a31450 (⌂|🏘|101) ►B-empty, ►ambiguous-01
    │           └── :5:►origin/B
    │               ├── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │               └── :2:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    └── :3:►origin/main
        └── →:2:►main
    ");
    // Now `HEAD` is outside a workspace, which goes to single-branch mode. But it knows it's in a workspace
    // and shows the surrounding parts, while marking the segment as entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:B <> origin/B⇡1 on fafd9d0
        ├── :3:B <> origin/B⇡1
        │   └── ·70e9a36 (🏘️)
        └── 👉:0:tags/without-ref
            ├── ·320e105 (🏘️)
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // We don't have to give it a ref-name
    let graph = but_graph::Workspace::from_commit_traversal(
        without_ref_id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘)
    │   └── :4:►B
    │       ├── ·70e9a36 (⌂|🏘|100)
    │       └── 👉:0:►anon:
    │           ├── ·320e105 (⌂|🏘|101) ►without-ref
    │           ├── ·2a31450 (⌂|🏘|101) ►B-empty, ►ambiguous-01
    │           └── :5:►origin/B
    │               ├── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │               └── :2:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    └── :3:►origin/main
        └── →:2:►main
    ");

    // Entrypoint is now unnamed (as no ref-name was provided for traversal)
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:B <> origin/B⇡1 on fafd9d0
        ├── :3:B <> origin/B⇡1
        │   └── ·70e9a36 (🏘️)
        └── 👉:0:anon:
            ├── ·320e105 (🏘️) ►tags/without-ref
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // Putting the entrypoint onto a commit in an anonymous segment with ambiguous refs makes no difference.
    let (b_id_1, tag_ref_name) = id_at(&repo, "B-empty");
    let graph = but_graph::Workspace::from_commit_traversal(
        b_id_1,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘)
    │   └── :4:►B
    │       ├── ·70e9a36 (⌂|🏘|100)
    │       ├── ·320e105 (⌂|🏘|100) ►without-ref
    │       └── 👉:0:►anon:
    │           ├── ·2a31450 (⌂|🏘|101) ►B-empty, ►ambiguous-01
    │           └── :5:►origin/B
    │               ├── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │               └── :2:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    └── :3:►origin/main
        └── →:2:►main
    ");

    // Doing this is very much like edit mode, and there is always a segment starting at the entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:B <> origin/B⇡2 on fafd9d0
        ├── :3:B <> origin/B⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        └── 👉:0:anon:
            ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
            └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    ");

    // If we pass an entrypoint ref name, it will be used as segment name (despite being ambiguous without it)
    let graph = but_graph::Workspace::from_commit_traversal(
        b_id_1,
        tag_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘)
    │   └── :4:►B
    │       ├── ·70e9a36 (⌂|🏘|100)
    │       ├── ·320e105 (⌂|🏘|100) ►without-ref
    │       └── 👉:0:►B-empty
    │           ├── ·2a31450 (⌂|🏘|101) ►ambiguous-01
    │           └── :5:►origin/B
    │               ├── ·70bde6b (⌂|🏘|1101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │               └── :2:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    └── :3:►origin/main
        └── →:2:►main
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:B <> origin/B⇡2 on fafd9d0
        ├── :3:B <> origin/B⇡2
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘|1)
    │   └── :4:►B
    │       ├── ·70e9a36 (⌂|🏘|101)
    │       ├── ·320e105 (⌂|🏘|101) ►without-ref
    │       └── :3:►B-empty
    │           ├── ·2a31450 (⌂|🏘|101) ►ambiguous-01
    │           └── :5:►origin/B
    │               ├── ·70bde6b (⌂|🏘|1101) ►A-empty-02
    │               └── :1:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :6:►A-empty-03
        └── :7:►A-empty-01
            └── :8:►A
                └── →:5:►origin/B
    ");

    // We pickup empty segments.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:B <> origin/B⇡2 on fafd9d0 {0}
        ├── 📙:3:B <> origin/B⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:2:B-empty
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘|1)
    │   └── :4:►B
    │       ├── ·70e9a36 (⌂|🏘|101)
    │       ├── ·320e105 (⌂|🏘|101) ►without-ref
    │       └── :3:►B-empty
    │           ├── ·2a31450 (⌂|🏘|101) ►ambiguous-01
    │           └── :5:►origin/B
    │               ├── ·70bde6b (⌂|🏘|1101)
    │               └── :1:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|1111) ►new-A, ►new-B
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :6:►A-empty-03
        └── :7:►A-empty-02
            └── :8:►A-empty-01
                └── :9:►A
                    └── →:5:►origin/B
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:B <> origin/B⇡2 on fafd9d0 {0}
        ├── 📙:3:B <> origin/B⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:2:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        └── 📙:10:A
            └── ❄70bde6b (🏘️)
    ");

    // Define only some of the branches, it should figure that out.
    // It respects the order of the mention in the stack, `A` before `A-empty-01`.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-empty-01"]);
    add_stack_with_segments(&mut meta, 1, "B-empty", StackState::InWorkspace, &["B"]);

    let (id, ref_name) = id_at(&repo, "A-empty-01");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►A
    │   └── :2:►A-empty-01
    │       └── 👉:0:►origin/B
    │           ├── ·70bde6b (⌂|🏘|101) ►A-empty-02, ►A-empty-03
    │           └── :4:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|✓|111) ►new-A, ►new-B
    ├── :3:►gitbutler/workspace
    │   ├── ·20de6ee (⌂|🏘)
    │   └── :7:►B
    │       ├── ·70e9a36 (⌂|🏘|100)
    │       ├── ·320e105 (⌂|🏘|100) ►without-ref
    │       └── :6:►B-empty
    │           ├── ·2a31450 (⌂|🏘|100) ►ambiguous-01
    │           └── →:0:►origin/B
    └── :5:►origin/main
        └── →:4:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:4:B <> origin/B⇡2 on fafd9d0 {1}
        ├── 📙:4:B <> origin/B⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:3:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        └── 👉📙:0:A-empty-01
            └── ❄70bde6b (🏘️) ►A-empty-02, ►A-empty-03
    ");

    add_stack_with_segments(&mut meta, 2, "new-A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "new-B", StackState::InWorkspace, &[]);

    let (id, ref_name) = id_at(&repo, "new-A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;

    // Empty stacks summoned from branches resting on the base fold away here — even one set as the
    // entrypoint — because the base is not a workspace-commit parent. Only the B stack remains.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:4:B <> origin/B⇡2 on fafd9d0 {1}
        ├── 📙:4:B <> origin/B⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:3:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        └── 📙:8:A-empty-01
            └── ❄70bde6b (🏘️) ►A-empty-02, ►A-empty-03
    ");
    Ok(())
}

#[test]
fn single_stack() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2c12d75 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 320e105 (B) segment-B
    * 2a31450 (B-sub) segment-B~1
    * 70bde6b (A) segment-A
    * fafd9d0 (origin/main, new-A, main) init
    ");

    // Just a workspace, no additional ref information.
    // It segments across the unambiguous ref names.
    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·2c12d75 (⌂|🏘|1)
    │   └── :3:►B
    │       ├── ·320e105 (⌂|🏘|1)
    │       └── :4:►B-sub
    │           ├── ·2a31450 (⌂|🏘|1)
    │           └── :5:►A
    │               ├── ·70bde6b (⌂|🏘|1)
    │               └── :1:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|11) ►new-A
    └── :2:►origin/main
        └── →:1:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:2:B on fafd9d0
        ├── :2:B
        │   └── ·320e105 (🏘️)
        ├── :3:B-sub
        │   └── ·2a31450 (🏘️)
        └── :4:A
            └── ·70bde6b (🏘️)
    ");

    meta.data_mut().branches.clear();
    // Repeat the existing segment verbatim and add a new unborn stack on the base — which folds
    // away (the base is not a workspace-commit parent), leaving only B.
    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["B-sub", "A"]);
    add_stack_with_segments(
        &mut meta,
        1,
        "new-A",
        StackState::InWorkspace,
        &["below-new-A"],
    );

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·2c12d75 (⌂|🏘|1)
    │   └── :4:►B
    │       ├── ·320e105 (⌂|🏘|1)
    │       └── :5:►B-sub
    │           ├── ·2a31450 (⌂|🏘|1)
    │           └── :6:►A
    │               ├── ·70bde6b (⌂|🏘|1)
    │               └── :1:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    ├── :2:►new-A
    │   └── →:1:►main
    └── :3:►origin/main
        └── →:1:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:2:B on fafd9d0 {0}
        ├── 📙:2:B
        │   └── ·320e105 (🏘️)
        ├── 📙:3:B-sub
        │   └── ·2a31450 (🏘️)
        └── 📙:4:A
            └── ·70bde6b (🏘️)
    ");

    Ok(())
}

// VALIDATION (Phase-2 repeated-parent model): an octopus whose parents are [S-tip, base, base]
// — base listed twice, once per fully-empty stack. Confirms the projection surfaces N distinct
// empty stacks when the base is a (repeated) direct ws-commit parent.
#[test]
fn repeated_base_parent_two_empty_stacks() -> anyhow::Result<()> {
    let (repo, mut meta) =
        named_read_only_in_memory_scenario("repeated-parents", "two-empty-at-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c08c631 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | 19536bb (S) s1
    |/  
    * fafd9d0 (origin/main, main, empty-B, empty-A) init
    ");

    add_stack_with_segments(&mut meta, 0, "S", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "empty-A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "empty-B", StackState::InWorkspace, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·c08c631 (⌂|🏘|1)
    │   ├── :5:►S
    │   │   ├── ·19536bb (⌂|🏘|1)
    │   │   └── :1:►main
    │   │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    │   ├── :3:►empty-B
    │   │   └── →:1:►main
    │   └── :2:►empty-A
    │       └── →:1:►main
    └── :4:►origin/main
        └── →:1:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:2:S on fafd9d0 {0}
    │   └── 📙:2:S
    │       └── ·19536bb (🏘️)
    ├── ≡📙:4:empty-A on fafd9d0 {1}
    │   └── 📙:4:empty-A
    └── ≡📙:5:empty-B on fafd9d0 {2}
        └── 📙:5:empty-B
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;

    // By default, everything with metadata on the branch will show up, even if on the base.
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    └── ≡📙:2:C on 0cc5a6f {0}
        ├── 📙:2:C
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

    let graph = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Default::default())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    └── ≡📙:2:C {0}
        └── 📙:2:C
            └── ·c6d714c (🏘️)
    ");

    // When 'merge' is instead an independent empty stack on the base, it folds away: an empty stack
    // projects only as a workspace-commit parent, and the base is not one here. Only `C` remains.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "merge", StackState::InWorkspace, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    └── ≡📙:2:C on 0cc5a6f {0}
        └── 📙:2:C
            └── ·c6d714c (🏘️)
    ");

    // Order metadata is honored — C lands at {1} because 'merge' is ordered before it — even though
    // the independent empty 'merge' on the base still folds, leaving only C.
    add_stack_with_segments(&mut meta, 1, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 0, "merge", StackState::InWorkspace, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
    └── ≡📙:2:C on 0cc5a6f {1}
        └── 📙:2:C
            └── ·c6d714c (🏘️)
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── 👉:0:►gitbutler/workspace
        ├── ·47e1cf1 (⌂|1)
        └── :1:►anon:
            ├── ·f40fb16 (⌂|1)
            ├── :2:►anon:
            │   ├── ·450c58a (⌂|1)
            │   └── :4:►anon:
            │       ├── ·0cc5a6f (⌂|1)
            │       ├── :5:►anon:
            │       │   ├── ·7fdb58d (⌂|1)
            │       │   └── :7:►anon:
            │       │       └── 🏁·fafd9d0 (⌂|1)
            │       └── :6:►anon:
            │           ├── ·e255adc (⌂|1)
            │           └── →:7:►anon:
            └── :3:►anon:
                ├── ·c6d714c (⌂|1)
                └── →:4:►anon:
    ");

    // This a very untypical setup, but it's not forbidden. Code might want to check
    // if the workspace commit is actually managed before proceeding.
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►entrypoint
    │   ├── ·98c5aba (⌂|1)
    │   ├── ·807b6ce (⌂|1)
    │   ├── ·6d05486 (⌂|1)
    │   └── :3:►anon:
    │       ├── ·b688f2d (⌂|🏘|1)
    │       └── 🏁·fafd9d0 (⌂|🏘|1)
    └── :1:►gitbutler/workspace
        ├── ·b6917c7 (⌂|🏘)
        └── :2:►main
            ├── ·f7fe830 (⌂|🏘)
            └── →:3:►anon:
    ");
    // This is an unmanaged workspace, even though commits from a workspace flow into it.
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·47e1cf1 (⌂|1)
    │   └── :1:►merge-2
    │       ├── ·f40fb16 (⌂|1)
    │       ├── :2:►D
    │       │   ├── ·450c58a (⌂|1)
    │       │   └── :4:►anon:
    │       │       ├── ·0cc5a6f (⌂|1) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    │       │       ├── :5:►B
    │       │       │   ├── ·7fdb58d (⌂|1)
    │       │       │   └── :7:►main
    │       │       │       └── 🏁·fafd9d0 (⌂|11)
    │       │       └── :6:►A
    │       │           ├── ·e255adc (⌂|1)
    │       │           └── →:7:►main
    │       └── :3:►C
    │           ├── ·c6d714c (⌂|1)
    │           └── →:4:►anon:
    └── :8:►origin/main
        └── →:7:►main
    ");

    // Without workspace data this becomes a single-branch workspace, with `main` as normal segment.
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
        └── :7:main <> origin/main
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·47e1cf1 (⌂|🏘|1)
    │   └── :9:►merge-2
    │       ├── ·f40fb16 (⌂|🏘|1)
    │       ├── :10:►D
    │       │   ├── ·450c58a (⌂|🏘|1)
    │       │   └── :3:►anon:
    │       │       ├── ·0cc5a6f (⌂|🏘|1)
    │       │       ├── :7:►B
    │       │       │   ├── ·7fdb58d (⌂|🏘|1)
    │       │       │   └── :1:►main
    │       │       │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    │       │       └── :8:►A
    │       │           ├── ·e255adc (⌂|🏘|1)
    │       │           └── →:1:►main
    │       └── :11:►C
    │           ├── ·c6d714c (⌂|🏘|1)
    │           └── →:3:►anon:
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :4:►empty-2-on-merge
        └── :5:►empty-1-on-merge
            └── :6:►merge
                └── →:3:►anon:
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:5:merge-2 on fafd9d0 {0}
        ├── :5:merge-2
        │   └── ·f40fb16 (🏘️)
        ├── :6:D
        │   └── ·450c58a (🏘️)
        ├── 📙:9:empty-2-on-merge
        ├── 📙:10:empty-1-on-merge
        ├── 📙:11:merge
        │   └── ·0cc5a6f (🏘️)
        └── :3:B
            └── ·7fdb58d (🏘️)
    ");
    Ok(())
}

#[test]
fn entrypoint_inside_second_parent_of_workspace_diamond_is_included() -> anyhow::Result<()> {
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
    add_workspace(&mut meta);
    let (id, name) = id_at(&repo, "C");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·47e1cf1 (⌂|🏘)
    │   └── :5:►merge-2
    │       ├── ·f40fb16 (⌂|🏘)
    │       ├── :8:►D
    │       │   ├── ·450c58a (⌂|🏘)
    │       │   └── :4:►anon:
    │       │       ├── ·0cc5a6f (⌂|🏘|1) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    │       │       ├── :6:►B
    │       │       │   ├── ·7fdb58d (⌂|🏘|1)
    │       │       │   └── :2:►main
    │       │       │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    │       │       └── :7:►A
    │       │           ├── ·e255adc (⌂|🏘|1)
    │       │           └── →:2:►main
    │       └── 👉:0:►C
    │           ├── ·c6d714c (⌂|🏘|1)
    │           └── →:4:►anon:
    └── :3:►origin/main
        └── →:2:►main
    ");

    let ws = graph;
    let entrypoint_stack_segment = ws
        .stacks
        .iter()
        .flat_map(|stack| stack.segments.iter())
        .find(|segment| segment.is_entrypoint)
        .expect("entrypoint segment must stay in a workspace stack");
    assert!(
        entrypoint_stack_segment
            .commits
            .iter()
            .any(|commit| commit.id == id.detach()),
        "the entrypoint stack segment must contain the custom traversal commit"
    );
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:merge-2 on fafd9d0
        ├── :4:merge-2
        │   └── ·f40fb16 (🏘️)
        ├── 👉:0:C
        │   ├── ·c6d714c (🏘️)
        │   └── ·0cc5a6f (🏘️) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
        └── :5:B
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        extra_target_options.clone(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►A
    │   └── :0:►gitbutler/workspace
    │       └── 🏁·fafd9d0 (⌂|🏘|1) ►main
    └── :2:►B
        └── →:0:►gitbutler/workspace
    ");
    let ws = graph;
    assert_eq!(
        ws.tip_commit().map(|commit| commit.id),
        extra_target_options.extra_target_commit_id,
        "workspace query falls back to the ref-info commit for ambiguous empty segments"
    );
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on fafd9d0
    ├── ≡📙:2:A on fafd9d0 {1}
    │   └── 📙:2:A
    └── ≡📙:3:B on fafd9d0 {2}
        └── 📙:3:B
    ");

    let (id, ref_name) = id_at(&repo, "B");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        extra_target_options.clone(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►A
    │   └── 👉:0:►gitbutler/workspace
    │       └── 🏁·fafd9d0 (⌂|🏘|1) ►main
    └── :2:►B
        └── →:0:►gitbutler/workspace
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on fafd9d0
    ├── ≡📙:2:A on fafd9d0 {1}
    │   └── 📙:2:A
    └── ≡👉📙:3:B on fafd9d0 {2}
        └── 👉📙:3:B
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        extra_target_options,
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►A
    │   └── 👉:0:►gitbutler/workspace
    │       └── 🏁·fafd9d0 (⌂|🏘|1) ►main
    └── :2:►B
        └── →:0:►gitbutler/workspace
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓! on fafd9d0
    ├── ≡👉📙:2:A on fafd9d0 {1}
    │   └── 👉📙:2:A
    └── ≡📙:3:B on fafd9d0 {2}
        └── 📙:3:B
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/main
    │   └── 👉:0:►main
    │       └── 🏁·fafd9d0 (⌂|🏘|✓|1) ►A, ►B, ►C, ►D, ►E, ►F
    └── :2:►gitbutler/workspace
        └── →:0:►main
    ");

    // There is no workspace as `main` is the base of the workspace, so it's shown directly,
    // outside the workspace.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:main[🌳] <> ✓refs/remotes/origin/main
    └── ≡:0:main[🌳] <> origin/main {1}
        └── :0:main[🌳] <> origin/main
    ");

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/main
    │   └── :0:►main
    │       └── 🏁·fafd9d0 (⌂|🏘|1) ►A, ►B, ►C, ►D, ►E, ►F
    └── 👉:2:►gitbutler/workspace
        └── →:0:►main
    ");

    // However, when the workspace is checked out, it's at least empty.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓!
    └── ≡:0:main[🌳] <> origin/main
        └── :0:main[🌳] <> origin/main
            └── ❄️fafd9d0 (🏘️) ►A, ►B, ►C, ►D, ►E, ►F
    ");

    // The simplest possible setup where we can define how the workspace should look like,
    // in terms of dependent and independent virtual segments.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►C
    │   └── :2:►B
    │       └── :3:►A
    │           └── 👉:0:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|1)
    ├── :4:►D
    │   └── :5:►E
    │       └── :6:►F
    │           └── →:0:►main
    ├── :7:►origin/main
    │   └── →:0:►main
    └── :8:►gitbutler/workspace
        └── →:0:►main
    ");

    // With empty project metadata, workspace segmentation is retained around the workspace ref.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓! on fafd9d0
    ├── ≡📙:3:C on fafd9d0 {0}
    │   ├── 📙:3:C
    │   ├── 📙:4:B
    │   └── 📙:5:A
    └── ≡📙:6:D on fafd9d0 {1}
        ├── 📙:6:D
        ├── 📙:7:E
        └── 📙:8:F
    ");

    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // Now the dependent segments are applied, and so is the separate stack.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►C
    │   └── :2:►B
    │       └── :3:►A
    │           └── :0:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|✓|1)
    ├── :4:►D
    │   └── :5:►E
    │       └── :6:►F
    │           └── →:0:►main
    ├── :7:►origin/main
    │   └── →:0:►main
    └── 👉:8:►gitbutler/workspace
        └── →:0:►main
    ");

    let mut ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:C on fafd9d0 {0}
    │   ├── 📙:3:C
    │   ├── 📙:4:B
    │   └── 📙:5:A
    └── ≡📙:6:D on fafd9d0 {1}
        ├── 📙:6:D
        ├── 📙:7:E
        └── 📙:8:F
    ");

    // Anonymization feeds support bundles, so every real name must be replaced. Here the workspace
    // (`gitbutler/workspace` → `H`) and target (`origin/main` → `remote-0/G`) prove it ran, and the
    // eight names `A`–`H` are all distinct (the mapping is injective). The stack branches read back
    // as `A`–`F` only because this fixture literally names them that. Unlike the former graph-based
    // path, the target/remote are anonymized rather than dropped — more useful, still leak-free.
    ws.anonymize(&repo.remote_names())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:1:H <> ✓refs/remotes/remote-0/G on fafd9d0
    ├── ≡📙:3:C on fafd9d0 {0}
    │   ├── 📙:3:C
    │   ├── 📙:4:B
    │   └── 📙:5:A
    └── ≡📙:6:D on fafd9d0 {1}
        ├── 📙:6:D
        ├── 📙:7:E
        └── 📙:8:F
    ");

    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ws_ref_name,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Options {
            dangerously_skip_postprocessing_for_debugging: true,
            ..standard_options()
        },
    )?;
    // Show how the lack of post-processing affects the graph - remotes are also not connected.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►C
    │   └── :2:►B
    │       └── :3:►A
    │           └── :0:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|✓|1)
    ├── :4:►D
    │   └── :5:►E
    │       └── :6:►F
    │           └── →:0:►main
    ├── :7:►origin/main
    │   └── →:0:►main
    └── 👉:8:►gitbutler/workspace
        └── →:0:►main
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0");

    Ok(())
}

#[test]
fn tips_equivalent_to_workspace_metadata_are_order_independent() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init");

    add_workspace(&mut meta);
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let commit_id = id.detach();
    let workspace_metadata = (*meta.workspace(ws_ref_name.as_ref())?).clone();
    let main_ref = super::ref_name("refs/heads/main");
    let origin_main_ref = super::ref_name("refs/remotes/origin/main");
    let stack_ref = |name: &str| super::ref_name(&format!("refs/heads/{name}"));

    let head_baseline =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    let head_baseline_tree = branch_tree(&head_baseline).to_string();
    let head_baseline_workspace = graph_workspace(&head_baseline).to_string();

    let head_tips = vec![
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("F"),
        }),
        Tip::new(commit_id)
            .with_ref_name(Some(ws_ref_name.clone()))
            .with_role(TipRole::Workspace)
            .with_metadata(SegmentMetadata::Workspace(workspace_metadata.clone())),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("B"),
        }),
        Tip::new(commit_id)
            .with_ref_name(Some(origin_main_ref.clone()))
            .with_role(TipRole::TargetRemote),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("A"),
        }),
        Tip::new(commit_id)
            .with_ref_name(Some(main_ref.clone()))
            .with_entrypoint(),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("E"),
        }),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("C"),
        }),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("D"),
        }),
    ];

    let workspace_baseline = but_graph::Workspace::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    let workspace_baseline_tree = branch_tree(&workspace_baseline).to_string();
    let workspace_baseline_workspace = graph_workspace(&workspace_baseline);
    insta::assert_snapshot!(workspace_baseline_workspace, @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:C on fafd9d0 {0}
    │   ├── 📙:3:C
    │   ├── 📙:4:B
    │   └── 📙:5:A
    └── ≡📙:6:D on fafd9d0 {1}
        ├── 📙:6:D
        ├── 📙:7:E
        └── 📙:8:F
    ");
    let workspace_baseline_workspace = workspace_baseline_workspace.to_string();

    let explicit_tips = vec![
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("E"),
        }),
        Tip::new(commit_id).with_role(TipRole::TargetLocal {
            local_ref_name: main_ref.clone(),
        }),
        Tip::new(commit_id)
            .with_ref_name(Some(ws_ref_name.clone()))
            .with_role(TipRole::Workspace)
            .with_metadata(SegmentMetadata::Workspace(workspace_metadata))
            .with_entrypoint(),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("C"),
        }),
        Tip::new(commit_id)
            .with_ref_name(Some(origin_main_ref))
            .with_role(TipRole::TargetRemote),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("F"),
        }),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("A"),
        }),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("D"),
        }),
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("B"),
        }),
    ];
    let graph = but_graph::Workspace::from_commit_traversal_tips(
        &repo,
        head_tips,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    assert_eq!(
        branch_tree(&graph).to_string(),
        head_baseline_tree,
        "unordered explicit tips with a reachable entrypoint should match HEAD traversal"
    );
    assert_eq!(
        graph_workspace(&graph).to_string(),
        head_baseline_workspace,
        "unordered explicit tips with a reachable entrypoint should match the HEAD workspace projection"
    );

    let graph = but_graph::Workspace::from_commit_traversal_tips(
        &repo,
        explicit_tips.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    assert_eq!(
        branch_tree(&graph).to_string(),
        workspace_baseline_tree,
        "unordered explicit tips should create the same graph as workspace metadata traversal"
    );
    let explicit_workspace = graph_workspace(&graph);
    insta::assert_snapshot!(explicit_workspace, @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:C on fafd9d0 {0}
    │   ├── 📙:3:C
    │   ├── 📙:4:B
    │   └── 📙:5:A
    └── ≡📙:6:D on fafd9d0 {1}
        ├── 📙:6:D
        ├── 📙:7:E
        └── 📙:8:F
    ");
    assert_eq!(
        explicit_workspace.to_string(),
        workspace_baseline_workspace,
        "unordered explicit tips should create the same workspace projection as workspace metadata traversal"
    );

    Ok(())
}

#[test]
fn workspace_target_commit_and_extra_target_commit_can_overlap() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-two-branches")?;
    let target_id = id_by_rev(&repo, "main").detach();
    add_workspace_with_target(&mut meta, target_id);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let baseline =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    let baseline_tree = branch_tree(&baseline).to_string();
    let baseline_workspace = graph_workspace(&baseline).to_string();

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(target_id),
    )?;

    assert_eq!(
        branch_tree(&graph).to_string(),
        baseline_tree,
        "duplicated synthetic integrated tips should not change graph traversal"
    );
    assert_eq!(
        graph_workspace(&graph).to_string(),
        baseline_workspace,
        "duplicated synthetic integrated tips should not change workspace projection"
    );

    Ok(())
}

#[test]
fn duplicate_workspace_stack_branch_tips_from_metadata_are_ignored() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-two-branches")?;
    add_workspace(&mut meta);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let baseline =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    let baseline_workspace = graph_workspace(&baseline).to_string();

    add_stack_with_segments(&mut meta, 3, "B", StackState::InWorkspace, &[]);
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;

    // The raw BranchGraph carries the duplicate routing node; the de-duplication that matters is
    // in the workspace projection, asserted here. (The old record-graph view de-duped during
    // minting, which masked this — branch_tree renders the canonical BranchGraph directly.)
    assert_eq!(
        graph_workspace(&graph).to_string(),
        baseline_workspace,
        "duplicate stack branch metadata should not change workspace projection"
    );

    Ok(())
}

#[test]
fn just_init_with_archived_branches() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    // Note the dedicated workspace branch without a workspace commit.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init");

    let stack_id = add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;

    // By default, we see both stacks as they are configured, which disambiguates them.
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    let graph = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Default::default())?;
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:C {0}
        └── 📙:3:C
    ");

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = false;

    // Now only the first one is archived.
    let graph = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Default::default())?;
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:C {0}
        ├── 📙:3:C
        └── 📙:4:B
    ");

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = true;
    heads[2].archived = true;

    // Archiving everything removes the stack entirely.
    let graph = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Default::default())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0");
    Ok(())
}

#[test]
fn two_stacks_many_refs() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/one-stacks-many-refs")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 298d938 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 16f132b (S1, G, F) 2
    * 917b9da (E, D) 1
    * fafd9d0 (origin/main, main, C, B, A) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    // Without any information it looks quite barren.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·298d938 (⌂|🏘|1)
    │   ├── ·16f132b (⌂|🏘|1) ►F, ►G, ►S1
    │   ├── ·917b9da (⌂|🏘|1) ►D, ►E
    │   └── :1:►main
    │       └── 🏁·fafd9d0 (⌂|🏘|✓|11) ►A, ►B, ►C
    └── :2:►origin/main
        └── →:1:►main
    ");

    // With no workspace at all as the workspace segment isn't split.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:anon: on fafd9d0
        └── :3:anon:
            ├── ·16f132b (🏘️) ►F, ►G, ►S1
            └── ·917b9da (🏘️) ►D, ►E
    ");

    let (id, ref_name) = id_at(&repo, "S1");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // The S1 starting position is a split, so there is more.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·298d938 (⌂|🏘)
    │   └── 👉:0:►S1
    │       ├── ·16f132b (⌂|🏘|1) ►F, ►G
    │       ├── ·917b9da (⌂|🏘|1) ►D, ►E
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|11) ►A, ►B, ►C
    └── :3:►origin/main
        └── →:2:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·298d938 (⌂|🏘|1)
    │   └── :7:►S1
    │       └── :8:►G
    │           └── :9:►F
    │               └── :6:►anon:
    │                   ├── ·16f132b (⌂|🏘|1)
    │                   └── :10:►anon:
    │                       ├── ·917b9da (⌂|🏘|1)
    │                       └── :1:►main
    │                           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    ├── :2:►C
    │   └── :3:►B
    │       └── →:1:►main
    ├── :4:►A
    │   └── →:1:►main
    ├── :5:►origin/main
    │   └── →:1:►main
    └── :11:►D
        └── :12:►E
            └── →:10:►anon:
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:S1 on fafd9d0 {3}
        ├── 📙:5:S1
        ├── 📙:6:G
        ├── 📙:2:F
        │   └── ·16f132b (🏘️)
        └── 📙:8:E
            └── ·917b9da (🏘️)
    ");

    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // This should look the same as before, despite the starting position.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►G
    │   └── :2:►F
    │       └── 👉:0:►S1
    │           ├── ·16f132b (⌂|🏘|1)
    │           └── :10:►D
    │               └── :11:►E
    │                   └── :9:►anon:
    │                       ├── ·917b9da (⌂|🏘|1)
    │                       └── :4:►main
    │                           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    ├── :3:►gitbutler/workspace
    │   ├── ·298d938 (⌂|🏘)
    │   └── →:0:►S1
    ├── :5:►C
    │   └── :6:►B
    │       └── →:4:►main
    ├── :7:►A
    │   └── →:4:►main
    └── :8:►origin/main
        └── →:4:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡👉📙:5:S1 on fafd9d0 {3}
        ├── 👉📙:5:S1
        ├── 📙:6:G
        ├── 📙:0:F
        │   └── ·16f132b (🏘️)
        └── 📙:8:E
            └── ·917b9da (🏘️)
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►C
    │   └── :2:►B
    │       └── :0:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|1)
    ├── :3:►A
    │   └── →:0:►main
    ├── :4:►D
    │   └── :5:►E
    │       └── →:0:►main
    ├── :6:►F
    │   └── →:0:►main
    ├── :7:►origin/main
    │   └── →:0:►main
    └── 👉:8:►gitbutler/workspace
        └── →:0:►main
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:C on fafd9d0 {0}
    │   ├── 📙:3:C
    │   └── 📙:4:B
    ├── ≡📙:5:A on fafd9d0 {1}
    │   └── 📙:5:A
    ├── ≡📙:6:D on fafd9d0 {2}
    │   ├── 📙:6:D
    │   └── 📙:7:E
    └── ≡📙:8:F on fafd9d0 {3}
        └── 📙:8:F
    ");

    let (id, ref_name) = id_at(&repo, "C");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // The entrypoint shouldn't affect the outcome (even though it changes the initial segmentation).
    // However, as the segment it's on is integrated, it's not considered to be part of the workspace.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►C
    │   └── :2:►B
    │       └── 👉:0:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|1)
    ├── :3:►A
    │   └── →:0:►main
    ├── :4:►D
    │   └── :5:►E
    │       └── →:0:►main
    ├── :6:►F
    │   └── →:0:►main
    ├── :7:►origin/main
    │   └── →:0:►main
    └── :8:►gitbutler/workspace
        └── →:0:►main
    ");

    // We should see the same stacks as we did before, just with a different entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡👉📙:3:C on fafd9d0 {0}
    │   ├── 👉📙:3:C
    │   └── 📙:4:B
    ├── ≡📙:5:A on fafd9d0 {1}
    │   └── 📙:5:A
    ├── ≡📙:6:D on fafd9d0 {2}
    │   ├── 📙:6:D
    │   └── 📙:7:E
    └── ≡📙:8:F on fafd9d0 {3}
        └── 📙:8:F
    ");
    Ok(())
}

#[test]
fn proper_remote_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/proper-remote-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 9bcd3af (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * ca7baa7 (origin/main) only-remote-02
    | * 7ea1468 only-remote-01
    |/  
    * 998eae6 (main) shared
    * fafd9d0 init
    ");

    // Remote segments are picked up automatically and traversed - they never take ownership of already assigned commits.
    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·9bcd3af (⌂|🏘|1)
    │   └── :2:►main
    │       ├── ·998eae6 (⌂|🏘|✓|11)
    │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    └── :1:►origin/main
        ├── ·ca7baa7 (✓)
        ├── ·7ea1468 (✓)
        └── →:2:►main
    ");

    // Everything in the workspace is integrated, thus it's empty.
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 998eae6");

    let (id, ref_name) = id_at(&repo, "main");
    // The integration branch can be in the workspace and be checked out.
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        Some(ref_name),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·9bcd3af (⌂|🏘)
    │   └── 👉:0:►main
    │       ├── ·998eae6 (⌂|🏘|✓|1)
    │       └── 🏁·fafd9d0 (⌂|🏘|✓|1)
    └── :2:►origin/main
        ├── ·ca7baa7 (✓)
        ├── ·7ea1468 (✓)
        └── →:0:►main
    ");

    // If it's checked out, we must show it, but it's not part of the workspace.
    // This is special as other segments still are.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:main <> ✓refs/remotes/origin/main⇣2
    └── ≡:0:main <> origin/main⇣2 {1}
        └── :0:main <> origin/main⇣2
            ├── 🟣ca7baa7 (✓)
            └── 🟣7ea1468 (✓)
    ");
    Ok(())
}

#[test]
fn deduced_remote_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/deduced-remote-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·8b39ce4 (⌂|🏘|1)
    │   └── :1:►A
    │       ├── ·9d34471 (⌂|🏘|11)
    │       ├── ·5b89c71 (⌂|🏘|11)
    │       └── :6:►anon:
    │           ├── ·998eae6 (⌂|🏘|111)
    │           └── :4:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|111)
    └── :3:►push-remote/A
        └── :2:►origin/A
            ├── ·3ea1a8f (0x0|100)
            ├── ·9c50f71 (0x0|100)
            └── :5:►anon:
                ├── ·2cfbb79 (0x0|100)
                ├── →:6:►anon:
                └── :7:►anon:
                    ├── ·e898cd0 (0x0|100)
                    └── →:6:►anon:
    ");
    // There is no target branch, so nothing is integrated, and `main` shows up.
    // It's not special.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> origin/A⇡2⇣4
        ├── :1:A <> origin/A⇡2⇣4
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·8b39ce4 (⌂|🏘)
    │   └── :2:►A
    │       ├── ·9d34471 (⌂|🏘|10)
    │       ├── ·5b89c71 (⌂|🏘|10)
    │       └── :6:►anon:
    │           ├── ·998eae6 (⌂|🏘|110)
    │           └── 👉:0:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|111)
    └── :4:►push-remote/A
        └── :3:►origin/A
            ├── ·3ea1a8f (0x0|100)
            ├── ·9c50f71 (0x0|100)
            └── :5:►anon:
                ├── ·2cfbb79 (0x0|100)
                ├── →:6:►anon:
                └── :7:►anon:
                    ├── ·e898cd0 (0x0|100)
                    └── →:6:►anon:
    ");
    // The whole workspace is visible, but it's clear where the entrypoint is.
    // As there is no target ref, `main` shows up.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡:2:A <> origin/A⇡2⇣4
        ├── :2:A <> origin/A⇡2⇣4
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
    let mut ws = meta.workspace(WORKSPACE_REF_NAME.try_into().expect("valid workspace ref"))?;
    let mut pm = ws.project_meta();
    pm.push_remote = Some("push-remote".into());
    ws.set_project_meta(pm);
    meta.set_workspace(&ws)?;
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·8b39ce4 (⌂|🏘|1)
    │   └── :1:►A
    │       ├── ·9d34471 (⌂|🏘|11)
    │       ├── ·5b89c71 (⌂|🏘|11)
    │       └── :6:►anon:
    │           ├── ·998eae6 (⌂|🏘|111)
    │           └── :4:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|111)
    └── :3:►origin/A
        └── :2:►push-remote/A
            ├── ·3ea1a8f (0x0|100)
            ├── ·9c50f71 (0x0|100)
            └── :5:►anon:
                ├── ·2cfbb79 (0x0|100)
                ├── →:6:►anon:
                └── :7:►anon:
                    ├── ·e898cd0 (0x0|100)
                    └── →:6:►anon:
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> push-remote/A⇡2⇣4
        ├── :1:A <> push-remote/A⇡2⇣4
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·7786959 (⌂|🏘|1)
    │   └── :1:►B
    │       ├── ·312f819 (⌂|🏘|1)
    │       └── :2:►A
    │           ├── ·e255adc (⌂|🏘|1)
    │           └── :3:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|11)
    └── :4:►origin/main
        └── →:3:►main
    ");
    // It's worth noting that we avoid double-listing remote commits that are also
    // directly owned by another remote segment.
    // they have to be considered as something relevant to the branch history.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:B
        ├── :1:B
        │   └── ·312f819 (🏘️)
        ├── :2:A
        │   └── ·e255adc (🏘️)
        └── :3:main <> origin/main
            └── ❄️fafd9d0 (🏘️)
    ");

    // The result is the same when changing the entrypoint.
    let (id, name) = id_at(&repo, "A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·7786959 (⌂|🏘)
    │   └── :5:►B
    │       ├── ·312f819 (⌂|🏘|1000)
    │       └── 👉:0:►A
    │           ├── ·e255adc (⌂|🏘|1001)
    │           └── :2:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|✓|11111)
    ├── :3:►origin/main
    │   └── →:2:►main
    └── :6:►origin/B
        ├── ·682be32 (0x0|10000)
        └── :4:►origin/A
            ├── ·e29c23d (0x0|10100)
            └── →:2:►main
    ");
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:B <> origin/B⇡1⇣1 on fafd9d0
        ├── :4:B <> origin/B⇡1⇣1
        │   ├── 🟣682be32
        │   └── ·312f819 (🏘️)
        └── 👉:0:A <> origin/A⇡1⇣1
            ├── 🟣e29c23d
            └── ·e255adc (🏘️)
    ");
    Ok(())
}

#[test]
fn target_with_remote_on_stack_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-ahead-and-on-stack-tip")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * dd0cca8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e255adc (main, A) A
    * fafd9d0 (origin/main) init
    ");
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·dd0cca8 (⌂|🏘|1)
    │   └── :2:►A
    │       ├── ·e255adc (⌂|🏘|11)
    │       └── :1:►origin/main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    └── :3:►main
        └── →:2:►A
    ");

    // The main branch is not present, as it's the target.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:2:A on fafd9d0 {1}
        └── 📙:2:A
            └── ·e255adc (🏘️)
    ");

    // But mention it if it's in the workspace. It should retain order.
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["main"]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        ├── 📙:3:A
        └── 📙:2:main <> origin/main⇡1
            └── ·e255adc (🏘️)
    ");

    // But mention it if it's in the workspace. It should retain order - inverting the order is fine.
    add_stack_with_segments(&mut meta, 1, "main", StackState::InWorkspace, &["A"]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:main <> origin/main on fafd9d0 {1}
        ├── 📙:3:main <> origin/main
        └── 📙:2:A
            └── ·e255adc (🏘️)
    ");
    Ok(())
}

#[test]
fn disambiguate_by_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/disambiguate-by-remote")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·e30f90c (⌂|🏘|1)
    │   └── :4:►origin/C
    │       ├── ·2173153 (⌂|🏘|101) ►C, ►ambiguous-C
    │       └── :8:►B
    │           ├── ·312f819 (⌂|🏘|11101) ►ambiguous-B
    │           └── :6:►A
    │               ├── ·e255adc (⌂|🏘|111101) ►ambiguous-A
    │               └── :1:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|111111)
    ├── :2:►origin/main
    │   └── →:1:►main
    ├── :3:►origin/B
    │   ├── ·ac24e74 (0x0|10000)
    │   └── →:8:►B
    ├── :5:►origin/ambiguous-C
    │   └── →:4:►origin/C
    └── :7:►origin/A
        └── →:6:►A
    ");

    // An anonymous segment to start with is alright, and can always happen for other situations as well.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:anon: on fafd9d0
        ├── :3:anon:
        │   └── ·2173153 (🏘️) ►C, ►ambiguous-C
        ├── :5:B <> origin/B⇣1
        │   ├── 🟣ac24e74
        │   └── ❄️312f819 (🏘️) ►ambiguous-B
        └── :4:A <> origin/A
            └── ❄️e255adc (🏘️) ►ambiguous-A
    ");

    // If 'C' is in the workspace, it's naturally disambiguated.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·e30f90c (⌂|🏘|1)
    │   └── :3:►C
    │       ├── ·2173153 (⌂|🏘|101) ►ambiguous-C
    │       └── :9:►B
    │           ├── ·312f819 (⌂|🏘|11101) ►ambiguous-B
    │           └── :7:►A
    │               ├── ·e255adc (⌂|🏘|111101) ►ambiguous-A
    │               └── :1:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|111111)
    ├── :2:►origin/main
    │   └── →:1:►main
    ├── :4:►origin/C
    │   └── →:3:►C
    ├── :5:►origin/ambiguous-C
    │   └── →:3:►C
    ├── :6:►origin/B
    │   ├── ·ac24e74 (0x0|10000)
    │   └── →:9:►B
    └── :8:►origin/A
        └── →:7:►A
    ");
    // And because `C` is in the workspace data, its data is denoted.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:2:C <> origin/C on fafd9d0 {0}
        ├── 📙:2:C <> origin/C
        │   └── ❄️2173153 (🏘️) ►ambiguous-C
        ├── :5:B <> origin/B⇣1
        │   ├── 🟣ac24e74
        │   └── ❄️312f819 (🏘️) ►ambiguous-B
        └── :4:A <> origin/A
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── 👉:0:►gitbutler/workspace
        ├── ·4077353 (⌂|🏘|1)
        └── :1:►B
            ├── ·6b1a13b (⌂|🏘|1)
            ├── ·03ad472 (⌂|🏘|1)
            └── :2:►A
                ├── ·79bbb29 (⌂|🏘|1)
                ├── ·fc98174 (⌂|🏘|1)
                ├── ·a381df5 (⌂|🏘|1)
                ├── ·777b552 (⌂|🏘|1)
                └── :3:►anon:
                    ├── ·ce4a760 (⌂|🏘|1)
                    ├── :4:►anon:
                    │   ├── ·01d0e1e (⌂|🏘|1)
                    │   └── :6:►main
                    │       ├── ·4b3e5a8 (⌂|🏘|1)
                    │       ├── ·34d0715 (⌂|🏘|1)
                    │       └── 🏁·eb5f731 (⌂|🏘|1)
                    └── :5:►A-feat
                        ├── ·fea59b5 (⌂|🏘|1)
                        ├── ·4deea74 (⌂|🏘|1)
                        └── →:4:►anon:
    ");
    // It's true that `A` is fully integrated so it isn't displayed. so from a workspace-perspective
    // it's the right answer.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:B
        ├── :1:B
        │   ├── ·6b1a13b (🏘️)
        │   └── ·03ad472 (🏘️)
        ├── :2:A
        │   ├── ·79bbb29 (🏘️)
        │   ├── ·fc98174 (🏘️)
        │   ├── ·a381df5 (🏘️)
        │   ├── ·777b552 (🏘️)
        │   ├── ·ce4a760 (🏘️)
        │   └── ·01d0e1e (🏘️)
        └── :6:main
            ├── ·4b3e5a8 (🏘️)
            ├── ·34d0715 (🏘️)
            └── ·eb5f731 (🏘️)
    ");

    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["A"]);
    // ~~Now that `A` is part of the workspace, it's not cut off anymore.~~
    // This special handling was removed for now, relying on limits and extensions.
    // And since it's integrated, traversal is stopped without convergence.
    // We see more though as we add workspace segments immediately.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·4077353 (⌂|🏘|1)
    │   └── :2:►B
    │       ├── ·6b1a13b (⌂|🏘|1)
    │       ├── ·03ad472 (⌂|🏘|1)
    │       └── :3:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           ├── ·777b552 (⌂|🏘|✓|1)
    │           └── :6:►anon:
    │               └── ✂·ce4a760 (⌂|🏘|✓|1)
    └── :1:►origin/main
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── :4:►anon:
            ├── ·7b9f260 (✓)
            ├── :5:►main
            │   ├── ·4b3e5a8 (✓)
            │   ├── ·34d0715 (✓)
            │   └── 🏁·eb5f731 (✓)
            └── →:3:►A
    ");
    // `A` is integrated, hence it's not shown.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡📙:2:B on 79bbb29 {0}
        └── 📙:2:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    // The limit is effective for integrated workspaces branches, and it doesn't unnecessarily
    // prolong the traversal once the all tips are known to be integrated.
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·4077353 (⌂|🏘|1)
    │   └── :2:►B
    │       ├── ·6b1a13b (⌂|🏘|1)
    │       ├── ·03ad472 (⌂|🏘|1)
    │       └── :3:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           └── ✂·777b552 (⌂|🏘|✓|1)
    └── :1:►origin/main
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── :4:►anon:
            ├── ·7b9f260 (✓)
            ├── :5:►main
            │   ├── ·4b3e5a8 (✓)
            │   ├── ·34d0715 (✓)
            │   └── 🏁·eb5f731 (✓)
            └── →:3:►A
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·4077353 (⌂|🏘)
    │   └── :4:►B
    │       ├── ·6b1a13b (⌂|🏘)
    │       ├── ·03ad472 (⌂|🏘)
    │       └── 👉:0:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           ├── ·777b552 (⌂|🏘|✓|1)
    │           └── :6:►anon:
    │               ├── ·ce4a760 (⌂|🏘|✓|1)
    │               ├── :7:►anon:
    │               │   ├── ·01d0e1e (⌂|🏘|✓|1)
    │               │   └── :5:►main
    │               │       ├── ·4b3e5a8 (⌂|🏘|✓|1)
    │               │       ├── ·34d0715 (⌂|🏘|✓|1)
    │               │       └── 🏁·eb5f731 (⌂|🏘|✓|1)
    │               └── :8:►A-feat
    │                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                   ├── ·4deea74 (⌂|🏘|✓|1)
    │                   └── →:7:►anon:
    └── :2:►origin/main
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── :3:►anon:
            ├── ·7b9f260 (✓)
            ├── →:5:►main
            └── →:0:►A
    ");
    // It looks like some commits are missing, but it's a first-parent traversal.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:A <> ✓refs/remotes/origin/main⇣3
    └── ≡:0:A on 4b3e5a8 {1}
        └── :0:A
    ");

    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?;
    // It's still getting quite far despite the limit due to other heads searching for their goals,
    // but also ends traversal early.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·4077353 (⌂|🏘)
    │   └── :4:►B
    │       ├── ·6b1a13b (⌂|🏘)
    │       ├── ·03ad472 (⌂|🏘)
    │       └── 👉:0:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           └── ✂·777b552 (⌂|🏘|✓|1)
    └── :2:►origin/main
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── :3:►anon:
            ├── ·7b9f260 (✓)
            ├── :5:►main
            │   ├── ·4b3e5a8 (✓)
            │   ├── ·34d0715 (✓)
            │   └── 🏁·eb5f731 (✓)
            └── →:0:►A
    ");
    // Because the branch is integrated, the surrounding workspace isn't shown.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:A <> ✓refs/remotes/origin/main⇣6
    └── ≡:0:A {1}
        └── :0:A
    ");

    // See what happens with an out-of-workspace HEAD and an arbitrary extra target.
    let (id, _ref_name) = id_at(&repo, "origin/main");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "gitbutler/workspace"),
    )?;
    // It keeps the tip-settings of the workspace it setup by itself, and doesn't override this
    // with the extra-target settings.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►origin/main
    │   ├── ·d0df794 (⌂|✓|1)
    │   ├── ·09c6e08 (⌂|✓|1)
    │   └── :2:►anon:
    │       ├── ·7b9f260 (⌂|✓|1)
    │       ├── :4:►main
    │       │   ├── ·4b3e5a8 (⌂|🏘|✓|1)
    │       │   ├── ·34d0715 (⌂|🏘|✓|1)
    │       │   └── 🏁·eb5f731 (⌂|🏘|✓|1)
    │       └── :5:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           ├── ·777b552 (⌂|🏘|✓|1)
    │           └── :6:►anon:
    │               ├── ·ce4a760 (⌂|🏘|✓|1)
    │               ├── :7:►anon:
    │               │   ├── ·01d0e1e (⌂|🏘|✓|1)
    │               │   └── →:4:►main
    │               └── :8:►A-feat
    │                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                   ├── ·4deea74 (⌂|🏘|✓|1)
    │                   └── →:7:►anon:
    └── :1:►gitbutler/workspace
        ├── ·4077353 (⌂|🏘)
        └── :3:►B
            ├── ·6b1a13b (⌂|🏘)
            ├── ·03ad472 (⌂|🏘)
            └── →:5:►A
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:DETACHED <> ✓refs/remotes/origin/main⇣3 on 79bbb29
    └── ≡:0:anon: on 4b3e5a8 {1}
        └── :0:anon:
            ├── ·d0df794 (✓)
            ├── ·09c6e08 (✓)
            └── ·7b9f260 (✓)
    ");

    // However, when choosing an initially unknown branch, it will get the extra target tip settings.
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "B"),
    )?;
    // For now we don't do anything to limit the each in single-branch mode using extra-targets.
    // Thanks to the limit-transplant we get to discover more of the workspace.
    // TODO(extra-target): make it work so they limit single branches even, but it's a special case
    //                     as we can't have remotes here.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►origin/main
    │   ├── ·d0df794 (⌂|✓|1)
    │   ├── ·09c6e08 (⌂|✓|1)
    │   └── :3:►anon:
    │       ├── ·7b9f260 (⌂|✓|1)
    │       ├── :4:►main
    │       │   ├── ·4b3e5a8 (⌂|🏘|✓|1)
    │       │   ├── ·34d0715 (⌂|🏘|✓|1)
    │       │   └── 🏁·eb5f731 (⌂|🏘|✓|1)
    │       └── :5:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           ├── ·777b552 (⌂|🏘|✓|1)
    │           └── :6:►anon:
    │               ├── ·ce4a760 (⌂|🏘|✓|1)
    │               ├── :7:►anon:
    │               │   ├── ·01d0e1e (⌂|🏘|✓|1)
    │               │   └── →:4:►main
    │               └── :8:►A-feat
    │                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                   ├── ·4deea74 (⌂|🏘|✓|1)
    │                   └── →:7:►anon:
    └── :1:►gitbutler/workspace
        ├── ·4077353 (⌂|🏘)
        └── :2:►B
            ├── ·6b1a13b (⌂|🏘|✓)
            ├── ·03ad472 (⌂|🏘|✓)
            └── →:5:►A
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:DETACHED <> ✓refs/remotes/origin/main⇣3 on 79bbb29
    └── ≡:0:anon: on 4b3e5a8 {1}
        └── :0:anon:
            ├── ·d0df794 (✓)
            ├── ·09c6e08 (✓)
            └── ·7b9f260 (✓)
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·4077353 (⌂|🏘|1)
    │   └── :4:►B
    │       ├── ·6b1a13b (⌂|🏘|1)
    │       ├── ·03ad472 (⌂|🏘|1)
    │       └── :5:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           ├── ·777b552 (⌂|🏘|✓|1)
    │           └── :6:►anon:
    │               ├── ·ce4a760 (⌂|🏘|✓|1)
    │               ├── :7:►anon:
    │               │   ├── ·01d0e1e (⌂|🏘|✓|1)
    │               │   └── :2:►main
    │               │       ├── ·4b3e5a8 (⌂|🏘|✓|11)
    │               │       ├── ·34d0715 (⌂|🏘|✓|11)
    │               │       └── 🏁·eb5f731 (⌂|🏘|✓|11)
    │               └── :8:►A-feat
    │                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                   ├── ·4deea74 (⌂|🏘|✓|1)
    │                   └── →:7:►anon:
    └── :1:►origin/main
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── :3:►anon:
            ├── ·7b9f260 (✓)
            ├── →:2:►main
            └── →:5:►A
    ");

    // This search discovers the whole workspace, without the integrated one.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
    └── ≡:4:B on 79bbb29
        └── :4:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    // However, we can specify an additional/old target segment to show integrated portions as well.
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·4077353 (⌂|🏘)
    │   └── :5:►B
    │       ├── ·6b1a13b (⌂|🏘)
    │       ├── ·03ad472 (⌂|🏘)
    │       └── 👉:0:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           ├── ·a381df5 (⌂|🏘|✓|1)
    │           ├── ·777b552 (⌂|🏘|✓|1)
    │           └── :6:►anon:
    │               ├── ·ce4a760 (⌂|🏘|✓|1)
    │               ├── :7:►anon:
    │               │   ├── ·01d0e1e (⌂|🏘|✓|1)
    │               │   └── :3:►main
    │               │       ├── ·4b3e5a8 (⌂|🏘|✓|11)
    │               │       ├── ·34d0715 (⌂|🏘|✓|11)
    │               │       └── 🏁·eb5f731 (⌂|🏘|✓|11)
    │               └── :8:►A-feat
    │                   ├── ·fea59b5 (⌂|🏘|✓|1)
    │                   ├── ·4deea74 (⌂|🏘|✓|1)
    │                   └── →:7:►anon:
    └── :2:►origin/main
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── :4:►anon:
            ├── ·7b9f260 (✓)
            ├── →:3:►main
            └── →:0:►A
    ");

    // The entrypoint isn't contained in the workspace anymore, so it's standalone.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:A <> ✓refs/remotes/origin/main⇣3
    └── ≡:0:A on 4b3e5a8 {1}
        └── :0:A
    ");

    // When converting to a workspace, we are still aware of the workspace membership as long as
    // the lower bound of the workspace includes it.
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // When the branch is below the forkpoint, the workspace also isn't shown anymore.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:main <> ✓refs/remotes/origin/main⇣3
    └── ≡:0:main <> origin/main⇣3 {1}
        └── :0:main <> origin/main⇣3
            ├── 🟣d0df794 (✓)
            ├── 🟣09c6e08 (✓)
            └── 🟣7b9f260 (✓)
    ");

    let id = id_by_rev(&repo, "main~1");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // Detached states are also possible.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:DETACHED <> ✓refs/remotes/origin/main⇣3
    └── ≡:0:anon: {1}
        └── :0:anon:
    ");
    Ok(())
}

#[test]
fn workspace_without_target_can_see_remote() -> anyhow::Result<()> {
    let (mut repo, _) = read_only_in_memory_scenario("ws/main-with-remote-and-workspace-ref")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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

    let graph =
        but_graph::Workspace::from_head(&repo, &meta, project_meta(&meta), standard_options())?;
    // Main is a normal branch, and its remote is known.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/main
    │   ├── ·956a3de (⌂)
    │   └── 👉:0:►main
    │       └── 🏁·3183e43 (⌂|🏘|1)
    └── :2:►gitbutler/workspace
        └── →:0:►main
    ");

    let ws = graph;
    // The workspace shows the remote commit, there is nothing special about the target.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:2:gitbutler/workspace <> ✓!
    └── ≡👉📙:0:main[🌳] <> origin/main⇡1 {0}
        └── 👉📙:0:main[🌳] <> origin/main⇡1
            └── ·3183e43 (🏘️)
    ");

    // If the remote isn't setup officially, deduction still works as we find
    // symbolic remote names for deduction in workspace ref names as well.
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));
    let graph = ws.redo_traversal_into_workspace_with_overlay(&repo, &meta, Overlay::default())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/main
    │   ├── ·956a3de (⌂)
    │   └── 👉:0:►main
    │       └── 🏁·3183e43 (⌂|🏘|1)
    └── :2:►gitbutler/workspace
        └── →:0:►main
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace <> ✓!
    └── ≡👉📙:0:main[🌳] <> origin/main⇡1 {0}
        └── 👉📙:0:main[🌳] <> origin/main⇡1
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(0),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── 👉:0:►gitbutler/workspace
        └── ✂·4077353 (⌂|🏘|1)
    ");
    // The commit in the workspace branch is always ignored and is expected to be the workspace merge commit.
    // So nothing to show here.
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓!");

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    assert!(
        meta.data_mut().default_target.is_some(),
        "But with workspace and target, we see everything"
    );
    // It's notable that there is no way to bypass the early abort when everything is integrated.
    // and there is no deductible remote relationship between origin/main and main (no remote not configured).
    // Then the traversal ends on integrated branches as `main` isn't a target.
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(0),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·4077353 (⌂|🏘|1)
    │   └── :3:►B
    │       ├── ·6b1a13b (⌂|🏘|1)
    │       ├── ·03ad472 (⌂|🏘|1)
    │       └── :5:►A
    │           ├── ·79bbb29 (⌂|🏘|✓|1)
    │           ├── ·fc98174 (⌂|🏘|✓|1)
    │           └── ✂·a381df5 (⌂|🏘|✓|1)
    └── :1:►origin/main
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── :2:►anon:
            ├── ·7b9f260 (✓)
            ├── :4:►main
            │   ├── ·4b3e5a8 (✓)
            │   ├── ·34d0715 (✓)
            │   └── 🏁·eb5f731 (✓)
            └── →:5:►A
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cbc6713 (origin/advanced-lane, on-top-of-dependent, dependent, advanced-lane) change
    * fafd9d0 (origin/main, main, lane) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·f8f33a7 (⌂|🏘|1)
    │   └── :3:►advanced-lane
    │       ├── ·cbc6713 (⌂|🏘|101) ►dependent, ►on-top-of-dependent
    │       └── :1:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|111) ►lane
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :4:►origin/advanced-lane
        └── →:3:►advanced-lane
    ");

    // By default, the advanced lane is simply frozen as its remote contains the commit.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:2:advanced-lane <> origin/advanced-lane on fafd9d0
        └── :2:advanced-lane <> origin/advanced-lane
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·f8f33a7 (⌂|🏘|1)
    │   └── :3:►dependent
    │       └── :4:►advanced-lane
    │           ├── ·cbc6713 (⌂|🏘|101) ►on-top-of-dependent
    │           └── :1:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|✓|111) ►lane
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :5:►origin/advanced-lane
        └── →:3:►dependent
    ");

    // When putting the dependent branch on top as empty segment, the frozen state is retained.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:dependent on fafd9d0 {1}
        ├── 📙:5:dependent
        └── 📙:2:advanced-lane <> origin/advanced-lane
            └── ❄️cbc6713 (🏘️) ►on-top-of-dependent
    ");
    Ok(())
}

#[test]
fn on_top_of_target_with_history() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/on-top-of-target-with-history")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2cde30a (HEAD -> gitbutler/workspace, origin/main, F, E, D, C, B, A) 5
    * 1c938f4 4
    * b82769f 3
    * 988032f 2
    * cd5b655 1
    * 2be54cd (main) outdated-main
    ");

    add_workspace(&mut meta);
    // It sees the entire history as it had to find `main`.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── 👉:2:►gitbutler/workspace
        └── :0:►origin/main
            ├── ·2cde30a (⌂|🏘|✓|1) ►A, ►B, ►C, ►D, ►E, ►F
            ├── ·1c938f4 (⌂|🏘|✓|1)
            ├── ·b82769f (⌂|🏘|✓|1)
            ├── ·988032f (⌂|🏘|✓|1)
            ├── ·cd5b655 (⌂|🏘|✓|1)
            └── :1:►main
                └── 🏁·2be54cd (⌂|🏘|✓|11)
    ");
    // Workspace is empty as everything is integrated.
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a");

    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►C
    │   └── :2:►B
    │       └── :3:►A
    │           └── :0:►origin/main
    │               ├── ·2cde30a (⌂|🏘|✓|1)
    │               ├── ·1c938f4 (⌂|🏘|✓|1)
    │               ├── ·b82769f (⌂|🏘|✓|1)
    │               ├── ·988032f (⌂|🏘|✓|1)
    │               ├── ·cd5b655 (⌂|🏘|✓|1)
    │               └── :7:►main
    │                   └── 🏁·2be54cd (⌂|🏘|✓|11)
    ├── :4:►D
    │   └── :5:►E
    │       └── :6:►F
    │           └── →:0:►origin/main
    └── 👉:8:►gitbutler/workspace
        └── →:0:►origin/main
    ");

    // Empty stack segments on top of integrated portions will show, and nothing integrated shows.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a
    ├── ≡📙:3:C on 2cde30a {0}
    │   ├── 📙:3:C
    │   ├── 📙:4:B
    │   └── 📙:5:A
    └── ≡📙:6:D on 2cde30a {1}
        ├── 📙:6:D
        ├── 📙:7:E
        └── 📙:8:F
    ");

    // However, when passing an additional old position of the target, we can show the now-integrated parts.
    // The stacks will always be created on top of the integrated segments as that's where their references are
    // (these segments are never conjured up out of thin air).
    //
    // KNOWN DIVERGENCE from the deleted segment-graph projection: where two metadata stacks share
    // the same fully-integrated tip commit and the extra-target reveals that integrated history,
    // the direct projection keeps a single stack owning the commits and renders the second stack's
    // branches as commit refs, rather than duplicating the integrated history into both stacks.
    // This only manifests under extra-target inspection of fully-integrated stacks; the real
    // workflows (no extra-target, or stacks with distinct commits) are covered by the projection
    // oracles and match exactly.
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2be54cd
    └── ≡📙:3:C on 2be54cd {0}
        ├── 📙:3:C
        ├── 📙:4:B
        └── 📙:0:A
            ├── ·2cde30a (🏘️|✓) ►D, ►E, ►F
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
    let graph = but_graph::Workspace::from_commit_traversal(
        main_id,
        main_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·41ed0e4 (⌂|🏘)
    │   └── :3:►workspace
    │       ├── ·9730cbf (⌂|🏘|✓)
    │       ├── :6:►main-to-workspace
    │       │   ├── ·dc7ab57 (⌂|🏘|✓)
    │       │   └── :8:►anon:
    │       │       ├── ·c056b75 (⌂|🏘|✓|1)
    │       │       ├── ·f49c977 (⌂|🏘|✓|1)
    │       │       ├── ·7b7ebb2 (⌂|🏘|✓|1)
    │       │       ├── ·dca4960 (⌂|🏘|✓|1)
    │       │       ├── ·11c29b8 (⌂|🏘|✓|1)
    │       │       ├── ·c32dd03 (⌂|🏘|✓|1)
    │       │       ├── ·b625665 (⌂|🏘|✓|1)
    │       │       ├── ·a821094 (⌂|🏘|✓|1)
    │       │       ├── ·bce0c5e (⌂|🏘|✓|1)
    │       │       └── 🏁·3183e43 (⌂|🏘|✓|1)
    │       └── :7:►long-main-to-workspace
    │           ├── ·77f31a0 (⌂|🏘|✓)
    │           ├── ·eb17e31 (⌂|🏘|✓)
    │           ├── ·fe2046b (⌂|🏘|✓)
    │           ├── ·5532ef5 (⌂|🏘|✓)
    │           └── 👉:0:►main
    │               ├── ·2438292 (⌂|🏘|✓|1)
    │               └── →:8:►anon:
    └── :2:►origin/main
        ├── ·232ed06 (✓)
        ├── :4:►workspace-to-target
        │   ├── ·abcfd9a (✓)
        │   ├── ·bc86eba (✓)
        │   ├── ·c7ae303 (✓)
        │   └── →:3:►workspace
        └── :5:►long-workspace-to-target
            ├── ·9e2a79e (✓)
            ├── ·fdeaa43 (✓)
            ├── ·30565ee (✓)
            ├── ·0c1c23a (✓)
            ├── ·56d152c (✓)
            ├── ·e6e1360 (✓)
            ├── ·1a22a39 (✓)
            └── →:3:►workspace
    ");
    // Entrypoint is outside of workspace.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:main <> ✓refs/remotes/origin/main⇣11
    └── ≡:0:main <> origin/main⇣11 {1}
        └── :0:main <> origin/main⇣11
            ├── 🟣232ed06 (✓)
            ├── 🟣abcfd9a (✓)
            ├── 🟣bc86eba (✓)
            ├── 🟣c7ae303 (✓)
            ├── 🟣9e2a79e (✓)
            ├── 🟣fdeaa43 (✓)
            ├── 🟣30565ee (✓)
            ├── 🟣0c1c23a (✓)
            ├── 🟣56d152c (✓)
            ├── 🟣e6e1360 (✓)
            └── 🟣1a22a39 (✓)
    ");

    // When setting a limit when traversing 'main', it is respected.
    // We still want it to be found and connected though, and it's notable that the limit kicks in
    // once everything reconciled.
    let graph = but_graph::Workspace::from_commit_traversal(
        main_id,
        main_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·41ed0e4 (⌂|🏘)
    │   └── :3:►workspace
    │       ├── ·9730cbf (⌂|🏘|✓)
    │       ├── :6:►main-to-workspace
    │       │   ├── ·dc7ab57 (⌂|🏘|✓)
    │       │   └── :8:►anon:
    │       │       ├── ·c056b75 (⌂|🏘|✓|1)
    │       │       ├── ·f49c977 (⌂|🏘|✓|1)
    │       │       ├── ·7b7ebb2 (⌂|🏘|✓|1)
    │       │       ├── ·dca4960 (⌂|🏘|✓|1)
    │       │       └── ✂·11c29b8 (⌂|🏘|✓|1)
    │       └── :7:►long-main-to-workspace
    │           ├── ·77f31a0 (⌂|🏘|✓)
    │           ├── ·eb17e31 (⌂|🏘|✓)
    │           ├── ·fe2046b (⌂|🏘|✓)
    │           ├── ·5532ef5 (⌂|🏘|✓)
    │           └── 👉:0:►main
    │               ├── ·2438292 (⌂|🏘|✓|1)
    │               └── →:8:►anon:
    └── :2:►origin/main
        ├── ·232ed06 (✓)
        ├── :4:►workspace-to-target
        │   ├── ·abcfd9a (✓)
        │   ├── ·bc86eba (✓)
        │   ├── ·c7ae303 (✓)
        │   └── →:3:►workspace
        └── :5:►long-workspace-to-target
            ├── ·9e2a79e (✓)
            ├── ·fdeaa43 (✓)
            ├── ·30565ee (✓)
            ├── ·0c1c23a (✓)
            ├── ·56d152c (✓)
            ├── ·e6e1360 (✓)
            ├── ·1a22a39 (✓)
            └── →:3:►workspace
    ");
    // The limit is visible as well.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:main <> ✓refs/remotes/origin/main⇣11
    └── ≡:0:main <> origin/main⇣11 {1}
        └── :0:main <> origin/main⇣11
            ├── 🟣232ed06 (✓)
            ├── 🟣abcfd9a (✓)
            ├── 🟣bc86eba (✓)
            ├── 🟣c7ae303 (✓)
            ├── 🟣9e2a79e (✓)
            ├── 🟣fdeaa43 (✓)
            ├── 🟣30565ee (✓)
            ├── 🟣0c1c23a (✓)
            ├── 🟣56d152c (✓)
            ├── 🟣e6e1360 (✓)
            └── 🟣1a22a39 (✓)
    ");

    // From the workspace, even without limit, we don't traverse all of 'main' as it's uninteresting.
    // However, we wait for the target to be fully reconciled to get the proper workspace configuration.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·41ed0e4 (⌂|🏘|1)
    │   └── :2:►workspace
    │       ├── ·9730cbf (⌂|🏘|✓|1)
    │       ├── :5:►main-to-workspace
    │       │   ├── ·dc7ab57 (⌂|🏘|✓|1)
    │       │   └── :8:►anon:
    │       │       ├── ·c056b75 (⌂|🏘|✓|1)
    │       │       ├── ·f49c977 (⌂|🏘|✓|1)
    │       │       ├── ·7b7ebb2 (⌂|🏘|✓|1)
    │       │       ├── ·dca4960 (⌂|🏘|✓|1)
    │       │       ├── ·11c29b8 (⌂|🏘|✓|1)
    │       │       ├── ·c32dd03 (⌂|🏘|✓|1)
    │       │       ├── ·b625665 (⌂|🏘|✓|1)
    │       │       └── ✂·a821094 (⌂|🏘|✓|1)
    │       └── :6:►long-main-to-workspace
    │           ├── ·77f31a0 (⌂|🏘|✓|1)
    │           ├── ·eb17e31 (⌂|🏘|✓|1)
    │           ├── ·fe2046b (⌂|🏘|✓|1)
    │           ├── ·5532ef5 (⌂|🏘|✓|1)
    │           └── :7:►main
    │               ├── ·2438292 (⌂|🏘|✓|1)
    │               └── →:8:►anon:
    └── :1:►origin/main
        ├── ·232ed06 (✓)
        ├── :3:►workspace-to-target
        │   ├── ·abcfd9a (✓)
        │   ├── ·bc86eba (✓)
        │   ├── ·c7ae303 (✓)
        │   └── →:2:►workspace
        └── :4:►long-workspace-to-target
            ├── ·9e2a79e (✓)
            ├── ·fdeaa43 (✓)
            ├── ·30565ee (✓)
            ├── ·0c1c23a (✓)
            ├── ·56d152c (✓)
            ├── ·e6e1360 (✓)
            ├── ·1a22a39 (✓)
            └── →:2:►workspace
    ");

    // Everything is integrated, nothing to see here.
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣11 on 9730cbf");
    Ok(())
}

#[test]
fn remote_far_in_ancestry() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-far-in-ancestry")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?;
    // It's critical that the main branch isn't cut off and the local and remote part find each other,
    // or else the remote part will go on forever create a lot of issues for those who want to display
    // all these incorrectly labeled commits.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·9412ebd (⌂|🏘|1)
    │   └── :3:►A
    │       ├── ·8407093 (⌂|🏘|101)
    │       ├── ·7dfaa0c (⌂|🏘|101)
    │       ├── ·544e458 (⌂|🏘|101)
    │       └── :1:►main
    │           ├── ·685d644 (⌂|🏘|✓|111)
    │           ├── ·cafdb27 (⌂|🏘|✓|111)
    │           ├── ·c056b75 (⌂|🏘|✓|111)
    │           ├── ·f49c977 (⌂|🏘|✓|111)
    │           ├── ·7b7ebb2 (⌂|🏘|✓|111)
    │           ├── ·dca4960 (⌂|🏘|✓|111)
    │           ├── ·11c29b8 (⌂|🏘|✓|111)
    │           ├── ·c32dd03 (⌂|🏘|✓|111)
    │           ├── ·b625665 (⌂|🏘|✓|111)
    │           ├── ·a821094 (⌂|🏘|✓|111)
    │           ├── ·bce0c5e (⌂|🏘|✓|111)
    │           └── :5:►anon:
    │               └── 🏁·3183e43 (⌂|🏘|✓|1111)
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :4:►origin/A
        ├── ·975754f (0x0|1000)
        ├── ·f48ff69 (0x0|1000)
        └── →:5:►anon:
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 685d644
    └── ≡:2:A <> origin/A⇡3⇣2 on 685d644
        └── :2:A <> origin/A⇡3⇣2
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·f514495 (⌂|🏘)
    │   └── :4:►workspace
    │       ├── ·c9120f1 (⌂|🏘|✓)
    │       ├── :5:►main-to-workspace
    │       │   ├── ·1126587 (⌂|🏘|✓)
    │       │   └── :7:►anon:
    │       │       └── 🏁·3183e43 (⌂|🏘|✓|1) ►A, ►B
    │       └── :6:►long-main-to-workspace
    │           ├── ·b39c7ec (⌂|🏘|✓)
    │           ├── ·2983a97 (⌂|🏘|✓)
    │           ├── ·144ea85 (⌂|🏘|✓)
    │           ├── ·5aecfd2 (⌂|🏘|✓)
    │           └── 👉:0:►main
    │               ├── ·bce0c5e (⌂|🏘|✓|1)
    │               └── →:7:►anon:
    └── :3:►origin/main
        └── :2:►long-workspace-to-target
            ├── ·024f837 (✓)
            ├── ·64a8284 (✓)
            ├── ·b72938c (✓)
            ├── ·9ccbf6f (✓)
            ├── ·5fa4905 (✓)
            ├── ·43074d3 (✓)
            ├── ·800d4a9 (✓)
            ├── ·742c068 (✓)
            ├── ·fe06afd (✓)
            └── :8:►anon:
                ├── ·3027746 (✓)
                ├── :9:►anon:
                │   ├── ·f0d2a35 (✓)
                │   └── →:4:►workspace
                └── :10:►longer-workspace-to-target
                    ├── ·edf041f (✓)
                    ├── ·d9f03f6 (✓)
                    ├── ·8d1d264 (✓)
                    ├── ·fa7ceae (✓)
                    ├── ·95bdbf1 (✓)
                    ├── ·5bac978 (✓)
                    └── →:5:►main-to-workspace
    ");
    // `main` is integrated, but the entrypoint so it's shown.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    ⌂:0:main <> ✓refs/remotes/origin/main⇣17
    └── ≡:0:main <> origin/main⇣17 {1}
        └── :0:main <> origin/main⇣17
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
            ├── 🟣f0d2a35 (✓)
            ├── 🟣edf041f (✓)
            ├── 🟣d9f03f6 (✓)
            ├── 🟣8d1d264 (✓)
            ├── 🟣fa7ceae (✓)
            ├── 🟣95bdbf1 (✓)
            └── 🟣5bac978 (✓)
    ");

    // Now the target looks for the entrypoint, which is the workspace, something it can do more easily.
    // We wait for targets to fully reconcile as well.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·f514495 (⌂|🏘|1)
    │   └── :3:►workspace
    │       ├── ·c9120f1 (⌂|🏘|✓|1)
    │       ├── :4:►main-to-workspace
    │       │   ├── ·1126587 (⌂|🏘|✓|1)
    │       │   └── :7:►anon:
    │       │       └── 🏁·3183e43 (⌂|🏘|✓|1) ►A, ►B
    │       └── :5:►long-main-to-workspace
    │           ├── ·b39c7ec (⌂|🏘|✓|1)
    │           ├── ·2983a97 (⌂|🏘|✓|1)
    │           ├── ·144ea85 (⌂|🏘|✓|1)
    │           ├── ·5aecfd2 (⌂|🏘|✓|1)
    │           └── :6:►main
    │               ├── ·bce0c5e (⌂|🏘|✓|1)
    │               └── →:7:►anon:
    └── :2:►origin/main
        └── :1:►long-workspace-to-target
            ├── ·024f837 (✓)
            ├── ·64a8284 (✓)
            ├── ·b72938c (✓)
            ├── ·9ccbf6f (✓)
            ├── ·5fa4905 (✓)
            ├── ·43074d3 (✓)
            ├── ·800d4a9 (✓)
            ├── ·742c068 (✓)
            ├── ·fe06afd (✓)
            └── :8:►anon:
                ├── ·3027746 (✓)
                ├── :9:►anon:
                │   ├── ·f0d2a35 (✓)
                │   └── →:3:►workspace
                └── :10:►longer-workspace-to-target
                    ├── ·edf041f (✓)
                    ├── ·d9f03f6 (✓)
                    ├── ·8d1d264 (✓)
                    ├── ·fa7ceae (✓)
                    ├── ·95bdbf1 (✓)
                    ├── ·5bac978 (✓)
                    └── →:4:►main-to-workspace
    ");

    let ws = graph;
    // Everything is integrated.
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1");

    // With a lower base for the target, we see more.
    let target_commit_id = repo.rev_parse_single("3183e43")?.detach();
    add_workspace_with_target(&mut meta, target_commit_id);

    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1");

    // We can also add independent virtual branches to that new base.
    add_stack(&mut meta, 3, "A", StackState::InWorkspace);
    add_stack(&mut meta, 4, "B", StackState::InWorkspace);
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1");

    // We can also add stacked virtual branches to that new base.
    meta.data_mut().branches.clear();
    add_workspace_with_target(&mut meta, target_commit_id);
    add_stack_with_segments(&mut meta, 3, "A", StackState::InWorkspace, &["B"]);
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1");
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

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·2b30d94 (⌂|🏘|1)
    │   ├── :3:►D
    │   │   ├── ·9895054 (⌂|🏘|1)
    │   │   └── :6:►C
    │   │       ├── ·de625cc (⌂|🏘|1)
    │   │       ├── ·23419f8 (⌂|🏘|1)
    │   │       ├── ·5dc4389 (⌂|🏘|1)
    │   │       └── :7:►shared
    │   │           ├── ·d4f537e (⌂|🏘|✓|1)
    │   │           ├── ·b448757 (⌂|🏘|✓|1)
    │   │           ├── ·e9a378d (⌂|🏘|✓|1)
    │   │           └── :2:►main
    │   │               └── 🏁·3183e43 (⌂|🏘|✓|11)
    │   ├── :4:►A
    │   │   ├── ·0bad3af (⌂|🏘|✓|1)
    │   │   └── →:7:►shared
    │   └── :5:►B
    │       ├── ·acdc49a (⌂|🏘|1)
    │       ├── ·f0117e0 (⌂|🏘|1)
    │       └── →:7:►shared
    └── :1:►origin/main
        ├── ·c08dc6b (✓)
        ├── →:2:►main
        └── →:4:►A
    ");

    // A is still shown despite it being fully integrated, as it's still enclosed by the
    // workspace tip and the fork-point, at least when we provide the previous known location of the target.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    ├── ≡:3:D on 3183e43
    │   ├── :3:D
    │   │   └── ·9895054 (🏘️)
    │   ├── :6:C
    │   │   ├── ·de625cc (🏘️)
    │   │   ├── ·23419f8 (🏘️)
    │   │   └── ·5dc4389 (🏘️)
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
    └── ≡:5:B on 3183e43
        ├── :5:B
        │   ├── ·acdc49a (🏘️)
        │   └── ·f0117e0 (🏘️)
        └── :7:shared
            ├── ·d4f537e (🏘️|✓)
            ├── ·b448757 (🏘️|✓)
            └── ·e9a378d (🏘️|✓)
    ");

    // If we do not, integrated portions are removed.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on d4f537e
    ├── ≡:3:D on d4f537e
    │   ├── :3:D
    │   │   └── ·9895054 (🏘️)
    │   └── :6:C
    │       ├── ·de625cc (🏘️)
    │       ├── ·23419f8 (🏘️)
    │       └── ·5dc4389 (🏘️)
    └── ≡:5:B on d4f537e
        └── :5:B
            ├── ·acdc49a (🏘️)
            └── ·f0117e0 (🏘️)
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·2b30d94 (⌂|🏘|1)
    │   ├── :2:►D
    │   │   ├── ·9895054 (⌂|🏘|1)
    │   │   └── :6:►C
    │   │       ├── ·de625cc (⌂|🏘|1)
    │   │       ├── ·23419f8 (⌂|🏘|1)
    │   │       ├── ·5dc4389 (⌂|🏘|1)
    │   │       └── :7:►shared
    │   │           ├── ·d4f537e (⌂|🏘|1)
    │   │           ├── ·b448757 (⌂|🏘|1)
    │   │           ├── ·e9a378d (⌂|🏘|1)
    │   │           └── :5:►main
    │   │               └── 🏁·3183e43 (⌂|🏘|✓|1)
    │   ├── :3:►A
    │   │   ├── ·0bad3af (⌂|🏘|1)
    │   │   └── →:7:►shared
    │   └── :4:►B
    │       ├── ·acdc49a (⌂|🏘|1)
    │       ├── ·f0117e0 (⌂|🏘|1)
    │       └── →:7:►shared
    └── :1:►origin/main
        ├── ·bce0c5e (✓)
        └── →:5:►main
    ");

    // Segments can definitely repeat
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    ├── ≡:2:D on 3183e43
    │   ├── :2:D
    │   │   └── ·9895054 (🏘️)
    │   ├── :6:C
    │   │   ├── ·de625cc (🏘️)
    │   │   ├── ·23419f8 (🏘️)
    │   │   └── ·5dc4389 (🏘️)
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
    └── ≡:4:B on 3183e43
        ├── :4:B
        │   ├── ·acdc49a (🏘️)
        │   └── ·f0117e0 (🏘️)
        └── :7:shared
            ├── ·d4f537e (🏘️)
            ├── ·b448757 (🏘️)
            └── ·e9a378d (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        Some(ref_name),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // Checking out anything inside the workspace yields the same result.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    ├── ≡:4:D on 3183e43
    │   ├── :4:D
    │   │   └── ·9895054 (🏘️)
    │   ├── :7:C
    │   │   ├── ·de625cc (🏘️)
    │   │   ├── ·23419f8 (🏘️)
    │   │   └── ·5dc4389 (🏘️)
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
    └── ≡:5:B on 3183e43
        ├── :5:B
        │   ├── ·acdc49a (🏘️)
        │   └── ·f0117e0 (🏘️)
        └── :3:shared
            ├── ·d4f537e (🏘️)
            ├── ·b448757 (🏘️)
            └── ·e9a378d (🏘️)
    ");
    Ok(())
}

#[test]
fn local_branch_tracking_the_target_does_not_duplicate_the_target_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/multi-lane-with-shared-segment")?;
    add_workspace(&mut meta);

    // `main` tracks the target `origin/main`. Remote-tracking discovery at `main` must
    // recognize the project-metadata target ref as already queued instead of inserting
    // a second `origin/main` segment, which can leave disconnected segments behind.
    let _graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·335d6f2 (⌂|🏘|1)
    │   ├── :1:►main
    │   │   └── 🏁·fafd9d0 (⌂|🏘|✓|111) ►lane
    │   └── :3:►dependent
    │       └── :4:►advanced-lane
    │           ├── ·cbc6713 (⌂|🏘|101)
    │           └── →:1:►main
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :5:►origin/advanced-lane
        └── →:3:►dependent
    ");

    // The dependent branch is empty and on top of the one with the remote
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:dependent on fafd9d0 {1}
        ├── 📙:5:dependent
        └── 📙:2:advanced-lane <> origin/advanced-lane
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·335d6f2 (⌂|🏘|1)
    │   ├── :1:►main
    │   │   └── 🏁·fafd9d0 (⌂|🏘|✓|111) ►lane
    │   └── :3:►advanced-lane
    │       └── :4:►dependent
    │           ├── ·cbc6713 (⌂|🏘|101)
    │           └── →:1:►main
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :5:►origin/advanced-lane
        └── →:3:►advanced-lane
    ");

    // Having done something unusual, which is to put the dependent branch
    // underneath the other already pushed, it creates a different view of ownership.
    // It's probably OK to leave it like this for now, and instead allow users to reorder
    // these more easily.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:advanced-lane <> origin/advanced-lane on fafd9d0 {1}
        ├── 📙:5:advanced-lane <> origin/advanced-lane
        └── 📙:2:dependent
            └── ❄cbc6713 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "advanced-lane");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡👉📙:5:advanced-lane <> origin/advanced-lane on fafd9d0 {1}
        ├── 👉📙:5:advanced-lane <> origin/advanced-lane
        └── 📙:0:dependent
            └── ❄cbc6713 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "dependent");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:advanced-lane <> origin/advanced-lane on fafd9d0 {1}
        ├── 📙:5:advanced-lane <> origin/advanced-lane
        └── 👉📙:0:dependent
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·e982e8a (⌂|🏘|1)
    │   ├── :3:►C-on-A
    │   │   ├── ·4f1bb32 (⌂|🏘|1)
    │   │   └── :4:►A
    │   │       ├── ·e255adc (⌂|🏘|1101)
    │   │       └── :1:►main
    │   │           └── 🏁·fafd9d0 (⌂|🏘|✓|1111)
    │   └── :6:►B-on-A
    │       ├── ·aff8449 (⌂|🏘|1)
    │       └── →:4:►A
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :5:►origin/A
        ├── ·b627ca7 (0x0|1000)
        └── →:4:►A
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:2:C-on-A on fafd9d0 {1}
    │   ├── 📙:2:C-on-A
    │   │   └── ·4f1bb32 (🏘️)
    │   └── :3:A <> origin/A⇣1
    │       ├── 🟣b627ca7
    │       └── ❄️e255adc (🏘️)
    └── ≡:5:B-on-A on fafd9d0
        ├── :5:B-on-A
        │   └── ·aff8449 (🏘️)
        └── :3:A <> origin/A⇣1
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·873d056 (⌂|🏘|1)
    │   ├── :2:►advanced-lane
    │   │   ├── ·cbc6713 (⌂|🏘|1)
    │   │   └── :3:►lane
    │   │       └── 🏁·fafd9d0 (⌂|🏘|1) ►main
    │   └── →:3:►lane
    └── :1:►origin/main
        └── 🏁·da83717 (✓)
    ");

    // Since `lane` is connected directly, no segment has to be created.
    // However, as nothing is integrated, it really is another name for `main` now,
    // `main` is nothing special.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:2:advanced-lane on fafd9d0 {0}
    │   └── 📙:2:advanced-lane
    │       └── ·cbc6713 (🏘️)
    └── ≡📙:4:lane on fafd9d0 {1}
        └── 📙:4:lane
    ");

    // Reverse the order of stacks in the worktree data.
    for (idx, name) in lanes.into_iter().rev().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·873d056 (⌂|🏘|1)
    │   ├── :3:►advanced-lane
    │   │   ├── ·cbc6713 (⌂|🏘|1)
    │   │   └── :2:►lane
    │   │       └── 🏁·fafd9d0 (⌂|🏘|1) ►main
    │   └── →:2:►lane
    └── :1:►origin/main
        └── 🏁·da83717 (✓)
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:4:lane on fafd9d0 {0}
    │   └── 📙:4:lane
    └── ≡📙:3:advanced-lane on fafd9d0 {1}
        └── 📙:3:advanced-lane
            └── ·cbc6713 (🏘️)
    ");
    Ok(())
}

#[test]
fn two_dependent_branches_with_embedded_remote() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-dependent-branches-with-interesting-remote-setup")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·a221221 (⌂|🏘|1)
    │   └── :3:►A
    │       ├── ·aadad9d (⌂|🏘|101)
    │       └── :1:►origin/main
    │           ├── ·96a2408 (⌂|🏘|✓|101)
    │           └── :5:►integrated
    │               ├── ·f15ca75 (⌂|🏘|✓|1101)
    │               ├── ·9456d79 (⌂|🏘|✓|1101)
    │               └── :2:►main
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|1111)
    └── :4:►origin/A
        ├── ·2b1808c (0x0|1000)
        └── →:5:►integrated
    ");

    // Remote tracking branches we just want to aggregate, just like anonymous segments,
    // but only when another target is provided (the old position, `main`).
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A <> origin/A⇡1⇣1 on fafd9d0 {1}
        ├── 📙:3:A <> origin/A⇡1⇣1
        │   ├── 🟣2b1808c
        │   ├── ·aadad9d (🏘️)
        │   └── ·96a2408 (🏘️|✓)
        └── :5:integrated
            ├── ❄f15ca75 (🏘️|✓)
            └── ❄9456d79 (🏘️|✓)
    ");

    // Otherwise, nothing that's integrated is shown. Note how 96a2408 seems missing,
    // but it's skipped because it's actually part of an integrated otherwise ignored segment.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 96a2408
    └── ≡📙:3:A <> origin/A⇡1⇣1 on 96a2408 {1}
        └── 📙:3:A <> origin/A⇡1⇣1
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·4f08b8d (⌂|🏘|1)
    │   └── :3:►B
    │       ├── ·da597e8 (⌂|🏘|101)
    │       └── :4:►A
    │           ├── ·1818c17 (⌂|🏘|✓|10101)
    │           └── :2:►main
    │               └── 🏁·281456a (⌂|🏘|✓|111111)
    ├── :1:►origin/main
    │   ├── ·b694668 (✓)
    │   ├── →:2:►main
    │   └── →:4:►A
    └── :5:►origin/B
        ├── ·e0bd0a7 (0x0|1000)
        └── :6:►origin/A
            ├── ·0b6b861 (0x0|101000)
            └── →:2:►main
    ");

    // This is the default as it includes both the integrated and non-integrated segment.
    // Note how there is no expensive computation to see if remote commits are the same,
    // it's all ID-based.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 281456a
    └── ≡📙:3:B <> origin/B⇡1⇣1 on 281456a {0}
        ├── 📙:3:B <> origin/B⇡1⇣1
        │   ├── 🟣e0bd0a7
        │   └── ·da597e8 (🏘️)
        └── 📙:4:A <> origin/A⇣1
            ├── 🟣0b6b861
            └── ·1818c17 (🏘️|✓)
    ");

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "A"),
    )?;
    // Pretending we are rebased onto A still shows the same remote commits.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1818c17
    └── ≡📙:4:B <> origin/B⇡1⇣1 on 1818c17 {0}
        └── 📙:4:B <> origin/B⇡1⇣1
            ├── 🟣e0bd0a7
            └── ·da597e8 (🏘️)
    ");
    Ok(())
}

#[test]
fn stacked_bottom_remote_still_points_at_now_split_top() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/stacked-bottom-remote-still-points-at-now-split-top")?;
    // origin/bottom still points at T (the previously combined push), but the
    // local stack is now split so that bottom holds only B and top holds T on
    // top of bottom. To remove T from origin/bottom we'd need to force-push,
    // so bottom must report `commits_on_remote` containing T.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 5c66c47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * bfbff44 (origin/bottom, top) T
    * 7fdb58d (bottom) B
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(&mut meta, 0, "top", StackState::InWorkspace, &["bottom"]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:2:top on fafd9d0 {0}
        ├── 📙:2:top
        │   └── ❄bfbff44 (🏘️)
        └── 📙:3:bottom <> origin/bottom⇣1
            ├── 🟣bfbff44 (🏘️)
            └── ❄️7fdb58d (🏘️)
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·1109eb2 (⌂|🏘|1)
    │   └── :3:►D
    │       ├── ·624e118 (⌂|🏘|101)
    │       └── :1:►main
    │           ├── ·0b6b861 (⌂|🏘|✓|111)
    │           └── :5:►anon:
    │               └── 🏁·281456a (⌂|🏘|✓|1111)
    ├── :2:►origin/main
    │   └── →:1:►main
    ├── :6:►origin/B
    │   └── :4:►origin/D
    │       ├── ·3045ea6 (0x0|1000)
    │       ├── ·1818c17 (0x0|1000)
    │       └── →:5:►anon:
    └── :7:►origin/C
        └── →:4:►origin/D
    ");

    insta::assert_snapshot!(graph_workspace(&graph), "only one remote commit as unrelated remotes split a linear segment", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
    └── ≡📙:2:D <> origin/D⇡1⇣1 on 0b6b861 {0}
        └── 📙:2:D <> origin/D⇡1⇣1
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·deeae50 (⌂|🏘|1)
    │   └── :3:►D
    │       ├── ·353471f (⌂|🏘|101)
    │       ├── ·8a4b945 (⌂|🏘|101)
    │       ├── ·e0bd0a7 (⌂|🏘|101)
    │       └── :1:►main
    │           ├── ·0b6b861 (⌂|🏘|✓|111)
    │           └── :5:►anon:
    │               └── 🏁·281456a (⌂|🏘|✓|1111)
    ├── :2:►origin/main
    │   └── →:1:►main
    └── :4:►origin/D
        ├── ·bbd4ff6 (0x0|1000)
        ├── ·e5f5a87 (0x0|1000)
        ├── ·da597e8 (0x0|1000)
        ├── ·1818c17 (0x0|1000)
        └── →:5:►anon:
    ");

    // We let each remote on the path down own a commit so we only see one remote commit here,
    // the one belonging to the last remaining associated remote tracking branch of D.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
    └── ≡📙:2:D <> origin/D⇡3⇣1 on 0b6b861 {0}
        └── 📙:2:D <> origin/D⇡3⇣1
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 4fe5a6f (origin/A) A-remote
    * a62b0de (HEAD -> gitbutler/workspace, A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/A
    │   ├── ·4fe5a6f (0x0|10)
    │   └── :0:►A
    │       ├── ·a62b0de (⌂|🏘|11)
    │       ├── ·120a217 (⌂|🏘|11)
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|11)
    └── 👉:3:►gitbutler/workspace
        └── →:0:►A
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡:0:A <> origin/A⇣1
        ├── :0:A <> origin/A⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :2:main
            └── ❄fafd9d0 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/A
    │   ├── ·4fe5a6f (0x0|10)
    │   └── 👉:0:►A
    │       ├── ·a62b0de (⌂|🏘|11)
    │       ├── ·120a217 (⌂|🏘|11)
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|11)
    └── :3:►gitbutler/workspace
        └── →:0:►A
    ");

    // Main can be a normal segment if there is no target ref.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:A <> origin/A⇣1
        ├── 👉:0:A <> origin/A⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :2:main
            └── ❄fafd9d0 (🏘️)
    ");
    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit_ambiguous() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-target-without-ws-commit-ambiguous")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 4fe5a6f (origin/A) A-remote
    * a62b0de (HEAD -> gitbutler/workspace, B, A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    // Without disambiguation, there is no segment name.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/A
    │   ├── ·4fe5a6f (0x0|10)
    │   └── :0:►A
    │       ├── ·a62b0de (⌂|🏘|11) ►B
    │       ├── ·120a217 (⌂|🏘|11)
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|11)
    └── 👉:3:►gitbutler/workspace
        └── →:0:►A
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡:0:A <> origin/A⇣1
        ├── :0:A <> origin/A⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️) ►B
        │   └── ❄️120a217 (🏘️)
        └── :2:main
            └── ❄fafd9d0 (🏘️)
    ");

    // We can help it by adding metadata.
    // Note how the selection still manages to hold on to the `A` which now gets its very own
    // empty segment.
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let (id, a_ref) = id_at(&repo, "A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/A
    │   ├── ·4fe5a6f (0x0|10)
    │   └── 👉:0:►B
    │       ├── ·a62b0de (⌂|🏘|11) ►A
    │       ├── ·120a217 (⌂|🏘|11)
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|11)
    └── :3:►gitbutler/workspace
        └── →:0:►B
    ");

    // Main can be a normal segment if there is no target ref.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:4:A <> origin/A⇣1 {1}
        ├── 👉:4:A <> origin/A⇣1
        │   └── 🟣4fe5a6f
        ├── 📙:0:B
        │   ├── ❄a62b0de (🏘️)
        │   └── ❄120a217 (🏘️)
        └── :2:main
            └── ❄fafd9d0 (🏘️)
    ");

    // Finally, show the normal version with just disambiguated 'B".
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/A
    │   ├── ·4fe5a6f (0x0|10)
    │   └── :0:►B
    │       ├── ·a62b0de (⌂|🏘|11) ►A
    │       ├── ·120a217 (⌂|🏘|11)
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|11)
    └── 👉:3:►gitbutler/workspace
        └── →:0:►B
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:0:B {1}
        ├── 📙:0:B
        │   ├── ·a62b0de (🏘️)
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
    ");

    // Order is respected
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // The remote tracking branch must remain linked.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:4:B {1}
        ├── 📙:4:B
        ├── 👉📙:0:A <> origin/A⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :2:main
            └── ❄fafd9d0 (🏘️)
    ");

    // Order is respected, vice-versa
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        a_ref,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉📙:4:A <> origin/A⇣1 {1}
        ├── 👉📙:4:A <> origin/A⇣1
        │   └── 🟣4fe5a6f
        ├── 📙:0:B
        │   ├── ❄a62b0de (🏘️)
        │   └── ❄120a217 (🏘️)
        └── :2:main
            └── ❄fafd9d0 (🏘️)
    ");

    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit_ambiguous_with_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-target-without-ws-commit-ambiguous-with-remotes")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a62b0de (HEAD -> gitbutler/workspace, origin/B, origin/A, B, A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    // Without disambiguation, there is no segment name.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/B
    │   └── :0:►origin/A
    │       ├── ·a62b0de (⌂|🏘|1) ►A, ►B
    │       ├── ·120a217 (⌂|🏘|1)
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|1)
    └── 👉:3:►gitbutler/workspace
        └── →:0:►origin/A
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓!
    └── ≡:0:anon:
        ├── :0:anon:
        │   ├── ·a62b0de (🏘️) ►A, ►B
        │   └── ·120a217 (🏘️)
        └── :1:main <> origin/main⇡1
            └── ·fafd9d0 (🏘️)
    ");

    // Remote handling is still happening when A is disambiguated by entrypoint.
    let (id, a_ref) = id_at(&repo, "A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/B
    │   └── 👉:0:►origin/A
    │       ├── ·a62b0de (⌂|🏘|1) ►A, ►B
    │       ├── ·120a217 (⌂|🏘|1)
    │       └── :2:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|1)
    └── :3:►gitbutler/workspace
        └── →:0:►origin/A
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:A <> origin/A
        ├── 👉:0:A <> origin/A
        │   ├── ❄️a62b0de (🏘️) ►B
        │   └── ❄️120a217 (🏘️)
        └── :1:main <> origin/main
            └── ❄fafd9d0 (🏘️)
    ");

    // The same is true when starting at a different ref.
    let (id, b_ref) = id_at(&repo, "B");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        b_ref,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:B <> origin/B
        ├── 👉:0:B <> origin/B
        │   ├── ❄️a62b0de (🏘️) ►A
        │   └── ❄️120a217 (🏘️)
        └── :1:main <> origin/main
            └── ❄fafd9d0 (🏘️)
    ");

    // If disambiguation happens through the workspace, 'A' still shows the right remote, and 'B' as well
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    // NOTE: origin/A points to :5, but origin/B now also points to :5 even though it should point to :0,
    //       a relationship still preserved though the sibling ID.
    //       There is no easy way of fixing this as we'd have to know that this one connection, which can
    //       indirectly reach the remote tracking segment, should remain on the local tracking segment when
    //       reconnecting them during the segment insertion.
    //       This is acceptable as graph connections aren't used for this, and ultimately they still
    //       reach the right segment, just through one more indirection. Empty segments are 'looked through'
    //       as well by all algorithms for exactly that reason.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/A
    │   └── 👉:0:►B
    │       ├── ·a62b0de (⌂|🏘|1) ►A
    │       ├── ·120a217 (⌂|🏘|1)
    │       └── :3:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|1)
    ├── :2:►origin/B
    │   └── →:0:►B
    └── :4:►gitbutler/workspace
        └── →:0:►B
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:5:A <> origin/A {1}
        ├── 👉:5:A <> origin/A
        ├── 📙:0:B <> origin/B
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :1:main <> origin/main
            └── ❄fafd9d0 (🏘️)
    ");
    Ok(())
}

#[test]
fn without_target_ref_with_managed_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-with-ws-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 4fe5a6f (origin/A) A-remote
    |/  
    * a62b0de (A) A2
    * 120a217 A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    // The commit is ambiguous, so there is just the entrypoint to split the segment.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·3ea2742 (⌂|🏘|1)
    │   └── :1:►A
    │       ├── ·a62b0de (⌂|🏘|111)
    │       ├── ·120a217 (⌂|🏘|111)
    │       └── :3:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|111)
    └── :2:►origin/A
        ├── ·4fe5a6f (0x0|100)
        └── →:1:►A
    ");
    // TODO: add more stacks.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:A <> origin/A⇣1
        ├── :1:A <> origin/A⇣1
        │   ├── 🟣4fe5a6f
        │   ├── ❄️a62b0de (🏘️)
        │   └── ❄️120a217 (🏘️)
        └── :3:main
            └── ❄fafd9d0 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·3ea2742 (⌂|🏘)
    │   └── 👉:0:►A
    │       ├── ·a62b0de (⌂|🏘|11)
    │       ├── ·120a217 (⌂|🏘|11)
    │       └── :3:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|11)
    └── :2:►origin/A
        ├── ·4fe5a6f (0x0|10)
        └── →:0:►A
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓!
    └── ≡👉:0:A <> origin/A⇣1
        ├── 👉:0:A <> origin/A⇣1
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 8ee08de (HEAD -> gitbutler/workspace, origin/main) GitButler Workspace Commit
    * 120a217 (A) A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── :1:►origin/main
        └── 👉:0:►gitbutler/workspace
            ├── ·8ee08de (⌂|🏘|✓|1)
            └── :2:►A
                ├── ·120a217 (⌂|🏘|✓|1)
                └── :3:►main
                    └── 🏁·fafd9d0 (⌂|🏘|✓|1)
    ");
    // Everything is integrated, so nothing is shown.
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 120a217");
    Ok(())
}

#[test]
fn no_workspace_no_target_commit_under_managed_ref() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-ws-no-target-commit-with-managed-ref")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * dca94a4 (HEAD -> gitbutler/workspace) unmanaged
    * 120a217 (A) A1
    * fafd9d0 (main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── :0:►gitbutler/workspace
        ├── ·dca94a4 (⌂|🏘|1)
        └── :1:►A
            ├── ·120a217 (⌂|🏘|1)
            └── :2:►main
                └── 🏁·fafd9d0 (⌂|🏘|1)
    ");

    // It's notable how hard the workspace ref tries to not own the commit
    // it's under unless it's a managed commit.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
    └── ≡:0:anon:
        ├── :0:anon:
        │   └── ·dca94a4 (🏘️)
        ├── :1:A
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
    ");
    Ok(())
}

#[test]
fn no_workspace_commit() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multiple-dependent-branches-per-stack-without-ws-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    // Notably we also pick up 'lane' which sits on the base.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►lane-segment-01
    │   └── :2:►lane-segment-02
    │       └── :0:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    ├── :3:►lane-2
    │   └── :4:►lane-2-segment-01
    │       └── :5:►lane-2-segment-02
    │           └── →:0:►main
    ├── :6:►origin/main
    │   └── →:0:►main
    └── 👉:8:►gitbutler/workspace
        └── :7:►lane
            ├── ·cbc6713 (⌂|🏘|1)
            └── →:0:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:1:lane on fafd9d0 {0}
    │   ├── 📙:1:lane
    │   │   └── ·cbc6713 (🏘️)
    │   ├── 📙:4:lane-segment-01
    │   └── 📙:5:lane-segment-02
    └── ≡📙:6:lane-2 on fafd9d0 {1}
        ├── 📙:6:lane-2
        ├── 📙:7:lane-2-segment-01
        └── 📙:8:lane-2-segment-02
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    // the order is maintained as provided in the workspace.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►lane-2
    │   └── :2:►lane-2-segment-01
    │       └── :3:►lane-2-segment-02
    │           └── :0:►main
    │               └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    ├── :4:►lane-segment-01
    │   └── :5:►lane-segment-02
    │       └── →:0:►main
    ├── :6:►origin/main
    │   └── →:0:►main
    └── 👉:8:►gitbutler/workspace
        └── :7:►lane
            ├── ·cbc6713 (⌂|🏘|1)
            └── →:0:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:4:lane-2 on fafd9d0 {0}
    │   ├── 📙:4:lane-2
    │   ├── 📙:5:lane-2-segment-01
    │   └── 📙:6:lane-2-segment-02
    └── ≡📙:1:lane on fafd9d0 {1}
        ├── 📙:1:lane
        │   └── ·cbc6713 (🏘️)
        ├── 📙:7:lane-segment-01
        └── 📙:8:lane-segment-02
    ");
    Ok(())
}

#[test]
fn two_dependent_branches_first_merged_by_rebase() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-dependent-branches-first-rebased-and-merged")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 0b6b861 (origin/main, origin/A) A
    | * 4f08b8d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * da597e8 (B) B
    | * 1818c17 (A) A
    |/  
    * 281456a (main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·4f08b8d (⌂|🏘|1)
    │   └── :4:►B
    │       ├── ·da597e8 (⌂|🏘|1)
    │       └── :5:►A
    │           ├── ·1818c17 (⌂|🏘|101)
    │           └── :3:►main
    │               └── 🏁·281456a (⌂|🏘|✓|1111)
    └── :2:►origin/A
        └── :1:►origin/main
            ├── ·0b6b861 (✓|1000)
            └── →:3:►main
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 281456a
    └── ≡:3:B on 281456a
        ├── :3:B
        │   └── ·da597e8 (🏘️)
        └── :4:A <> origin/A⇡1⇣1
            ├── 🟣0b6b861 (✓)
            └── ·1818c17 (🏘️)
    ");
    Ok(())
}

#[test]
fn special_branch_names_do_not_end_up_in_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/special-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 8926b15 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 3686017 (main) top
    * 9725482 (gitbutler/edit) middle
    * fafd9d0 (gitbutler/target) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    // Standard handling after traversal and post-processing.
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── 👉:0:►gitbutler/workspace
        ├── ·8926b15 (⌂|🏘|1)
        └── :1:►main
            ├── ·3686017 (⌂|🏘|1)
            └── :2:►gitbutler/edit
                ├── ·9725482 (⌂|🏘|1)
                └── :3:►gitbutler/target
                    └── 🏁·fafd9d0 (⌂|🏘|1)
    ");

    // But special handling for workspace views.
    insta::assert_snapshot!(graph_workspace(&graph), @"
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 270738b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c59457b (A) top
    * e146f13 (gitbutler/edit) middle
    * 971953d (origin/main, main) M2
    * ce09734 (origin/gitbutler/target, gitbutler/target) M1
    * fafd9d0 init
    ");

    add_workspace(&mut meta);
    let mut md = meta.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    let mut project_meta = md.project_meta();
    project_meta.target_ref = Some("refs/remotes/origin/gitbutler/target".try_into()?);
    md.set_project_meta(project_meta);
    meta.set_workspace(&md)?;

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        md.project_meta(),
        // standard_options_with_extra_target(&repo, "gitbutler/target"),
        standard_options(),
    )?;
    // Standard handling after traversal and post-processing.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·270738b (⌂|🏘|1)
    │   └── :3:►A
    │       ├── ·c59457b (⌂|🏘|1)
    │       └── :4:►gitbutler/edit
    │           ├── ·e146f13 (⌂|🏘|1)
    │           └── :5:►main
    │               ├── ·971953d (⌂|🏘|101)
    │               └── :1:►gitbutler/target
    │                   ├── ·ce09734 (⌂|🏘|✓|111)
    │                   └── 🏁·fafd9d0 (⌂|🏘|✓|111)
    ├── :2:►origin/gitbutler/target
    │   └── →:1:►gitbutler/target
    └── :6:►origin/main
        └── →:5:►main
    ");

    // But special handling for workspace views. Note how we don't overshoot
    // and stop exactly where we have to, magically even.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/gitbutler/target on ce09734
    └── ≡:2:A on ce09734
        ├── :2:A
        │   ├── ·c59457b (🏘️)
        │   └── ·e146f13 (🏘️)
        └── :4:main <> origin/main
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·fe6ba62 (⌂|🏘|1)
    │   ├── :5:►anon:
    │   │   ├── ·a62b0de (⌂|🏘|✓|11)
    │   │   ├── ·120a217 (⌂|🏘|✓|11)
    │   │   └── :9:►anon:
    │   │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    │   ├── :6:►B
    │   │   ├── ·2f8f06d (⌂|🏘|1)
    │   │   └── :4:►origin/B-middle
    │   │       ├── ·91bc3fc (⌂|🏘|✓|11)
    │   │       ├── ·cf9330f (⌂|🏘|✓|11)
    │   │       └── →:9:►anon:
    │   ├── :7:►C
    │   │   ├── ·3f7c4e6 (⌂|🏘|1)
    │   │   ├── ·b6895d7 (⌂|🏘|1)
    │   │   └── →:9:►anon:
    │   └── :8:►new-name-for-D
    │       ├── ·ed36e3b (⌂|🏘|1)
    │       └── →:9:►anon:
    └── :2:►origin/main
        └── :1:►main
            ├── ·867927f (⌂|✓|10)
            ├── :3:►anon:
            │   ├── ·6e03461 (⌂|✓|10)
            │   ├── →:9:►anon:
            │   └── →:5:►anon:
            └── →:4:►origin/B-middle
    ");

    // If it doesn't know how the workspace should be looking like, i.e. which branches are contained,
    // nothing special happens.
    // The branches that are outside the workspace don't exist and segments are flattened.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on fafd9d0
    ├── ≡:5:B on fafd9d0
    │   └── :5:B
    │       └── ·2f8f06d (🏘️)
    ├── ≡:6:C on fafd9d0
    │   └── :6:C
    │       ├── ·3f7c4e6 (🏘️)
    │       └── ·b6895d7 (🏘️)
    └── ≡:7:new-name-for-D on fafd9d0
        └── :7:new-name-for-D
            └── ·ed36e3b (🏘️)
    ");

    // However, when the desired workspace is set up, the traversal will include these extra tips.
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-middle"]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["B-middle"]);
    add_stack_with_segments(&mut meta, 2, "C", StackState::InWorkspace, &["C-bottom"]);
    add_stack_with_segments(&mut meta, 3, "D", StackState::InWorkspace, &[]);

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, ":/init"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·fe6ba62 (⌂|🏘|1)
    │   ├── :18:►anon:
    │   │   ├── ·a62b0de (⌂|🏘|✓|11)
    │   │   └── :20:►anon:
    │   │       ├── ·120a217 (⌂|🏘|✓|111)
    │   │       └── :3:►anon:
    │   │           └── 🏁·fafd9d0 (⌂|🏘|✓|11111)
    │   ├── :7:►B
    │   │   ├── ·2f8f06d (⌂|🏘|1)
    │   │   └── :14:►origin/B-middle
    │   │       ├── ·91bc3fc (⌂|🏘|✓|11011)
    │   │       ├── ·cf9330f (⌂|🏘|✓|11011)
    │   │       └── →:3:►anon:
    │   ├── :9:►C
    │   │   ├── ·3f7c4e6 (⌂|🏘|1)
    │   │   └── :19:►anon:
    │   │       ├── ·b6895d7 (⌂|🏘|1)
    │   │       └── →:3:►anon:
    │   └── :17:►new-name-for-D
    │       ├── ·ed36e3b (⌂|🏘|1)
    │       └── →:3:►anon:
    ├── :2:►origin/main
    │   └── :1:►main
    │       ├── ·867927f (⌂|✓|10)
    │       ├── :12:►anon:
    │       │   ├── ·6e03461 (⌂|✓|10)
    │       │   ├── →:3:►anon:
    │       │   └── →:18:►anon:
    │       └── →:14:►origin/B-middle
    ├── :4:►A
    │   ├── ·c83f258 (⌂)
    │   └── →:18:►anon:
    ├── :6:►origin/A-middle
    │   └── :5:►A-middle
    │       ├── ·27c2545 (⌂|100)
    │       └── →:20:►anon:
    ├── :8:►B-middle
    │   ├── ·c8f73c7 (⌂|1000)
    │   └── :15:►intermediate-branch
    │       ├── ·ff75b80 (⌂|1000)
    │       └── →:14:►origin/B-middle
    ├── :10:►C-bottom
    │   ├── ·790a17d (⌂)
    │   ├── :13:►tmp
    │   │   ├── ·631be19 (⌂)
    │   │   └── →:19:►anon:
    │   └── :16:►anon:
    │       ├── ·969aaec (⌂)
    │       └── →:19:►anon:
    └── :11:►D
        ├── ·71dad1a (⌂)
        └── →:17:►new-name-for-D
    ");

    // The workspace itself contains information about the outside tips.
    // We collect it no matter the location of the tip, e.g.
    // - anon segment directly below the workspace commit
    // - middle anon segment leading to the named branch over intermediate branches
    // - middle anon segment leading to the named branch over two outgoing connections
    // - except: if the segment with a known named segment in its future has a (new) name,
    //   we leave it and don't attempt to reconstruct the original (out-of-workspace) reference
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on fafd9d0
    ├── ≡📙:16:A on fafd9d0 {0}
    │   ├── 📙:16:A
    │   │   ├── ·c83f258*
    │   │   └── ·a62b0de (🏘️|✓)
    │   └── 📙:18:A-middle <> origin/A-middle
    │       ├── ·27c2545*
    │       └── ·120a217 (🏘️|✓)
    ├── ≡📙:5:B on fafd9d0 {1}
    │   ├── 📙:5:B
    │   │   └── ·2f8f06d (🏘️)
    │   └── 📙:12:B-middle <> origin/B-middle
    │       ├── ·c8f73c7*
    │       ├── ·ff75b80*
    │       ├── ·91bc3fc (🏘️|✓)
    │       └── ·cf9330f (🏘️|✓)
    ├── ≡📙:7:C on fafd9d0 {2}
    │   ├── 📙:7:C
    │   │   └── ·3f7c4e6 (🏘️)
    │   └── 📙:17:C-bottom
    │       ├── ·790a17d*
    │       ├── ·969aaec*
    │       ├── ·631be19*
    │       └── ·b6895d7 (🏘️)
    └── ≡:15:new-name-for-D on fafd9d0
        └── :15:new-name-for-D
            └── ·ed36e3b (🏘️)
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
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►gitbutler/workspace
    │   ├── ·873d056 (⌂|🏘)
    │   ├── :3:►advanced-lane
    │   │   ├── ·cbc6713 (⌂|🏘)
    │   │   └── 👉:0:►lane
    │   │       └── 🏁·fafd9d0 (⌂|🏘|1) ►main
    │   └── →:0:►lane
    └── :2:►origin/main
        └── 🏁·da83717 (✓)
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡👉📙:4:lane on fafd9d0 {0}
    │   └── 👉📙:4:lane
    └── ≡📙:3:advanced-lane on fafd9d0 {1}
        └── 📙:3:advanced-lane
            └── ·cbc6713 (🏘️)
    ");

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·873d056 (⌂|🏘|1)
    │   ├── :3:►advanced-lane
    │   │   ├── ·cbc6713 (⌂|🏘|1)
    │   │   └── :2:►lane
    │   │       └── 🏁·fafd9d0 (⌂|🏘|✓|1) ►main
    │   └── →:2:►lane
    └── :1:►origin/main
        └── 🏁·da83717 (✓)
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:4:lane on fafd9d0 {0}
    │   └── 📙:4:lane
    └── ≡📙:3:advanced-lane on fafd9d0 {1}
        └── 📙:3:advanced-lane
            └── ·cbc6713 (🏘️)
    ");

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·873d056 (⌂|🏘|1)
    │   ├── :3:►advanced-lane
    │   │   ├── ·cbc6713 (⌂|🏘|1)
    │   │   └── :2:►lane
    │   │       └── 🏁·fafd9d0 (⌂|🏘|1) ►main
    │   └── →:2:►lane
    └── :1:►origin/main
        └── 🏁·da83717 (✓)
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:4:lane on fafd9d0 {0}
    │   └── 📙:4:lane
    └── ≡📙:3:advanced-lane on fafd9d0 {1}
        └── 📙:3:advanced-lane
            └── ·cbc6713 (🏘️)
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/main
    │   └── :0:►main
    │       ├── ·bce0c5e (⌂|🏘|✓|11)
    │       └── 🏁·3183e43 (⌂|🏘|✓|11)
    └── :4:►gitbutler/workspace
        ├── ·a7131b1 (⌂|🏘|1)
        └── :5:►intermediate-ref
            ├── ·4d3831e (⌂|🏘|1)
            └── :6:►anon:
                ├── ·468357f (⌂|🏘|1)
                ├── :7:►anon:
                │   ├── ·118ddbb (⌂|🏘|1)
                │   └── :9:►anon:
                │       ├── ·619d548 (⌂|🏘|1)
                │       ├── :3:►B
                │       │   ├── ·8a352d5 (⌂|🏘|1)
                │       │   └── →:0:►main
                │       └── :2:►A
                │           ├── ·6fdab32 (⌂|🏘|1)
                │           └── →:0:►main
                └── :8:►branch-on-top
                    ├── ·d3166f7 (⌂|🏘|1)
                    └── →:7:►anon:
    ");

    // We show the original 'native' configuration without pruning anything, even though
    // it contains the workspace commit 619d548.
    // It's up to the caller to deal with this situation as the workspace now is marked differently.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡:3:anon: on bce0c5e {1}
        ├── :3:anon:
        │   └── ·a7131b1 (🏘️)
        ├── :4:intermediate-ref
        │   ├── ·4d3831e (🏘️)
        │   ├── ·468357f (🏘️)
        │   ├── ·118ddbb (🏘️)
        │   └── ·619d548 (🏘️)
        └── 📙:2:B
            └── ·8a352d5 (🏘️)
    ");

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    // The extra-target as would happen in the typical case would change nothing though.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/main
    │   └── :0:►main
    │       ├── ·bce0c5e (⌂|🏘|✓|11)
    │       └── 🏁·3183e43 (⌂|🏘|✓|11)
    └── :4:►gitbutler/workspace
        ├── ·a7131b1 (⌂|🏘|1)
        └── :5:►intermediate-ref
            ├── ·4d3831e (⌂|🏘|1)
            └── :6:►anon:
                ├── ·468357f (⌂|🏘|1)
                ├── :7:►anon:
                │   ├── ·118ddbb (⌂|🏘|1)
                │   └── :9:►anon:
                │       ├── ·619d548 (⌂|🏘|1)
                │       ├── :3:►B
                │       │   ├── ·8a352d5 (⌂|🏘|1)
                │       │   └── →:0:►main
                │       └── :2:►A
                │           ├── ·6fdab32 (⌂|🏘|1)
                │           └── →:0:►main
                └── :8:►branch-on-top
                    ├── ·d3166f7 (⌂|🏘|1)
                    └── →:7:►anon:
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡:3:anon: on bce0c5e {1}
        ├── :3:anon:
        │   └── ·a7131b1 (🏘️)
        ├── :4:intermediate-ref
        │   ├── ·4d3831e (🏘️)
        │   ├── ·468357f (🏘️)
        │   ├── ·118ddbb (🏘️)
        │   └── ·619d548 (🏘️)
        └── 📙:2:B
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/main
    │   └── :0:►main
    │       ├── ·bce0c5e (⌂|🏘|✓|11)
    │       └── 🏁·3183e43 (⌂|🏘|✓|11)
    └── :3:►gitbutler/workspace
        ├── ·da912a8 (⌂|🏘|1)
        └── :4:►intermediate-ref
            ├── ·198eaf8 (⌂|🏘|1)
            └── :5:►anon:
                ├── ·3147997 (⌂|🏘|1)
                ├── :6:►anon:
                │   ├── ·9785229 (⌂|🏘|1)
                │   ├── ·c58f157 (⌂|🏘|1)
                │   └── :2:►A
                │       ├── ·6fdab32 (⌂|🏘|1)
                │       └── →:0:►main
                └── :7:►branch-on-top
                    ├── ·dd7bb9a (⌂|🏘|1)
                    └── →:6:►anon:
    ");

    // Here we'd show what happens if the workspace commit is somewhere in the middle
    // of the segment. This is relevant for code trying to find it, which isn't done here.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️⚠️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡:2:anon: on bce0c5e {0}
        ├── :2:anon:
        │   └── ·da912a8 (🏘️)
        ├── :3:intermediate-ref
        │   ├── ·198eaf8 (🏘️)
        │   ├── ·3147997 (🏘️)
        │   ├── ·9785229 (🏘️)
        │   └── ·c58f157 (🏘️)
        └── 📙:1:A
            └── ·6fdab32 (🏘️)
    ");
    Ok(())
}

#[test]
fn shallow_boundary_below_workspace_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario(
        "special-conditions",
        "shallow-workspace-boundary-below-lower-bound",
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 00e1860 (HEAD -> gitbutler/workspace, origin/gitbutler/workspace, origin/HEAD) GitButler Workspace Commit
    * 6507810 (origin/A, A) A1
    * b625665 (origin/main, main) M4
    * a821094 M3
    * bce0c5e (grafted) M2
    ");

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :1:►origin/gitbutler/workspace
    │   └── 👉:0:►gitbutler/workspace
    │       ├── ·00e1860 (⌂|🏘|1)
    │       └── :4:►A
    │           ├── ·6507810 (⌂|🏘|101)
    │           └── :2:►main
    │               ├── ·b625665 (⌂|🏘|✓|111)
    │               ├── ·a821094 (⌂|🏘|✓|111)
    │               └── ✂·bce0c5e (⌂|🏘|✓|⛰|111)
    ├── :3:►origin/main
    │   └── →:2:►main
    └── :5:►origin/A
        └── →:4:►A
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on b625665
    └── ≡📙:2:A <> origin/A on b625665 {1}
        └── 📙:2:A <> origin/A
            └── ❄️6507810 (🏘️)
    ");

    Ok(())
}

#[test]
fn shallow_boundary_in_workspace_prevents_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario(
        "special-conditions",
        "shallow-workspace-boundary-in-workspace",
    )?;

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── :1:►origin/gitbutler/workspace
        └── 👉:0:►gitbutler/workspace
            ├── ·00e1860 (⌂|🏘|1)
            └── :2:►A
                └── ✂·6507810 (⌂|🏘|⛰|1)
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡📙:1:A {1}
        └── 📙:1:A
            └── ·6507810 (🏘️|⛰)
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    └── 👉:0:►gitbutler/workspace
        ├── ·e82dfab (⌂|🏘|1)
        ├── :1:►B
        │   ├── ·78b1b59 (⌂|🏘|1)
        │   ├── ·f52fcec (⌂|🏘|1)
        │   └── :3:►anon:
        │       ├── ·bce0c5e (⌂|🏘|1)
        │       └── 🏁·3183e43 (⌂|🏘|1)
        └── :2:►A
            ├── ·6fdab32 (⌂|🏘|1)
            └── →:3:►anon:
    ");

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on bce0c5e
    ├── ≡:1:B on bce0c5e
    │   └── :1:B
    │       ├── ·78b1b59 (🏘️)
    │       └── ·f52fcec (🏘️)
    └── ≡:2:A on bce0c5e
        └── :2:A
            └── ·6fdab32 (🏘️)
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    // The same is true if stacks are known in workspace metadata.
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·e82dfab (⌂|🏘|1)
    │   ├── :4:►B
    │   │   ├── ·78b1b59 (⌂|🏘|1)
    │   │   └── :5:►anon:
    │   │       ├── ·f52fcec (⌂|🏘|✓|11)
    │   │       └── :6:►anon:
    │   │           ├── ·bce0c5e (⌂|🏘|✓|11)
    │   │           └── 🏁·3183e43 (⌂|🏘|✓|11)
    │   └── :3:►A
    │       ├── ·6fdab32 (⌂|🏘|1)
    │       └── →:6:►anon:
    └── :2:►origin/main
        └── :1:►main
            ├── ·938e6f2 (⌂|✓|10)
            └── →:5:►anon:
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e
    ├── ≡📙:2:A on bce0c5e {0}
    │   └── 📙:2:A
    │       └── ·6fdab32 (🏘️)
    └── ≡📙:3:B on bce0c5e {1}
        └── 📙:3:B
            └── ·78b1b59 (🏘️)
    ");

    // Finally, if the extra-target, indicating an old stored base that isn't valid anymore.
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, ":/M3"),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·e82dfab (⌂|🏘|1)
    │   ├── :5:►B
    │   │   ├── ·78b1b59 (⌂|🏘|1)
    │   │   └── :3:►anon:
    │   │       ├── ·f52fcec (⌂|🏘|✓|11)
    │   │       └── :6:►anon:
    │   │           ├── ·bce0c5e (⌂|🏘|✓|11)
    │   │           └── 🏁·3183e43 (⌂|🏘|✓|11)
    │   └── :4:►A
    │       ├── ·6fdab32 (⌂|🏘|1)
    │       └── →:6:►anon:
    └── :2:►origin/main
        └── :1:►main
            ├── ·938e6f2 (⌂|✓|10)
            └── →:3:►anon:
    ");

    // The base is still adjusted so it matches the actual stacks. With the extra-target
    // resolved as the target commit, the integrated `f52fcec` is at the target and is
    // pruned - consistent with the no-extra-target case above.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e
    ├── ≡📙:3:A on bce0c5e {0}
    │   └── 📙:3:A
    │       └── ·6fdab32 (🏘️)
    └── ≡📙:4:B on f52fcec {1}
        └── 📙:4:B
            └── ·78b1b59 (🏘️)
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·c5587c9 (⌂|🏘|1)
    │   ├── :1:►B
    │   │   ├── ·ce25240 (⌂|🏘|1)
    │   │   └── :5:►anon:
    │   │       ├── ·bce0c5e (⌂|🏘|11)
    │   │       └── 🏁·3183e43 (⌂|🏘|11)
    │   └── :2:►A
    │       ├── ·de6d39c (⌂|🏘|1)
    │       └── :3:►main
    │           ├── ·a821094 (⌂|🏘|11)
    │           └── →:5:►anon:
    └── :4:►origin/main
        └── →:3:►main
    ");

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    // Interestingly, A now gets to see integrated parts of the target branch.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on bce0c5e
    ├── ≡:1:B on bce0c5e
    │   └── :1:B
    │       └── ·ce25240 (🏘️)
    └── ≡:2:A on bce0c5e
        ├── :2:A
        │   └── ·de6d39c (🏘️)
        └── :3:main <> origin/main
            └── ❄️a821094 (🏘️)
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·a0385a8 (⌂|🏘|1)
    │   ├── :7:►below-C
    │   │   └── :8:►below-below-C
    │   │       └── :1:►main
    │   │           └── 🏁·3183e43 (⌂|🏘|✓|11)
    │   ├── :4:►B
    │   │   └── :5:►below-B
    │   │       └── :6:►below-below-B
    │   │           └── →:1:►main
    │   ├── :2:►below-A
    │   │   └── :3:►below-below-A
    │   │       └── →:1:►main
    │   ├── :12:►C
    │   │   └── :13:►C2-1
    │   │       └── :14:►C2-2
    │   │           └── :15:►C2-3
    │   │               └── :11:►anon:
    │   │                   ├── ·f9e2cb7 (⌂|🏘|1)
    │   │                   └── :16:►anon:
    │   │                       ├── ·aaa195b (⌂|🏘|1)
    │   │                       └── →:1:►main
    │   └── :10:►A
    │       ├── ·49d4b34 (⌂|🏘|1)
    │       └── →:1:►main
    ├── :9:►origin/main
    │   └── →:1:►main
    └── :17:►C1-3
        └── :18:►C1-2
            └── :19:►C1-1
                └── →:16:►anon:
    ");

    // Both stacks will look the same, with the dependent branch inserted at the very bottom.
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:2:A on 3183e43 {1}
    │   ├── 📙:2:A
    │   │   └── ·49d4b34 (🏘️)
    │   ├── 📙:6:below-A
    │   └── 📙:7:below-below-A
    ├── ≡📙:8:B on 3183e43 {2}
    │   ├── 📙:8:B
    │   ├── 📙:9:below-B
    │   └── 📙:10:below-below-B
    └── ≡📙:11:C on 3183e43 {3}
        ├── 📙:11:C
        ├── 📙:12:C2-1
        ├── 📙:13:C2-2
        ├── 📙:3:C2-3
        │   └── ·f9e2cb7 (🏘️)
        ├── 📙:14:C1-3
        ├── 📙:15:C1-2
        ├── 📙:16:C1-1
        │   └── ·aaa195b (🏘️)
        ├── 📙:17:below-C
        └── 📙:18:below-below-C
    ");

    let wrongly_inactive = StackState::Inactive;
    add_stack_with_segments(
        &mut meta,
        1,
        "A",
        wrongly_inactive,
        &["below-A", "below-below-A"],
    );
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    // The stack-id could still be found, even though `A` is wrongly marked as outside the workspace.
    // Below A doesn't apply as it's marked inactive.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    ├── ≡📙:6:B on 3183e43 {2}
    │   ├── 📙:6:B
    │   ├── 📙:7:below-B
    │   └── 📙:8:below-below-B
    ├── ≡📙:9:C on 3183e43 {3}
    │   ├── 📙:9:C
    │   ├── 📙:10:C2-1
    │   ├── 📙:11:C2-2
    │   ├── 📙:2:C2-3
    │   │   └── ·f9e2cb7 (🏘️)
    │   ├── 📙:12:C1-3
    │   ├── 📙:13:C1-2
    │   ├── 📙:14:C1-1
    │   │   └── ·aaa195b (🏘️)
    │   ├── 📙:15:below-C
    │   └── 📙:16:below-below-C
    └── ≡📙:4:A on 3183e43 {1}
        └── 📙:4:A
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

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
    └── ≡📙:8:A <> origin/A⇣1 on 1ee1e34 {1}
        └── 📙:8:A <> origin/A⇣1
            └── 🟣2181501
    ");

    Ok(())
}

#[test]
fn remote_and_integrated_tracking_branch_on_linear_segment() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/remote-and-integrated-tracking-linear")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 21e584f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 8dc508f (origin/main, main) M-advanced
    |/  
    | * 197ddce (origin/A) A-remote
    |/  
    * 081bae9 (A) M-base
    * 3183e43 M1
    ");
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
    └── ≡📙:5:A <> origin/A⇣1 on 081bae9 {1}
        └── 📙:5:A <> origin/A⇣1
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
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
    └── ≡📙:2:A <> origin/A⇡1⇣1 on 1ee1e34 {1}
        └── 📙:2:A <> origin/A⇡1⇣1
            ├── 🟣2181501
            └── ·9f47a25 (🏘️)
    ");

    Ok(())
}

#[test]
fn unapplied_branch_on_base() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/unapplied-branch-on-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * fafd9d0 (origin/main, unapplied, main) init
    ");
    add_workspace(&mut meta);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·a26ae77 (⌂|🏘|1)
    │   └── :1:►main
    │       └── 🏁·fafd9d0 (⌂|🏘|✓|11) ►unapplied
    └── :2:►origin/main
        └── →:1:►main
    ");

    // if the branch was never seen, it's not visible as one would expect.
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0");

    // An applied branch would be present, but has no commit.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:unapplied on fafd9d0 {1}
        └── 📙:3:unapplied
    ");

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;

    // This will be an empty workspace.
    insta::assert_snapshot!(graph_workspace(&graph), @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0");

    Ok(())
}

#[test]
fn shared_target_base_keeps_exact_target_segment_with_inactive_unapplied_branch()
-> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/target-shared-with-unapplied-and-origin-head")?;
    add_workspace(&mut meta);
    add_stack_with_segments(&mut meta, 1, "survivor", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "unapplied", StackState::Inactive, &[]);

    let target_ref: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
    let target_head_ref: gix::refs::FullName = "refs/remotes/origin/HEAD".try_into()?;

    assert!(
        repo.try_find_reference(target_ref.as_ref())?.is_some(),
        "fixture must contain {target_ref}",
    );
    assert!(
        repo.try_find_reference(target_head_ref.as_ref())?.is_some(),
        "fixture must contain {target_head_ref}",
    );

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·20f65b7 (⌂|🏘|1)
    │   └── :4:►survivor
    │       ├── ·4ca0966 (⌂|🏘|1)
    │       ├── ·a3b180e (⌂|🏘|1)
    │       └── :1:►unapplied
    │           ├── ·ce09734 (⌂|🏘|✓|11) ►base-peer, ►base-peer-1, ►base-peer-2, ►base-peer-3, ►base-peer-4, ►base-peer-5, ►base-peer-6, ►base-peer-7, ►base-peer-8
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    ├── :2:►origin/HEAD
    │   └── →:1:►unapplied
    ├── :3:►origin/main
    │   └── →:1:►unapplied
    └── :5:►main
        └── →:1:►unapplied
    ");
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ce09734
    └── ≡📙:2:survivor on ce09734 {1}
        └── 📙:2:survivor
            ├── ·4ca0966 (🏘️)
            └── ·a3b180e (🏘️)
    ");

    assert_eq!(
        ws.target_ref.as_ref().map(|t| t.ref_name.as_ref()),
        Some(target_ref.as_ref()),
        "expected workspace target_ref to resolve from exact target segment"
    );

    // When it's applied, it will show up though.
    add_stack_with_segments(&mut meta, 2, "unapplied", StackState::InWorkspace, &[]);
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ce09734
    └── ≡📙:2:survivor on ce09734 {1}
        └── 📙:2:survivor
            ├── ·4ca0966 (🏘️)
            └── ·a3b180e (🏘️)
    ");

    Ok(())
}

#[test]
fn unapplied_branch_on_base_no_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/unapplied-branch-on-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * fafd9d0 (origin/main, unapplied, main) init
    ");
    add_workspace(&mut meta);
    remove_target(&mut meta);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·a26ae77 (⌂|🏘|1)
    │   └── :1:►main
    │       └── 🏁·fafd9d0 (⌂|🏘|11) ►unapplied
    └── :2:►origin/main
        └── →:1:►main
    ");

    // the main branch is disambiguated by its remote reference.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓!
    └── ≡:1:main <> origin/main
        └── :1:main <> origin/main
            └── ❄️fafd9d0 (🏘️) ►unapplied
    ");

    // The 'unapplied' branch can be added on top of that, and we make clear we want `main` as well.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "main", StackState::InWorkspace, &[]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·a26ae77 (⌂|🏘|1)
    │   └── :2:►unapplied
    │       └── :1:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    └── :3:►origin/main
        └── →:1:►main
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:unapplied on fafd9d0 {1}
    │   └── 📙:3:unapplied
    └── ≡📙:4:main <> origin/main on fafd9d0 {2}
        └── 📙:4:main <> origin/main
    ");

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;

    // Now only `main` shows up.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:main <> origin/main on fafd9d0 {2}
        └── 📙:3:main <> origin/main
    ");

    Ok(())
}

#[test]
fn no_ws_commit_two_branches_no_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-ws-ref-no-ws-commit-two-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * bce0c5e (HEAD -> gitbutler/workspace, origin/main, main, B, A) M2
    * 3183e43 M1
    ");
    remove_target(&mut meta);
    add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), "notably the target ref and local tracking branch have sibling links setup", @"

    ├── :1:►A
    │   └── :0:►main
    │       └── ✂·bce0c5e (⌂|🏘|✓|1) ►B
    ├── :2:►origin/main
    │   └── →:0:►main
    └── 👉:3:►gitbutler/workspace
        └── →:0:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), "sibling links between origin/main and main are also set", @"
    📕🏘️⚠️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    ├── ≡📙:3:main <> origin/main {0}
    │   └── 📙:3:main <> origin/main
    └── ≡📙:4:A {1}
        └── 📙:4:A
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·a5f94a2 (⌂|🏘|1)
    │   ├── :3:►A
    │   │   ├── ·081bae9 (⌂|🏘|✓|1111) ►A-inside[📁wt-A-inside], ►A-outside[📁wt-A-outside]
    │   │   └── 🏁·3183e43 (⌂|🏘|✓|1111)
    │   └── :5:►B
    │       ├── ·3e01e28 (⌂|🏘|1)
    │       └── →:3:►A
    ├── :2:►origin/main
    │   └── :1:►main
    │       ├── ·8dc508f (⌂|✓|10)
    │       └── →:3:►A
    └── :4:►origin/A
        ├── ·197ddce (0x0|1000)
        └── →:3:►A
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳@repo] <> ✓refs/remotes/origin/main⇣1 on 081bae9
    ├── ≡📙:6:A <> origin/A⇣1 on 081bae9 {0}
    │   └── 📙:6:A <> origin/A⇣1
    │       └── 🟣197ddce
    └── ≡:4:B[📁wt-B-inside] on 081bae9
        └── :4:B[📁wt-B-inside]
            └── ·3e01e28 (🏘️)
    ");

    let linked_repo = gix::open_opts(
        repo.path()
            .parent()
            .expect("repository git dir is inside the worktree")
            .join("wt-B-inside"),
        gix::open::Options::isolated(),
    )?
    .with_object_memory();
    let graph = but_graph::Workspace::from_head(
        &linked_repo,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), "when the graph is built from the B linked worktree repository, the workspace remains visible but the B worktree owns the entrypoint branch", @"

    ├── :1:►gitbutler/workspace
    │   ├── ·a5f94a2 (⌂|🏘)
    │   ├── :4:►A
    │   │   ├── ·081bae9 (⌂|🏘|✓|1111) ►A-inside[📁wt-A-inside], ►A-outside[📁wt-A-outside]
    │   │   └── 🏁·3183e43 (⌂|🏘|✓|1111)
    │   └── 👉:0:►B
    │       ├── ·3e01e28 (⌂|🏘|1)
    │       └── →:4:►A
    ├── :3:►origin/main
    │   └── :2:►main
    │       ├── ·8dc508f (⌂|✓|10)
    │       └── →:4:►A
    └── :5:►origin/A
        ├── ·197ddce (0x0|1000)
        └── →:4:►A
    ");

    insta::assert_snapshot!(graph_workspace(&graph), "workspace projection should keep the linked-worktree ownership marker on the focused stack while leaving the workspace ref itself unowned", @"
    📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
    ├── ≡📙:6:A <> origin/A⇣1 on 081bae9 {0}
    │   └── 📙:6:A <> origin/A⇣1
    │       └── 🟣197ddce
    └── ≡👉:0:B[📁wt-B-inside@repo] on 081bae9
        └── 👉:0:B[📁wt-B-inside@repo]
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
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·f18d244 (⌂|🏘|1)
    │   ├── :2:►A
    │   │   └── :1:►main
    │   │       └── 🏁·fafd9d0 (⌂|🏘|✓|11) ►B
    │   └── →:2:►A
    └── :3:►origin/main
        └── →:1:►main
    ");

    // Branch should be visible in workspace once.
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        └── 📙:3:A
    ");

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:A on fafd9d0 {1}
    │   └── 📙:3:A
    └── ≡📙:4:B on fafd9d0 {2}
        └── 📙:4:B
    ");

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·f18d244 (⌂|🏘|1)
    │   ├── :3:►A
    │   │   └── :2:►main
    │   │       └── 🏁·fafd9d0 (⌂|🏘|✓|11) ►B
    │   └── →:3:►A
    └── :1:►origin/main
        ├── ·12b42b0 (✓)
        └── →:2:►main
    ");

    // Branch should be visible in workspace once.
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        └── 📙:3:A
    ");

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:A on fafd9d0 {1}
    │   └── 📙:3:A
    └── ≡📙:4:B on fafd9d0 {2}
        └── 📙:4:B
    ");

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws.redo_traversal_into_workspace_with_overlay(&repo, &*meta, Overlay::default())?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        ├── 📙:3:A
        └── 📙:4:B
    ");

    // With extra-target these cases work as well
    meta.data_mut().branches.clear();
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:A on fafd9d0 {1}
    │   └── 📙:3:A
    └── ≡📙:4:B on fafd9d0 {2}
        └── 📙:4:B
    ");

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    └── ≡📙:3:A on fafd9d0 {1}
        ├── 📙:3:A
        └── 📙:4:B
    ");

    Ok(())
}

mod edit_commit {
    use but_testsupport::{branch_tree, graph_workspace, visualize_commit_graph_all};

    use super::project_meta;
    use crate::init::{add_workspace, id_at, read_only_in_memory_scenario, standard_options};

    #[test]
    fn applied_stack_below_explicit_lower_bound() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("ws/edit-commit/simple")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        * a62b0de (A) A2
        * 120a217 (gitbutler/edit) A1
        * fafd9d0 (origin/main, main) init
        ");

        add_workspace(&mut meta);
        let graph = but_graph::Workspace::from_head(
            &repo,
            &*meta,
            project_meta(&*meta),
            standard_options(),
        )?;
        insta::assert_snapshot!(branch_tree(&graph), @"

        ├── 👉:0:►gitbutler/workspace
        │   ├── ·3ea2742 (⌂|🏘|1)
        │   └── :3:►A
        │       ├── ·a62b0de (⌂|🏘|1)
        │       └── :4:►gitbutler/edit
        │           ├── ·120a217 (⌂|🏘|1)
        │           └── :1:►main
        │               └── 🏁·fafd9d0 (⌂|🏘|✓|11)
        └── :2:►origin/main
            └── →:1:►main
        ");

        // special branch names are skipped by default and entirely invisible.
        insta::assert_snapshot!(graph_workspace(&graph), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
        └── ≡:2:A on fafd9d0
            └── :2:A
                ├── ·a62b0de (🏘️)
                └── ·120a217 (🏘️)
        ");

        // However, if the HEAD points to that reference…
        let (id, ref_name) = id_at(&repo, "gitbutler/edit");
        let graph = but_graph::Workspace::from_commit_traversal(
            id,
            ref_name,
            &*meta,
            project_meta(&*meta),
            standard_options(),
        )?;
        insta::assert_snapshot!(branch_tree(&graph), @"

        ├── :1:►gitbutler/workspace
        │   ├── ·3ea2742 (⌂|🏘)
        │   └── :4:►A
        │       ├── ·a62b0de (⌂|🏘)
        │       └── 👉:0:►gitbutler/edit
        │           ├── ·120a217 (⌂|🏘|1)
        │           └── :2:►main
        │               └── 🏁·fafd9d0 (⌂|🏘|✓|11)
        └── :3:►origin/main
            └── →:2:►main
        ");
        // …then the segment becomes visible.
        insta::assert_snapshot!(graph_workspace(&graph), @"
        📕🏘️:1:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
        └── ≡:3:A on fafd9d0
            ├── :3:A
            │   └── ·a62b0de (🏘️)
            └── 👉:5:gitbutler/edit
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣10 on 68e62aa
    └── ≡📙:13:reimplement-insert-blank-commit on 68e62aa {0}
        ├── 📙:13:reimplement-insert-blank-commit
        └── 📙:2:reconstructed-insert-blank-commit-branch
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

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·d77ecda (⌂|🏘|1)
    │   ├── :3:►A
    │   │   ├── ·81d4e38 (⌂|🏘|1)
    │   │   └── :1:►main
    │   │       └── 🏁·e32cf47 (⌂|🏘|✓|11)
    │   └── :4:►B
    │       ├── ·7163661 (⌂|🏘|1)
    │       └── →:3:►A
    └── :2:►origin/main
        └── →:1:►main
    ");

    // The sibling ID is not set, and we see only two stacks: B owns 7163661,
    // and both A and B include the shared base commit 81d4e38 (A only has 81d4e38).
    let ws = &graph;
    insta::assert_snapshot!(graph_workspace(ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e32cf47
    ├── ≡📙:2:A on e32cf47 {0}
    │   └── 📙:2:A
    │       └── ·81d4e38 (🏘️)
    └── ≡📙:3:B on e32cf47 {1}
        └── 📙:3:B
            ├── ·7163661 (🏘️)
            └── ·81d4e38 (🏘️)
    ");

    Ok(())
}

/// A stack where a local merge commit at the bottom is already integrated into
/// origin/main (the same PR was merged upstream). The merge commit is kept
/// because it is above the workspace target — integrated commits are only
/// pruned at or below the target.
#[test]
fn integrated_merge_at_bottom_is_kept() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/integrated-merge-at-bottom")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 732604f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 66ea651 (local-stack) D
    * e5a88a7 C
    *   0b3ccaf Merge pull request #1 from fix
    |\  
    | | * f46830d (origin/main, main) Merge pull request #1 from fix
    | |/| 
    |/|/  
    | * f5f42e0 (fix) fix
    |/  
    * fafd9d0 init
    ");

    add_stack_with_segments(&mut meta, 0, "local-stack", StackState::InWorkspace, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on f5f42e0
    └── ≡📙:2:local-stack {0}
        └── 📙:2:local-stack
            ├── ·66ea651 (🏘️)
            ├── ·e5a88a7 (🏘️)
            └── ·0b3ccaf (🏘️)
    ");

    Ok(())
}

/// A branch that has a commit, merges main into itself, then has another commit.
/// The fork-point approach finds the original divergence point, so all branch
/// commits (including those below the merge-from-main) remain visible.
#[test]
fn merge_from_main_keeps_all_branch_commits() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/merge-from-main-in-branch")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 891e228 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cd76046 (my-branch) branch-commit-2
    *   f8ff9a3 Merge main into my-branch
    |\  
    | * ef56fab (origin/main, main) main-advance
    * | 6f65768 branch-commit-1
    |/  
    * fafd9d0 init
    ");

    add_stack_with_segments(&mut meta, 0, "my-branch", StackState::InWorkspace, &[]);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·891e228 (⌂|🏘|1)
    │   └── :3:►my-branch
    │       ├── ·cd76046 (⌂|🏘|1)
    │       └── :4:►anon:
    │           ├── ·f8ff9a3 (⌂|🏘|1)
    │           ├── :5:►anon:
    │           │   ├── ·6f65768 (⌂|🏘|1)
    │           │   └── :6:►anon:
    │           │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    │           └── :1:►main
    │               ├── ·ef56fab (⌂|🏘|✓|11)
    │               └── →:6:►anon:
    └── :2:►origin/main
        └── →:1:►main
    ");

    // The fork-point approach correctly finds the original divergence point (fafd9d0)
    // instead of the moved merge base (ef56fab), so all 3 branch commits are visible:
    // branch-commit-2, the merge commit, and branch-commit-1.
    let ws = graph;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ef56fab
    └── ≡📙:2:my-branch {0}
        └── 📙:2:my-branch
            ├── ·cd76046 (🏘️)
            ├── ·f8ff9a3 (🏘️)
            └── ·6f65768 (🏘️)
    ");

    Ok(())
}

/// A branch whose commits are integrated (reachable from origin/main after
/// upstream merged them) but the workspace target hasn't advanced yet.
/// Integrated commits above the target must be kept so `integrate_upstream`
/// can detect them. Once the target advances past them, they are pruned.
#[test]
fn integrated_commits_above_target_are_kept() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/integrated-above-target")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 7786959 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | *   1af5972 (origin/main, main) Merge branch my-branch
    | |\  
    | |/  
    |/|   
    * | 312f819 (my-branch) B
    * | e255adc A
    |/  
    * fafd9d0 init
    ");

    let init_id = repo.rev_parse_single("main~1")?.detach();
    add_workspace_with_target(&mut meta, init_id);
    add_stack_with_segments(&mut meta, 0, "my-branch", StackState::InWorkspace, &[]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    // With the target at "init", A and B are above the target and should be
    // kept even though they are marked integrated.
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    └── ≡📙:3:my-branch on fafd9d0 {0}
        └── 📙:3:my-branch
            ├── ·312f819 (🏘️|✓)
            └── ·e255adc (🏘️|✓)
    ");

    // Now advance the target to origin/main (which includes the merge).
    // Both commits are at or below the new target and should be pruned,
    // but the metadata-tracked branch entry is preserved.
    let main_id = repo.rev_parse_single("main")?.detach();
    add_workspace_with_target(&mut meta, main_id);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 312f819
    └── ≡📙:5:my-branch on 312f819 {0}
        └── 📙:5:my-branch
    ");

    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_hard_limit(usize::MAX),
    )?;
    assert!(
        !graph.hard_limit_hit,
        "pruning integrated tips should not report a hard-limit traversal stop"
    );

    Ok(())
}

/// Regression: an old branch applied below the stored target drags the workspace base
/// below it, exposing the integrated trunk between base and target. Those commits must be
/// pruned even though `origin/main` has advanced past the target - which previously
/// disabled integrated-commit pruning entirely.
#[test]
fn integrated_commits_below_target_pruned_when_upstream_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/integrated-below-target-upstream-ahead")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   aca392b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * f458f7d (old-branch) O
    * | f5055a1 (my-branch) W
    | | * 7282cb5 (origin/main, main) upstream
    | |/  
    |/|   
    * | 2121f9c target
    |/  
    * 322cb14 base
    * fafd9d0 init
    ");

    // Stored target is 'target' (main~1); origin/main is one commit ahead at 'upstream'.
    let target_id = repo.rev_parse_single("main~1")?.detach();
    add_workspace_with_target(&mut meta, target_id);
    add_stack_with_segments(&mut meta, 0, "my-branch", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "old-branch", StackState::InWorkspace, &[]);

    // 'W' and 'O' are above/beside the target and kept; 'target' and 'base' are
    // integrated and at or below the target, so they are pruned from both stacks
    // even though origin/main has advanced past the target.
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 322cb14
    ├── ≡📙:3:my-branch on 2121f9c {0}
    │   └── 📙:3:my-branch
    │       └── ·f5055a1 (🏘️)
    └── ≡📙:4:old-branch on 322cb14 {1}
        └── 📙:4:old-branch
            └── ·f458f7d (🏘️)
    ");
    Ok(())
}

/// A branch that forks below the target and catches up via `merge origin/main`, so the
/// target enters X only through the merge's second parent (off X's first-parent spine).
/// X is floored at its fork point - where its own first-parent work meets the trunk - so
/// the trunk below the fork (`c1`, `init`) is pruned, leaving X's own commits.
#[test]
fn catchup_merge_below_target_floors_at_fork() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/catchup-merge-leak")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 254106a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f210f41 (X) x2
    *   f8cd0ce catch up to origin/main
    |\  
    | * 0975125 (origin/main, main) U
    | * a7db886 B
    | * d263f88 T
    | * 8bd7dc1 c2
    * | 4eec82a x1
    |/  
    * b4bd43f c1
    * fafd9d0 init
    ");

    // Stored target is 'T' (main~2); origin/main is two commits ahead at 'U'.
    let target_id = repo.rev_parse_single("main~2")?.detach();
    add_workspace_with_target(&mut meta, target_id);
    add_stack_with_segments(&mut meta, 0, "X", StackState::InWorkspace, &[]);

    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on d263f88
    └── ≡📙:3:X on b4bd43f {0}
        └── 📙:3:X
            ├── ·f210f41 (🏘️)
            ├── ·f8cd0ce (🏘️)
            └── ·4eec82a (🏘️)
    ");
    Ok(())
}

/// A non-workspace ref (tag) points at the workspace commit itself,
/// and that ref is used as the entrypoint for traversal.
/// This verifies that the entrypoint is correctly identified even when it
/// coincides with the workspace commit.
#[test]
fn entrypoint_on_workspace_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/entrypoint-on-workspace-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 3ea2742 (HEAD -> gitbutler/workspace, tag: my-tag) GitButler Workspace Commit
    * a62b0de (A) A2
    * 120a217 A1
    * fafd9d0 (origin/main, main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·3ea2742 (⌂|🏘|1) ►my-tag
    │   └── :3:►A
    │       ├── ·a62b0de (⌂|🏘|1)
    │       ├── ·120a217 (⌂|🏘|1)
    │       └── :1:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    └── :2:►origin/main
        └── →:1:►main
    ");

    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:2:A on fafd9d0
        └── :2:A
            ├── ·a62b0de (🏘️)
            └── ·120a217 (🏘️)
    ");

    // Now traverse from the tag that points at the workspace commit.
    let (id, name) = id_at(&repo, "my-tag");
    let graph = but_graph::Workspace::from_commit_traversal(
        id,
        name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── :0:►gitbutler/workspace
    │   ├── ·3ea2742 (⌂|🏘|1) ►my-tag
    │   └── :3:►A
    │       ├── ·a62b0de (⌂|🏘|1)
    │       ├── ·120a217 (⌂|🏘|1)
    │       └── :1:►main
    │           └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    └── :2:►origin/main
        └── →:1:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:2:A on fafd9d0
        └── :2:A
            ├── ·a62b0de (🏘️)
            └── ·120a217 (🏘️)
    ");
    Ok(())
}

/// A workspace where the local branch was deleted, leaving only origin/A.
/// The workspace commit still references the old branch tip as a parent.
/// This probes whether a remote-only segment at the top of a stack is handled
/// correctly (previously protected by front-pruning workaround).
#[test]
fn remote_only_stack_top() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-only-stack-top")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a62b0de (origin/A) A2
    * 120a217 A1
    * fafd9d0 (origin/main, main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·3ea2742 (⌂|🏘|1)
    │   ├── ·a62b0de (⌂|🏘|1)
    │   ├── ·120a217 (⌂|🏘|1)
    │   └── :1:►main
    │       └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    └── :2:►origin/main
        └── →:1:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:3:anon: on fafd9d0
        └── :3:anon:
            ├── ·a62b0de (🏘️)
            └── ·120a217 (🏘️)
    ");
    Ok(())
}

/// A local branch B is stacked on top of a remote-only origin/A (no local A).
/// origin/A's commits are on the first-parent path between B and main.
/// This probes whether a remote-only segment appearing after a local segment
/// in a stack is handled correctly (previously protected by tail-pruning workaround).
#[test]
fn remote_trailing_local_stack() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-trailing-local-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 5638b41 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cb7021b (B) B2
    * ce3278a B1
    * a62b0de (origin/A) A2
    * 120a217 A1
    * fafd9d0 (origin/main, main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·5638b41 (⌂|🏘|1)
    │   └── :3:►B
    │       ├── ·cb7021b (⌂|🏘|1)
    │       └── 🏁·ce3278a (⌂|🏘|1)
    └── :2:►origin/main
        └── :1:►main
            └── 🏁·fafd9d0 (⌂|✓|10)
    ");
    insta::assert_snapshot!(graph_workspace(&graph), "this is a weird state as the target is actually disjoint from the workspace - it appears empty now", @"📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on cb7021b");
    Ok(())
}

/// A workspace that merges a remote-only branch (origin/A) with no local counterpart.
/// Unlike `remote_only_stack_top` where the local was deleted after workspace creation,
/// here the local never existed. This tests whether the `is_pruned` check correctly
/// handles a stack that starts with a remote-only segment.
#[test]
fn remote_ref_as_stack_top() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-ref-as-stack-top")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   21bff1f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * a62b0de (origin/A) A2
    | * 120a217 A1
    |/  
    * fafd9d0 (origin/main, main) init
    ");

    add_workspace(&mut meta);
    let graph =
        but_graph::Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    insta::assert_snapshot!(branch_tree(&graph), @"

    ├── 👉:0:►gitbutler/workspace
    │   ├── ·21bff1f (⌂|🏘|1)
    │   ├── :1:►main
    │   │   └── 🏁·fafd9d0 (⌂|🏘|✓|11)
    │   └── :3:►origin/A
    │       ├── ·a62b0de (⌂|🏘|1)
    │       ├── ·120a217 (⌂|🏘|1)
    │       └── →:1:►main
    └── :2:►origin/main
        └── →:1:►main
    ");
    insta::assert_snapshot!(graph_workspace(&graph), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:2:anon: on fafd9d0
        └── :2:anon:
            ├── ·a62b0de (🏘️)
            └── ·120a217 (🏘️)
    ");
    Ok(())
}
