use crate::graph_tree;
use crate::init::utils::{add_workspace_without_target, standard_options_with_extra_target};
use crate::init::{StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev};
use crate::init::{read_only_in_memory_scenario, standard_options};
use crate::vis::utils::graph_workspace;
use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;

#[test]
fn single_stack_ambigous() -> anyhow::Result<()> {
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️|1)
    │       └── ►:3[1]:B <> origin/B →:4:
    │           ├── ·70e9a36 (⌂|🏘️|101)
    │           ├── ·320e105 (⌂|🏘️|101) ►tags/without-ref
    │           └── ·2a31450 (⌂|🏘️|101) ►B-empty, ►ambiguous-01
    │               └── ►:4[2]:origin/B →:3:
    │                   └── ·70bde6b (⌂|🏘️|101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // All non-integrated segments are visible.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           └── ·70e9a36 (⌂|🏘️|100)
    │               └── 👉►:0[2]:tags/without-ref
    │                   ├── ·320e105 (⌂|🏘️|101)
    │                   └── ·2a31450 (⌂|🏘️|101) ►B-empty, ►ambiguous-01
    │                       └── ►:5[3]:origin/B →:4:
    │                           └── ·70bde6b (⌂|🏘️|101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    └── ►:2[0]:origin/main →:3:
        └── →:3: (main →:2:)
    ");
    // Now `HEAD` is outside a workspace, which goes to single-branch mode. But it knows it's in a workspace
    // and shows the surrounding parts, while marking the segment as entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           └── ·70e9a36 (⌂|🏘️|100)
    │               └── ►:0[2]:anon:
    │                   ├── 👉·320e105 (⌂|🏘️|101) ►tags/without-ref
    │                   └── ·2a31450 (⌂|🏘️|101) ►B-empty, ►ambiguous-01
    │                       └── ►:6[3]:anon:
    │                           └── ·70bde6b (⌂|🏘️|101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:5[0]:origin/B →:4:
        └── →:6:
    ");

    // Entrypoint is now unnamed (as no ref-name was provided for traversal)
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           ├── ·70e9a36 (⌂|🏘️|100)
    │           └── ·320e105 (⌂|🏘️|100) ►tags/without-ref
    │               └── ►:0[2]:anon:
    │                   └── 👉·2a31450 (⌂|🏘️|101) ►B-empty, ►ambiguous-01
    │                       └── ►:6[3]:anon:
    │                           └── ·70bde6b (⌂|🏘️|101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:5[0]:origin/B →:4:
        └── →:6:
    ");

    // Doing this is very much like edit mode, and there is always a segment starting at the entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️)
    │       └── ►:4[1]:B <> origin/B →:5:
    │           ├── ·70e9a36 (⌂|🏘️|100)
    │           └── ·320e105 (⌂|🏘️|100) ►tags/without-ref
    │               └── 👉►:0[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘️|101) ►ambiguous-01
    │                       └── ►:6[3]:anon:
    │                           └── ·70bde6b (⌂|🏘️|101) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03
    │                               └── ►:3[4]:main <> origin/main →:2:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:5[0]:origin/B →:4:
        └── →:6:
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️|1)
    │       └── 📙►:4[1]:B <> origin/B →:6:
    │           ├── ·70e9a36 (⌂|🏘️|101)
    │           └── ·320e105 (⌂|🏘️|101) ►tags/without-ref
    │               └── 📙►:3[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘️|101) ►ambiguous-01
    │                       └── 📙►:5[3]:A-empty-03
    │                           └── 📙►:7[4]:A-empty-01
    │                               └── 📙►:8[5]:A
    │                                   └── ·70bde6b (⌂|🏘️|101) ►A-empty-02
    │                                       └── ►:2[6]:main <> origin/main →:1:
    │                                           └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:6[0]:origin/B →:4:
        └── →:5: (A-empty-03)
    ");

    // We pickup empty segments.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:4:B <> origin/B →:6:⇡2 on fafd9d0
        ├── 📙:4:B <> origin/B →:6:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:3:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        ├── 📙:5:A-empty-03
        ├── 📙:7:A-empty-01
        └── 📙:8:A
            └── ❄70bde6b (🏘️) ►A-empty-02
    ");

    // Now something similar but with two stacks.
    // As the actual topology is different, we can't really comply with that's desired.
    // Instead, we re-use as many of the named segments as possible, even if they are from multiple branches.
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️|1)
    │       └── 📙►:4[1]:B <> origin/B →:6:
    │           ├── ·70e9a36 (⌂|🏘️|101)
    │           └── ·320e105 (⌂|🏘️|101) ►tags/without-ref
    │               └── 📙►:3[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘️|101) ►ambiguous-01
    │                       └── 📙►:5[3]:A-empty-03
    │                           └── 📙►:7[4]:A-empty-02
    │                               └── 📙►:8[5]:A-empty-01
    │                                   └── 📙►:9[6]:A
    │                                       └── ·70bde6b (⌂|🏘️|101)
    │                                           └── ►:2[7]:main <> origin/main →:1:
    │                                               └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:6[0]:origin/B →:4:
        └── →:5: (A-empty-03)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:4:B <> origin/B →:6:⇡2 on fafd9d0
        ├── 📙:4:B <> origin/B →:6:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:3:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        ├── 📙:5:A-empty-03
        ├── 📙:7:A-empty-02
        ├── 📙:8:A-empty-01
        └── 📙:9:A
            └── ❄70bde6b (🏘️)
    ");

    // Define only some of the branches, it should figure that out.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-empty-01"]);
    add_stack_with_segments(&mut meta, 1, "B-empty", StackState::InWorkspace, &["B"]);

    let (id, ref_name) = id_at(&repo, "A-empty-01");
    let graph = Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·20de6ee (⌂|🏘️)
    │       └── 📙►:5[1]:B <> origin/B →:6:
    │           ├── ·70e9a36 (⌂|🏘️|100)
    │           └── ·320e105 (⌂|🏘️|100) ►tags/without-ref
    │               └── 📙►:4[2]:B-empty
    │                   └── ·2a31450 (⌂|🏘️|100) ►ambiguous-01
    │                       └── 👉📙►:0[3]:A-empty-01
    │                           └── 📙►:7[4]:A
    │                               └── ·70bde6b (⌂|🏘️|101) ►A-empty-02, ►A-empty-03
    │                                   └── ►:3[5]:main <> origin/main →:2:
    │                                       └── ·fafd9d0 (⌂|🏘️|✓|111) ►new-A, ►new-B
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:6[0]:origin/B →:5:
        └── →:0: (A-empty-01)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:5:B <> origin/B →:6:⇡2 on fafd9d0
        ├── 📙:5:B <> origin/B →:6:⇡2
        │   ├── ·70e9a36 (🏘️)
        │   └── ·320e105 (🏘️) ►tags/without-ref
        ├── 📙:4:B-empty
        │   └── ·2a31450 (🏘️) ►ambiguous-01
        ├── 👉📙:0:A-empty-01
        └── 📙:7:A
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·2c12d75 (⌂|🏘️|1)
    │       └── ►:3[1]:B
    │           └── ·320e105 (⌂|🏘️|1)
    │               └── ►:4[2]:B-sub
    │                   └── ·2a31450 (⌂|🏘️|1)
    │                       └── ►:5[3]:A
    │                           └── ·70bde6b (⌂|🏘️|1)
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|11) ►new-A
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·2c12d75 (⌂|🏘️|1)
    │       ├── 📙►:3[1]:B
    │       │   └── ·320e105 (⌂|🏘️|1)
    │       │       └── 📙►:4[2]:B-sub
    │       │           └── ·2a31450 (⌂|🏘️|1)
    │       │               └── 📙►:5[3]:A
    │       │                   └── ·70bde6b (⌂|🏘️|1)
    │       │                       └── ►:2[4]:main <> origin/main →:1:
    │       │                           └── ·fafd9d0 (⌂|🏘️|✓|11)
    │       └── 📙►:6[1]:new-A
    │           └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:6:new-A on fafd9d0
    │   └── 📙:6:new-A
    └── ≡📙:3:B on fafd9d0
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
    └── 👉►:0[0]:gitbutler/workspace
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:gitbutler/workspace <> ✓!
    └── ≡:0:gitbutler/workspace
        └── :0:gitbutler/workspace
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
    │           ├── ·b688f2d (⌂|🏘️|1)
    │           └── ·fafd9d0 (⌂|🏘️|1)
    └── 📕►►►:1[0]:gitbutler/workspace
        └── ·b6917c7 (⌂|🏘️)
            └── ►:2[1]:main
                └── ·f7fe830 (⌂|🏘️)
                    └── →:3:
    ");
    // This is an unmanaged workspace, even though commits from a workspace flow into it.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:entrypoint <> ✓!
    └── ≡:0:entrypoint
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
    ├── 👉►:0[0]:gitbutler/workspace
    │   └── ·47e1cf1 (⌂|1)
    │       └── ►:1[1]:merge-2
    │           └── ·f40fb16 (⌂|1)
    │               ├── ►:2[2]:D
    │               │   └── ·450c58a (⌂|1)
    │               │       └── ►:4[3]:anon:
    │               │           └── ·0cc5a6f (⌂|1) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    │               │               ├── ►:5[4]:B
    │               │               │   └── ·7fdb58d (⌂|1)
    │               │               │       └── ►:7[5]:main <> origin/main →:8:
    │               │               │           └── ·fafd9d0 (⌂|11)
    │               │               └── ►:6[4]:A
    │               │                   └── ·e255adc (⌂|1)
    │               │                       └── →:7: (main →:8:)
    │               └── ►:3[2]:C
    │                   └── ·c6d714c (⌂|1)
    │                       └── →:4:
    └── ►:8[0]:origin/main →:7:
        └── →:7: (main →:8:)
    ");

    // Without workspace data this becomes a single-branch workspace, with `main` as normal segment.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:gitbutler/workspace <> ✓!
    └── ≡:0:gitbutler/workspace
        ├── :0:gitbutler/workspace
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·47e1cf1 (⌂|🏘️|1)
    │       └── ►:6[1]:merge-2
    │           └── ·f40fb16 (⌂|🏘️|1)
    │               ├── ►:7[2]:D
    │               │   └── ·450c58a (⌂|🏘️|1)
    │               │       └── 📙►:3[3]:empty-2-on-merge
    │               │           └── 📙►:9[4]:empty-1-on-merge
    │               │               └── 📙►:10[5]:merge
    │               │                   └── ·0cc5a6f (⌂|🏘️|1)
    │               │                       ├── ►:4[6]:B
    │               │                       │   └── ·7fdb58d (⌂|🏘️|1)
    │               │                       │       └── ►:2[7]:main <> origin/main →:1:
    │               │                       │           └── ·fafd9d0 (⌂|🏘️|✓|11)
    │               │                       └── ►:5[6]:A
    │               │                           └── ·e255adc (⌂|🏘️|1)
    │               │                               └── →:2: (main →:1:)
    │               └── ►:8[2]:C
    │                   └── ·c6d714c (⌂|🏘️|1)
    │                       └── →:3: (empty-2-on-merge)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:6:merge-2 on fafd9d0
        ├── :6:merge-2
        │   └── ·f40fb16 (🏘️)
        ├── :7:D
        │   └── ·450c58a (🏘️)
        ├── 📙:3:empty-2-on-merge
        ├── 📙:9:empty-1-on-merge
        ├── 📙:10:merge
        │   └── ·0cc5a6f (🏘️)
        └── :4:B
            └── ·7fdb58d (🏘️)
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
    │   └── 👉►:0[1]:main <> origin/main →:2:
    │       └── ·fafd9d0 (⌂|🏘️|✓|1) ►A, ►B, ►C, ►D, ►E, ►F
    └── ►:2[0]:origin/main →:0:
        └── →:0: (main →:2:)
    ");

    // There is no workspace as `main` is the base of the workspace, so it's shown directly,
    // outside the workspace.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:
        └── :0:main <> origin/main →:2:
            └── ❄️fafd9d0 (🏘️|✓) ►A, ►B, ►C, ►D, ►E, ►F
    ");

    let (id, ref_name) = id_at(&repo, "gitbutler/workspace");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ►:2[1]:main <> origin/main →:1:
    │       └── ·fafd9d0 (⌂|🏘️|✓|1) ►A, ►B, ►C, ►D, ►E, ►F
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // However, when the workspace is checked out, it's at least empty.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0");

    // The simplest possible setup where we can define how the workspace should look like,
    // in terms of dependent and independent virtual segments.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   ├── 👉►:0[4]:main <> origin/main →:2:
    │   │   └── ·fafd9d0 (⌂|🏘️|✓|1)
    │   ├── 📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── 📙►:5[3]:A
    │   │           └── →:0: (main →:2:)
    │   └── 📙►:6[1]:D
    │       └── 📙►:7[2]:E
    │           └── 📙►:8[3]:F
    │               └── →:0: (main →:2:)
    └── ►:2[0]:origin/main →:0:
        └── →:0: (main →:2:)
    ");

    // ~~There is no segmentation outside the workspace.~~ workspace segmentation always happens so the view is consistent.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:
        └── :0:main <> origin/main →:2:
            └── ❄️fafd9d0 (🏘️|✓)
    ");

    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    // Now the dependent segments are applied, and so is the separate stack.
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   ├── ►:2[4]:main <> origin/main →:1:
    │   │   └── ·fafd9d0 (⌂|🏘️|✓|1)
    │   ├── 📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── 📙►:5[3]:A
    │   │           └── →:2: (main →:1:)
    │   └── 📙►:6[1]:D
    │       └── 📙►:7[2]:E
    │           └── 📙►:8[3]:F
    │               └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:6:D on fafd9d0
    │   ├── 📙:6:D
    │   ├── 📙:7:E
    │   └── 📙:8:F
    └── ≡📙:3:C on fafd9d0
        ├── 📙:3:C
        ├── 📙:4:B
        └── 📙:5:A
    ");
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·298d938 (⌂|🏘️|1)
    │       └── ►:3[1]:anon:
    │           ├── ·16f132b (⌂|🏘️|1) ►F, ►G, ►S1
    │           └── ·917b9da (⌂|🏘️|1) ►D, ►E
    │               └── ►:2[2]:main <> origin/main →:1:
    │                   └── ·fafd9d0 (⌂|🏘️|✓|11) ►A, ►B, ►C
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // With no workspace at all as the workspace segment isn't split.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·298d938 (⌂|🏘️)
    │       └── 👉►:0[1]:S1
    │           ├── ·16f132b (⌂|🏘️|1) ►F, ►G
    │           └── ·917b9da (⌂|🏘️|1) ►D, ►E
    │               └── ►:3[2]:main <> origin/main →:2:
    │                   └── ·fafd9d0 (⌂|🏘️|✓|11) ►A, ►B, ►C
    └── ►:2[0]:origin/main →:3:
        └── →:3: (main →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡👉:0:S1 on fafd9d0
        └── 👉:0:S1
            ├── ·16f132b (🏘️) ►F, ►G
            └── ·917b9da (🏘️) ►D, ►E
    ");

    // Define the workspace.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B"]);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "S1", StackState::InWorkspace, &["G", "F"]);
    add_stack_with_segments(&mut meta, 3, "D", StackState::InWorkspace, &["E"]);

    // We see that all segments are used: S1 C B A E D G F
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·298d938 (⌂|🏘️|1)
    │       ├── 📙►:5[1]:C
    │       │   └── 📙►:6[2]:B
    │       │       └── ►:2[6]:main <> origin/main →:1:
    │       │           └── ·fafd9d0 (⌂|🏘️|✓|11)
    │       ├── 📙►:7[1]:A
    │       │   └── →:2: (main →:1:)
    │       └── 📙►:3[1]:S1
    │           └── 📙►:8[2]:G
    │               └── 📙►:9[3]:F
    │                   └── ·16f132b (⌂|🏘️|1)
    │                       └── 📙►:4[4]:D
    │                           └── 📙►:10[5]:E
    │                               └── ·917b9da (⌂|🏘️|1)
    │                                   └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:S1 on fafd9d0
    │   ├── 📙:3:S1
    │   ├── 📙:8:G
    │   ├── 📙:9:F
    │   │   └── ·16f132b (🏘️)
    │   ├── 📙:4:D
    │   └── 📙:10:E
    │       └── ·917b9da (🏘️)
    ├── ≡📙:7:A on fafd9d0
    │   └── 📙:7:A
    └── ≡📙:5:C on fafd9d0
        ├── 📙:5:C
        └── 📙:6:B
    ");

    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    // This should look the same as before, despite the starting position.
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·298d938 (⌂|🏘️)
    │       ├── 📙►:5[1]:C
    │       │   └── 📙►:6[2]:B
    │       │       └── ►:3[6]:main <> origin/main →:2:
    │       │           └── ·fafd9d0 (⌂|🏘️|✓|11)
    │       ├── 📙►:7[1]:A
    │       │   └── →:3: (main →:2:)
    │       └── 👉📙►:0[1]:S1
    │           └── 📙►:8[2]:G
    │               └── 📙►:9[3]:F
    │                   └── ·16f132b (⌂|🏘️|1)
    │                       └── 📙►:4[4]:D
    │                           └── 📙►:10[5]:E
    │                               └── ·917b9da (⌂|🏘️|1)
    │                                   └── →:3: (main →:2:)
    └── ►:2[0]:origin/main →:3:
        └── →:3: (main →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡👉📙:0:S1 on fafd9d0
    │   ├── 👉📙:0:S1
    │   ├── 📙:8:G
    │   ├── 📙:9:F
    │   │   └── ·16f132b (🏘️)
    │   ├── 📙:4:D
    │   └── 📙:10:E
    │       └── ·917b9da (🏘️)
    ├── ≡📙:7:A on fafd9d0
    │   └── 📙:7:A
    └── ≡📙:5:C on fafd9d0
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
    │   ├── ►:2[3]:main <> origin/main →:1:
    │   │   └── ·fafd9d0 (⌂|🏘️|✓|1)
    │   ├── 📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── →:2: (main →:1:)
    │   ├── 📙►:5[1]:A
    │   │   └── →:2: (main →:1:)
    │   ├── 📙►:6[1]:D
    │   │   └── 📙►:7[2]:E
    │   │       └── →:2: (main →:1:)
    │   └── 📙►:8[1]:F
    │       └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:8:F on fafd9d0
    │   └── 📙:8:F
    ├── ≡📙:6:D on fafd9d0
    │   ├── 📙:6:D
    │   └── 📙:7:E
    ├── ≡📙:5:A on fafd9d0
    │   └── 📙:5:A
    └── ≡📙:3:C on fafd9d0
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
    │   ├── ►:0[3]:main <> origin/main →:2:
    │   │   └── ·fafd9d0 (⌂|🏘️|✓|1)
    │   ├── 👉📙►:3[1]:C
    │   │   └── 📙►:4[2]:B
    │   │       └── →:0: (main →:2:)
    │   ├── 📙►:5[1]:A
    │   │   └── →:0: (main →:2:)
    │   ├── 📙►:6[1]:D
    │   │   └── 📙►:7[2]:E
    │   │       └── →:0: (main →:2:)
    │   └── 📙►:8[1]:F
    │       └── →:0: (main →:2:)
    └── ►:2[0]:origin/main
        └── →:0: (main →:2:)
    ");

    // We should see the same stacks as we did before, just with a different entrypoint.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:8:F on fafd9d0
    │   └── 📙:8:F
    ├── ≡📙:6:D on fafd9d0
    │   ├── 📙:6:D
    │   └── 📙:7:E
    ├── ≡📙:5:A on fafd9d0
    │   └── 📙:5:A
    └── ≡👉📙:3:C on fafd9d0
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·9bcd3af (⌂|🏘️|1)
    │       └── ►:2[1]:main <> origin/main →:1:
    │           ├── ·998eae6 (⌂|🏘️|✓|11)
    │           └── ·fafd9d0 (⌂|🏘️|✓|11)
    └── ►:1[0]:origin/main →:2:
        ├── 🟣ca7baa7 (✓)
        └── 🟣7ea1468 (✓)
            └── →:2: (main →:1:)
    ");

    // Everything in the workspace is integrated, thus it's empty.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣2 on 998eae6");

    let (id, ref_name) = id_at(&repo, "main");
    // The integration branch can be in the workspace and be checked out.
    let graph = Graph::from_commit_traversal(id, Some(ref_name), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·9bcd3af (⌂|🏘️)
    │       └── 👉►:0[1]:main <> origin/main →:2:
    │           ├── ·998eae6 (⌂|🏘️|✓|1)
    │           └── ·fafd9d0 (⌂|🏘️|✓|1)
    └── ►:2[0]:origin/main →:0:
        ├── 🟣ca7baa7 (✓)
        └── 🟣7ea1468 (✓)
            └── →:0: (main →:2:)
    ");

    // If it's checked out, we must show it, but it's not part of the workspace.
    // This is special as other segments still are.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:⇣2
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·8b39ce4 (⌂|🏘️|1)
    │       └── ►:1[1]:A <> origin/A →:2:
    │           ├── ·9d34471 (⌂|🏘️|11)
    │           └── ·5b89c71 (⌂|🏘️|11)
    │               └── ►:5[3]:anon:
    │                   └── ·998eae6 (⌂|🏘️|11)
    │                       └── ►:3[4]:main
    │                           └── ·fafd9d0 (⌂|🏘️|11)
    └── ►:2[0]:origin/A →:1:
        ├── 🟣3ea1a8f
        └── 🟣9c50f71
            └── ►:4[1]:anon:
                └── 🟣2cfbb79
                    ├── →:5:
                    └── ►:6[2]:anon:
                        └── 🟣e898cd0
                            └── →:5:
    ");
    // There is no target branch, so nothing is integrated, and `main` shows up.
    // It's not special.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
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
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·8b39ce4 (⌂|🏘️)
    │       └── ►:2[1]:A <> origin/A →:3:
    │           ├── ·9d34471 (⌂|🏘️|10)
    │           └── ·5b89c71 (⌂|🏘️|10)
    │               └── ►:5[3]:anon:
    │                   └── ·998eae6 (⌂|🏘️|10)
    │                       └── 👉►:0[4]:main
    │                           └── ·fafd9d0 (⌂|🏘️|11)
    └── ►:3[0]:origin/A →:2:
        ├── 🟣3ea1a8f
        └── 🟣9c50f71
            └── ►:4[1]:anon:
                └── 🟣2cfbb79
                    ├── →:5:
                    └── ►:6[2]:anon:
                        └── 🟣e898cd0
                            └── →:5:
    ");
    // The whole workspace is visible, but it's clear where the entrypoint is.
    // As there is no target ref, `main` shows up.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓!
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·7786959 (⌂|🏘️|1)
    │       └── ►:3[1]:B <> origin/B →:4:
    │           └── ·312f819 (⌂|🏘️|101)
    │               └── ►:5[2]:A <> origin/A →:6:
    │                   └── ·e255adc (⌂|🏘️|1101)
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·fafd9d0 (⌂|🏘️|✓|1111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/B →:3:
        └── 🟣682be32
            └── ►:6[1]:origin/A →:5:
                └── 🟣e29c23d
                    └── →:2: (main →:1:)
    ");
    // It's worth noting that we avoid double-listing remote commits that are also
    // directly owned by another remote segment.
    // they have to be considered as something relevant to the branch history.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
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
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·7786959 (⌂|🏘️)
    │       └── ►:5[1]:B <> origin/B →:6:
    │           └── ·312f819 (⌂|🏘️|100)
    │               └── 👉►:0[2]:A <> origin/A →:4:
    │                   └── ·e255adc (⌂|🏘️|101)
    │                       └── ►:3[3]:main <> origin/main →:2:
    │                           └── ·fafd9d0 (⌂|🏘️|✓|111)
    ├── ►:2[0]:origin/main →:3:
    │   └── →:3: (main →:2:)
    └── ►:6[0]:origin/B →:5:
        └── 🟣682be32
            └── ►:4[1]:origin/A →:0:
                └── 🟣e29c23d
                    └── →:3: (main →:2:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:5:B <> origin/B →:6:⇡1⇣1 on fafd9d0
        ├── :5:B <> origin/B →:6:⇡1⇣1
        │   ├── 🟣682be32
        │   └── ·312f819 (🏘️)
        └── 👉:0:A <> origin/A →:4:⇡1⇣1
            ├── 🟣e29c23d
            └── ·e255adc (🏘️)
    ");
    insta::assert_debug_snapshot!(graph.statistics(), @r#"
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
                None,
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·e30f90c (⌂|🏘️|1)
    │       └── ►:6[1]:anon:
    │           └── ·2173153 (⌂|🏘️|101) ►C, ►ambiguous-C
    │               └── ►:9[2]:B <> origin/B →:5:
    │                   └── ·312f819 (⌂|🏘️|1101) ►ambiguous-B
    │                       └── ►:8[3]:A <> origin/A →:7:
    │                           └── ·e255adc (⌂|🏘️|11101) ►ambiguous-A
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|11111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    ├── ►:3[0]:origin/C
    │   └── →:6:
    ├── ►:4[0]:origin/ambiguous-C
    │   └── →:6:
    ├── ►:5[0]:origin/B
    │   └── 🟣ac24e74
    │       └── →:9: (B →:5:)
    └── ►:7[0]:origin/A
        └── →:8: (A →:7:)
    ");
    // An anonymous segment to start with is alright, and can always happen for other situations as well.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:6:anon: on fafd9d0
        ├── :6:anon:
        │   └── ·2173153 (🏘️) ►C, ►ambiguous-C
        ├── :9:B <> origin/B →:5:⇣1
        │   ├── 🟣ac24e74
        │   └── ❄️312f819 (🏘️) ►ambiguous-B
        └── :8:A <> origin/A →:7:
            └── ❄️e255adc (🏘️) ►ambiguous-A
    ");

    assert_eq!(
        graph.partial_segments().count(),
        0,
        "a fully realized graph"
    );

    // If 'C' is in the workspace, it's naturally disambiguated.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·e30f90c (⌂|🏘️|1)
    │       └── 📙►:3[1]:C <> origin/C →:4:
    │           └── ·2173153 (⌂|🏘️|101) ►ambiguous-C
    │               └── ►:9[2]:B <> origin/B →:6:
    │                   └── ·312f819 (⌂|🏘️|1101) ►ambiguous-B
    │                       └── ►:8[3]:A <> origin/A →:7:
    │                           └── ·e255adc (⌂|🏘️|11101) ►ambiguous-A
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|11111)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    ├── ►:4[0]:origin/C →:3:
    │   └── →:3: (C →:4:)
    ├── ►:5[0]:origin/ambiguous-C
    │   └── →:3: (C →:4:)
    ├── ►:6[0]:origin/B
    │   └── 🟣ac24e74
    │       └── →:9: (B →:6:)
    └── ►:7[0]:origin/A
        └── →:8: (A →:7:)
    ");
    // And because `C` is in the workspace data, its data is denoted.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:C <> origin/C →:4: on fafd9d0
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️|1)
    │       └── ►:2[1]:B
    │           ├── ·6b1a13b (⌂|🏘️|1)
    │           └── ·03ad472 (⌂|🏘️|1)
    │               └── ►:4[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   └── ✂️·a381df5 (⌂|🏘️|✓|1)
    └── ►:1[0]:origin/main
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:3[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:5[2]:main
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:4: (A)
    ");
    // It's true that `A` is fully integrated so it isn't displayed. so from a workspace-perspective
    // it's the right answer.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡:2:B on 79bbb29
        └── :2:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["A"]);
    // ~~Now that `A` is part of the workspace, it's not cut off anymore.~~
    // This special handling was removed for now, relying on limits and extensions.
    // And since it's integrated, traversal is stopped without convergence.
    // We see more though as we add workspace segments immediately.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️|1)
    │       └── 📙►:2[1]:B
    │           ├── ·6b1a13b (⌂|🏘️|1)
    │           └── ·03ad472 (⌂|🏘️|1)
    │               └── 📙►:3[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)
    │                   └── ·777b552 (⌂|🏘️|✓|1)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)
    │                               ├── ►:7[4]:anon:
    │                               │   └── ✂️·01d0e1e (⌂|🏘️|✓|1)
    │                               └── ►:8[4]:A-feat
    │                                   └── ✂️·fea59b5 (⌂|🏘️|✓|1)
    └── ►:1[0]:origin/main
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:5[2]:main
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:3: (A)
    ");
    // `A` is integrated, hence it's not shown.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡📙:2:B on 79bbb29
        └── 📙:2:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    // The limit is effective for integrated workspaces branches, but the traversal proceeds until
    // the integration branch finds its goal.
    let graph =
        Graph::from_head(&repo, &*meta, standard_options().with_limit_hint(1))?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️|1)
    │       └── 📙►:2[1]:B
    │           ├── ·6b1a13b (⌂|🏘️|1)
    │           └── ·03ad472 (⌂|🏘️|1)
    │               └── 📙►:3[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)
    │                   └── ·777b552 (⌂|🏘️|✓|1)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)
    │                               ├── ►:7[4]:anon:
    │                               │   └── ✂️·01d0e1e (⌂|🏘️|✓|1)
    │                               └── ►:8[4]:A-feat
    │                                   └── ✂️·fea59b5 (⌂|🏘️|✓|1)
    └── ►:1[0]:origin/main
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:5[2]:main
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:3: (A)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡📙:2:B on 79bbb29
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘️)
    │           └── ·03ad472 (⌂|🏘️)
    │               └── 👉►:0[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)
    │                   └── ·777b552 (⌂|🏘️|✓|1)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)
    │                               ├── ►:7[5]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘️|✓|1)
    │                               │       └── ►:5[6]:main
    │                               │           ├── ·4b3e5a8 (⌂|🏘️|✓|1)
    │                               │           ├── ·34d0715 (⌂|🏘️|✓|1)
    │                               │           └── ·eb5f731 (⌂|🏘️|✓|1)
    │                               └── ►:8[4]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘️|✓|1)
    │                                   └── ·4deea74 (⌂|🏘️|✓|1)
    │                                       └── →:7:
    └── ►:2[0]:origin/main
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── →:5: (main)
                    └── →:0: (A)
    ");
    // It looks like some commits are missing, but it's a first-parent traversal.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:A <> ✓!
    └── ≡:0:A
        ├── :0:A
        │   ├── ·79bbb29 (🏘️|✓)
        │   ├── ·fc98174 (🏘️|✓)
        │   ├── ·a381df5 (🏘️|✓)
        │   ├── ·777b552 (🏘️|✓)
        │   ├── ·ce4a760 (🏘️|✓)
        │   └── ·01d0e1e (🏘️|✓)
        └── :5:main
            ├── ·4b3e5a8 (🏘️|✓)
            ├── ·34d0715 (🏘️|✓)
            └── ·eb5f731 (🏘️|✓)
    ");

    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options().with_limit_hint(1))?
            .validated()?;
    // It's still getting quite far despite the limit due to other heads searching for their goals,
    // but also ends traversal early.
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘️)
    │           └── ·03ad472 (⌂|🏘️)
    │               └── 👉►:0[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)
    │                   └── ·777b552 (⌂|🏘️|✓|1)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)
    │                               ├── ►:7[5]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘️|✓|1)
    │                               │       └── ►:5[6]:main
    │                               │           ├── ·4b3e5a8 (⌂|🏘️|✓|1)
    │                               │           ├── ·34d0715 (⌂|🏘️|✓|1)
    │                               │           └── ·eb5f731 (⌂|🏘️|✓|1)
    │                               └── ►:8[4]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘️|✓|1)
    │                                   └── ·4deea74 (⌂|🏘️|✓|1)
    │                                       └── →:7:
    └── ►:2[0]:origin/main
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── →:5: (main)
                    └── →:0: (A)
    ");
    // Because the branch is integrated, the surrounding workspace isn't shown.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:A <> ✓!
    └── ≡:0:A
        ├── :0:A
        │   ├── ·79bbb29 (🏘️|✓)
        │   ├── ·fc98174 (🏘️|✓)
        │   ├── ·a381df5 (🏘️|✓)
        │   ├── ·777b552 (🏘️|✓)
        │   ├── ·ce4a760 (🏘️|✓)
        │   └── ·01d0e1e (🏘️|✓)
        └── :5:main
            ├── ·4b3e5a8 (🏘️|✓)
            ├── ·34d0715 (🏘️|✓)
            └── ·eb5f731 (🏘️|✓)
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘️)
    │           └── ·03ad472 (⌂|🏘️)
    │               └── ►:6[3]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)
    │                   └── ·777b552 (⌂|🏘️|✓|1)
    │                       └── ►:7[4]:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)
    │                               ├── ►:8[6]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘️|✓|1)
    │                               │       └── ►:5[7]:main
    │                               │           ├── ·4b3e5a8 (⌂|🏘️|✓|1)
    │                               │           ├── ·34d0715 (⌂|🏘️|✓|1)
    │                               │           └── ·eb5f731 (⌂|🏘️|✓|1)
    │                               └── ►:9[5]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘️|✓|1)
    │                                   └── ·4deea74 (⌂|🏘️|✓|1)
    │                                       └── →:8:
    └── ►:2[0]:origin/main
        └── ►:0[1]:anon:
            ├── 👉·d0df794 (⌂|✓|1)
            └── ·09c6e08 (⌂|✓|1)
                └── ►:4[2]:anon:
                    └── ·7b9f260 (⌂|✓|1)
                        ├── →:5: (main)
                        └── →:6: (A)
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
    // TODO(extra-target): make it work so they limit single branches even.
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘️|✓)
    │           └── ·03ad472 (⌂|🏘️|✓)
    │               └── ►:5[3]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   └── ✂️·fc98174 (⌂|🏘️|✓|1)
    └── ►:2[0]:origin/main
        └── ►:0[1]:anon:
            ├── 👉·d0df794 (⌂|✓|1)
            └── ·09c6e08 (⌂|✓|1)
                └── ►:4[2]:anon:
                    └── ·7b9f260 (⌂|✓|1)
                        ├── ►:6[3]:main
                        │   ├── ·4b3e5a8 (⌂|✓|1)
                        │   ├── ·34d0715 (⌂|✓|1)
                        │   └── ·eb5f731 (⌂|✓|1)
                        └── →:5: (A)
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️|1)
    │       └── ►:3[1]:B
    │           ├── ·6b1a13b (⌂|🏘️|1)
    │           └── ·03ad472 (⌂|🏘️|1)
    │               └── ►:5[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)
    │                   └── ·777b552 (⌂|🏘️|✓|1)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)
    │                               ├── ►:7[5]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘️|✓|1)
    │                               │       └── ►:2[6]:main <> origin/main →:1:
    │                               │           ├── ·4b3e5a8 (⌂|🏘️|✓|11)
    │                               │           ├── ·34d0715 (⌂|🏘️|✓|11)
    │                               │           └── ·eb5f731 (⌂|🏘️|✓|11)
    │                               └── ►:8[4]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘️|✓|1)
    │                                   └── ·4deea74 (⌂|🏘️|✓|1)
    │                                       └── →:7:
    └── ►:1[0]:origin/main →:2:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:4[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── →:2: (main →:1:)
                    └── →:5: (A)
    ");

    // This search discovers the whole workspace, without the integrated one.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣3 on 79bbb29
    └── ≡:3:B on 79bbb29
        └── :3:B
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣3 on 4b3e5a8
    └── ≡:3:B on 4b3e5a8
        ├── :3:B
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
    // the limit isn't respected, and we still konw the whole workspace.
    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️)
    │       └── ►:4[1]:B
    │           ├── ·6b1a13b (⌂|🏘️)
    │           └── ·03ad472 (⌂|🏘️)
    │               └── 👉►:0[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   ├── ·a381df5 (⌂|🏘️|✓|1)
    │                   └── ·777b552 (⌂|🏘️|✓|1)
    │                       └── ►:6[3]:anon:
    │                           └── ·ce4a760 (⌂|🏘️|✓|1)
    │                               ├── ►:7[5]:anon:
    │                               │   └── ·01d0e1e (⌂|🏘️|✓|1)
    │                               │       └── ►:3[6]:main <> origin/main →:2:
    │                               │           ├── ·4b3e5a8 (⌂|🏘️|✓|11)
    │                               │           ├── ·34d0715 (⌂|🏘️|✓|11)
    │                               │           └── ·eb5f731 (⌂|🏘️|✓|11)
    │                               └── ►:8[4]:A-feat
    │                                   ├── ·fea59b5 (⌂|🏘️|✓|1)
    │                                   └── ·4deea74 (⌂|🏘️|✓|1)
    │                                       └── →:7:
    └── ►:2[0]:origin/main →:3:
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:5[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── →:3: (main →:2:)
                    └── →:0: (A)
    ");

    // The entrypoint isn't contained in the workspace anymore, so it's standalone.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:A <> ✓!
    └── ≡:0:A
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main⇣3 on 4b3e5a8
    └── ≡:4:B on 4b3e5a8
        ├── :4:B
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main <> origin/main →:2:⇣3
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:DETACHED <> ✓!
    └── ≡:0:anon:
        └── :0:anon:
            ├── ·34d0715 (🏘️|✓)
            └── ·eb5f731 (🏘️|✓)
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
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── ✂️·4077353 (⌂|🏘️|1)
    ");
    // The commit in the workspace branch is always ignored and is expected to be the workspace merge commit.
    // So nothing to show here.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️:0:gitbutler/workspace <> ✓!");

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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·4077353 (⌂|🏘️|1)
    │       └── ►:2[1]:B
    │           ├── ·6b1a13b (⌂|🏘️|1)
    │           └── ·03ad472 (⌂|🏘️|1)
    │               └── ►:4[2]:A
    │                   ├── ·79bbb29 (⌂|🏘️|✓|1)
    │                   ├── ·fc98174 (⌂|🏘️|✓|1)
    │                   └── ✂️·a381df5 (⌂|🏘️|✓|1)
    └── ►:1[0]:origin/main
        ├── 🟣d0df794 (✓)
        └── 🟣09c6e08 (✓)
            └── ►:3[1]:anon:
                └── 🟣7b9f260 (✓)
                    ├── ►:5[2]:main
                    │   ├── 🟣4b3e5a8 (✓)
                    │   ├── 🟣34d0715 (✓)
                    │   └── 🟣eb5f731 (✓)
                    └── →:4: (A)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣6 on 79bbb29
    └── ≡:2:B on 79bbb29
        └── :2:B
            ├── ·6b1a13b (🏘️)
            └── ·03ad472 (🏘️)
    ");

    Ok(())
}

#[test]
fn three_branches_one_advanced_ws_commit_advanced_fully_pushed_empty_dependant()
-> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependant",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cbc6713 (origin/advanced-lane, on-top-of-dependant, dependant, advanced-lane) change
    * fafd9d0 (origin/main, main, lane) init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·f8f33a7 (⌂|🏘️|1)
    │       └── ►:4[1]:advanced-lane <> origin/advanced-lane →:3:
    │           └── ·cbc6713 (⌂|🏘️|101) ►dependant, ►on-top-of-dependant
    │               └── ►:2[2]:main <> origin/main →:1:
    │                   └── ·fafd9d0 (⌂|🏘️|✓|111) ►lane
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:3[0]:origin/advanced-lane
        └── →:4: (advanced-lane →:3:)
    ");

    // By default, the advanced lane is simply frozen as its remote contains the commit.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡:4:advanced-lane <> origin/advanced-lane →:3: on fafd9d0
        └── :4:advanced-lane <> origin/advanced-lane →:3:
            └── ❄️cbc6713 (🏘️) ►dependant, ►on-top-of-dependant
    ");

    add_stack_with_segments(
        &mut meta,
        1,
        "dependant",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

    // Lanes are properly ordered
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·f8f33a7 (⌂|🏘️|1)
    │       └── 📙►:3[1]:dependant
    │           └── 📙►:5[2]:advanced-lane <> origin/advanced-lane →:4:
    │               └── ·cbc6713 (⌂|🏘️|101) ►on-top-of-dependant
    │                   └── ►:2[3]:main <> origin/main →:1:
    │                       └── ·fafd9d0 (⌂|🏘️|✓|111) ►lane
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/advanced-lane →:5:
        └── →:3: (dependant)
    ");

    // When putting the dependent branch on top as empty segment, the frozen state is retained.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:dependant on fafd9d0
        ├── 📙:3:dependant
        └── 📙:5:advanced-lane <> origin/advanced-lane →:4:
            └── ❄️cbc6713 (🏘️) ►on-top-of-dependant
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
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── ►:1[1]:origin/main →:2:
            ├── ·2cde30a (⌂|🏘️|✓|1) ►A, ►B, ►C, ►D, ►E, ►F
            ├── ·1c938f4 (⌂|🏘️|✓|1)
            ├── ·b82769f (⌂|🏘️|✓|1)
            ├── ·988032f (⌂|🏘️|✓|1)
            └── ·cd5b655 (⌂|🏘️|✓|1)
                └── ►:2[2]:main <> origin/main →:1:
                    └── ·2be54cd (⌂|🏘️|✓|11)
    ");
    // Workspace is empty as everything is integrated.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 2cde30a");

    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉📕►►►:0[0]:gitbutler/workspace
        ├── ►:1[4]:origin/main →:2:
        │   ├── ·2cde30a (⌂|🏘️|✓|1)
        │   ├── ·1c938f4 (⌂|🏘️|✓|1)
        │   ├── ·b82769f (⌂|🏘️|✓|1)
        │   ├── ·988032f (⌂|🏘️|✓|1)
        │   └── ·cd5b655 (⌂|🏘️|✓|1)
        │       └── ►:2[5]:main <> origin/main →:1:
        │           └── ·2be54cd (⌂|🏘️|✓|11)
        ├── 📙►:3[1]:C
        │   └── 📙►:4[2]:B
        │       └── 📙►:5[3]:A
        │           └── →:1: (origin/main →:2:)
        └── 📙►:6[1]:D
            └── 📙►:7[2]:E
                └── 📙►:8[3]:F
                    └── →:1: (origin/main →:2:)
    ");

    // Empty stack segments on top of integrated portions will show, and nothing integrated shows.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 2cde30a
    ├── ≡📙:6:D on 2cde30a
    │   ├── 📙:6:D
    │   ├── 📙:7:E
    │   └── 📙:8:F
    └── ≡📙:3:C on 2cde30a
        ├── 📙:3:C
        ├── 📙:4:B
        └── 📙:5:A
    ");

    // However, when passing an additional old position of the target, we can show now integrated parts.
    // The stacks will always be created on top of the integrated segments as that's where their references are
    // (these segments are never conjured up out of thin air).
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 2be54cd
    ├── ≡📙:6:D on 2be54cd
    │   ├── 📙:6:D
    │   ├── 📙:7:E
    │   └── 📙:8:F
    │       ├── ·2cde30a (🏘️|✓)
    │       ├── ·1c938f4 (🏘️|✓)
    │       ├── ·b82769f (🏘️|✓)
    │       ├── ·988032f (🏘️|✓)
    │       └── ·cd5b655 (🏘️|✓)
    └── ≡📙:3:C on 2be54cd
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
    let (id, ref_name) = id_at(&repo, "main");
    // Validate that we will perform long searches to connect connectable segments, without interfering
    // with other searches that may take even longer.
    // Also, without limit, we should be able to see all of 'main' without cut-off
    let graph = Graph::from_commit_traversal(id, ref_name.clone(), &*meta, standard_options())?
        .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·41ed0e4 (⌂|🏘️)
    │       └── ►:3[2]:workspace
    │           └── ·9730cbf (⌂|🏘️|✓)
    │               ├── ►:6[3]:main-to-workspace
    │               │   └── ·dc7ab57 (⌂|🏘️|✓)
    │               │       └── ►:8[5]:anon:
    │               │           ├── ·c056b75 (⌂|🏘️|✓|1)
    │               │           ├── ·f49c977 (⌂|🏘️|✓|1)
    │               │           ├── ·7b7ebb2 (⌂|🏘️|✓|1)
    │               │           ├── ·dca4960 (⌂|🏘️|✓|1)
    │               │           ├── ·11c29b8 (⌂|🏘️|✓|1)
    │               │           ├── ·c32dd03 (⌂|🏘️|✓|1)
    │               │           ├── ·b625665 (⌂|🏘️|✓|1)
    │               │           ├── ·a821094 (⌂|🏘️|✓|1)
    │               │           ├── ·bce0c5e (⌂|🏘️|✓|1)
    │               │           └── ·3183e43 (⌂|🏘️|✓|1)
    │               └── ►:7[3]:long-main-to-workspace
    │                   ├── ·77f31a0 (⌂|🏘️|✓)
    │                   ├── ·eb17e31 (⌂|🏘️|✓)
    │                   ├── ·fe2046b (⌂|🏘️|✓)
    │                   └── ·5532ef5 (⌂|🏘️|✓)
    │                       └── 👉►:0[4]:main
    │                           └── ·2438292 (⌂|🏘️|✓|1)
    │                               └── →:8:
    └── ►:2[0]:origin/main
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
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
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options().with_limit_hint(1))?
            .validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·41ed0e4 (⌂|🏘️)
    │       └── ►:3[2]:workspace
    │           └── ·9730cbf (⌂|🏘️|✓)
    │               ├── ►:6[3]:main-to-workspace
    │               │   └── ·dc7ab57 (⌂|🏘️|✓)
    │               │       └── ►:8[5]:anon:
    │               │           ├── ·c056b75 (⌂|🏘️|✓|1)
    │               │           ├── ·f49c977 (⌂|🏘️|✓|1)
    │               │           ├── ·7b7ebb2 (⌂|🏘️|✓|1)
    │               │           ├── ·dca4960 (⌂|🏘️|✓|1)
    │               │           ├── ·11c29b8 (⌂|🏘️|✓|1)
    │               │           ├── ·c32dd03 (⌂|🏘️|✓|1)
    │               │           ├── ·b625665 (⌂|🏘️|✓|1)
    │               │           ├── ·a821094 (⌂|🏘️|✓|1)
    │               │           ├── ·bce0c5e (⌂|🏘️|✓|1)
    │               │           └── ·3183e43 (⌂|🏘️|✓|1)
    │               └── ►:7[3]:long-main-to-workspace
    │                   ├── ·77f31a0 (⌂|🏘️|✓)
    │                   ├── ·eb17e31 (⌂|🏘️|✓)
    │                   ├── ·fe2046b (⌂|🏘️|✓)
    │                   └── ·5532ef5 (⌂|🏘️|✓)
    │                       └── 👉►:0[4]:main
    │                           └── ·2438292 (⌂|🏘️|✓|1)
    │                               └── →:8:
    └── ►:2[0]:origin/main
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
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

    // From the workspace, even without limit, we don't traverse all of 'main' as it's uninteresting.
    // However, we wait for the target to be fully reconciled to get the proper workspace configuration.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·41ed0e4 (⌂|🏘️|1)
    │       └── ►:2[2]:workspace
    │           └── ·9730cbf (⌂|🏘️|✓|1)
    │               ├── ►:5[3]:main-to-workspace
    │               │   └── ·dc7ab57 (⌂|🏘️|✓|1)
    │               │       └── ►:8[5]:anon:
    │               │           ├── ·c056b75 (⌂|🏘️|✓|1)
    │               │           ├── ·f49c977 (⌂|🏘️|✓|1)
    │               │           ├── ·7b7ebb2 (⌂|🏘️|✓|1)
    │               │           ├── ·dca4960 (⌂|🏘️|✓|1)
    │               │           ├── ·11c29b8 (⌂|🏘️|✓|1)
    │               │           ├── ·c32dd03 (⌂|🏘️|✓|1)
    │               │           └── ✂️·b625665 (⌂|🏘️|✓|1)
    │               └── ►:6[3]:long-main-to-workspace
    │                   ├── ·77f31a0 (⌂|🏘️|✓|1)
    │                   ├── ·eb17e31 (⌂|🏘️|✓|1)
    │                   ├── ·fe2046b (⌂|🏘️|✓|1)
    │                   └── ·5532ef5 (⌂|🏘️|✓|1)
    │                       └── ►:7[4]:main
    │                           └── ·2438292 (⌂|🏘️|✓|1)
    │                               └── →:8:
    └── ►:1[0]:origin/main
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣11 on 9730cbf");
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 📕►►►:1[0]:gitbutler/workspace
    │   └── ·f514495 (⌂|🏘️)
    │       └── ►:3[3]:workspace
    │           └── ·c9120f1 (⌂|🏘️|✓)
    │               ├── ►:4[4]:main-to-workspace
    │               │   └── ·1126587 (⌂|🏘️|✓)
    │               │       └── ►:6[6]:anon:
    │               │           └── ·3183e43 (⌂|🏘️|✓|1)
    │               └── ►:5[4]:long-main-to-workspace
    │                   ├── ·b39c7ec (⌂|🏘️|✓)
    │                   ├── ·2983a97 (⌂|🏘️|✓)
    │                   ├── ·144ea85 (⌂|🏘️|✓)
    │                   └── ·5aecfd2 (⌂|🏘️|✓)
    │                       └── 👉►:0[5]:main
    │                           └── ·bce0c5e (⌂|🏘️|✓|1)
    │                               └── →:6:
    └── ►:2[0]:origin/main
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
            ├── ·bce0c5e (🏘️|✓)
            └── ·3183e43 (🏘️|✓)
    ");

    // Now the target looks for the entrypoint, which is the workspace, something it can do more easily.
    // We wait for targets to fully reconcile as well.
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·f514495 (⌂|🏘️|1)
    │       └── ►:2[3]:workspace
    │           └── ·c9120f1 (⌂|🏘️|✓|1)
    │               ├── ►:3[4]:main-to-workspace
    │               │   └── ·1126587 (⌂|🏘️|✓|1)
    │               │       └── ►:6[6]:anon:
    │               │           └── ·3183e43 (⌂|🏘️|✓|1)
    │               └── ►:4[4]:long-main-to-workspace
    │                   ├── ·b39c7ec (⌂|🏘️|✓|1)
    │                   ├── ·2983a97 (⌂|🏘️|✓|1)
    │                   ├── ·144ea85 (⌂|🏘️|✓|1)
    │                   └── ·5aecfd2 (⌂|🏘️|✓|1)
    │                       └── ►:5[5]:main
    │                           └── ·bce0c5e (⌂|🏘️|✓|1)
    │                               └── →:6:
    └── ►:1[0]:origin/main
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
    // Everything is integrated.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣17 on c9120f1");
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·2b30d94 (⌂|🏘️|1)
    │       ├── ►:3[1]:D
    │       │   └── ·9895054 (⌂|🏘️|1)
    │       │       └── ►:6[2]:C
    │       │           ├── ·de625cc (⌂|🏘️|1)
    │       │           ├── ·23419f8 (⌂|🏘️|1)
    │       │           └── ·5dc4389 (⌂|🏘️|1)
    │       │               └── ►:7[3]:shared
    │       │                   ├── ·d4f537e (⌂|🏘️|✓|1)
    │       │                   ├── ·b448757 (⌂|🏘️|✓|1)
    │       │                   └── ·e9a378d (⌂|🏘️|✓|1)
    │       │                       └── ►:2[4]:main <> origin/main →:1:
    │       │                           └── ·3183e43 (⌂|🏘️|✓|11)
    │       ├── ►:4[1]:A
    │       │   └── ·0bad3af (⌂|🏘️|✓|1)
    │       │       └── →:7: (shared)
    │       └── ►:5[1]:B
    │           ├── ·acdc49a (⌂|🏘️|1)
    │           └── ·f0117e0 (⌂|🏘️|1)
    │               └── →:7: (shared)
    └── ►:1[0]:origin/main →:2:
        └── 🟣c08dc6b (✓)
            ├── →:2: (main →:1:)
            └── →:4: (A)
    ");

    // A is still shown despite it being fully integrated, as it's still enclosed by the
    // workspace tip and the fork-point, at least when we provide the previous known location of the target.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on 3183e43
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on d4f537e
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·2b30d94 (⌂|🏘️|1)
    │       ├── ►:2[1]:D
    │       │   └── ·9895054 (⌂|🏘️|1)
    │       │       └── ►:6[2]:C
    │       │           ├── ·de625cc (⌂|🏘️|1)
    │       │           ├── ·23419f8 (⌂|🏘️|1)
    │       │           └── ·5dc4389 (⌂|🏘️|1)
    │       │               └── ►:7[3]:shared
    │       │                   ├── ·d4f537e (⌂|🏘️|1)
    │       │                   ├── ·b448757 (⌂|🏘️|1)
    │       │                   └── ·e9a378d (⌂|🏘️|1)
    │       │                       └── ►:5[4]:main
    │       │                           └── ·3183e43 (⌂|🏘️|✓|1)
    │       ├── ►:3[1]:A
    │       │   └── ·0bad3af (⌂|🏘️|1)
    │       │       └── →:7: (shared)
    │       └── ►:4[1]:B
    │           ├── ·acdc49a (⌂|🏘️|1)
    │           └── ·f0117e0 (⌂|🏘️|1)
    │               └── →:7: (shared)
    └── ►:1[0]:origin/main
        └── 🟣bce0c5e (✓)
            └── →:5: (main)
    ");

    // Segments can definitely repeat
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on 3183e43
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on 3183e43
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
        "ws/two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependant",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   335d6f2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * cbc6713 (origin/advanced-lane, dependant, advanced-lane) change
    |/  
    * fafd9d0 (origin/main, main, lane) init
    ");

    add_stack_with_segments(
        &mut meta,
        1,
        "dependant",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·335d6f2 (⌂|🏘️|1)
    │       ├── ►:2[3]:main <> origin/main →:1:
    │       │   └── ·fafd9d0 (⌂|🏘️|✓|111) ►lane
    │       └── 📙►:3[1]:dependant
    │           └── 📙►:5[2]:advanced-lane <> origin/advanced-lane →:4:
    │               └── ·cbc6713 (⌂|🏘️|101)
    │                   └── →:2: (main →:1:)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/advanced-lane →:5:
        └── →:3: (dependant)
    ");

    // The dependant branch is empty and on top of the one with the remote
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:dependant on fafd9d0
        ├── 📙:3:dependant
        └── 📙:5:advanced-lane <> origin/advanced-lane →:4:
            └── ❄️cbc6713 (🏘️)
    ");

    // Create the dependent branch below.
    add_stack_with_segments(
        &mut meta,
        1,
        "advanced-lane",
        StackState::InWorkspace,
        &["dependant"],
    );

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·335d6f2 (⌂|🏘️|1)
    │       ├── ►:2[3]:main <> origin/main →:1:
    │       │   └── ·fafd9d0 (⌂|🏘️|✓|111) ►lane
    │       └── 📙►:3[1]:advanced-lane <> origin/advanced-lane →:4:
    │           └── 📙►:5[2]:dependant
    │               └── ·cbc6713 (⌂|🏘️|101)
    │                   └── →:2: (main →:1:)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:4[0]:origin/advanced-lane →:3:
        └── →:3: (advanced-lane →:4:)
    ");

    // Having done something unusual, which is to put the dependant branch
    // underneath the other already pushed, it creates a different view of ownership.
    // It's probably OK to leave it like this for now, and instead allow users to reorder
    // these more easily.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:advanced-lane <> origin/advanced-lane →:4: on fafd9d0
        ├── 📙:3:advanced-lane <> origin/advanced-lane →:4:
        └── 📙:5:dependant
            └── ❄cbc6713 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "advanced-lane");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡👉📙:0:advanced-lane <> origin/advanced-lane →:4: on fafd9d0
        ├── 👉📙:0:advanced-lane <> origin/advanced-lane →:4:
        └── 📙:5:dependant
            └── ❄cbc6713 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "dependant");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡👉📙:0:dependant on fafd9d0
        ├── 👉📙:0:dependant
        └── 📙:5:advanced-lane <> origin/advanced-lane →:4:
            └── ❄️cbc6713 (🏘️)
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·e982e8a (⌂|🏘️|1)
    │       ├── 📙►:3[1]:C-on-A
    │       │   └── ·4f1bb32 (⌂|🏘️|1)
    │       │       └── ►:4[2]:A <> origin/A →:5:
    │       │           └── ·e255adc (⌂|🏘️|101)
    │       │               └── ►:2[3]:main <> origin/main →:1:
    │       │                   └── ·fafd9d0 (⌂|🏘️|✓|111)
    │       └── ►:6[1]:B-on-A
    │           └── ·aff8449 (⌂|🏘️|1)
    │               └── →:4: (A →:5:)
    ├── ►:1[0]:origin/main →:2:
    │   └── →:2: (main →:1:)
    └── ►:5[0]:origin/A →:4:
        └── 🟣b627ca7
            └── →:4: (A →:5:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡:6:B-on-A on fafd9d0
    │   ├── :6:B-on-A
    │   │   └── ·aff8449 (🏘️)
    │   └── :4:A <> origin/A →:5:⇣1
    │       ├── 🟣b627ca7
    │       └── ❄️e255adc (🏘️)
    └── ≡📙:3:C-on-A on fafd9d0
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·873d056 (⌂|🏘️|1)
    │       ├── 📙►:2[1]:advanced-lane
    │       │   └── ·cbc6713 (⌂|🏘️|1)
    │       │       └── 📙►:3[2]:lane
    │       │           └── ·fafd9d0 (⌂|🏘️|1) ►main
    │       └── →:3: (lane)
    └── ►:1[0]:origin/main
        └── 🟣da83717 (✓)
    ");

    // Since `lane` is connected directly, no segment has to be created.
    // However, as nothing is integrated, it really is another name for `main` now,
    // `main` is nothing special.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:lane
    │   └── 📙:3:lane
    │       └── ·fafd9d0 (🏘️) ►main
    └── ≡📙:2:advanced-lane on fafd9d0
        └── 📙:2:advanced-lane
            └── ·cbc6713 (🏘️)
    ");

    // Reverse the order of stacks in the worktree data.
    for (idx, name) in lanes.into_iter().rev().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·873d056 (⌂|🏘️|1)
    │       ├── 📙►:2[2]:lane
    │       │   └── ·fafd9d0 (⌂|🏘️|1) ►main
    │       └── 📙►:3[1]:advanced-lane
    │           └── ·cbc6713 (⌂|🏘️|1)
    │               └── →:2: (lane)
    └── ►:1[0]:origin/main
        └── 🟣da83717 (✓)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on fafd9d0
    ├── ≡📙:3:advanced-lane on fafd9d0
    │   └── 📙:3:advanced-lane
    │       └── ·cbc6713 (🏘️)
    └── ≡📙:2:lane
        └── 📙:2:lane
            └── ·fafd9d0 (🏘️) ►main
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·a221221 (⌂|🏘️|1)
    │       └── 📙►:3[1]:A <> origin/A →:4:
    │           └── ·aadad9d (⌂|🏘️|101)
    │               └── ►:1[2]:origin/main →:2:
    │                   └── ·96a2408 (⌂|🏘️|✓|101)
    │                       └── ►:5[3]:integrated
    │                           ├── ·f15ca75 (⌂|🏘️|✓|101)
    │                           └── ·9456d79 (⌂|🏘️|✓|101)
    │                               └── ►:2[4]:main <> origin/main →:1:
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|111)
    └── ►:4[0]:origin/A →:3:
        └── 🟣2b1808c
            └── →:5: (integrated)
    ");

    // Remote tracking branches we just want to aggregate, just like anonymous segments,
    // but only when another target is provided (the old position, `main`).
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    └── ≡📙:3:A <> origin/A →:4:⇡1⇣1 on fafd9d0
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
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 96a2408
    └── ≡📙:3:A <> origin/A →:4:⇡1⇣1 on 96a2408
        └── 📙:3:A <> origin/A →:4:⇡1⇣1
            ├── 🟣2b1808c
            └── ·aadad9d (🏘️)
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── ►:1[1]:A
            ├── ·a62b0de (⌂|🏘️|1)
            └── ·120a217 (⌂|🏘️|1)
                └── ►:2[2]:main
                    └── ·fafd9d0 (⌂|🏘️|1)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
    └── ≡:1:A
        ├── :1:A
        │   ├── ·a62b0de (🏘️)
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 📕►►►:1[0]:gitbutler/workspace
        └── 👉►:0[1]:A
            ├── ·a62b0de (⌂|🏘️|1)
            └── ·120a217 (⌂|🏘️|1)
                └── ►:2[2]:main
                    └── ·fafd9d0 (⌂|🏘️|1)
    ");

    // Main can be a normal segment if there is no target ref.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓!
    └── ≡👉:0:A
        ├── 👉:0:A
        │   ├── ·a62b0de (🏘️)
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── ►:1[1]:anon:
            ├── ·a62b0de (⌂|🏘️|1) ►A, ►B
            └── ·120a217 (⌂|🏘️|1)
                └── ►:2[2]:main
                    └── ·fafd9d0 (⌂|🏘️|1)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
    └── ≡:1:anon:
        ├── :1:anon:
        │   ├── ·a62b0de (🏘️) ►A, ►B
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
    ");

    // We can help it by adding metadata.
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let (id, ref_name) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 📕►►►:1[0]:gitbutler/workspace
        └── 👉►:0[1]:A
            └── 📙►:3[2]:B
                ├── ·a62b0de (⌂|🏘️|1)
                └── ·120a217 (⌂|🏘️|1)
                    └── ►:2[3]:main
                        └── ·fafd9d0 (⌂|🏘️|1)
    ");

    // Main can be a normal segment if there is no target ref.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓!
    └── ≡👉:0:A
        ├── 👉:0:A
        ├── 📙:3:B
        │   ├── ·a62b0de (🏘️)
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
    ");

    // Finally, show the normal version with just disambiguated 'B".
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── 📙►:1[1]:B
            ├── ·a62b0de (⌂|🏘️|1) ►A
            └── ·120a217 (⌂|🏘️|1)
                └── ►:2[2]:main
                    └── ·fafd9d0 (⌂|🏘️|1)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
    └── ≡📙:1:B
        ├── 📙:1:B
        │   ├── ·a62b0de (🏘️) ►A
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── ·3ea2742 (⌂|🏘️|1)
            └── ►:1[1]:A
                ├── ·a62b0de (⌂|🏘️|1)
                └── ·120a217 (⌂|🏘️|1)
                    └── ►:2[2]:main
                        └── ·fafd9d0 (⌂|🏘️|1)
    ");
    // TODO: add more stacks.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
    └── ≡:1:A
        ├── :1:A
        │   ├── ·a62b0de (🏘️)
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
    ");

    let (id, ref_name) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, ref_name, &*meta, standard_options())?.validated()?;
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── 📕►►►:1[0]:gitbutler/workspace
        └── ·3ea2742 (⌂|🏘️)
            └── 👉►:0[1]:A
                ├── ·a62b0de (⌂|🏘️|1)
                └── ·120a217 (⌂|🏘️|1)
                    └── ►:2[2]:main
                        └── ·fafd9d0 (⌂|🏘️|1)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:1:gitbutler/workspace <> ✓!
    └── ≡👉:0:A
        ├── 👉:0:A
        │   ├── ·a62b0de (🏘️)
        │   └── ·120a217 (🏘️)
        └── :2:main
            └── ·fafd9d0 (🏘️)
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
    insta::assert_snapshot!(graph_tree(&graph), @r"
    └── ►:1[0]:origin/main
        └── 👉📕►►►:0[1]:gitbutler/workspace
            └── ·8ee08de (⌂|🏘️|✓|1)
                └── ►:2[2]:A
                    └── ·120a217 (⌂|🏘️|✓|1)
                        └── ►:3[3]:main
                            └── ·fafd9d0 (⌂|🏘️|✓|1)
    ");
    // Everything is integrated, so nothing is shown.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 120a217");
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
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── ►:1[1]:anon:
            └── ·dca94a4 (⌂|🏘️|1)
                └── ►:2[2]:A
                    └── ·120a217 (⌂|🏘️|1)
                        └── ►:3[3]:main
                            └── ·fafd9d0 (⌂|🏘️|1)
    ");

    // It's notable how hard the workspace ref tries to not own the commit
    // it's under unless it's a managed commit.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
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
    // It's notable that `lane` can't pick up its additional segments as these aren't on the actual
    // segment, they are on the base which isn't where it can pick them up from and isn't were they
    // would be created.
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   ├── 📙►:3[1]:lane
    │   │   └── ·cbc6713 (⌂|🏘️|1)
    │   │       └── ►:2[4]:main <> origin/main →:1:
    │   │           └── ·fafd9d0 (⌂|🏘️|✓|11) ►lane-segment-01, ►lane-segment-02
    │   └── 📙►:4[1]:lane-2
    │       └── 📙►:5[2]:lane-2-segment-01
    │           └── 📙►:6[3]:lane-2-segment-02
    │               └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:4:lane-2 on fafd9d0
    │   ├── 📙:4:lane-2
    │   ├── 📙:5:lane-2-segment-01
    │   └── 📙:6:lane-2-segment-02
    └── ≡📙:3:lane on fafd9d0
        └── 📙:3:lane
            └── ·cbc6713 (🏘️)
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   ├── 📙►:4[1]:lane-2
    │   │   └── 📙►:5[2]:lane-2-segment-01
    │   │       └── 📙►:6[3]:lane-2-segment-02
    │   │           └── ►:2[4]:main <> origin/main →:1:
    │   │               └── ·fafd9d0 (⌂|🏘️|✓|11) ►lane-segment-01, ►lane-segment-02
    │   └── 📙►:3[1]:lane
    │       └── ·cbc6713 (⌂|🏘️|1)
    │           └── →:2: (main →:1:)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
    ├── ≡📙:3:lane on fafd9d0
    │   └── 📙:3:lane
    │       └── ·cbc6713 (🏘️)
    └── ≡📙:4:lane-2 on fafd9d0
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·4f08b8d (⌂|🏘️|1)
    │       └── ►:3[1]:B
    │           └── ·da597e8 (⌂|🏘️|1)
    │               └── ►:4[2]:A <> origin/A →:5:
    │                   └── ·1818c17 (⌂|🏘️|101)
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·281456a (⌂|🏘️|✓|111)
    └── ►:5[0]:origin/A →:4:
        └── ►:1[1]:origin/main →:2:
            └── 🟣0b6b861 (✓)
                └── →:2: (main →:1:)
    ");

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on 281456a
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
    └── 👉📕►►►:0[0]:gitbutler/workspace
        └── ·8926b15 (⌂|🏘️|1)
            └── ►:1[1]:main
                └── ·3686017 (⌂|🏘️|1)
                    └── ►:2[2]:gitbutler/edit
                        └── ·9725482 (⌂|🏘️|1)
                            └── ►:3[3]:gitbutler/target
                                └── ·fafd9d0 (⌂|🏘️|1)
    ");

    // But special handling for workspace views.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
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
    * ce09734 (gitbutler/target) M1
    * fafd9d0 init
    ");

    add_workspace(&mut meta);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "gitbutler/target"),
    )?
    .validated()?;
    // Standard handling after traversal and post-processing.
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·270738b (⌂|🏘️|1)
    │       └── ►:4[1]:A
    │           └── ·c59457b (⌂|🏘️|1)
    │               └── ►:5[2]:gitbutler/edit
    │                   └── ·e146f13 (⌂|🏘️|1)
    │                       └── ►:2[3]:main <> origin/main →:1:
    │                           └── ·971953d (⌂|🏘️|✓|11)
    │                               └── ►:3[4]:gitbutler/target
    │                                   ├── ·ce09734 (⌂|🏘️|✓|11)
    │                                   └── ·fafd9d0 (⌂|🏘️|✓|11)
    └── ►:1[0]:origin/main →:2:
        └── →:2: (main →:1:)
    ");

    // But special handling for workspace views. Note how we don't overshoot
    // and stop exactly where we have to, magically even.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on ce09734
    └── ≡:4:A on ce09734
        ├── :4:A
        │   ├── ·c59457b (🏘️)
        │   └── ·e146f13 (🏘️)
        └── :2:main <> origin/main →:1:
            └── ❄️971953d (🏘️|✓)
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
    | * c83f258 (A) A2-outside
    | | * c8f73c7 (B-middle) B3-outside
    | | * ff75b80 (intermediate-branch) B2-outside
    | | | *   59ae3cd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | | |/|\  
    | |/| | | 
    | | | | * 3f7c4e6 (C) C2
    | |_|_|/  
    |/| | |   
    * | | | b6895d7 C1
    | | | * 2f8f06d (B) B3
    | | |/  
    | | | *   867927f (origin/main, main) Merge branch 'B-middle'
    | | | |\  
    | | | |/  
    | | |/|   
    | | * | 91bc3fc B2
    | | * | cf9330f B1
    | |/ /  
    |/| |   
    | | * 6e03461 Merge branch 'A'
    | |/| 
    |/|/  
    | * a62b0de A2
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
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·59ae3cd (⌂|🏘️|1)
    │       ├── ►:5[3]:anon:
    │       │   ├── ·a62b0de (⌂|🏘️|✓|11)
    │       │   └── ·120a217 (⌂|🏘️|✓|11)
    │       │       └── ►:8[4]:anon:
    │       │           └── ·fafd9d0 (⌂|🏘️|✓|11)
    │       ├── ►:6[1]:B
    │       │   └── ·2f8f06d (⌂|🏘️|1)
    │       │       └── ►:4[2]:anon:
    │       │           ├── ·91bc3fc (⌂|🏘️|✓|11)
    │       │           └── ·cf9330f (⌂|🏘️|✓|11)
    │       │               └── →:8:
    │       └── ►:7[1]:C
    │           ├── ·3f7c4e6 (⌂|🏘️|1)
    │           └── ·b6895d7 (⌂|🏘️|1)
    │               └── →:8:
    └── ►:1[0]:origin/main →:2:
        └── ►:2[1]:main <> origin/main →:1:
            └── ·867927f (⌂|✓|10)
                ├── ►:3[2]:anon:
                │   └── ·6e03461 (⌂|✓|10)
                │       ├── →:8:
                │       └── →:5:
                └── →:4:
    ");

    // If it doesn't know how the workspace should be looking like, i.e. which branches are contained,
    // nothing special happens.
    // The branches that are outside the workspace don't exist and segments are flattened.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣2 on fafd9d0
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
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["B-middle"]);
    add_stack_with_segments(&mut meta, 2, "C", StackState::InWorkspace, &["C-bottom"]);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, ":/init"),
    )?
    .validated()?;
    // TODO: Is it possible for the traversal not to be cut?
    //       Why is it cut in the first place? Because the target tip
    //       finds it first and has no allowance after it meets with the workspace.
    //       So tips meeting limits have to follow-through and continue with their
    //       own limit settings, maybe with a combination/merge?
    insta::assert_snapshot!(graph_tree(&graph), @r"
    ├── 👉📕►►►:0[0]:gitbutler/workspace
    │   └── ·59ae3cd (⌂|🏘️|1)
    │       ├── ►:14[3]:anon:
    │       │   ├── ·a62b0de (⌂|🏘️|✓|11)
    │       │   └── ·120a217 (⌂|🏘️|✓|11)
    │       │       └── ►:3[4]:anon:
    │       │           └── ·fafd9d0 (⌂|🏘️|✓|11)
    │       ├── 📙►:5[1]:B
    │       │   └── ·2f8f06d (⌂|🏘️|1)
    │       │       └── ►:10[2]:anon:
    │       │           ├── ·91bc3fc (⌂|🏘️|✓|11)
    │       │           └── ✂️·cf9330f (⌂|🏘️|✓|11)
    │       └── 📙►:7[1]:C
    │           └── ·3f7c4e6 (⌂|🏘️|1)
    │               └── ►:15[2]:anon:
    │                   └── ·b6895d7 (⌂|🏘️|1)
    │                       └── →:3:
    ├── ►:1[0]:origin/main →:2:
    │   └── ►:2[1]:main <> origin/main →:1:
    │       └── ·867927f (⌂|✓|10)
    │           ├── ►:9[2]:anon:
    │           │   └── ·6e03461 (⌂|✓|10)
    │           │       ├── →:3:
    │           │       └── →:14:
    │           └── →:10:
    ├── 📙►:4[0]:A
    │   └── ·c83f258 (⌂)
    │       └── →:14:
    ├── 📙►:6[0]:B-middle
    │   └── ·c8f73c7 (⌂)
    │       └── ►:11[1]:intermediate-branch
    │           └── ·ff75b80 (⌂)
    │               └── →:10:
    └── 📙►:8[0]:C-bottom
        └── ·790a17d (⌂)
            ├── ►:12[1]:anon:
            │   └── ·969aaec (⌂)
            │       └── →:15:
            └── ►:13[1]:tmp
                └── ·631be19 (⌂)
                    └── →:15:
    ");

    // The workspace itself contains information about the outside tips.
    // TODO: make it work.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣2 on fafd9d0
    ├── ≡📙:7:C on fafd9d0
    │   └── 📙:7:C
    │       ├── ·3f7c4e6 (🏘️)
    │       └── ·b6895d7 (🏘️)
    ├── ≡📙:5:B
    │   └── 📙:5:B
    │       ├── ·2f8f06d (🏘️)
    │       ├── ·91bc3fc (🏘️|✓)
    │       └── ✂️·cf9330f (🏘️|✓)
    └── ≡:14:anon: on fafd9d0
        └── :14:anon:
            ├── ·a62b0de (🏘️|✓)
            └── ·120a217 (🏘️|✓)
    ");
    Ok(())
}
