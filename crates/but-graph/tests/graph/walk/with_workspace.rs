use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
    ref_metadata::{
        ProjectMeta, StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_graph::{
    SegmentMetadata, Workspace,
    walk::{Overlay, Seed, SeedRole},
};
use but_testsupport::{
    InMemoryRefMetadata, graph_dag, graph_workspace, visualize_commit_graph_all,
};
use snapbox::prelude::*;

use crate::walk::{
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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);
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
*    👉·59a427f (⌂|🏘)
├─╮
* │  ·0a415d8 (⌂|🏘) ►main <> origin/main
│ *  ·a62b0de (⌂|🏘) ►A
│ *  ·120a217 (⌂|🏘)
│ │ *  🟣1f5c47b ►origin/main
├───╯
* │  ·73ba99d (⌂|🏘)
├─╯
*  🏁·fafd9d0 (⌂|🏘)
layout:
  materialized parents: 59a427f: 0a415d8 a62b0de
"#]]
    );

    let managed_id = ws
        .managed_entrypoint_commit_id(&repo)?
        .expect("this is managed workspace commit");
    snapbox::assert_data_eq!(
        ws.commit_graph()
            .node(managed_id)
            .expect("managed commit is in the graph")
            .to_debug(),
        snapbox::str![[r#"
Commit(59a427f, ⌂|🏘►gitbutler/workspace[🌳])

"#]]
    );

    // It's perfectly valid to have the local tracking branch of our target in the workspace,
    // and the low-bound computation works as well.
    let ws = &ws;
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on fafd9d0
├── ≡:main <> origin/main⇡1⇣1 on fafd9d0
│   └── :main <> origin/main⇡1⇣1
│       ├── 🟣1f5c47b
│       ├── ·0a415d8 (🏘️)
│       └── ❄️73ba99d (🏘️)
└── ≡:A on fafd9d0
    └── :A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn workspace_with_only_local_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-contained-and-target-ahead")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5e2623 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * cb54dca (origin/main) RM1
|/  
* 0a415d8 (main) M3
* 73ba99d M2
* fafd9d0 init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·e5e2623 (⌂|🏘)
│ *  🟣cb54dca (✓) ►origin/main
├─╯
*  ·0a415d8 (⌂|🏘|✓) ►main <> origin/main
*  ·73ba99d (⌂|🏘|✓)
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: e5e2623: 0a415d8
"#]]
    );

    let ws = &ws;
    // It's notable how the local tracking branch of our target (origin/main) is ignored, it's not part of our workspace,
    // but acts as base.
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 0a415d8

"#]]
    );

    Ok(())
}

#[test]
fn reproduce_11483() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/reproduce-11483")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   3562fcd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 7236012 (A) A
* | 68c8a9d (B) B
|/  
* 3183e43 (origin/main, main, below) M1

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &["below"]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let ws = &ws;
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:B on 3183e43 {2}
│   ├── 📙:B
│   │   └── ·68c8a9d (🏘️)
│   └── 📙:below
└── ≡📙:A on 3183e43 {1}
    └── 📙:A
        └── ·7236012 (🏘️)

"#]]
    );

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["below"]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:B on 3183e43 {2}
│   └── 📙:B
│       └── ·68c8a9d (🏘️)
└── ≡📙:A on 3183e43 {1}
    ├── 📙:A
    │   └── ·7236012 (🏘️)
    └── 📙:below

"#]]
    );

    Ok(())
}

#[test]
fn workspace_projection_with_advanced_stack_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/advanced-stack-tip-outside-workspace")?;
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* cc0bf57 (B) B-outside
| * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* d69fe94 B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·cc0bf57 (⌂) ►B
│ *  👉·2076060 (⌂|🏘)
├─╯
*  ·d69fe94 (⌂|🏘)
*  ·09d8e52 (⌂|🏘) ►A
*  🏁·85efbe4 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 2076060: d69fe94
  empty chain anchors: 09d8e52^
"#]]
    );
    let ws = &ws;
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    │   ├── ·cc0bf57*
    │   └── ·d69fe94 (🏘️)
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn no_overzealous_stacks_due_to_workspace_metadata() -> anyhow::Result<()> {
    // NOTE: Was supposed to reproduce #11459, but it found another issue instead.
    let (repo, mut meta) = read_only_in_memory_scenario("ws/reproduce-11459")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 1, "X", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "feat-2", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡:anon: on a821094 {2}
│   └── :anon:
│       ├── ·835086d (🏘️) ►four, ►three
│       └── ·ff310d3 (🏘️)
└── ≡📙:X <> origin/X⇡1 on 3183e43 {1}
    └── 📙:X <> origin/X⇡1
        ├── ·0b203b5 (🏘️)
        └── ❄️4840f3b (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn single_stack_ambiguous() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack-ambiguous")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 20de6ee (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 70e9a36 (B) with-ref
* 320e105 (tag: without-ref) segment-B
* 2a31450 (ambiguous-01, B-empty) segment-B~1
* 70bde6b (origin/B, A-empty-03, A-empty-02, A-empty-01, A) segment-A
* fafd9d0 (origin/main, new-B, new-A, main) init

"#]]
    );

    // Just a workspace, no additional ref information.
    // As the segments are ambiguous, there are many unnamed segments.
    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  ·320e105 (⌂|🏘) ►tags/without-ref
*  ·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  ·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
layout:
  materialized parents: 20de6ee: 70e9a36
"#]]
    );

    // All non-integrated segments are visible.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:B <> origin/B⇡3 on fafd9d0
    └── :B <> origin/B⇡3
        ├── ·70e9a36 (🏘️)
        ├── ·320e105 (🏘️) ►tags/without-ref
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄️70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03

"#]]
    );

    // There is always a segment for the entrypoint, and code working with the graph
    // deals with that naturally.
    let (without_ref_id, ref_name) = id_at(&repo, "without-ref");
    let ws = Workspace::from_tip(
        without_ref_id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // See how tags ARE allowed to name a segment, at least when used as entrypoint.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  👉·320e105 (⌂|🏘) ►tags/without-ref
*  ·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  ·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
"#]]
    );
    // Now `HEAD` is outside a workspace, which goes to single-branch mode. But it knows it's in a workspace
    // and shows the surrounding parts, while marking the segment as entrypoint.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:B <> origin/B⇡1 on fafd9d0
    ├── :B <> origin/B⇡1
    │   └── ·70e9a36 (🏘️)
    └── 👉:tags/without-ref
        ├── ·320e105 (🏘️)
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03

"#]]
    );

    // We don't have to give it a ref-name
    let ws = Workspace::from_tip(
        without_ref_id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  👉·320e105 (⌂|🏘) ►tags/without-ref
*  ·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  ·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
"#]]
    );

    // Entrypoint is now unnamed (as no ref-name was provided for traversal)
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:B <> origin/B⇡1 on fafd9d0
    ├── :B <> origin/B⇡1
    │   └── ·70e9a36 (🏘️)
    └── 👉:anon:
        ├── ·320e105 (🏘️) ►tags/without-ref
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03

"#]]
    );

    // Putting the entrypoint onto a commit in an anonymous segment with ambiguous refs makes no difference.
    let (b_id_1, tag_ref_name) = id_at(&repo, "B-empty");
    let ws = Workspace::from_tip(
        b_id_1,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  ·320e105 (⌂|🏘) ►tags/without-ref
*  👉·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  ·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
"#]]
    );

    // Doing this is very much like edit mode, and there is always a segment starting at the entrypoint.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:B <> origin/B⇡2 on fafd9d0
    ├── :B <> origin/B⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►tags/without-ref
    └── 👉:anon:
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03

"#]]
    );

    // If we pass an entrypoint ref name, it will be used as segment name (despite being ambiguous without it)
    let ws = Workspace::from_tip(
        b_id_1,
        tag_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  ·320e105 (⌂|🏘) ►tags/without-ref
*  👉·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  ·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:B <> origin/B⇡2 on fafd9d0
    ├── :B <> origin/B⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►tags/without-ref
    └── 👉:B-empty
        ├── ·2a31450 (🏘️) ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03

"#]]
    );
    Ok(())
}

#[test]
fn single_stack_ws_insertions() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack-ambiguous")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 20de6ee (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 70e9a36 (B) with-ref
* 320e105 (tag: without-ref) segment-B
* 2a31450 (ambiguous-01, B-empty) segment-B~1
* 70bde6b (origin/B, A-empty-03, A-empty-02, A-empty-01, A) segment-A
* fafd9d0 (origin/main, new-B, new-A, main) init

"#]]
    );
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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  ·320e105 (⌂|🏘) ►tags/without-ref
*  ·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  ·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
layout:
  materialized parents: 20de6ee: 70e9a36
  empty chain anchors: 2a31450^
"#]]
    );

    // We pickup empty segments.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:B <> origin/B⇡2 on fafd9d0 {0}
    ├── 📙:B <> origin/B⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►tags/without-ref
    ├── 📙:B-empty
    │   └── ·2a31450 (🏘️) ►ambiguous-01
    ├── 📙:A-empty-03
    ├── 📙:A-empty-01
    └── 📙:A
        └── ❄70bde6b (🏘️) ►A-empty-02

"#]]
    );

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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  ·320e105 (⌂|🏘) ►tags/without-ref
*  ·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  ·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
layout:
  materialized parents: 20de6ee: 70e9a36
  empty chain anchors: 2a31450^ 70bde6b^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:B <> origin/B⇡2 on fafd9d0 {0}
    ├── 📙:B <> origin/B⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►tags/without-ref
    ├── 📙:B-empty
    │   └── ·2a31450 (🏘️) ►ambiguous-01
    └── 📙:A
        └── ❄70bde6b (🏘️)

"#]]
    );

    // Define only some of the branches, it should figure that out.
    // It respects the order of the mention in the stack, `A` before `A-empty-01`.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-empty-01"]);
    add_stack_with_segments(&mut meta, 1, "B-empty", StackState::InWorkspace, &["B"]);

    let (id, ref_name) = id_at(&repo, "A-empty-01");
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·20de6ee (⌂|🏘)
*  ·70e9a36 (⌂|🏘) ►B <> origin/B
*  ·320e105 (⌂|🏘) ►tags/without-ref
*  ·2a31450 (⌂|🏘) ►B-empty, ►ambiguous-01
*  👉·70bde6b (⌂|🏘) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►new-B, ►origin/main <> origin/main
layout:
  empty chain anchors: 70bde6b^ 2a31450^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:B <> origin/B⇡2 on fafd9d0 {1}
    ├── 📙:B <> origin/B⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►tags/without-ref
    ├── 📙:B-empty
    │   └── ·2a31450 (🏘️) ►ambiguous-01
    └── 👉📙:A-empty-01
        └── ❄70bde6b (🏘️) ►A-empty-02, ►A-empty-03

"#]]
    );

    add_stack_with_segments(&mut meta, 2, "new-A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "new-B", StackState::InWorkspace, &[]);

    let (id, ref_name) = id_at(&repo, "new-A");
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;

    // We can also summon new empty stacks from branches resting on the base, and set them
    // as entrypoint, to have two more stacks.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:B <> origin/B⇡2 on fafd9d0 {1}
│   ├── 📙:B <> origin/B⇡2
│   │   ├── ·70e9a36 (🏘️)
│   │   └── ·320e105 (🏘️) ►tags/without-ref
│   ├── 📙:B-empty
│   │   └── ·2a31450 (🏘️) ►ambiguous-01
│   └── 📙:A-empty-01
│       └── ❄70bde6b (🏘️) ►A-empty-02, ►A-empty-03
├── ≡👉📙:new-A on fafd9d0 {2}
│   └── 👉📙:new-A
└── ≡📙:new-B on fafd9d0 {3}
    └── 📙:new-B

"#]]
    );
    Ok(())
}

#[test]
fn first_parent_reachability_traverses_empty_segments() -> anyhow::Result<()> {
    // Regression guard: deriving the first-parent edge from the source commit's `parent_ids` must
    // not dead-end when a commit-less branch segment sits on the first-parent path. The same
    // `ws/single-stack-ambiguous` setup as `single_stack_ws_insertions` puts the empty segments
    // `A-empty-03`/`A-empty-01` between commit `2a31450` (B's side) and `70bde6b` (origin/B / A).
    // A first-parent excluded walk from `B` must descend through those empties to reach `70bde6b`
    // and `fafd9d0`; otherwise they leak into `origin/B..B` even though both are first-parent
    // ancestors of `B`.
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack-ambiguous")?;
    add_stack_with_segments(
        &mut meta,
        0,
        "B-empty",
        StackState::InWorkspace,
        &["B", "A-empty-03", "not-A-empty-02", "A-empty-01", "A"],
    );
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    let b = repo.rev_parse_single("B")?.detach(); // 70e9a36, above the empty chain
    let origin_b = repo.rev_parse_single("origin/B")?.detach(); // 70bde6b, below it

    // `origin/B` (70bde6b) is a first-parent ancestor of `B`, so nothing is reachable from it but
    // not from `B`.
    let leaked = ws.commit_ids_in_a_not_b(origin_b, b, but_graph::FirstParent::Yes)?;
    assert!(
        leaked.is_empty(),
        "empty segments on the first-parent path dead-ended the excluded walk, leaking \
         first-parent ancestors of B into origin/B..B: {leaked:?}"
    );
    Ok(())
}

#[test]
fn single_stack() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-stack")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2c12d75 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 320e105 (B) segment-B
* 2a31450 (B-sub) segment-B~1
* 70bde6b (A) segment-A
* fafd9d0 (origin/main, new-A, main) init

"#]]
    );

    // Just a workspace, no additional ref information.
    // It segments across the unambiguous ref names.
    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·2c12d75 (⌂|🏘)
*  ·320e105 (⌂|🏘) ►B
*  ·2a31450 (⌂|🏘) ►B-sub
*  ·70bde6b (⌂|🏘) ►A
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►origin/main <> origin/main
layout:
  materialized parents: 2c12d75: 320e105
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:B on fafd9d0
    ├── :B
    │   └── ·320e105 (🏘️)
    ├── :B-sub
    │   └── ·2a31450 (🏘️)
    └── :A
        └── ·70bde6b (🏘️)

"#]]
    );

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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·2c12d75 (⌂|🏘)
*  ·320e105 (⌂|🏘) ►B
*  ·2a31450 (⌂|🏘) ►B-sub
*  ·70bde6b (⌂|🏘) ►A
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►new-A, ►origin/main <> origin/main
layout:
  materialized parents: 2c12d75: 320e105 fafd9d0
  empty chain anchors: 320e105^ fafd9d0
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:B on fafd9d0 {0}
│   ├── 📙:B
│   │   └── ·320e105 (🏘️)
│   ├── 📙:B-sub
│   │   └── ·2a31450 (🏘️)
│   └── 📙:A
│       └── ·70bde6b (🏘️)
└── ≡📙:new-A on fafd9d0 {1}
    └── 📙:new-A

"#]]
    );

    Ok(())
}

#[test]
fn single_merge_into_main_base_archived() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/single-merge-into-main")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 866c905 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* c6d714c (C) C
*   0cc5a6f (origin/main, merge, main) Merge branch 'A' into merge
|\  
| * e255adc (A) A
* | 7fdb58d (B) B
|/  
* fafd9d0 init

"#]]
        .raw()
    );

    let stack_id = add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["merge"]);

    // By default, everything with metadata on the branch will show up, even if on the base.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
└── ≡📙:C on 0cc5a6f {0}
    ├── 📙:C
    │   └── ·c6d714c (🏘️)
    └── 📙:merge

"#]]
    );

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

    let ws = ws.redo(&repo, &*meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
└── ≡📙:C on 0cc5a6f {0}
    └── 📙:C
        └── ·c6d714c (🏘️)

"#]]
    );

    // Finally, when the 'merge' branch is independent, it still works as it should.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "merge", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
├── ≡📙:C on 0cc5a6f {0}
│   └── 📙:C
│       └── ·c6d714c (🏘️)
└── ≡📙:merge on 0cc5a6f {1}
    └── 📙:merge

"#]]
    );

    // The order is respected.
    add_stack_with_segments(&mut meta, 1, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 0, "merge", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
├── ≡📙:merge on 0cc5a6f {0}
│   └── 📙:merge
└── ≡📙:C on 0cc5a6f {1}
    └── 📙:C
        └── ·c6d714c (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn minimal_merge_no_refs() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("ws/dual-merge-no-refs")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    // Without hints.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·47e1cf1 (⌂)
*    ·f40fb16 (⌂)
├─╮
* │  ·450c58a (⌂)
│ *  ·c6d714c (⌂)
├─╯
*    ·0cc5a6f (⌂)
├─╮
* │  ·7fdb58d (⌂)
│ *  ·e255adc (⌂)
├─╯
*  🏁·fafd9d0 (⌂)
layout:
  materialized parents: 47e1cf1: f40fb16
"#]]
    );

    // This a very untypical setup, but it's not forbidden. Code might want to check
    // if the workspace commit is actually managed before proceeding.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:gitbutler/workspace[🌳] <> ✓!
└── ≡:gitbutler/workspace[🌳] {1}
    └── :gitbutler/workspace[🌳]
        ├── ·47e1cf1
        ├── ·f40fb16
        ├── ·450c58a
        ├── ·0cc5a6f
        ├── ·7fdb58d
        └── ·fafd9d0

"#]]
    );
    Ok(())
}

#[test]
fn segment_on_each_incoming_connection() -> anyhow::Result<()> {
    // Validate that the graph is truly having segments whenever there is an incoming connection.
    // This is required to not need special edge-weights.
    let (repo, mut meta) = read_only_in_memory_scenario("ws/graph-splitting")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 98c5aba (entrypoint) C
* 807b6ce B
* 6d05486 A
| * b6917c7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * f7fe830 (main) other-2
|/  
* b688f2d other-1
* fafd9d0 init

"#]]
    );

    // Without hints - needs to split `refs/heads/main` at `b688f2d`
    let (id, name) = id_at(&repo, "entrypoint");
    add_workspace(&mut meta);
    let ws = Workspace::from_tip(id, name, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·98c5aba (⌂) ►entrypoint
*  ·807b6ce (⌂)
*  ·6d05486 (⌂)
│ *  ·b6917c7 (⌂|🏘)
│ *  ·f7fe830 (⌂|🏘) ►main
├─╯
*  ·b688f2d (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘)
"#]]
    );
    // This is an unmanaged workspace, even though commits from a workspace flow into it.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:entrypoint <> ✓!
└── ≡:entrypoint {1}
    └── :entrypoint
        ├── ·98c5aba
        ├── ·807b6ce
        ├── ·6d05486
        ├── ·b688f2d (🏘️)
        └── ·fafd9d0 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn minimal_merge() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/dual-merge")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    // Without hints, and no workspace data, the branch is normal!
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·47e1cf1 (⌂)
*    ·f40fb16 (⌂) ►merge-2
├─╮
* │  ·450c58a (⌂) ►D
│ *  ·c6d714c (⌂) ►C
├─╯
*    ·0cc5a6f (⌂) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
├─╮
* │  ·7fdb58d (⌂) ►B
│ *  ·e255adc (⌂) ►A
├─╯
*  🏁·fafd9d0 (⌂) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 47e1cf1: f40fb16
"#]]
    );

    // Without workspace data this becomes a single-branch workspace, with `main` as normal segment.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:gitbutler/workspace[🌳] <> ✓!
└── ≡:gitbutler/workspace[🌳] {1}
    ├── :gitbutler/workspace[🌳]
    │   └── ·47e1cf1
    ├── :merge-2
    │   └── ·f40fb16
    ├── :D
    │   ├── ·450c58a
    │   └── ·0cc5a6f ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    ├── :B
    │   └── ·7fdb58d
    └── :main <> origin/main
        └── ❄️fafd9d0

"#]]
    );

    // There is empty stacks on top of `merge`, and they need to be connected to the incoming segments and the outgoing ones.
    // This also would leave the original segment empty unless we managed to just put empty stacks on top.
    add_stack_with_segments(
        &mut meta,
        0,
        "empty-2-on-merge",
        StackState::InWorkspace,
        &["empty-1-on-merge", "merge"],
    );
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·47e1cf1 (⌂|🏘)
*    ·f40fb16 (⌂|🏘) ►merge-2
├─╮
* │  ·450c58a (⌂|🏘) ►D
│ *  ·c6d714c (⌂|🏘) ►C
├─╯
*    ·0cc5a6f (⌂|🏘) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
├─╮
* │  ·7fdb58d (⌂|🏘) ►B
│ *  ·e255adc (⌂|🏘) ►A
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 47e1cf1: f40fb16
  empty chain anchors: 0cc5a6f^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:merge-2 on fafd9d0 {0}
    ├── :merge-2
    │   └── ·f40fb16 (🏘️)
    ├── :D
    │   └── ·450c58a (🏘️)
    ├── 📙:empty-2-on-merge
    ├── 📙:empty-1-on-merge
    ├── 📙:merge
    │   └── ·0cc5a6f (🏘️)
    └── :B
        └── ·7fdb58d (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn entrypoint_inside_second_parent_of_workspace_diamond_is_included() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/dual-merge")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );
    add_workspace(&mut meta);
    let (id, name) = id_at(&repo, "C");
    let ws = Workspace::from_tip(id, name, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·47e1cf1 (⌂|🏘)
*    ·f40fb16 (⌂|🏘) ►merge-2
├─╮
* │  ·450c58a (⌂|🏘) ►D
│ *  👉·c6d714c (⌂|🏘) ►C
├─╯
*    ·0cc5a6f (⌂|🏘) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
├─╮
* │  ·7fdb58d (⌂|🏘) ►B
│ *  ·e255adc (⌂|🏘) ►A
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:merge-2 on fafd9d0
    ├── :merge-2
    │   └── ·f40fb16 (🏘️)
    ├── 👉:C
    │   ├── ·c6d714c (🏘️)
    │   └── ·0cc5a6f (🏘️) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    └── :B
        └── ·7fdb58d (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn stack_configuration_is_respected_if_one_of_them_is_an_entrypoint() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-two-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* fafd9d0 (HEAD -> gitbutler/workspace, main, B, A) init

"#]]
    );

    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let extra_target_options = standard_options_with_extra_target(&repo, "main");
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        extra_target_options.clone(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►main
layout:
  empty chain anchors: fafd9d0 fafd9d0
"#]]
    );
    assert_eq!(
        ws.entrypoint_commit_id()?,
        extra_target_options.extra_target_commit_id,
        "entrypoint points to a virtual workspace tip segment \
        which can't unambiguously find the commit"
    );
    assert_eq!(
        ws.tip_commit_id(),
        extra_target_options.extra_target_commit_id,
        "workspace query falls back to the ref-info commit for ambiguous empty segments"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on fafd9d0
├── ≡📙:A on fafd9d0 {1}
│   └── 📙:A
└── ≡📙:B on fafd9d0 {2}
    └── 📙:B

"#]]
    );

    let (id, ref_name) = id_at(&repo, "B");
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        extra_target_options.clone(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►main
layout:
  empty chain anchors: fafd9d0 fafd9d0
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on fafd9d0
├── ≡📙:A on fafd9d0 {1}
│   └── 📙:A
└── ≡👉📙:B on fafd9d0 {2}
    └── 👉📙:B

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        extra_target_options,
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►main
layout:
  empty chain anchors: fafd9d0 fafd9d0
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on fafd9d0
├── ≡👉📙:A on fafd9d0 {1}
│   └── 👉📙:A
└── ≡📙:B on fafd9d0 {2}
    └── 📙:B

"#]]
    );

    Ok(())
}

#[test]
fn just_init_with_branches() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    // Note the dedicated workspace branch without a workspace commit.
    // All is fair game, and we use it to validate 'empty parent branch handling after new children took the commit'.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init

"#]]
    );

    // Without hints - `main` is picked up as it's the entrypoint.
    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![
            "*  👉🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►D, ►E, ►F, ►main[🌳], ►origin/main <> origin/main"
        ]
    );

    // There is no workspace as `main` is the base of the workspace, so it's shown directly
    // as a downgraded single-branch view. The target context is preserved, and the fully
    // integrated base commit is pruned while keeping the branch container.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main
└── ≡:main[🌳] <> origin/main {1}
    └── :main[🌳] <> origin/main

"#]]
    );

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let ws = Workspace::from_tip(
        id,
        ws_ref_name.clone(),
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![
            "*  👉🏁·fafd9d0 (⌂|🏘) ►A, ►B, ►C, ►D, ►E, ►F, ►main[🌳], ►origin/main <> origin/main"
        ]
    );

    // However, when the workspace is checked out, it's at least empty.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓!
└── ≡:main[🌳] <> origin/main
    └── :main[🌳] <> origin/main
        └── ❄️fafd9d0 (🏘️) ►A, ►B, ►C, ►D, ►E, ►F

"#]]
    );

    // The simplest possible setup where we can define how the workspace should look like,
    // in terms of dependent and independent virtual segments.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·fafd9d0 (⌂|🏘) ►A, ►B, ►C, ►D, ►E, ►F, ►main[🌳], ►origin/main <> origin/main
layout:
  empty chain anchors: fafd9d0 fafd9d0
"#]]
    );

    // With empty project metadata, workspace segmentation is retained around the workspace ref.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓! on fafd9d0
├── ≡📙:C on fafd9d0 {0}
│   ├── 📙:C
│   ├── 📙:B
│   └── 📙:A
└── ≡📙:D on fafd9d0 {1}
    ├── 📙:D
    ├── 📙:E
    └── 📙:F

"#]]
    );

    let ws = Workspace::from_tip(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // Now the dependent segments are applied, and so is the separate stack.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►D, ►E, ►F, ►main[🌳], ►origin/main <> origin/main
layout:
  empty chain anchors: fafd9d0 fafd9d0
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:C on fafd9d0 {0}
│   ├── 📙:C
│   ├── 📙:B
│   └── 📙:A
└── ≡📙:D on fafd9d0 {1}
    ├── 📙:D
    ├── 📙:E
    └── 📙:F

"#]]
    );

    let ws = ws.anonymized(&repo.remote_names())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:G <> ✓refs/remotes/remote-0/H on fafd9d0
├── ≡📙:A on fafd9d0 {0}
│   ├── 📙:A
│   ├── 📙:B
│   └── 📙:C
└── ≡📙:D on fafd9d0 {1}
    ├── 📙:D
    ├── 📙:E
    └── 📙:F

"#]]
    );

    Ok(())
}

#[test]
fn tips_equivalent_to_workspace_metadata_are_order_independent() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init

"#]]
    );

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
        Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    let head_baseline_tree = graph_dag(&head_baseline);
    let head_baseline_workspace = graph_workspace(&head_baseline).to_string();

    let head_tips = vec![
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("F"),
        }),
        Seed::new(commit_id)
            .with_ref_name(Some(ws_ref_name.clone()))
            .with_role(SeedRole::Workspace)
            .with_metadata(SegmentMetadata::Workspace(workspace_metadata.clone())),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("B"),
        }),
        Seed::new(commit_id)
            .with_ref_name(Some(origin_main_ref.clone()))
            .with_role(SeedRole::TargetRemote),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("A"),
        }),
        Seed::new(commit_id)
            .with_ref_name(Some(main_ref.clone()))
            .with_entrypoint(),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("E"),
        }),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("C"),
        }),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("D"),
        }),
    ];

    let workspace_baseline = Workspace::from_tip(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    let workspace_baseline_tree = graph_dag(&workspace_baseline);
    let workspace_baseline_workspace = graph_workspace(&workspace_baseline);
    snapbox::assert_data_eq!(
        workspace_baseline_workspace.to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:C on fafd9d0 {0}
│   ├── 📙:C
│   ├── 📙:B
│   └── 📙:A
└── ≡📙:D on fafd9d0 {1}
    ├── 📙:D
    ├── 📙:E
    └── 📙:F

"#]]
    );
    let workspace_baseline_workspace = workspace_baseline_workspace.to_string();

    let explicit_seeds = vec![
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("E"),
        }),
        Seed::new(commit_id).with_role(SeedRole::TargetLocal {
            local_ref_name: main_ref.clone(),
        }),
        Seed::new(commit_id)
            .with_ref_name(Some(ws_ref_name.clone()))
            .with_role(SeedRole::Workspace)
            .with_metadata(SegmentMetadata::Workspace(workspace_metadata))
            .with_entrypoint(),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("C"),
        }),
        Seed::new(commit_id)
            .with_ref_name(Some(origin_main_ref))
            .with_role(SeedRole::TargetRemote),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("F"),
        }),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("A"),
        }),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("D"),
        }),
        Seed::new(commit_id).with_role(SeedRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("B"),
        }),
    ];
    // NOTE: `from_seeds` remains WALK-backed (explicit seeds have no flip
    // counterpart yet), while `from_head`/`from_tip` build via the flip — the
    // cross-API tree equality of the walk era no longer applies. Order-independence is asserted
    // by snapshotting both orderings directly.
    let ws = Workspace::from_seeds(
        &repo,
        head_tips,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    let _ = (head_baseline_tree, head_baseline_workspace);
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓refs/remotes/origin/main
└── ≡:main[🌳] <> origin/main {1}
    └── :main[🌳] <> origin/main

"#]]
    );

    let ws = Workspace::from_seeds(
        &repo,
        explicit_seeds.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    let _ = workspace_baseline_tree;
    let explicit_workspace = graph_workspace(&ws);
    snapbox::assert_data_eq!(
        explicit_workspace.to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:C on fafd9d0 {0}
│   ├── 📙:C
│   ├── 📙:B
│   └── 📙:A
└── ≡📙:D on fafd9d0 {1}
    ├── 📙:D
    ├── 📙:E
    └── 📙:F

"#]]
    );
    let _ = (explicit_workspace, workspace_baseline_workspace);

    Ok(())
}

#[test]
fn workspace_target_commit_and_extra_target_commit_can_overlap() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-two-branches")?;
    let target_id = id_by_rev(&repo, "main").detach();
    add_workspace_with_target(&mut meta, target_id);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let baseline = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let baseline_tree = graph_dag(&baseline);
    let baseline_workspace = graph_workspace(&baseline).to_string();

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(target_id),
    )?
    .validated()?;

    assert_eq!(
        graph_dag(&ws),
        baseline_tree,
        "duplicated synthetic integrated tips should not change graph traversal"
    );
    assert_eq!(
        graph_workspace(&ws).to_string(),
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

    let baseline = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let baseline_tree = graph_dag(&baseline);
    let baseline_workspace = graph_workspace(&baseline).to_string();

    add_stack_with_segments(&mut meta, 3, "B", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    assert_eq!(
        graph_dag(&ws),
        baseline_tree,
        "duplicate stack branch metadata (B) should not enqueue the same stack branch traversal twice"
    );
    assert_eq!(
        graph_workspace(&ws).to_string(),
        baseline_workspace,
        "duplicate stack branch metadata should not change workspace projection"
    );

    Ok(())
}

#[test]
fn just_init_with_archived_branches() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    // Note the dedicated workspace branch without a workspace commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* fafd9d0 (HEAD -> main, origin/main, gitbutler/workspace, F, E, D, C, B, A) init

"#]]
    );

    let stack_id = add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let ws = Workspace::from_tip(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;

    // By default, we see both stacks as they are configured, which disambiguates them.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:C on fafd9d0 {0}
    ├── 📙:C
    ├── 📙:B
    └── 📙:A

"#]]
    );

    meta.data_mut()
        .branches
        .get_mut(&stack_id)
        .expect("just added")
        .heads[1]
        .archived = true;

    // The first archived segment causes everything else to be hidden.
    let ws = ws.redo(&repo, &*meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:C {0}
    └── 📙:C

"#]]
    );

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = false;

    // Now only the first one is archived.
    let ws = ws.redo(&repo, &*meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:C {0}
    ├── 📙:C
    └── 📙:B

"#]]
    );

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = true;
    heads[2].archived = true;

    // Archiving everything removes the stack entirely.
    let ws = ws.redo(&repo, &*meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0

"#]]
    );
    Ok(())
}

#[test]
fn two_stacks_many_refs() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/one-stacks-many-refs")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 298d938 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 16f132b (S1, G, F) 2
* 917b9da (E, D) 1
* fafd9d0 (origin/main, main, C, B, A) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // Without any information it looks quite barren.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·298d938 (⌂|🏘)
*  ·16f132b (⌂|🏘) ►F, ►G, ►S1
*  ·917b9da (⌂|🏘) ►D, ►E
*  🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►main, ►origin/main <> origin/main
layout:
  materialized parents: 298d938: 16f132b
"#]]
    );

    // With no workspace at all as the workspace segment isn't split.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:anon: on fafd9d0
    └── :anon:
        ├── ·16f132b (🏘️) ►F, ►G, ►S1
        └── ·917b9da (🏘️) ►D, ►E

"#]]
    );

    let (id, ref_name) = id_at(&repo, "S1");
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // The S1 starting position is a split, so there is more.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·298d938 (⌂|🏘)
*  👉·16f132b (⌂|🏘) ►F, ►G, ►S1
*  ·917b9da (⌂|🏘) ►D, ►E
*  🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►main, ►origin/main <> origin/main
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡👉:S1 on fafd9d0
    └── 👉:S1
        ├── ·16f132b (🏘️) ►F, ►G
        └── ·917b9da (🏘️) ►D, ►E

"#]]
    );

    // Define the workspace.
    add_stack_with_segments(&mut meta, 1, "C", StackState::InWorkspace, &["B"]);
    add_stack_with_segments(&mut meta, 2, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "S1", StackState::InWorkspace, &["G", "F"]);
    add_stack_with_segments(&mut meta, 4, "D", StackState::InWorkspace, &["E"]);

    // We see that all segments are used, stacks in metadata order: C B A S1 G F D E
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·298d938 (⌂|🏘)
*  ·16f132b (⌂|🏘) ►F, ►G, ►S1
*  ·917b9da (⌂|🏘) ►D, ►E
*  🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►main, ►origin/main <> origin/main
layout:
  materialized parents: 298d938: fafd9d0 fafd9d0 16f132b
  empty chain anchors: fafd9d0 fafd9d0 16f132b^ 917b9da^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:C on fafd9d0 {1}
│   ├── 📙:C
│   └── 📙:B
├── ≡📙:A on fafd9d0 {2}
│   └── 📙:A
└── ≡📙:S1 on fafd9d0 {3}
    ├── 📙:S1
    ├── 📙:G
    ├── 📙:F
    │   └── ·16f132b (🏘️)
    └── 📙:E
        └── ·917b9da (🏘️)

"#]]
    );

    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // This should look the same as before, despite the starting position.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·298d938 (⌂|🏘)
*  👉·16f132b (⌂|🏘) ►F, ►G, ►S1
*  ·917b9da (⌂|🏘) ►D, ►E
*  🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►main, ►origin/main <> origin/main
layout:
  empty chain anchors: fafd9d0 fafd9d0 16f132b^ 917b9da^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:C on fafd9d0 {1}
│   ├── 📙:C
│   └── 📙:B
├── ≡📙:A on fafd9d0 {2}
│   └── 📙:A
└── ≡👉📙:S1 on fafd9d0 {3}
    ├── 👉📙:S1
    ├── 📙:G
    ├── 📙:F
    │   └── ·16f132b (🏘️)
    └── 📙:E
        └── ·917b9da (🏘️)

"#]]
    );
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
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►D, ►E, ►F, ►main[🌳], ►origin/main <> origin/main
layout:
  empty chain anchors: fafd9d0 fafd9d0 fafd9d0 fafd9d0
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:C on fafd9d0 {0}
│   ├── 📙:C
│   └── 📙:B
├── ≡📙:A on fafd9d0 {1}
│   └── 📙:A
├── ≡📙:D on fafd9d0 {2}
│   ├── 📙:D
│   └── 📙:E
└── ≡📙:F on fafd9d0 {3}
    └── 📙:F

"#]]
    );

    let (id, ref_name) = id_at(&repo, "C");
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // The entrypoint shouldn't affect the outcome (even though it changes the initial segmentation).
    // However, as the segment it's on is integrated, it's not considered to be part of the workspace.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►C, ►D, ►E, ►F, ►main[🌳], ►origin/main <> origin/main
layout:
  empty chain anchors: fafd9d0 fafd9d0 fafd9d0 fafd9d0
"#]]
    );

    // We should see the same stacks as we did before, just with a different entrypoint.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡👉📙:C on fafd9d0 {0}
│   ├── 👉📙:C
│   └── 📙:B
├── ≡📙:A on fafd9d0 {1}
│   └── 📙:A
├── ≡📙:D on fafd9d0 {2}
│   ├── 📙:D
│   └── 📙:E
└── ≡📙:F on fafd9d0 {3}
    └── 📙:F

"#]]
    );
    Ok(())
}

#[test]
fn proper_remote_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/proper-remote-ahead")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 9bcd3af (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * ca7baa7 (origin/main) only-remote-02
| * 7ea1468 only-remote-01
|/  
* 998eae6 (main) shared
* fafd9d0 init

"#]]
    );

    // Remote segments are picked up automatically and traversed - they never take ownership of already assigned commits.
    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·9bcd3af (⌂|🏘)
│ *  🟣ca7baa7 (✓) ►origin/main
│ *  🟣7ea1468 (✓)
├─╯
*  ·998eae6 (⌂|🏘|✓) ►main <> origin/main
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: 9bcd3af: 998eae6
"#]]
    );

    // Everything in the workspace is integrated, thus it's empty.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 998eae6

"#]]
    );

    let (id, ref_name) = id_at(&repo, "main");
    // The integration branch can be in the workspace and be checked out.
    let ws = Workspace::from_tip(
        id,
        Some(ref_name),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·9bcd3af (⌂|🏘)
│ *  🟣ca7baa7 (✓) ►origin/main
│ *  🟣7ea1468 (✓)
├─╯
*  👉·998eae6 (⌂|🏘|✓) ►main <> origin/main
*  🏁·fafd9d0 (⌂|🏘|✓)
"#]]
    );

    // If it's checked out, we must show the branch container, but it's not part of the
    // managed workspace. The target context is preserved and integrated local/base commits
    // are pruned, leaving only target-side commits ahead of the stored target.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main <> ✓refs/remotes/origin/main⇣2
└── ≡:main <> origin/main⇣2 {1}
    └── :main <> origin/main⇣2
        ├── 🟣ca7baa7 (✓)
        └── 🟣7ea1468 (✓)

"#]]
    );
    Ok(())
}

#[test]
fn deduced_remote_ahead() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/deduced-remote-ahead")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
    );

    // Remote segments are picked up automatically and traversed - they never take ownership of already assigned commits.
    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·8b39ce4 (⌂|🏘)
*  ·9d34471 (⌂|🏘) ►A <> origin/A
*  ·5b89c71 (⌂|🏘)
│ *  🟣3ea1a8f ►origin/A, ►push-remote/A
│ *  🟣9c50f71
│ *  🟣2cfbb79
╭─┤
│ *  🟣e898cd0
├─╯
*  ·998eae6 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
layout:
  materialized parents: 8b39ce4: 9d34471
"#]]
    );
    // There is no target branch, so nothing is integrated, and `main` shows up.
    // It's not special.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:A <> origin/A⇡2⇣4
    ├── :A <> origin/A⇡2⇣4
    │   ├── 🟣3ea1a8f
    │   ├── 🟣9c50f71
    │   ├── 🟣2cfbb79
    │   ├── 🟣e898cd0
    │   ├── ·9d34471 (🏘️)
    │   ├── ·5b89c71 (🏘️)
    │   └── ❄️998eae6 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    let id = id_by_rev(&repo, ":/init");
    let ws = Workspace::from_tip(id, None, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·8b39ce4 (⌂|🏘)
*  ·9d34471 (⌂|🏘) ►A <> origin/A
*  ·5b89c71 (⌂|🏘)
│ *  🟣3ea1a8f ►origin/A, ►push-remote/A
│ *  🟣9c50f71
│ *  🟣2cfbb79
╭─┤
│ *  🟣e898cd0
├─╯
*  ·998eae6 (⌂|🏘)
*  👉🏁·fafd9d0 (⌂|🏘) ►main
"#]]
    );
    // The whole workspace is visible, but it's clear where the entrypoint is.
    // As there is no target ref, `main` shows up.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:A <> origin/A⇡2⇣4
    ├── :A <> origin/A⇡2⇣4
    │   ├── 🟣3ea1a8f
    │   ├── 🟣9c50f71
    │   ├── 🟣2cfbb79
    │   ├── 🟣e898cd0
    │   ├── ·9d34471 (🏘️)
    │   ├── ·5b89c71 (🏘️)
    │   └── ❄️998eae6 (🏘️)
    └── 👉:main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    // When the push-remote is configured, it overrides the remote we use for listing, even if a fetch remote is available.
    let mut ws = meta.workspace(WORKSPACE_REF_NAME.try_into().expect("valid workspace ref"))?;
    let mut pm = ws.project_meta();
    pm.push_remote = Some("push-remote".into());
    ws.set_project_meta(pm);
    meta.set_workspace(&ws)?;
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·8b39ce4 (⌂|🏘)
*  ·9d34471 (⌂|🏘) ►A <> push-remote/A
*  ·5b89c71 (⌂|🏘)
│ *  🟣3ea1a8f ►origin/A, ►push-remote/A
│ *  🟣9c50f71
│ *  🟣2cfbb79
╭─┤
│ *  🟣e898cd0
├─╯
*  ·998eae6 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
layout:
  materialized parents: 8b39ce4: 9d34471
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:A <> push-remote/A⇡2⇣4
    ├── :A <> push-remote/A⇡2⇣4
    │   ├── 🟣3ea1a8f
    │   ├── 🟣9c50f71
    │   ├── 🟣2cfbb79
    │   ├── 🟣e898cd0
    │   ├── ·9d34471 (🏘️)
    │   ├── ·5b89c71 (🏘️)
    │   └── ❄️998eae6 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn stacked_rebased_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-includes-another-remote")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 682be32 (origin/B) B
* e29c23d (origin/A) A
| * 7786959 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 312f819 (B) B
| * e255adc (A) A
|/  
* fafd9d0 (origin/main, main) init

"#]]
    );

    // This is like remotes have been stacked and are completely rebased so they differ from their local
    // commits. This also means they include each other.
    add_workspace(&mut meta);
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
*  👉·7786959 (⌂|🏘)
*  ·312f819 (⌂|🏘) ►B
*  ·e255adc (⌂|🏘) ►A
*  🏁·fafd9d0 (⌂|🏘) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 7786959: 312f819
"#]]
    );
    // It's worth noting that we avoid double-listing remote commits that are also
    // directly owned by another remote segment.
    // they have to be considered as something relevant to the branch history.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:B
    ├── :B
    │   └── ·312f819 (🏘️)
    ├── :A
    │   └── ·e255adc (🏘️)
    └── :main <> origin/main
        └── ❄️fafd9d0 (🏘️)

"#]]
    );

    // The result is the same when changing the entrypoint.
    let (id, name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(id, name, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·7786959 (⌂|🏘)
*  ·312f819 (⌂|🏘) ►B <> origin/B
*  👉·e255adc (⌂|🏘) ►A <> origin/A
│ *  🟣682be32 ►origin/B
│ *  🟣e29c23d ►origin/A
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:B <> origin/B⇡1⇣1 on fafd9d0
    ├── :B <> origin/B⇡1⇣1
    │   ├── 🟣682be32
    │   └── ·312f819 (🏘️)
    └── 👉:A <> origin/A⇡1⇣1
        ├── 🟣e29c23d
        └── ·e255adc (🏘️)

"#]]
    );
    snapbox::assert_data_eq!(
        format!("{:#?}", ws.statistics()).as_str(),
        snapbox::str![[r#"
CommitGraphStatistics {
    commits: 6,
    edges_connected: 5,
    edges_cut: 0,
    refs: 7,
    commits_at_tip: 2,
    commits_at_bottom: 1,
    commits_in_workspace: 4,
    commits_integrated: 1,
    commits_not_in_remote: 4,
    layout_refs: Some(
        7,
    ),
    hard_limit_hit: false,
    entrypoint: Some(
        Sha1(e255adcd9be0ffabbed19f4ef85c338e54a34376),
    ),
}
"#]]
    );
    Ok(())
}

#[test]
fn target_with_remote_on_stack_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-ahead-and-on-stack-tip")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* dd0cca8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* e255adc (main, A) A
* fafd9d0 (origin/main) init

"#]]
    );
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·dd0cca8 (⌂|🏘)
*  ·e255adc (⌂|🏘) ►A, ►main <> origin/main
*  🏁·fafd9d0 (⌂|🏘|✓) ►origin/main
layout:
  materialized parents: dd0cca8: e255adc
  empty chain anchors: e255adc^
"#]]
    );

    // The main branch is not present, as it's the target.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:A on fafd9d0 {1}
    └── 📙:A
        └── ·e255adc (🏘️) ►main

"#]]
    );

    // But mention it if it's in the workspace. It should retain order.
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["main"]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:A on fafd9d0 {1}
    ├── 📙:A
    └── 📙:main <> origin/main⇡1
        └── ·e255adc (🏘️)

"#]]
    );

    // But mention it if it's in the workspace. It should retain order - inverting the order is fine.
    add_stack_with_segments(&mut meta, 1, "main", StackState::InWorkspace, &["A"]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:main <> origin/main on fafd9d0 {1}
    ├── 📙:main <> origin/main
    └── 📙:A
        └── ·e255adc (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn disambiguate_by_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/disambiguate-by-remote")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e30f90c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 2173153 (origin/ambiguous-C, origin/C, ambiguous-C, C) C
| * ac24e74 (origin/B) remote-of-B
|/  
* 312f819 (ambiguous-B, B) B
* e255adc (origin/A, ambiguous-A, A) A
* fafd9d0 (origin/main, main) init

"#]]
    );

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
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·e30f90c (⌂|🏘)
*  ·2173153 (⌂|🏘) ►C, ►ambiguous-C, ►origin/C, ►origin/ambiguous-C <> origin/C, origin/ambiguous-C
│ *  🟣ac24e74 ►origin/B
├─╯
*  ·312f819 (⌂|🏘) ►B, ►ambiguous-B <> origin/B
*  ·e255adc (⌂|🏘) ►A, ►ambiguous-A, ►origin/A <> origin/A
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: e30f90c: 2173153
"#]]
    );

    assert!(
        {
            let cg = ws.commit_graph();
            cg.commit_ids().all(|id| !cg.has_cut_parents(id))
        },
        "a fully realized graph"
    );
    // An anonymous segment to start with is alright, and can always happen for other situations as well.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:anon: on fafd9d0
    ├── :anon:
    │   └── ·2173153 (🏘️) ►C, ►ambiguous-C
    ├── :B <> origin/B⇣1
    │   ├── 🟣ac24e74
    │   └── ❄️312f819 (🏘️) ►ambiguous-B
    └── :A <> origin/A
        └── ❄️e255adc (🏘️) ►ambiguous-A

"#]]
    );

    // If 'C' is in the workspace, it's naturally disambiguated.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·e30f90c (⌂|🏘)
*  ·2173153 (⌂|🏘) ►C, ►ambiguous-C, ►origin/C, ►origin/ambiguous-C <> origin/C, origin/ambiguous-C
│ *  🟣ac24e74 ►origin/B
├─╯
*  ·312f819 (⌂|🏘) ►B, ►ambiguous-B <> origin/B
*  ·e255adc (⌂|🏘) ►A, ►ambiguous-A, ►origin/A <> origin/A
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: e30f90c: 2173153
  empty chain anchors: 2173153^
"#]]
    );
    // And because `C` is in the workspace data, its data is denoted.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:C <> origin/C on fafd9d0 {0}
    ├── 📙:C <> origin/C
    │   └── ❄️2173153 (🏘️) ►ambiguous-C
    ├── :B <> origin/B⇣1
    │   ├── 🟣ac24e74
    │   └── ❄️312f819 (🏘️) ►ambiguous-B
    └── :A <> origin/A
        └── ❄️e255adc (🏘️) ►ambiguous-A

"#]]
    );
    Ok(())
}

#[test]
fn integrated_tips_stop_early_if_remote_is_not_configured() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-segments-one-integrated-without-remote")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    // We can abort early if there is only integrated commits left, but also if there is *no remote setup*.
    // We also abort integrated named segments early, unless these are named as being part of the
    // workspace - here `A` is cut off.
    // Without remote, the traversal can't setup `main` as target for the workspace entrypoint to find.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let cg = ws.commit_graph();
    assert!(cg.commit_ids().all(|id| !cg.has_cut_parents(id)));
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
*  ·79bbb29 (⌂|🏘) ►A
*  ·fc98174 (⌂|🏘)
*  ·a381df5 (⌂|🏘)
*  ·777b552 (⌂|🏘)
*    ·ce4a760 (⌂|🏘)
├─╮
│ *  ·fea59b5 (⌂|🏘) ►A-feat
│ *  ·4deea74 (⌂|🏘)
├─╯
*  ·01d0e1e (⌂|🏘)
*  ·4b3e5a8 (⌂|🏘) ►main
*  ·34d0715 (⌂|🏘)
*  🏁·eb5f731 (⌂|🏘)
layout:
  materialized parents: 4077353: 6b1a13b
"#]]
    );
    // It's true that `A` is fully integrated so it isn't displayed. so from a workspace-perspective
    // it's the right answer.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:B
    ├── :B
    │   ├── ·6b1a13b (🏘️)
    │   └── ·03ad472 (🏘️)
    ├── :A
    │   ├── ·79bbb29 (🏘️)
    │   ├── ·fc98174 (🏘️)
    │   ├── ·a381df5 (🏘️)
    │   ├── ·777b552 (🏘️)
    │   ├── ·ce4a760 (🏘️)
    │   └── ·01d0e1e (🏘️)
    └── :main
        ├── ·4b3e5a8 (🏘️)
        ├── ·34d0715 (🏘️)
        └── ·eb5f731 (🏘️)

"#]]
    );

    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["A"]);
    // ~~Now that `A` is part of the workspace, it's not cut off anymore.~~
    // This special handling was removed for now, relying on limits and extensions.
    // And since it's integrated, traversal is stopped without convergence.
    // We see more though as we add workspace segments immediately.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
│ *  🟣d0df794 (✓) ►origin/main
│ *  🟣09c6e08 (✓)
│ *  🟣7b9f260 (✓)
╭─┤
│ *  🟣4b3e5a8 (✓) ►main <> origin/main
│ *  🟣34d0715 (✓)
│ *  🏁🟣eb5f731 (✓)
*  ·79bbb29 (⌂|🏘|✓) ►A
*  ·fc98174 (⌂|🏘|✓)
*  ·a381df5 (⌂|🏘|✓)
*  ·777b552 (⌂|🏘|✓)
*  ✂·ce4a760 (⌂|🏘|✓)
layout:
  materialized parents: 4077353: 6b1a13b
  empty chain anchors: 6b1a13b^
"#]]
    );
    // `A` is integrated, hence it's not shown.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
└── ≡📙:B on 79bbb29 {0}
    └── 📙:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    // The limit is effective for integrated workspaces branches, and it doesn't unnecessarily
    // prolong the traversal once the all tips are known to be integrated.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
│ *  🟣d0df794 (✓) ►origin/main
│ *  🟣09c6e08 (✓)
│ *  🟣7b9f260 (✓)
╭─┤
│ *  🟣4b3e5a8 (✓) ►main <> origin/main
│ *  🟣34d0715 (✓)
│ *  🏁🟣eb5f731 (✓)
*  ·79bbb29 (⌂|🏘|✓) ►A
*  ·fc98174 (⌂|🏘|✓)
*  ·a381df5 (⌂|🏘|✓)
*  ✂·777b552 (⌂|🏘|✓)
layout:
  materialized parents: 4077353: 6b1a13b
  empty chain anchors: 6b1a13b^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
└── ≡📙:B on 79bbb29 {0}
    └── 📙:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    // When looking from an integrated branch within the workspace, but without limit,
    // the (lack of) limit is respected.
    // When the entrypoint starts on an integrated commit, the 'all-tips-are-integrated' condition doesn't
    // kick in anymore.
    let (id, ref_name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
│ *  🟣d0df794 (✓) ►origin/main
│ *  🟣09c6e08 (✓)
│ *  🟣7b9f260 (✓)
╭─┤
* │  👉·79bbb29 (⌂|🏘|✓) ►A
* │  ·fc98174 (⌂|🏘|✓)
* │  ·a381df5 (⌂|🏘|✓)
* │  ·777b552 (⌂|🏘|✓)
* │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ *  ·fea59b5 (⌂|🏘|✓) ►A-feat
│ │ *  ·4deea74 (⌂|🏘|✓)
├───╯
* │  ·01d0e1e (⌂|🏘|✓)
├─╯
*  ·4b3e5a8 (⌂|🏘|✓) ►main <> origin/main
*  ·34d0715 (⌂|🏘|✓)
*  🏁·eb5f731 (⌂|🏘|✓)
"#]]
    );
    // The entrypoint branch is downgraded to a single-branch view with target context
    // preserved. All commits on this branch are integrated, so the branch container remains
    // but its commit list is pruned.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A <> ✓refs/remotes/origin/main⇣3
└── ≡:A on 4b3e5a8 {1}
    └── :A

"#]]
    );

    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    // It's still getting quite far despite the limit due to other heads searching for their goals,
    // but also ends traversal early.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
│ *  🟣d0df794 (✓) ►origin/main
│ *  🟣09c6e08 (✓)
│ *  🟣7b9f260 (✓)
╭─┤
│ *  🟣4b3e5a8 (✓) ►main <> origin/main
│ *  🟣34d0715 (✓)
│ *  🏁🟣eb5f731 (✓)
*  👉·79bbb29 (⌂|🏘|✓) ►A
*  ·fc98174 (⌂|🏘|✓)
*  ·a381df5 (⌂|🏘|✓)
*  ✂·777b552 (⌂|🏘|✓)
"#]]
    );
    // Because the branch is integrated, the surrounding workspace isn't shown. The downgraded
    // branch view keeps target context and prunes the integrated commits.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A <> ✓refs/remotes/origin/main⇣6
└── ≡:A {1}
    └── :A

"#]]
    );

    // See what happens with an out-of-workspace HEAD and an arbitrary extra target.
    let (id, _ref_name) = id_at(&repo, "origin/main");
    let ws = Workspace::from_tip(
        id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "gitbutler/workspace"),
    )?
    .validated()?;
    // It keeps the tip-settings of the workspace it setup by itself, and doesn't override this
    // with the extra-target settings.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·d0df794 (⌂|✓) ►origin/main
*  ·09c6e08 (⌂|✓)
*    ·7b9f260 (⌂|✓)
├─╮
│ │ *  ·4077353 (⌂|🏘|✓)
│ │ *  ·6b1a13b (⌂|🏘|✓) ►B
│ │ *  ·03ad472 (⌂|🏘|✓)
│ ├─╯
│ *  ·79bbb29 (⌂|🏘|✓) ►A
│ *  ·fc98174 (⌂|🏘|✓)
│ *  ·a381df5 (⌂|🏘|✓)
│ *  ·777b552 (⌂|🏘|✓)
│ *    ·ce4a760 (⌂|🏘|✓)
│ ├─╮
│ │ *  ·fea59b5 (⌂|🏘|✓) ►A-feat
│ │ *  ·4deea74 (⌂|🏘|✓)
│ ├─╯
│ *  ·01d0e1e (⌂|🏘|✓)
├─╯
*  ·4b3e5a8 (⌂|🏘|✓) ►main <> origin/main
*  ·34d0715 (⌂|🏘|✓)
*  🏁·eb5f731 (⌂|🏘|✓)
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:DETACHED <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:anon: on 4b3e5a8 {1}
    └── :anon:
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── ·7b9f260 (✓)

"#]]
    );

    // However, when choosing an initially unknown branch, it will get the extra target tip settings.
    let ws = Workspace::from_tip(
        id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "B"),
    )?
    .validated()?;
    // For now we don't do anything to limit the each in single-branch mode using extra-targets.
    // Thanks to the limit-transplant we get to discover more of the workspace.
    // TODO(extra-target): make it work so they limit single branches even, but it's a special case
    //                     as we can't have remotes here.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·d0df794 (⌂|✓) ►origin/main
*  ·09c6e08 (⌂|✓)
*    ·7b9f260 (⌂|✓)
├─╮
│ │ *  ·4077353 (⌂|🏘)
│ │ *  ·6b1a13b (⌂|🏘|✓) ►B
│ │ *  ·03ad472 (⌂|🏘|✓)
│ ├─╯
│ *  ·79bbb29 (⌂|🏘|✓) ►A
│ *  ·fc98174 (⌂|🏘|✓)
│ *  ·a381df5 (⌂|🏘|✓)
│ *  ·777b552 (⌂|🏘|✓)
│ *    ·ce4a760 (⌂|🏘|✓)
│ ├─╮
│ │ *  ·fea59b5 (⌂|🏘|✓) ►A-feat
│ │ *  ·4deea74 (⌂|🏘|✓)
│ ├─╯
│ *  ·01d0e1e (⌂|🏘|✓)
├─╯
*  ·4b3e5a8 (⌂|🏘|✓) ►main <> origin/main
*  ·34d0715 (⌂|🏘|✓)
*  🏁·eb5f731 (⌂|🏘|✓)
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:DETACHED <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:anon: on 4b3e5a8 {1}
    └── :anon:
        ├── ·d0df794 (✓)
        ├── ·09c6e08 (✓)
        └── ·7b9f260 (✓)

"#]]
    );

    Ok(())
}

#[test]
fn integrated_tips_do_not_stop_early() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-segments-one-integrated")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    // Thanks to the remote `main` is searched for by the entrypoint.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
│ *  🟣d0df794 (✓) ►origin/main
│ *  🟣09c6e08 (✓)
│ *  🟣7b9f260 (✓)
╭─┤
* │  ·79bbb29 (⌂|🏘|✓) ►A
* │  ·fc98174 (⌂|🏘|✓)
* │  ·a381df5 (⌂|🏘|✓)
* │  ·777b552 (⌂|🏘|✓)
* │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ *  ·fea59b5 (⌂|🏘|✓) ►A-feat
│ │ *  ·4deea74 (⌂|🏘|✓)
├───╯
* │  ·01d0e1e (⌂|🏘|✓)
├─╯
*  ·4b3e5a8 (⌂|🏘|✓) ►main <> origin/main
*  ·34d0715 (⌂|🏘|✓)
*  🏁·eb5f731 (⌂|🏘|✓)
layout:
  materialized parents: 4077353: 6b1a13b
"#]]
    );

    // This search discovers the whole workspace, without the integrated one.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:B on 79bbb29
    └── :B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    // However, we can specify an additional/old target segment to show integrated portions as well.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 4b3e5a8
└── ≡:B on 4b3e5a8
    ├── :B
    │   ├── ·6b1a13b (🏘️)
    │   └── ·03ad472 (🏘️)
    └── :A
        ├── ·79bbb29 (🏘️|✓)
        ├── ·fc98174 (🏘️|✓)
        ├── ·a381df5 (🏘️|✓)
        ├── ·777b552 (🏘️|✓)
        ├── ·ce4a760 (🏘️|✓)
        └── ·01d0e1e (🏘️|✓)

"#]]
    );

    // When looking from an integrated branch within the workspace, and without limit
    // the limit isn't respected, and we still know the whole workspace.
    let (id, ref_name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
│ *  🟣d0df794 (✓) ►origin/main
│ *  🟣09c6e08 (✓)
│ *  🟣7b9f260 (✓)
╭─┤
* │  👉·79bbb29 (⌂|🏘|✓) ►A
* │  ·fc98174 (⌂|🏘|✓)
* │  ·a381df5 (⌂|🏘|✓)
* │  ·777b552 (⌂|🏘|✓)
* │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ *  ·fea59b5 (⌂|🏘|✓) ►A-feat
│ │ *  ·4deea74 (⌂|🏘|✓)
├───╯
* │  ·01d0e1e (⌂|🏘|✓)
├─╯
*  ·4b3e5a8 (⌂|🏘|✓) ►main <> origin/main
*  ·34d0715 (⌂|🏘|✓)
*  🏁·eb5f731 (⌂|🏘|✓)
"#]]
    );

    // The entrypoint isn't contained in the managed workspace anymore, so it's a standalone
    // single-branch view. Target context is preserved, so integrated commits are pruned while
    // the branch container remains visible.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A <> ✓refs/remotes/origin/main⇣3
└── ≡:A on 4b3e5a8 {1}
    └── :A

"#]]
    );

    // When converting to a workspace, we are still aware of the workspace membership as long as
    // the lower bound of the workspace includes it.
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 4b3e5a8
└── ≡:B on 4b3e5a8
    ├── :B
    │   ├── ·6b1a13b (🏘️)
    │   └── ·03ad472 (🏘️)
    └── 👉:A
        ├── ·79bbb29 (🏘️|✓)
        ├── ·fc98174 (🏘️|✓)
        ├── ·a381df5 (🏘️|✓)
        ├── ·777b552 (🏘️|✓)
        ├── ·ce4a760 (🏘️|✓)
        └── ·01d0e1e (🏘️|✓)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "main");
    let ws = Workspace::from_tip(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // When the branch is below the forkpoint, the workspace also isn't shown anymore.
    // The downgraded branch view keeps target context and prunes integrated base commits.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main <> ✓refs/remotes/origin/main⇣3
└── ≡:main <> origin/main⇣3 {1}
    └── :main <> origin/main⇣3
        ├── 🟣d0df794 (✓)
        ├── 🟣09c6e08 (✓)
        └── 🟣7b9f260 (✓)

"#]]
    );

    let id = id_by_rev(&repo, "main~1");
    let ws = Workspace::from_tip(id, None, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // Detached states are also possible. They keep the anonymous container while
    // preserving target context and pruning integrated commits.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:DETACHED <> ✓refs/remotes/origin/main⇣3
└── ≡:anon: {1}
    └── :anon:

"#]]
    );
    Ok(())
}

#[test]
fn workspace_without_target_can_see_remote() -> anyhow::Result<()> {
    let (mut repo, _) = read_only_in_memory_scenario("ws/main-with-remote-and-workspace-ref")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 956a3de (origin/main) on-remote-only
* 3183e43 (HEAD -> main, gitbutler/workspace) M1

"#]]
    );

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

    let ws =
        Workspace::from_head(&repo, &meta, project_meta(&meta), standard_options())?.validated()?;
    // Main is a normal branch, and its remote is known.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·956a3de (⌂) ►origin/main
*  👉🏁·3183e43 (⌂|🏘) ►main[🌳] <> origin/main
layout:
  empty chain anchors: 3183e43^
"#]]
    );

    // The workspace shows the remote commit, there is nothing special about the target.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓!
└── ≡👉📙:main[🌳] <> origin/main⇡1 {0}
    └── 👉📙:main[🌳] <> origin/main⇡1
        └── ·3183e43 (🏘️)

"#]]
    );

    // If the remote isn't setup officially, deduction still works as we find
    // symbolic remote names for deduction in workspace ref names as well.
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));
    let ws = ws.redo(&repo, &meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·956a3de (⌂) ►origin/main
*  👉🏁·3183e43 (⌂|🏘) ►main[🌳]
layout:
  empty chain anchors: 3183e43^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓!
└── ≡👉📙:main[🌳] {0}
    └── 👉📙:main[🌳]
        └── ·3183e43 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn workspace_obeys_limit_when_target_branch_is_missing() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-segments-one-integrated-without-remote")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );
    add_workspace_without_target(&mut meta);
    assert!(
        meta.data_mut().default_target.is_none(),
        "without target, limits affect workspaces too"
    );
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(0),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉✂·4077353 (⌂|🏘)
layout:
  materialized parents: 4077353: 
"#]]
    );
    // The commit in the workspace branch is always ignored and is expected to be the workspace merge commit.
    // So nothing to show here.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!

"#]]
    );

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    assert!(
        meta.data_mut().default_target.is_some(),
        "But with workspace and target, we see everything"
    );
    // It's notable that there is no way to bypass the early abort when everything is integrated.
    // and there is no deductible remote relationship between origin/main and main (no remote not configured).
    // Then the traversal ends on integrated branches as `main` isn't a target.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(0),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·4077353 (⌂|🏘)
*  ·6b1a13b (⌂|🏘) ►B
*  ·03ad472 (⌂|🏘)
│ *  🟣d0df794 (✓) ►origin/main
│ *  🟣09c6e08 (✓)
│ *  🟣7b9f260 (✓)
╭─┤
│ *  🟣4b3e5a8 (✓) ►main <> origin/main
│ *  🟣34d0715 (✓)
│ *  🏁🟣eb5f731 (✓)
*  ·79bbb29 (⌂|🏘|✓) ►A
*  ·fc98174 (⌂|🏘|✓)
*  ✂·a381df5 (⌂|🏘|✓)
layout:
  materialized parents: 4077353: 6b1a13b
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
└── ≡:B on 79bbb29
    └── :B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn three_branches_one_advanced_ws_commit_advanced_fully_pushed_empty_dependent()
-> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependent",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* cbc6713 (origin/advanced-lane, on-top-of-dependent, dependent, advanced-lane) change
* fafd9d0 (origin/main, main, lane) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·f8f33a7 (⌂|🏘)
*  ·cbc6713 (⌂|🏘) ►advanced-lane, ►dependent, ►on-top-of-dependent, ►origin/advanced-lane <> origin/advanced-lane
*  🏁·fafd9d0 (⌂|🏘|✓) ►lane, ►main, ►origin/main <> origin/main
layout:
  materialized parents: f8f33a7: cbc6713
"#]]
    );

    // By default, the advanced lane is simply frozen as its remote contains the commit.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:advanced-lane <> origin/advanced-lane on fafd9d0
    └── :advanced-lane <> origin/advanced-lane
        └── ❄️cbc6713 (🏘️) ►dependent, ►on-top-of-dependent

"#]]
    );

    add_stack_with_segments(
        &mut meta,
        1,
        "dependent",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

    // Lanes are properly ordered
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·f8f33a7 (⌂|🏘)
*  ·cbc6713 (⌂|🏘) ►advanced-lane, ►dependent, ►on-top-of-dependent, ►origin/advanced-lane <> origin/advanced-lane
*  🏁·fafd9d0 (⌂|🏘|✓) ►lane, ►main, ►origin/main <> origin/main
layout:
  materialized parents: f8f33a7: cbc6713
  empty chain anchors: cbc6713^
"#]]
    );

    // When putting the dependent branch on top as empty segment, the frozen state is retained.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:dependent on fafd9d0 {1}
    ├── 📙:dependent
    └── 📙:advanced-lane <> origin/advanced-lane
        └── ❄️cbc6713 (🏘️) ►on-top-of-dependent

"#]]
    );
    Ok(())
}

#[test]
fn on_top_of_target_with_history() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/on-top-of-target-with-history")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2cde30a (HEAD -> gitbutler/workspace, origin/main, F, E, D, C, B, A) 5
* 1c938f4 4
* b82769f 3
* 988032f 2
* cd5b655 1
* 2be54cd (main) outdated-main

"#]]
    );

    add_workspace(&mut meta);
    // It sees the entire history as it had to find `main`.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·2cde30a (⌂|🏘|✓) ►A, ►B, ►C, ►D, ►E, ►F, ►origin/main
*  ·1c938f4 (⌂|🏘|✓)
*  ·b82769f (⌂|🏘|✓)
*  ·988032f (⌂|🏘|✓)
*  ·cd5b655 (⌂|🏘|✓)
*  🏁·2be54cd (⌂|🏘|✓) ►main <> origin/main
"#]]
    );
    // Workspace is empty as everything is integrated.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a

"#]]
    );

    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·2cde30a (⌂|🏘|✓) ►A, ►B, ►C, ►D, ►E, ►F, ►origin/main
*  ·1c938f4 (⌂|🏘|✓)
*  ·b82769f (⌂|🏘|✓)
*  ·988032f (⌂|🏘|✓)
*  ·cd5b655 (⌂|🏘|✓)
*  🏁·2be54cd (⌂|🏘|✓) ►main <> origin/main
layout:
  empty chain anchors: 2cde30a 2cde30a
"#]]
    );

    // Empty stack segments on top of integrated portions will show, and nothing integrated shows.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a
├── ≡📙:C on 2cde30a {0}
│   ├── 📙:C
│   ├── 📙:B
│   └── 📙:A
└── ≡📙:D on 2cde30a {1}
    ├── 📙:D
    ├── 📙:E
    └── 📙:F

"#]]
    );

    // However, when passing an additional old position of the target, we can show the now-integrated parts.
    // The stacks will always be created on top of the integrated segments as that's where their references are
    // (these segments are never conjured up out of thin air).
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2be54cd
├── ≡📙:C on 2be54cd {0}
│   ├── 📙:C
│   ├── 📙:B
│   └── 📙:A
│       ├── ·2cde30a (🏘️|✓)
│       ├── ·1c938f4 (🏘️|✓)
│       ├── ·b82769f (🏘️|✓)
│       ├── ·988032f (🏘️|✓)
│       └── ·cd5b655 (🏘️|✓)
└── ≡📙:D on 2be54cd {1}
    ├── 📙:D
    ├── 📙:E
    └── 📙:F
        ├── ·2cde30a (🏘️|✓)
        ├── ·1c938f4 (🏘️|✓)
        ├── ·b82769f (🏘️|✓)
        ├── ·988032f (🏘️|✓)
        └── ·cd5b655 (🏘️|✓)

"#]]
    );
    Ok(())
}

#[test]
fn partitions_with_long_and_short_connections_to_each_other() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/gitlab-case")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    let (main_id, main_ref_name) = id_at(&repo, "main");
    // Validate that we will perform long searches to connect connectable segments, without interfering
    // with other searches that may take even longer.
    // Also, without limit, we should be able to see all of 'main' without cut-off
    let ws = Workspace::from_tip(
        main_id,
        main_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·41ed0e4 (⌂|🏘)
│ *    🟣232ed06 (✓) ►origin/main
│ ├─╮
│ * │  🟣abcfd9a (✓) ►workspace-to-target
│ * │  🟣bc86eba (✓)
│ * │  🟣c7ae303 (✓)
├─╯ │
│   *  🟣9e2a79e (✓) ►long-workspace-to-target
│   *  🟣fdeaa43 (✓)
│   *  🟣30565ee (✓)
│   *  🟣0c1c23a (✓)
│   *  🟣56d152c (✓)
│   *  🟣e6e1360 (✓)
│   *  🟣1a22a39 (✓)
├───╯
*    ·9730cbf (⌂|🏘|✓) ►workspace
├─╮
* │  ·dc7ab57 (⌂|🏘|✓) ►main-to-workspace
│ *  ·77f31a0 (⌂|🏘|✓) ►long-main-to-workspace
│ *  ·eb17e31 (⌂|🏘|✓)
│ *  ·fe2046b (⌂|🏘|✓)
│ *  ·5532ef5 (⌂|🏘|✓)
│ *  👉·2438292 (⌂|🏘|✓) ►main <> origin/main
├─╯
*  ·c056b75 (⌂|🏘|✓)
*  ·f49c977 (⌂|🏘|✓)
*  ·7b7ebb2 (⌂|🏘|✓)
*  ·dca4960 (⌂|🏘|✓)
*  ·11c29b8 (⌂|🏘|✓)
*  ·c32dd03 (⌂|🏘|✓)
*  ·b625665 (⌂|🏘|✓)
*  ·a821094 (⌂|🏘|✓)
*  ·bce0c5e (⌂|🏘|✓)
*  🏁·3183e43 (⌂|🏘|✓)
"#]]
    );
    // Entrypoint is outside of the managed workspace, so it is projected as a
    // single-branch view. Target context is preserved and integrated commits below
    // the target trunk are pruned.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main <> ✓refs/remotes/origin/main⇣11
└── ≡:main <> origin/main⇣11 {1}
    └── :main <> origin/main⇣11
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

"#]]
    );

    // When setting a limit when traversing 'main', it is respected.
    // We still want it to be found and connected though, and it's notable that the limit kicks in
    // once everything reconciled.
    let ws = Workspace::from_tip(
        main_id,
        main_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·41ed0e4 (⌂|🏘)
│ *    🟣232ed06 (✓) ►origin/main
│ ├─╮
│ * │  🟣abcfd9a (✓) ►workspace-to-target
│ * │  🟣bc86eba (✓)
│ * │  🟣c7ae303 (✓)
├─╯ │
│   *  🟣9e2a79e (✓) ►long-workspace-to-target
│   *  🟣fdeaa43 (✓)
│   *  🟣30565ee (✓)
│   *  🟣0c1c23a (✓)
│   *  🟣56d152c (✓)
│   *  🟣e6e1360 (✓)
│   *  🟣1a22a39 (✓)
├───╯
*    ·9730cbf (⌂|🏘|✓) ►workspace
├─╮
* │  ·dc7ab57 (⌂|🏘|✓) ►main-to-workspace
│ *  ·77f31a0 (⌂|🏘|✓) ►long-main-to-workspace
│ *  ·eb17e31 (⌂|🏘|✓)
│ *  ·fe2046b (⌂|🏘|✓)
│ *  ·5532ef5 (⌂|🏘|✓)
│ *  👉·2438292 (⌂|🏘|✓) ►main <> origin/main
├─╯
*  ·c056b75 (⌂|🏘|✓)
*  ·f49c977 (⌂|🏘|✓)
*  ·7b7ebb2 (⌂|🏘|✓)
*  ·dca4960 (⌂|🏘|✓)
*  ✂·11c29b8 (⌂|🏘|✓)
"#]]
    );
    // The limit is visible as well. Target context is preserved in the downgraded
    // branch view, so integrated local/base commits are pruned.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main <> ✓refs/remotes/origin/main⇣11
└── ≡:main <> origin/main⇣11 {1}
    └── :main <> origin/main⇣11
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

"#]]
    );

    // From the workspace, even without limit, we don't traverse all of 'main' as it's uninteresting.
    // However, we wait for the target to be fully reconciled to get the proper workspace configuration.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·41ed0e4 (⌂|🏘)
│ *    🟣232ed06 (✓) ►origin/main
│ ├─╮
│ * │  🟣abcfd9a (✓) ►workspace-to-target
│ * │  🟣bc86eba (✓)
│ * │  🟣c7ae303 (✓)
├─╯ │
│   *  🟣9e2a79e (✓) ►long-workspace-to-target
│   *  🟣fdeaa43 (✓)
│   *  🟣30565ee (✓)
│   *  🟣0c1c23a (✓)
│   *  🟣56d152c (✓)
│   *  🟣e6e1360 (✓)
│   *  🟣1a22a39 (✓)
├───╯
*    ·9730cbf (⌂|🏘|✓) ►workspace
├─╮
* │  ·dc7ab57 (⌂|🏘|✓) ►main-to-workspace
│ *  ·77f31a0 (⌂|🏘|✓) ►long-main-to-workspace
│ *  ·eb17e31 (⌂|🏘|✓)
│ *  ·fe2046b (⌂|🏘|✓)
│ *  ·5532ef5 (⌂|🏘|✓)
│ *  ·2438292 (⌂|🏘|✓) ►main <> origin/main
├─╯
*  ·c056b75 (⌂|🏘|✓)
*  ·f49c977 (⌂|🏘|✓)
*  ·7b7ebb2 (⌂|🏘|✓)
*  ·dca4960 (⌂|🏘|✓)
*  ·11c29b8 (⌂|🏘|✓)
*  ·c32dd03 (⌂|🏘|✓)
*  ·b625665 (⌂|🏘|✓)
*  ✂·a821094 (⌂|🏘|✓)
layout:
  materialized parents: 41ed0e4: 9730cbf
"#]]
    );

    // Everything is integrated, nothing to see here.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣11 on 9730cbf

"#]]
    );
    Ok(())
}

#[test]
fn remote_far_in_ancestry() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-far-in-ancestry")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    // It's critical that the main branch isn't cut off and the local and remote part find each other,
    // or else the remote part will go on forever create a lot of issues for those who want to display
    // all these incorrectly labeled commits.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·9412ebd (⌂|🏘)
*  ·8407093 (⌂|🏘) ►A <> origin/A
*  ·7dfaa0c (⌂|🏘)
*  ·544e458 (⌂|🏘)
*  ·685d644 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
*  ·cafdb27 (⌂|🏘|✓)
*  ·c056b75 (⌂|🏘|✓)
*  ·f49c977 (⌂|🏘|✓)
*  ·7b7ebb2 (⌂|🏘|✓)
*  ·dca4960 (⌂|🏘|✓)
*  ·11c29b8 (⌂|🏘|✓)
*  ·c32dd03 (⌂|🏘|✓)
*  ·b625665 (⌂|🏘|✓)
*  ·a821094 (⌂|🏘|✓)
*  ·bce0c5e (⌂|🏘|✓)
│ *  🟣975754f ►origin/A
│ *  🟣f48ff69
├─╯
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  materialized parents: 9412ebd: 8407093
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 685d644
└── ≡:A <> origin/A⇡3⇣2 on 685d644
    └── :A <> origin/A⇡3⇣2
        ├── 🟣975754f
        ├── 🟣f48ff69
        ├── ·8407093 (🏘️)
        ├── ·7dfaa0c (🏘️)
        └── ·544e458 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn partitions_with_long_and_short_connections_to_each_other_part_2() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/gitlab-case2")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    let (id, ref_name) = id_at(&repo, "main");
    // Here the target shouldn't be cut off from finding its workspace
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·f514495 (⌂|🏘)
│ *  🟣024f837 (✓) ►long-workspace-to-target, ►origin/main
│ *  🟣64a8284 (✓)
│ *  🟣b72938c (✓)
│ *  🟣9ccbf6f (✓)
│ *  🟣5fa4905 (✓)
│ *  🟣43074d3 (✓)
│ *  🟣800d4a9 (✓)
│ *  🟣742c068 (✓)
│ *  🟣fe06afd (✓)
│ *    🟣3027746 (✓)
│ ├─╮
│ * │  🟣f0d2a35 (✓)
├─╯ │
*   │  ·c9120f1 (⌂|🏘|✓) ►workspace
├─╮ │
│ * │  ·b39c7ec (⌂|🏘|✓) ►long-main-to-workspace
│ * │  ·2983a97 (⌂|🏘|✓)
│ * │  ·144ea85 (⌂|🏘|✓)
│ * │  ·5aecfd2 (⌂|🏘|✓)
│ * │  👉·bce0c5e (⌂|🏘|✓) ►main <> origin/main
│ │ *  🟣edf041f (✓) ►longer-workspace-to-target
│ │ *  🟣d9f03f6 (✓)
│ │ *  🟣8d1d264 (✓)
│ │ *  🟣fa7ceae (✓)
│ │ *  🟣95bdbf1 (✓)
│ │ *  🟣5bac978 (✓)
├───╯
* │  ·1126587 (⌂|🏘|✓) ►main-to-workspace
├─╯
*  🏁·3183e43 (⌂|🏘|✓) ►A, ►B
"#]]
    );
    // `main` is integrated, but it is the entrypoint, so the branch container is shown.
    // With preserved target context, integrated commits below the target trunk are pruned.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main <> ✓refs/remotes/origin/main⇣17
└── ≡:main <> origin/main⇣17 {1}
    └── :main <> origin/main⇣17
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

"#]]
    );

    // Now the target looks for the entrypoint, which is the workspace, something it can do more easily.
    // We wait for targets to fully reconcile as well.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·f514495 (⌂|🏘)
│ *  🟣024f837 (✓) ►long-workspace-to-target, ►origin/main
│ *  🟣64a8284 (✓)
│ *  🟣b72938c (✓)
│ *  🟣9ccbf6f (✓)
│ *  🟣5fa4905 (✓)
│ *  🟣43074d3 (✓)
│ *  🟣800d4a9 (✓)
│ *  🟣742c068 (✓)
│ *  🟣fe06afd (✓)
│ *    🟣3027746 (✓)
│ ├─╮
│ * │  🟣f0d2a35 (✓)
├─╯ │
*   │  ·c9120f1 (⌂|🏘|✓) ►workspace
├─╮ │
│ * │  ·b39c7ec (⌂|🏘|✓) ►long-main-to-workspace
│ * │  ·2983a97 (⌂|🏘|✓)
│ * │  ·144ea85 (⌂|🏘|✓)
│ * │  ·5aecfd2 (⌂|🏘|✓)
│ * │  ·bce0c5e (⌂|🏘|✓) ►main <> origin/main
│ │ *  🟣edf041f (✓) ►longer-workspace-to-target
│ │ *  🟣d9f03f6 (✓)
│ │ *  🟣8d1d264 (✓)
│ │ *  🟣fa7ceae (✓)
│ │ *  🟣95bdbf1 (✓)
│ │ *  🟣5bac978 (✓)
├───╯
* │  ·1126587 (⌂|🏘|✓) ►main-to-workspace
├─╯
*  🏁·3183e43 (⌂|🏘|✓) ►A, ►B
layout:
  materialized parents: f514495: c9120f1
"#]]
    );

    // Everything is integrated.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1

"#]]
    );

    // With a lower base for the target, we see more.
    let target_commit_id = repo.rev_parse_single("3183e43")?.detach();
    add_workspace_with_target(&mut meta, target_commit_id);

    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1

"#]]
    );

    // We can also add independent virtual branches to that new base.
    add_stack(&mut meta, 3, "A", StackState::InWorkspace);
    add_stack(&mut meta, 4, "B", StackState::InWorkspace);
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on 3183e43
├── ≡📙:A on 3183e43 {3}
│   └── 📙:A
└── ≡📙:B on 3183e43 {4}
    └── 📙:B

"#]]
    );

    // We can also add stacked virtual branches to that new base.
    meta.data_mut().branches.clear();
    add_workspace_with_target(&mut meta, target_commit_id);
    add_stack_with_segments(&mut meta, 3, "A", StackState::InWorkspace, &["B"]);
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on 3183e43
└── ≡📙:A on 3183e43 {3}
    └── 📙:A

"#]]
    );
    Ok(())
}

#[test]
fn multi_lane_with_shared_segment_one_integrated() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multi-lane-with-shared-segment-one-integrated")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   1cf594d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 9895054 (D) D1
| | * de625cc (C) C3
| | * 23419f8 C2
| | * 5dc4389 C1
| * | acdc49a (B) B2
| * | f0117e0 B1
| |/  
| | *   c08dc6b (origin/main) Merge branch 'A' into soon-remote-main
| | |\  
| |_|/  
|/| |   
* | | 0bad3af (A) A1
|/ /  
* | d4f537e (shared) S3
* | b448757 S2
* | e9a378d S1
|/  
* 3183e43 (main) M1

"#]]
        .raw()
    );

    add_workspace(&mut meta);

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·1cf594d (⌂|🏘)
├─┬─╮
│ * │  ·acdc49a (⌂|🏘) ►B
│ * │  ·f0117e0 (⌂|🏘)
│ │ *  ·9895054 (⌂|🏘) ►D
│ │ *  ·de625cc (⌂|🏘) ►C
│ │ *  ·23419f8 (⌂|🏘)
│ │ *  ·5dc4389 (⌂|🏘)
│ ├─╯
│ │ *  🟣c08dc6b (✓) ►origin/main
╭───┤
* │ │  ·0bad3af (⌂|🏘|✓) ►A
├─╯ │
*   │  ·d4f537e (⌂|🏘|✓) ►shared
*   │  ·b448757 (⌂|🏘|✓)
*   │  ·e9a378d (⌂|🏘|✓)
├───╯
*  🏁·3183e43 (⌂|🏘|✓) ►main <> origin/main
layout:
  materialized parents: 1cf594d: 0bad3af acdc49a 9895054
"#]]
    );

    // A is still shown despite it being fully integrated, as it's still enclosed by the
    // workspace tip and the fork-point, at least when we provide the previous known location of the target.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡:A on 3183e43
│   ├── :A
│   │   └── ·0bad3af (🏘️|✓)
│   └── :shared
│       ├── ·d4f537e (🏘️|✓)
│       ├── ·b448757 (🏘️|✓)
│       └── ·e9a378d (🏘️|✓)
├── ≡:B on 3183e43
│   ├── :B
│   │   ├── ·acdc49a (🏘️)
│   │   └── ·f0117e0 (🏘️)
│   └── :shared
│       ├── ·d4f537e (🏘️|✓)
│       ├── ·b448757 (🏘️|✓)
│       └── ·e9a378d (🏘️|✓)
└── ≡:D on 3183e43
    ├── :D
    │   └── ·9895054 (🏘️)
    ├── :C
    │   ├── ·de625cc (🏘️)
    │   ├── ·23419f8 (🏘️)
    │   └── ·5dc4389 (🏘️)
    └── :shared
        ├── ·d4f537e (🏘️|✓)
        ├── ·b448757 (🏘️|✓)
        └── ·e9a378d (🏘️|✓)

"#]]
    );

    // If we do not, integrated portions are removed.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on d4f537e
├── ≡:B on d4f537e
│   └── :B
│       ├── ·acdc49a (🏘️)
│       └── ·f0117e0 (🏘️)
└── ≡:D on d4f537e
    ├── :D
    │   └── ·9895054 (🏘️)
    └── :C
        ├── ·de625cc (🏘️)
        ├── ·23419f8 (🏘️)
        └── ·5dc4389 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn multi_lane_with_shared_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/multi-lane-with-shared-segment")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   1cf594d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 9895054 (D) D1
| | * de625cc (C) C3
| | * 23419f8 C2
| | * 5dc4389 C1
| * | acdc49a (B) B2
| * | f0117e0 B1
| |/  
* / 0bad3af (A) A1
|/  
* d4f537e (shared) S3
* b448757 S2
* e9a378d S1
| * bce0c5e (origin/main) M2
|/  
* 3183e43 (main) M1

"#]]
        .raw()
    );

    add_workspace(&mut meta);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·1cf594d (⌂|🏘)
├─┬─╮
* │ │  ·0bad3af (⌂|🏘) ►A
│ * │  ·acdc49a (⌂|🏘) ►B
│ * │  ·f0117e0 (⌂|🏘)
├─╯ │
│   *  ·9895054 (⌂|🏘) ►D
│   *  ·de625cc (⌂|🏘) ►C
│   *  ·23419f8 (⌂|🏘)
│   *  ·5dc4389 (⌂|🏘)
├───╯
*  ·d4f537e (⌂|🏘) ►shared
*  ·b448757 (⌂|🏘)
*  ·e9a378d (⌂|🏘)
│ *  🟣bce0c5e (✓) ►origin/main
├─╯
*  🏁·3183e43 (⌂|🏘|✓) ►main <> origin/main
layout:
  materialized parents: 1cf594d: 0bad3af acdc49a 9895054
"#]]
    );

    // Segments can definitely repeat
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡:A on 3183e43
│   ├── :A
│   │   └── ·0bad3af (🏘️)
│   └── :shared
│       ├── ·d4f537e (🏘️)
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
├── ≡:B on 3183e43
│   ├── :B
│   │   ├── ·acdc49a (🏘️)
│   │   └── ·f0117e0 (🏘️)
│   └── :shared
│       ├── ·d4f537e (🏘️)
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
└── ≡:D on 3183e43
    ├── :D
    │   └── ·9895054 (🏘️)
    ├── :C
    │   ├── ·de625cc (🏘️)
    │   ├── ·23419f8 (🏘️)
    │   └── ·5dc4389 (🏘️)
    └── :shared
        ├── ·d4f537e (🏘️)
        ├── ·b448757 (🏘️)
        └── ·e9a378d (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        Some(ref_name),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // Checking out anything inside the workspace yields the same result.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡👉:A on 3183e43
│   ├── 👉:A
│   │   └── ·0bad3af (🏘️)
│   └── :shared
│       ├── ·d4f537e (🏘️)
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
├── ≡:B on 3183e43
│   ├── :B
│   │   ├── ·acdc49a (🏘️)
│   │   └── ·f0117e0 (🏘️)
│   └── :shared
│       ├── ·d4f537e (🏘️)
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
└── ≡:D on 3183e43
    ├── :D
    │   └── ·9895054 (🏘️)
    ├── :C
    │   ├── ·de625cc (🏘️)
    │   ├── ·23419f8 (🏘️)
    │   └── ·5dc4389 (🏘️)
    └── :shared
        ├── ·d4f537e (🏘️)
        ├── ·b448757 (🏘️)
        └── ·e9a378d (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn local_branch_tracking_the_target_does_not_duplicate_the_target_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/multi-lane-with-shared-segment")?;
    add_workspace(&mut meta);

    // `main` tracks the target `origin/main`. Remote-tracking discovery at `main` must
    // recognize the project-metadata target ref as already queued instead of inserting
    // a second `origin/main` segment, which can leave disconnected segments behind.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    let target_positions = ws.commit_graph().layout().map_or(0, |l| {
        l.placements()
            .filter(|(name, _)| name.as_bstr() == "refs/remotes/origin/main")
            .count()
    });
    assert_eq!(
        target_positions, 1,
        "the initial target tip owns the only position for the target ref"
    );
    Ok(())
}

#[test]
fn dependent_branch_insertion() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependent",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   335d6f2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * cbc6713 (origin/advanced-lane, dependent, advanced-lane) change
|/  
* fafd9d0 (origin/main, main, lane) init

"#]]
        .raw()
    );

    add_stack_with_segments(
        &mut meta,
        1,
        "dependent",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·335d6f2 (⌂|🏘)
├─╮
│ *  ·cbc6713 (⌂|🏘) ►advanced-lane, ►dependent, ►origin/advanced-lane <> origin/advanced-lane
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►lane, ►main, ►origin/main <> origin/main
layout:
  materialized parents: 335d6f2: fafd9d0 cbc6713
  empty chain anchors: cbc6713^
"#]]
    );

    // The dependent branch is empty and on top of the one with the remote
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:dependent on fafd9d0 {1}
    ├── 📙:dependent
    └── 📙:advanced-lane <> origin/advanced-lane
        └── ❄️cbc6713 (🏘️)

"#]]
    );

    // Create the dependent branch below.
    add_stack_with_segments(
        &mut meta,
        1,
        "advanced-lane",
        StackState::InWorkspace,
        &["dependent"],
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·335d6f2 (⌂|🏘)
├─╮
│ *  ·cbc6713 (⌂|🏘) ►advanced-lane, ►dependent, ►origin/advanced-lane <> origin/advanced-lane
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►lane, ►main, ►origin/main <> origin/main
layout:
  materialized parents: 335d6f2: fafd9d0 cbc6713
  empty chain anchors: cbc6713^
"#]]
    );

    // Having done something unusual, which is to put the dependent branch
    // underneath the other already pushed, it creates a different view of ownership.
    // It's probably OK to leave it like this for now, and instead allow users to reorder
    // these more easily.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:advanced-lane <> origin/advanced-lane on fafd9d0 {1}
    ├── 📙:advanced-lane <> origin/advanced-lane
    └── 📙:dependent
        └── ❄cbc6713 (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "advanced-lane");
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡👉📙:advanced-lane <> origin/advanced-lane on fafd9d0 {1}
    ├── 👉📙:advanced-lane <> origin/advanced-lane
    └── 📙:dependent
        └── ❄cbc6713 (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "dependent");
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:advanced-lane <> origin/advanced-lane on fafd9d0 {1}
    ├── 📙:advanced-lane <> origin/advanced-lane
    └── 👉📙:dependent
        └── ❄cbc6713 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn multiple_stacks_with_shared_parent_and_remote() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multiple-stacks-with-shared-segment-and-remote")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   baed751 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 4f1bb32 (C-on-A) C-on-A
* | aff8449 (B-on-A) B-on-A
|/  
| * b627ca7 (origin/A) A-on-remote
|/  
* e255adc (A) A
* fafd9d0 (origin/main, main) init

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 1, "C-on-A", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·baed751 (⌂|🏘)
├─╮
* │  ·aff8449 (⌂|🏘) ►B-on-A
│ *  ·4f1bb32 (⌂|🏘) ►C-on-A
├─╯
│ *  🟣b627ca7 ►origin/A
├─╯
*  ·e255adc (⌂|🏘) ►A <> origin/A
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: baed751: aff8449 4f1bb32
  empty chain anchors: 4f1bb32^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡:B-on-A on fafd9d0
│   ├── :B-on-A
│   │   └── ·aff8449 (🏘️)
│   └── :A <> origin/A⇣1
│       ├── 🟣b627ca7
│       └── ❄️e255adc (🏘️)
└── ≡📙:C-on-A on fafd9d0 {1}
    ├── 📙:C-on-A
    │   └── ·4f1bb32 (🏘️)
    └── :A <> origin/A⇣1
        ├── 🟣b627ca7
        └── ❄️e255adc (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn a_stack_segment_can_be_a_segment_elsewhere_and_stack_order() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-branches-one-advanced-two-parent-ws-commit-diverged-ttb",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   873d056 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | cbc6713 (advanced-lane) change
|/  
* fafd9d0 (main, lane) init
* da83717 (origin/main) disjoint remote target

"#]]
        .raw()
    );

    let lanes = ["advanced-lane", "lane"];
    for (idx, name) in lanes.into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·873d056 (⌂|🏘)
├─╮
* │  ·cbc6713 (⌂|🏘) ►advanced-lane
├─╯
*  🏁·fafd9d0 (⌂|🏘) ►lane, ►main <> origin/main
*  🏁🟣da83717 (✓) ►origin/main
layout:
  materialized parents: 873d056: cbc6713 fafd9d0
  empty chain anchors: cbc6713^ fafd9d0^
"#]]
    );

    // Since `lane` is connected directly, no segment has to be created.
    // However, as nothing is integrated, it really is another name for `main` now,
    // `main` is nothing special.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:advanced-lane on fafd9d0 {0}
│   └── 📙:advanced-lane
│       └── ·cbc6713 (🏘️)
└── ≡📙:lane on fafd9d0 {1}
    └── 📙:lane

"#]]
    );

    // Reverse the order of stacks in the worktree data.
    for (idx, name) in lanes.into_iter().rev().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·873d056 (⌂|🏘)
├─╮
* │  ·cbc6713 (⌂|🏘) ►advanced-lane
├─╯
*  🏁·fafd9d0 (⌂|🏘) ►lane, ►main <> origin/main
*  🏁🟣da83717 (✓) ►origin/main
layout:
  materialized parents: 873d056: cbc6713 fafd9d0
  empty chain anchors: fafd9d0^ cbc6713^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:advanced-lane on fafd9d0 {1}
│   └── 📙:advanced-lane
│       └── ·cbc6713 (🏘️)
└── ≡📙:lane on fafd9d0 {0}
    └── 📙:lane

"#]]
    );
    Ok(())
}

#[test]
fn two_dependent_branches_with_embedded_remote() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-dependent-branches-with-interesting-remote-setup")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a221221 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* aadad9d (A) shared by name
* 96a2408 (origin/main) another unrelated
| * 2b1808c (origin/A) shared by name
|/  
* f15ca75 (integrated) other integrated
* 9456d79 integrated in target
* fafd9d0 (main) init

"#]]
    );

    // Just a single explicit reference we want to know of.
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    // Note how the target remote tracking branch is integrated into the stack
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a221221 (⌂|🏘)
*  ·aadad9d (⌂|🏘) ►A <> origin/A
*  ·96a2408 (⌂|🏘|✓) ►origin/main
│ *  🟣2b1808c ►origin/A
├─╯
*  ·f15ca75 (⌂|🏘|✓) ►integrated
*  ·9456d79 (⌂|🏘|✓)
*  🏁·fafd9d0 (⌂|🏘|✓) ►main <> origin/main
layout:
  materialized parents: a221221: aadad9d
  empty chain anchors: aadad9d^
"#]]
    );

    // Remote tracking branches we just want to aggregate, just like anonymous segments,
    // but only when another target is provided (the old position, `main`).
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:A <> origin/A⇡1⇣1 on fafd9d0 {1}
    ├── 📙:A <> origin/A⇡1⇣1
    │   ├── 🟣2b1808c
    │   ├── ·aadad9d (🏘️)
    │   └── ·96a2408 (🏘️|✓)
    └── :integrated
        ├── ❄f15ca75 (🏘️|✓)
        └── ❄9456d79 (🏘️|✓)

"#]]
    );

    // Otherwise, nothing that's integrated is shown. Note how 96a2408 seems missing,
    // but it's skipped because it's actually part of an integrated otherwise ignored segment.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 96a2408
└── ≡📙:A <> origin/A⇡1⇣1 on 96a2408 {1}
    └── 📙:A <> origin/A⇡1⇣1
        ├── 🟣2b1808c
        └── ·aadad9d (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn two_dependent_branches_rebased_with_remotes_merge_local() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-dependent-branches-rebased-with-remotes-merge-one-local",
    )?;
    // Each of the stacked branches has a remote, and the local branch was merged into main.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
    );

    add_stack_with_segments(&mut meta, 0, "B", StackState::InWorkspace, &["A"]);

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·4f08b8d (⌂|🏘)
*  ·da597e8 (⌂|🏘) ►B <> origin/B
│ *  🟣b694668 (✓) ►origin/main
╭─┤
* │  ·1818c17 (⌂|🏘|✓) ►A <> origin/A
├─╯
│ *  🟣e0bd0a7 ►origin/B
│ *  🟣0b6b861 ►origin/A
├─╯
*  🏁·281456a (⌂|🏘|✓) ►main <> origin/main
layout:
  materialized parents: 4f08b8d: da597e8
  empty chain anchors: da597e8^
"#]]
    );

    // This is the default as it includes both the integrated and non-integrated segment.
    // Note how there is no expensive computation to see if remote commits are the same,
    // it's all ID-based.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 281456a
└── ≡📙:B <> origin/B⇡1⇣1 on 281456a {0}
    ├── 📙:B <> origin/B⇡1⇣1
    │   ├── 🟣e0bd0a7
    │   └── ·da597e8 (🏘️)
    └── 📙:A <> origin/A⇣1
        ├── 🟣0b6b861
        └── ·1818c17 (🏘️|✓)

"#]]
    );

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "A"),
    )?
    .validated()?;
    // Pretending we are rebased onto A still shows the same remote commits.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1818c17
└── ≡📙:B <> origin/B⇡1⇣1 on 1818c17 {0}
    └── 📙:B <> origin/B⇡1⇣1
        ├── 🟣e0bd0a7
        └── ·da597e8 (🏘️)

"#]]
    );
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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 5c66c47 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* bfbff44 (origin/bottom, top) T
* 7fdb58d (bottom) B
* fafd9d0 (origin/main, main) init

"#]]
    );

    add_stack_with_segments(&mut meta, 0, "top", StackState::InWorkspace, &["bottom"]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:top on fafd9d0 {0}
    ├── 📙:top
    │   └── ❄bfbff44 (🏘️)
    └── 📙:bottom <> origin/bottom⇣1
        ├── 🟣bfbff44 (🏘️)
        └── ❄️7fdb58d (🏘️)

"#]]
    );
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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 1109eb2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 624e118 (D) D
* 0b6b861 (origin/main, main) A
| * 3045ea6 (origin/D) D
| * 1818c17 (origin/C, origin/B, origin/A) A
|/  
* 281456a init

"#]]
    );

    // The branch A, B, C are not in the workspace anymore, and we *could* signal it by removing metadata.
    // But even with metadata, it still works fine.
    add_stack_with_segments(&mut meta, 0, "D", StackState::InWorkspace, &["C", "B", "A"]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·1109eb2 (⌂|🏘)
*  ·624e118 (⌂|🏘) ►D <> origin/D
*  ·0b6b861 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
│ *  🟣3045ea6 ►origin/D
│ *  🟣1818c17 ►origin/A, ►origin/B, ►origin/C
├─╯
*  🏁·281456a (⌂|🏘|✓)
layout:
  materialized parents: 1109eb2: 624e118
  empty chain anchors: 624e118^
"#]]
    );

    let ambiguous_remote_tip = repo.rev_parse_single("origin/A")?.detach();
    for remote_ref in [
        "refs/remotes/origin/A",
        "refs/remotes/origin/B",
        "refs/remotes/origin/C",
    ] {
        let remote_ref = super::ref_name(remote_ref);
        assert_eq!(
            ws.commit_graph().commit_by_ref(remote_ref.as_ref()),
            Some(ambiguous_remote_tip),
            "{remote_ref} should resolve to the commit its Git ref points to, showing that something special happened here"
        );
    }

    // only one remote commit as unrelated remotes split a linear segment
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
└── ≡📙:D <> origin/D⇡1⇣1 on 0b6b861 {0}
    └── 📙:D <> origin/D⇡1⇣1
        ├── 🟣3045ea6
        └── ·624e118 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn two_dependent_branches_rebased_with_remotes_squash_merge_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-dependent-branches-rebased-with-remotes-squash-merge-one-remote",
    )?;
    // Each of the stacked branches has a remote, the remote branch was merged into main,
    // and the remaining branch B was rebased onto the merge, simulating a workspace update.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
    );

    // The branch A, B, C are not in the workspace anymore, and we *could* signal it by removing metadata.
    // But even with metadata, it still works fine.
    add_stack_with_segments(&mut meta, 0, "D", StackState::InWorkspace, &["C", "B", "A"]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·deeae50 (⌂|🏘)
*  ·353471f (⌂|🏘) ►D <> origin/D
*  ·8a4b945 (⌂|🏘)
*  ·e0bd0a7 (⌂|🏘)
*  ·0b6b861 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
│ *  🟣bbd4ff6 ►origin/D
│ *  🟣e5f5a87 ►origin/C
│ *  🟣da597e8 ►origin/B
│ *  🟣1818c17 ►origin/A
├─╯
*  🏁·281456a (⌂|🏘|✓)
layout:
  materialized parents: deeae50: 353471f
  empty chain anchors: 353471f^
"#]]
    );

    // We let each remote on the path down own a commit so we only see one remote commit here,
    // the one belonging to the last remaining associated remote tracking branch of D.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
└── ≡📙:D <> origin/D⇡3⇣1 on 0b6b861 {0}
    └── 📙:D <> origin/D⇡3⇣1
        ├── 🟣bbd4ff6
        ├── ·353471f (🏘️)
        ├── ·8a4b945 (🏘️)
        └── ·e0bd0a7 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4fe5a6f (origin/A) A-remote
* a62b0de (HEAD -> gitbutler/workspace, A) A2
* 120a217 A1
* fafd9d0 (main) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🟣4fe5a6f ►origin/A
*  👉·a62b0de (⌂|🏘) ►A <> origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡:A <> origin/A⇣1
    ├── :A <> origin/A⇣1
    │   ├── 🟣4fe5a6f
    │   ├── ❄️a62b0de (🏘️)
    │   └── ❄️120a217 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🟣4fe5a6f ►origin/A
*  👉·a62b0de (⌂|🏘) ►A <> origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
"#]]
    );

    // Main can be a normal segment if there is no target ref.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:A <> origin/A⇣1
    ├── 👉:A <> origin/A⇣1
    │   ├── 🟣4fe5a6f
    │   ├── ❄️a62b0de (🏘️)
    │   └── ❄️120a217 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit_ambiguous() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-target-without-ws-commit-ambiguous")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4fe5a6f (origin/A) A-remote
* a62b0de (HEAD -> gitbutler/workspace, B, A) A2
* 120a217 A1
* fafd9d0 (main) init

"#]]
    );

    add_workspace(&mut meta);
    // Without disambiguation, there is no segment name.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🟣4fe5a6f ►origin/A
*  👉·a62b0de (⌂|🏘) ►A, ►B <> origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡:A <> origin/A⇣1
    ├── :A <> origin/A⇣1
    │   ├── 🟣4fe5a6f
    │   ├── ❄️a62b0de (🏘️) ►B
    │   └── ❄️120a217 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    // We can help it by adding metadata.
    // Note how the selection still manages to hold on to the `A` which now gets its very own
    // empty segment.
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let (id, a_ref) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🟣4fe5a6f ►origin/A
*  👉·a62b0de (⌂|🏘) ►A, ►B <> origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
layout:
  empty chain anchors: a62b0de^
"#]]
    );

    // Main can be a normal segment if there is no target ref.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:A <> origin/A {1}
    ├── 👉:A <> origin/A
    ├── 📙:B
    │   ├── ·a62b0de (🏘️)
    │   └── ·120a217 (🏘️)
    └── :main
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // Finally, show the normal version with just disambiguated 'B".
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🟣4fe5a6f ►origin/A
*  👉·a62b0de (⌂|🏘) ►A, ►B <> origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
layout:
  empty chain anchors: a62b0de^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:B {1}
    ├── 📙:B
    │   ├── ·a62b0de (🏘️) ►A
    │   └── ·120a217 (🏘️)
    └── :main
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // Order is respected
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);
    let ws = Workspace::from_tip(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // The remote tracking branch must remain linked.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:B {1}
    ├── 📙:B
    ├── 👉📙:A <> origin/A⇣1
    │   ├── 🟣4fe5a6f
    │   ├── ❄️a62b0de (🏘️)
    │   └── ❄️120a217 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    // Order is respected, vice-versa
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = Workspace::from_tip(id, a_ref, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡👉📙:A <> origin/A⇣1 {1}
    ├── 👉📙:A <> origin/A⇣1
    │   └── 🟣4fe5a6f
    ├── 📙:B
    │   ├── ❄a62b0de (🏘️)
    │   └── ❄120a217 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn without_target_ref_or_managed_commit_ambiguous_with_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-target-without-ws-commit-ambiguous-with-remotes")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a62b0de (HEAD -> gitbutler/workspace, origin/B, origin/A, B, A) A2
* 120a217 A1
* fafd9d0 (main) init

"#]]
    );

    add_workspace(&mut meta);
    // Without disambiguation, there is no segment name.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a62b0de (⌂|🏘) ►A, ►B, ►origin/A, ►origin/B <> origin/A, origin/B
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main <> origin/main
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡:anon:
    ├── :anon:
    │   ├── ·a62b0de (🏘️) ►A, ►B
    │   └── ·120a217 (🏘️)
    └── :main <> origin/main⇡1
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // Remote handling is still happening when A is disambiguated by entrypoint.
    let (id, a_ref) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a62b0de (⌂|🏘) ►A, ►B, ►origin/A, ►origin/B <> origin/A, origin/B
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main <> origin/main
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:A <> origin/A⇡2
    ├── 👉:A <> origin/A⇡2
    │   ├── ·a62b0de (🏘️) ►B
    │   └── ·120a217 (🏘️)
    └── :main <> origin/main⇡1
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // The same is true when starting at a different ref.
    let (id, b_ref) = id_at(&repo, "B");
    let ws = Workspace::from_tip(id, b_ref, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:B <> origin/B⇡2
    ├── 👉:B <> origin/B⇡2
    │   ├── ·a62b0de (🏘️) ►A
    │   └── ·120a217 (🏘️)
    └── :main <> origin/main⇡1
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // If disambiguation happens through the workspace, 'A' still shows the right remote, and 'B' as well
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let ws = Workspace::from_tip(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // NOTE: origin/A points to :5, but origin/B now also points to :5 even though it should point to :0,
    //       a relationship still preserved though the sibling ID.
    //       There is no easy way of fixing this as we'd have to know that this one connection, which can
    //       indirectly reach the remote tracking segment, should remain on the local tracking segment when
    //       reconnecting them during the segment insertion.
    //       This is acceptable as graph connections aren't used for this, and ultimately they still
    //       reach the right segment, just through one more indirection. Empty segments are 'looked through'
    //       as well by all algorithms for exactly that reason.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a62b0de (⌂|🏘) ►A, ►B, ►origin/A, ►origin/B <> origin/A, origin/B
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main <> origin/main
layout:
  empty chain anchors: a62b0de^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:A <> origin/A {1}
    ├── 👉:A <> origin/A
    ├── 📙:B <> origin/B
    │   ├── ❄️a62b0de (🏘️)
    │   └── ❄️120a217 (🏘️)
    └── :main <> origin/main
        └── ❄fafd9d0 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn without_target_ref_with_managed_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-with-ws-commit")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 4fe5a6f (origin/A) A-remote
|/  
* a62b0de (A) A2
* 120a217 A1
* fafd9d0 (main) init

"#]]
    );

    add_workspace(&mut meta);
    // The commit is ambiguous, so there is just the entrypoint to split the segment.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·3ea2742 (⌂|🏘)
│ *  🟣4fe5a6f ►origin/A
├─╯
*  ·a62b0de (⌂|🏘) ►A <> origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
layout:
  materialized parents: 3ea2742: a62b0de
"#]]
    );
    // TODO: add more stacks.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:A <> origin/A⇣1
    ├── :A <> origin/A⇣1
    │   ├── 🟣4fe5a6f
    │   ├── ❄️a62b0de (🏘️)
    │   └── ❄️120a217 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·3ea2742 (⌂|🏘)
│ *  🟣4fe5a6f ►origin/A
├─╯
*  👉·a62b0de (⌂|🏘) ►A <> origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:A <> origin/A⇣1
    ├── 👉:A <> origin/A⇣1
    │   ├── 🟣4fe5a6f
    │   ├── ❄️a62b0de (🏘️)
    │   └── ❄️120a217 (🏘️)
    └── :main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn workspace_commit_pushed_to_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/ws-commit-pushed-to-target")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 8ee08de (HEAD -> gitbutler/workspace, origin/main) GitButler Workspace Commit
* 120a217 (A) A1
* fafd9d0 (main) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·8ee08de (⌂|🏘|✓) ►origin/main
*  ·120a217 (⌂|🏘|✓) ►A
*  🏁·fafd9d0 (⌂|🏘|✓) ►main <> origin/main
layout:
  materialized parents: 8ee08de: 120a217
"#]]
    );
    // Everything is integrated, so nothing is shown.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 120a217

"#]]
    );
    Ok(())
}

#[test]
fn no_workspace_no_target_commit_under_managed_ref() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/no-ws-no-target-commit-with-managed-ref")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* dca94a4 (HEAD -> gitbutler/workspace) unmanaged
* 120a217 (A) A1
* fafd9d0 (main) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·dca94a4 (⌂|🏘)
*  ·120a217 (⌂|🏘) ►A
*  🏁·fafd9d0 (⌂|🏘) ►main
"#]]
    );

    // It's notable how hard the workspace ref tries to not own the commit
    // it's under unless it's a managed commit.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡:anon:
    ├── :anon:
    │   └── ·dca94a4 (🏘️)
    ├── :A
    │   └── ·120a217 (🏘️)
    └── :main
        └── ·fafd9d0 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn no_workspace_commit() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multiple-dependent-branches-per-stack-without-ws-commit")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* cbc6713 (HEAD -> gitbutler/workspace, lane) change
* fafd9d0 (origin/main, main, lane-segment-02, lane-segment-01, lane-2-segment-02, lane-2-segment-01, lane-2) init

"#]]
    );

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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // Notably we also pick up 'lane' which sits on the base.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·cbc6713 (⌂|🏘) ►lane
*  🏁·fafd9d0 (⌂|🏘|✓) ►lane-2, ►lane-2-segment-01, ►lane-2-segment-02, ►lane-segment-01, ►lane-segment-02, ►main, ►origin/main <> origin/main
layout:
  empty chain anchors: cbc6713^ fafd9d0
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:lane on fafd9d0 {0}
│   ├── 📙:lane
│   │   └── ·cbc6713 (🏘️)
│   ├── 📙:lane-segment-01
│   └── 📙:lane-segment-02
└── ≡📙:lane-2 on fafd9d0 {1}
    ├── 📙:lane-2
    ├── 📙:lane-2-segment-01
    └── 📙:lane-2-segment-02

"#]]
    );

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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // the order is maintained as provided in the workspace.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·cbc6713 (⌂|🏘) ►lane
*  🏁·fafd9d0 (⌂|🏘|✓) ►lane-2, ►lane-2-segment-01, ►lane-2-segment-02, ►lane-segment-01, ►lane-segment-02, ►main, ►origin/main <> origin/main
layout:
  empty chain anchors: fafd9d0 cbc6713^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:lane-2 on fafd9d0 {0}
│   ├── 📙:lane-2
│   ├── 📙:lane-2-segment-01
│   └── 📙:lane-2-segment-02
└── ≡📙:lane on fafd9d0 {1}
    ├── 📙:lane
    │   └── ·cbc6713 (🏘️)
    ├── 📙:lane-segment-01
    └── 📙:lane-segment-02

"#]]
    );
    Ok(())
}

#[test]
fn two_dependent_branches_first_merged_by_rebase() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/two-dependent-branches-first-rebased-and-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0b6b861 (origin/main, origin/A) A
| * 4f08b8d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * da597e8 (B) B
| * 1818c17 (A) A
|/  
* 281456a (main) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·4f08b8d (⌂|🏘)
*  ·da597e8 (⌂|🏘) ►B
*  ·1818c17 (⌂|🏘) ►A <> origin/A
│ *  🟣0b6b861 (✓) ►origin/A, ►origin/main
├─╯
*  🏁·281456a (⌂|🏘|✓) ►main <> origin/main
layout:
  materialized parents: 4f08b8d: da597e8
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 281456a
└── ≡:B on 281456a
    ├── :B
    │   └── ·da597e8 (🏘️)
    └── :A <> origin/A⇡1⇣1
        ├── 🟣0b6b861 (✓)
        └── ·1818c17 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn special_branch_names_do_not_end_up_in_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/special-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 8926b15 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 3686017 (main) top
* 9725482 (gitbutler/edit) middle
* fafd9d0 (gitbutler/target) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // Standard handling after traversal and post-processing.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·8926b15 (⌂|🏘)
*  ·3686017 (⌂|🏘) ►main
*  ·9725482 (⌂|🏘) ►gitbutler/edit
*  🏁·fafd9d0 (⌂|🏘) ►gitbutler/target
layout:
  materialized parents: 8926b15: 3686017
"#]]
    );

    // But special handling for workspace views.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:main
    └── :main
        ├── ·3686017 (🏘️)
        ├── ·9725482 (🏘️)
        └── ·fafd9d0 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn special_branch_do_not_allow_overly_long_segments() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/special-branches-edgecase")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 270738b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* c59457b (A) top
* e146f13 (gitbutler/edit) middle
* 971953d (origin/main, main) M2
* ce09734 (origin/gitbutler/target, gitbutler/target) M1
* fafd9d0 init

"#]]
    );

    add_workspace(&mut meta);
    let mut md = meta.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    let mut project_meta = md.project_meta();
    project_meta.target_ref = Some("refs/remotes/origin/gitbutler/target".try_into()?);
    md.set_project_meta(project_meta);
    meta.set_workspace(&md)?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        md.project_meta(),
        // standard_options_with_extra_target(&repo, "gitbutler/target"),
        standard_options(),
    )?
    .validated()?;
    // Standard handling after traversal and post-processing.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·270738b (⌂|🏘)
*  ·c59457b (⌂|🏘) ►A
*  ·e146f13 (⌂|🏘) ►gitbutler/edit
*  ·971953d (⌂|🏘) ►main, ►origin/main <> origin/main
*  ·ce09734 (⌂|🏘|✓) ►gitbutler/target, ►origin/gitbutler/target <> origin/gitbutler/target
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: 270738b: c59457b
"#]]
    );

    // But special handling for workspace views. Note how we don't overshoot
    // and stop exactly where we have to, magically even.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/gitbutler/target on ce09734
└── ≡:A on ce09734
    ├── :A
    │   ├── ·c59457b (🏘️)
    │   └── ·e146f13 (🏘️)
    └── :main <> origin/main
        └── ❄️971953d (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn branch_ahead_of_workspace() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/branches-ahead-of-workspace")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    ·867927f (⌂|✓) ►main, ►origin/main <> origin/main
├─╮
* │    ·6e03461 (⌂|✓)
├───╮
│ │ │ *      👉·fe6ba62 (⌂|🏘)
│ │ ╭─┼─┬─╮
│ │ * │ │ │  ·a62b0de (⌂|🏘|✓)
│ │ * │ │ │  ·120a217 (⌂|🏘|✓)
├───╯ │ │ │
│ │   * │ │  ·2f8f06d (⌂|🏘) ►B
│ ├───╯ │ │
│ *     │ │  ·91bc3fc (⌂|🏘|✓) ►origin/B-middle
│ *     │ │  ·cf9330f (⌂|🏘|✓)
├─╯     │ │
│       * │  ·3f7c4e6 (⌂|🏘) ►C
│       * │  ·b6895d7 (⌂|🏘)
├───────╯ │
│         *  ·ed36e3b (⌂|🏘) ►new-name-for-D
├─────────╯
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: fe6ba62: a62b0de 2f8f06d 3f7c4e6 ed36e3b
"#]]
    );

    // If it doesn't know how the workspace should be looking like, i.e. which branches are contained,
    // nothing special happens.
    // The branches that are outside the workspace don't exist and segments are flattened.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on fafd9d0
├── ≡:B on 91bc3fc
│   └── :B
│       └── ·2f8f06d (🏘️)
├── ≡:C on fafd9d0
│   └── :C
│       ├── ·3f7c4e6 (🏘️)
│       └── ·b6895d7 (🏘️)
└── ≡:new-name-for-D on fafd9d0
    └── :new-name-for-D
        └── ·ed36e3b (🏘️)

"#]]
    );

    // However, when the desired workspace is set up, the traversal will include these extra tips.
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-middle"]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["B-middle"]);
    add_stack_with_segments(&mut meta, 2, "C", StackState::InWorkspace, &["C-bottom"]);
    add_stack_with_segments(&mut meta, 3, "D", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, ":/init"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    ·867927f (⌂|✓) ►main, ►origin/main <> origin/main
├─╮
* │    ·6e03461 (⌂|✓)
├───╮
│ │ │ *  ·c83f258 (⌂) ►A
│ │ ├─╯
│ │ │ *  ·27c2545 (⌂) ►A-middle, ►origin/A-middle <> origin/A-middle
│ │ │ │ *  ·c8f73c7 (⌂) ►B-middle <> origin/B-middle
│ │ │ │ *  ·ff75b80 (⌂) ►intermediate-branch
│ ├─────╯
│ │ │ │ *    ·790a17d (⌂) ►C-bottom
│ │ │ │ ├─╮
│ │ │ │ * │  ·969aaec (⌂)
│ │ │ │ │ *  ·631be19 (⌂) ►tmp
│ │ │ │ ├─╯
│ │ │ │ │ *  ·71dad1a (⌂) ►D
│ │ │ │ │ │ *    👉·fe6ba62 (⌂|🏘)
│ │ ╭─────┬─┼─╮
│ │ * │ │ │ │ │  ·a62b0de (⌂|🏘|✓)
│ │ ├─╯ │ │ │ │
│ │ *   │ │ │ │  ·120a217 (⌂|🏘|✓)
├───╯   │ │ │ │
│ │     │ │ * │  ·2f8f06d (⌂|🏘) ►B
│ ├─────────╯ │
│ *     │ │   │  ·91bc3fc (⌂|🏘|✓) ►origin/B-middle
│ *     │ │   │  ·cf9330f (⌂|🏘|✓)
├─╯     │ │   │
│       │ │   *  ·3f7c4e6 (⌂|🏘) ►C
│       ├─────╯
│       * │  ·b6895d7 (⌂|🏘)
├───────╯ │
│         *  ·ed36e3b (⌂|🏘) ►new-name-for-D
├─────────╯
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: fe6ba62: a62b0de 2f8f06d 3f7c4e6 ed36e3b
  empty chain anchors: 2f8f06d^ 3f7c4e6^
"#]]
    );

    // The workspace itself contains information about the outside tips.
    // We collect it no matter the location of the tip, e.g.
    // - anon segment directly below the workspace commit
    // - middle anon segment leading to the named branch over intermediate branches
    // - middle anon segment leading to the named branch over two outgoing connections
    // - except: if the segment with a known named segment in its future has a (new) name,
    //   we leave it and don't attempt to reconstruct the original (out-of-workspace) reference
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on fafd9d0
├── ≡📙:A on fafd9d0 {0}
│   ├── 📙:A
│   │   ├── ·c83f258*
│   │   └── ·a62b0de (🏘️|✓)
│   └── 📙:A-middle <> origin/A-middle
│       ├── ·27c2545*
│       └── ·120a217 (🏘️|✓)
├── ≡📙:B on fafd9d0 {1}
│   ├── 📙:B
│   │   └── ·2f8f06d (🏘️)
│   └── 📙:B-middle <> origin/B-middle
│       ├── ·c8f73c7*
│       ├── ·ff75b80 ►intermediate-branch*
│       ├── ·91bc3fc (🏘️|✓)
│       └── ·cf9330f (🏘️|✓)
├── ≡📙:C on fafd9d0 {2}
│   ├── 📙:C
│   │   └── ·3f7c4e6 (🏘️)
│   └── 📙:C-bottom
│       ├── ·790a17d*
│       ├── ·969aaec*
│       └── ·b6895d7 (🏘️)
└── ≡:new-name-for-D on fafd9d0
    └── :new-name-for-D
        └── ·ed36e3b (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn two_branches_one_advanced_two_parent_ws_commit_diverged_ttb() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-branches-one-advanced-two-parent-ws-commit-diverged-ttb",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   873d056 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | cbc6713 (advanced-lane) change
|/  
* fafd9d0 (main, lane) init
* da83717 (origin/main) disjoint remote target

"#]]
        .raw()
    );

    for (idx, name) in ["lane", "advanced-lane"].into_iter().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }

    let (id, ref_name) = id_at(&repo, "lane");
    let ws = Workspace::from_tip(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    ·873d056 (⌂|🏘)
├─╮
* │  ·cbc6713 (⌂|🏘) ►advanced-lane
├─╯
*  👉🏁·fafd9d0 (⌂|🏘|✓) ►lane, ►main <> origin/main
*  🏁🟣da83717 (✓) ►origin/main
layout:
  empty chain anchors: fafd9d0 cbc6713^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:advanced-lane on fafd9d0 {1}
│   └── 📙:advanced-lane
│       └── ·cbc6713 (🏘️)
└── ≡👉📙:lane on fafd9d0 {0}
    └── 👉📙:lane

"#]]
    );

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·873d056 (⌂|🏘)
├─╮
* │  ·cbc6713 (⌂|🏘) ►advanced-lane
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►lane, ►main <> origin/main
*  🏁🟣da83717 (✓) ►origin/main
layout:
  materialized parents: 873d056: cbc6713 fafd9d0
  empty chain anchors: fafd9d0 cbc6713^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:advanced-lane on fafd9d0 {1}
│   └── 📙:advanced-lane
│       └── ·cbc6713 (🏘️)
└── ≡📙:lane on fafd9d0 {0}
    └── 📙:lane

"#]]
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·873d056 (⌂|🏘)
├─╮
* │  ·cbc6713 (⌂|🏘) ►advanced-lane
├─╯
*  🏁·fafd9d0 (⌂|🏘) ►lane, ►main <> origin/main
*  🏁🟣da83717 (✓) ►origin/main
layout:
  materialized parents: 873d056: cbc6713 fafd9d0
  empty chain anchors: fafd9d0^ cbc6713^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:advanced-lane on fafd9d0 {1}
│   └── 📙:advanced-lane
│       └── ·cbc6713 (🏘️)
└── ≡📙:lane on fafd9d0 {0}
    └── 📙:lane

"#]]
    );
    Ok(())
}

#[test]
fn advanced_workspace_ref() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/advanced-workspace-ref")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a7131b1 (⌂|🏘)
*  ·4d3831e (⌂|🏘) ►intermediate-ref
*    ·468357f (⌂|🏘)
├─╮
│ *  ·d3166f7 (⌂|🏘) ►branch-on-top
├─╯
*  ·118ddbb (⌂|🏘)
*    ·619d548 (⌂|🏘)
├─╮
* │  ·8a352d5 (⌂|🏘) ►B
│ *  ·6fdab32 (⌂|🏘) ►A
├─╯
*  ·bce0c5e (⌂|🏘|✓) ►main, ►origin/main <> origin/main
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  empty chain anchors: 6fdab32^ 8a352d5^
"#]]
    );

    // We show the original 'native' configuration without pruning anything, even though
    // it contains the workspace commit 619d548.
    // It's up to the caller to deal with this situation as the workspace now is marked differently.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡:anon: on bce0c5e {1}
    ├── :anon:
    │   └── ·a7131b1 (🏘️)
    ├── :intermediate-ref
    │   ├── ·4d3831e (🏘️)
    │   ├── ·468357f (🏘️)
    │   ├── ·118ddbb (🏘️)
    │   └── ·619d548 (🏘️)
    └── 📙:B
        └── ·8a352d5 (🏘️)

"#]]
    );

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    // The extra-target as would happen in the typical case would change nothing though.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a7131b1 (⌂|🏘)
*  ·4d3831e (⌂|🏘) ►intermediate-ref
*    ·468357f (⌂|🏘)
├─╮
│ *  ·d3166f7 (⌂|🏘) ►branch-on-top
├─╯
*  ·118ddbb (⌂|🏘)
*    ·619d548 (⌂|🏘)
├─╮
* │  ·8a352d5 (⌂|🏘) ►B
│ *  ·6fdab32 (⌂|🏘) ►A
├─╯
*  ·bce0c5e (⌂|🏘|✓) ►main, ►origin/main <> origin/main
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  empty chain anchors: 6fdab32^ 8a352d5^
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡:anon: on bce0c5e {1}
    ├── :anon:
    │   └── ·a7131b1 (🏘️)
    ├── :intermediate-ref
    │   ├── ·4d3831e (🏘️)
    │   ├── ·468357f (🏘️)
    │   ├── ·118ddbb (🏘️)
    │   └── ·619d548 (🏘️)
    └── 📙:B
        └── ·8a352d5 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn advanced_workspace_ref_single_stack() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/advanced-workspace-ref-and-single-stack")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·da912a8 (⌂|🏘)
*  ·198eaf8 (⌂|🏘) ►intermediate-ref
*    ·3147997 (⌂|🏘)
├─╮
│ *  ·dd7bb9a (⌂|🏘) ►branch-on-top
├─╯
*  ·9785229 (⌂|🏘)
*  ·c58f157 (⌂|🏘)
*  ·6fdab32 (⌂|🏘) ►A
*  ·bce0c5e (⌂|🏘|✓) ►main, ►origin/main <> origin/main
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  empty chain anchors: 6fdab32^
"#]]
    );

    // Here we'd show what happens if the workspace commit is somewhere in the middle
    // of the segment. This is relevant for code trying to find it, which isn't done here.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡:anon: on bce0c5e {0}
    ├── :anon:
    │   └── ·da912a8 (🏘️)
    ├── :intermediate-ref
    │   ├── ·198eaf8 (🏘️)
    │   ├── ·3147997 (🏘️)
    │   ├── ·9785229 (🏘️)
    │   └── ·c58f157 (🏘️)
    └── 📙:A
        └── ·6fdab32 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn shallow_boundary_below_workspace_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario(
        "special-conditions",
        "shallow-workspace-boundary-below-lower-bound",
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 00e1860 (HEAD -> gitbutler/workspace, origin/gitbutler/workspace, origin/HEAD) GitButler Workspace Commit
* 6507810 (origin/A, A) A1
* b625665 (origin/main, main) M4
* a821094 M3
* bce0c5e (grafted) M2

"#]]
    );

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·00e1860 (⌂|🏘) ►origin/gitbutler/workspace
*  ·6507810 (⌂|🏘) ►A, ►origin/A <> origin/A
*  ·b625665 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
*  ·a821094 (⌂|🏘|✓)
*  ⛰·bce0c5e (⌂|🏘|✓|⛰)
layout:
  materialized parents: 00e1860: 6507810
  empty chain anchors: 6507810^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on b625665
└── ≡📙:A <> origin/A on b625665 {1}
    └── 📙:A <> origin/A
        └── ❄️6507810 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn shallow_boundary_in_workspace_prevents_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario(
        "special-conditions",
        "shallow-workspace-boundary-in-workspace",
    )?;

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·00e1860 (⌂|🏘) ►origin/gitbutler/workspace
*  ⛰·6507810 (⌂|🏘|⛰) ►A
layout:
  materialized parents: 00e1860: 6507810
  empty chain anchors: 6507810^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:A {1}
    └── 📙:A
        └── ·6507810 (🏘️|⛰)

"#]]
    );

    Ok(())
}

#[test]
fn applied_stack_below_explicit_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-branches-one-below-base")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    meta.data_mut().default_target = None;
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·e82dfab (⌂|🏘)
├─╮
* │  ·78b1b59 (⌂|🏘) ►B
* │  ·f52fcec (⌂|🏘)
│ *  ·6fdab32 (⌂|🏘) ►A
├─╯
*  ·bce0c5e (⌂|🏘)
*  🏁·3183e43 (⌂|🏘)
layout:
  materialized parents: e82dfab: 78b1b59 6fdab32
"#]]
    );

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on bce0c5e
├── ≡:B on bce0c5e
│   └── :B
│       ├── ·78b1b59 (🏘️)
│       └── ·f52fcec (🏘️)
└── ≡:A on bce0c5e
    └── :A
        └── ·6fdab32 (🏘️)

"#]]
    );

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // The same is true if stacks are known in workspace metadata.
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·938e6f2 (⌂|✓) ►main, ►origin/main <> origin/main
│ *    👉·e82dfab (⌂|🏘)
│ ├─╮
│ * │  ·78b1b59 (⌂|🏘) ►B
├─╯ │
*   │  ·f52fcec (⌂|🏘|✓)
│   *  ·6fdab32 (⌂|🏘) ►A
├───╯
*  ·bce0c5e (⌂|🏘|✓)
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  materialized parents: e82dfab: 78b1b59 6fdab32
  empty chain anchors: 6fdab32^ 78b1b59^
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e
├── ≡📙:B on f52fcec {1}
│   └── 📙:B
│       └── ·78b1b59 (🏘️)
└── ≡📙:A on bce0c5e {0}
    └── 📙:A
        └── ·6fdab32 (🏘️)

"#]]
    );

    // Finally, if the extra-target, indicating an old stored base that isn't valid anymore.
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, ":/M3"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·938e6f2 (⌂|✓) ►main, ►origin/main <> origin/main
│ *    👉·e82dfab (⌂|🏘)
│ ├─╮
│ * │  ·78b1b59 (⌂|🏘) ►B
├─╯ │
*   │  ·f52fcec (⌂|🏘|✓)
│   *  ·6fdab32 (⌂|🏘) ►A
├───╯
*  ·bce0c5e (⌂|🏘|✓)
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  materialized parents: e82dfab: 78b1b59 6fdab32
  empty chain anchors: 6fdab32^ 78b1b59^
"#]]
    );

    // The base is still adjusted so it matches the actual stacks. With the extra-target
    // resolved as the target commit, the integrated `f52fcec` is at the target and is
    // pruned - consistent with the no-extra-target case above.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e
├── ≡📙:B on f52fcec {1}
│   └── 📙:B
│       └── ·78b1b59 (🏘️)
└── ≡📙:A on bce0c5e {0}
    └── 📙:A
        └── ·6fdab32 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn applied_stack_above_explicit_lower_bound() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-branches-one-above-base")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   c5587c9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * de6d39c (A) A1
| * a821094 (origin/main, main) M3
* | ce25240 (B) B1
|/  
* bce0c5e M2
* 3183e43 M1

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    meta.data_mut().default_target = None;
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·c5587c9 (⌂|🏘)
├─╮
* │  ·ce25240 (⌂|🏘) ►B
│ *  ·de6d39c (⌂|🏘) ►A
│ *  ·a821094 (⌂|🏘) ►main, ►origin/main <> origin/main
├─╯
*  ·bce0c5e (⌂|🏘)
*  🏁·3183e43 (⌂|🏘)
layout:
  materialized parents: c5587c9: ce25240 de6d39c
"#]]
    );

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    // Interestingly, A now gets to see integrated parts of the target branch.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓! on bce0c5e
├── ≡:B on bce0c5e
│   └── :B
│       └── ·ce25240 (🏘️)
└── ≡:A on bce0c5e
    ├── :A
    │   └── ·de6d39c (🏘️)
    └── :main <> origin/main
        └── ❄️a821094 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn dependent_branch_on_base() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/dependent-branch-on-base")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   a0385a8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 49d4b34 (A) A1
| |/  
|/|   
| * f9e2cb7 (C2-3, C2-2, C2-1, C) C2
| * aaa195b (C1-3, C1-2, C1-1) C1
|/  
* 3183e43 (origin/main, main, below-below-C, below-below-B, below-below-A, below-C, below-B, below-A, B) M1

"#]].raw()
    );

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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·a0385a8 (⌂|🏘)
├─┬─╮
│ * │  ·f9e2cb7 (⌂|🏘) ►C, ►C2-1, ►C2-2, ►C2-3
│ * │  ·aaa195b (⌂|🏘) ►C1-1, ►C1-2, ►C1-3
├─╯ │
│   *  ·49d4b34 (⌂|🏘) ►A
├───╯
*  🏁·3183e43 (⌂|🏘|✓) ►B, ►below-A, ►below-B, ►below-C, ►below-below-A, ►below-below-B, ►below-below-C, ►main, ►origin/main <> origin/main
layout:
  materialized parents: a0385a8: 49d4b34 3183e43 f9e2cb7
  empty chain anchors: 49d4b34^ 3183e43 f9e2cb7^
"#]]
    );

    // Both stacks will look the same, with the dependent branch inserted at the very bottom.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:B on 3183e43 {2}
│   ├── 📙:B
│   ├── 📙:below-B
│   └── 📙:below-below-B
├── ≡📙:C on 3183e43 {3}
│   ├── 📙:C
│   ├── 📙:C2-1
│   ├── 📙:C2-2
│   ├── 📙:C2-3
│   │   └── ·f9e2cb7 (🏘️)
│   ├── 📙:C1-3
│   ├── 📙:C1-2
│   ├── 📙:C1-1
│   │   └── ·aaa195b (🏘️)
│   ├── 📙:below-C
│   └── 📙:below-below-C
└── ≡📙:A on 3183e43 {1}
    ├── 📙:A
    │   └── ·49d4b34 (🏘️)
    ├── 📙:below-A
    └── 📙:below-below-A

"#]]
    );

    let wrongly_inactive = StackState::Inactive;
    add_stack_with_segments(
        &mut meta,
        1,
        "A",
        wrongly_inactive,
        &["below-A", "below-below-A"],
    );
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    // The stack-id could still be found, even though `A` is wrongly marked as outside the workspace.
    // Below A doesn't apply as it's marked inactive.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:B on 3183e43 {2}
│   ├── 📙:B
│   ├── 📙:below-B
│   └── 📙:below-below-B
├── ≡📙:C on 3183e43 {3}
│   ├── 📙:C
│   ├── 📙:C2-1
│   ├── 📙:C2-2
│   ├── 📙:C2-3
│   │   └── ·f9e2cb7 (🏘️)
│   ├── 📙:C1-3
│   ├── 📙:C1-2
│   ├── 📙:C1-1
│   │   └── ·aaa195b (🏘️)
│   ├── 📙:below-C
│   └── 📙:below-below-C
└── ≡📙:A on 3183e43 {1}
    └── 📙:A
        └── ·49d4b34 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn remote_and_integrated_tracking_branch_on_merge() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-and-integrated-tracking")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
└── ≡📙:A <> origin/A⇣1 on 1ee1e34 {1}
    └── 📙:A <> origin/A⇣1
        └── 🟣2181501

"#]]
    );

    Ok(())
}

#[test]
fn remote_and_integrated_tracking_branch_on_linear_segment() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/remote-and-integrated-tracking-linear")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 21e584f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 8dc508f (origin/main, main) M-advanced
|/  
| * 197ddce (origin/A) A-remote
|/  
* 081bae9 (A) M-base
* 3183e43 M1

"#]]
    );
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
└── ≡📙:A <> origin/A⇣1 on 081bae9 {1}
    └── 📙:A <> origin/A⇣1
        └── 🟣197ddce

"#]]
    );

    Ok(())
}

#[test]
fn remote_and_integrated_tracking_branch_on_merge_extra_target() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/remote-and-integrated-tracking-extra-commit")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
└── ≡📙:A <> origin/A⇡1⇣1 on 1ee1e34 {1}
    └── 📙:A <> origin/A⇡1⇣1
        ├── 🟣2181501
        └── ·9f47a25 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn unapplied_branch_on_base() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/unapplied-branch-on-base")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* fafd9d0 (origin/main, unapplied, main) init

"#]]
    );
    add_workspace(&mut meta);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a26ae77 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►unapplied, ►origin/main <> origin/main
layout:
  materialized parents: a26ae77: fafd9d0
"#]]
    );

    // if the branch was never seen, it's not visible as one would expect.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0

"#]]
    );

    // An applied branch would be present, but has no commit.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:unapplied on fafd9d0 {1}
    └── 📙:unapplied

"#]]
    );

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    // This will be an empty workspace.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0

"#]]
    );

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

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·20f65b7 (⌂|🏘)
*  ·4ca0966 (⌂|🏘) ►survivor
*  ·a3b180e (⌂|🏘)
*  ·ce09734 (⌂|🏘|✓) ►base-peer, ►base-peer-1, ►base-peer-2, ►base-peer-3, ►base-peer-4, ►base-peer-5, ►base-peer-6, ►base-peer-7, ►base-peer-8, ►main, ►unapplied, ►origin/HEAD, ►origin/main <> origin/main
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: 20f65b7: 4ca0966
  empty chain anchors: 4ca0966^
"#]]
    );
    let debug_graph = graph_dag(&ws);
    let target_facts = ws
        .commit_graph()
        .layout()
        .and_then(|l| l.facts_of(target_ref.as_ref()))
        .filter(|facts| facts.names_segment)
        .unwrap_or_else(|| {
            panic!(
                "expected exact target segment for existing ref {target_ref}, graph was:\n{debug_graph}"
            )
        });

    assert!(
        target_facts.names_empty_segment,
        "expected exact target segment to stay empty when the target rests on main, graph was:\n{debug_graph}"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ce09734
└── ≡📙:survivor on ce09734 {1}
    └── 📙:survivor
        ├── ·4ca0966 (🏘️)
        └── ·a3b180e (🏘️)

"#]]
    );

    assert_eq!(
        ws.target_ref.as_ref().map(|t| t.ref_name.as_ref()),
        Some(target_ref.as_ref()),
        "expected workspace target_ref to resolve from exact target segment"
    );

    // When it's applied, it will show up though.
    add_stack_with_segments(&mut meta, 2, "unapplied", StackState::InWorkspace, &[]);
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ce09734
├── ≡📙:survivor on ce09734 {1}
│   └── 📙:survivor
│       ├── ·4ca0966 (🏘️)
│       └── ·a3b180e (🏘️)
└── ≡📙:unapplied on ce09734 {2}
    └── 📙:unapplied

"#]]
    );

    Ok(())
}

#[test]
fn unapplied_branch_on_base_no_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/unapplied-branch-on-base")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* fafd9d0 (origin/main, unapplied, main) init

"#]]
    );
    add_workspace(&mut meta);
    remove_target(&mut meta);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a26ae77 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘) ►main, ►unapplied, ►origin/main <> origin/main
layout:
  materialized parents: a26ae77: fafd9d0
"#]]
    );

    // the main branch is disambiguated by its remote reference.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓!
└── ≡:main <> origin/main
    └── :main <> origin/main
        └── ❄️fafd9d0 (🏘️) ►unapplied

"#]]
    );

    // The 'unapplied' branch can be added on top of that, and we make clear we want `main` as well.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "main", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·a26ae77 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►unapplied, ►origin/main <> origin/main
layout:
  materialized parents: a26ae77: fafd9d0 fafd9d0
  empty chain anchors: fafd9d0 fafd9d0
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:unapplied on fafd9d0 {1}
│   └── 📙:unapplied
└── ≡📙:main <> origin/main on fafd9d0 {2}
    └── 📙:main <> origin/main

"#]]
    );

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    // Now only `main` shows up.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:main <> origin/main on fafd9d0 {2}
    └── 📙:main <> origin/main

"#]]
    );

    Ok(())
}

#[test]
fn no_ws_commit_two_branches_no_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-ws-ref-no-ws-commit-two-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bce0c5e (HEAD -> gitbutler/workspace, origin/main, main, B, A) M2
* 3183e43 M1

"#]]
    );
    remove_target(&mut meta);
    add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // notably the target ref and local tracking branch have sibling links setup
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉✂·bce0c5e (⌂|🏘|✓) ►A, ►B, ►main, ►origin/main <> origin/main
layout:
  empty chain anchors: bce0c5e bce0c5e
"#]]
    );
    // sibling links between origin/main and main are also set
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:main <> origin/main on bce0c5e {0}
│   └── 📙:main <> origin/main
└── ≡📙:A on bce0c5e {1}
    └── 📙:A

"#]]
    );
    Ok(())
}

#[test]
fn ambiguous_worktrees() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/ambiguous-worktrees")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·8dc508f (⌂|✓) ►main, ►origin/main <> origin/main
│ *  👉·a5f94a2 (⌂|🏘)
╭─┤
│ *  ·3e01e28 (⌂|🏘) ►B[📁wt-B-inside]
├─╯
│ *  🟣197ddce ►origin/A
├─╯
*  ·081bae9 (⌂|🏘|✓) ►A, ►A-inside[📁wt-A-inside], ►A-outside[📁wt-A-outside] <> origin/A
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  materialized parents: a5f94a2: 3e01e28 081bae9
  empty chain anchors: 081bae9
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳@repo] <> ✓refs/remotes/origin/main⇣1 on 081bae9
├── ≡📙:A <> origin/A⇣1 on 081bae9 {0}
│   └── 📙:A <> origin/A⇣1
│       └── 🟣197ddce
└── ≡:B[📁wt-B-inside] on 081bae9
    └── :B[📁wt-B-inside]
        └── ·3e01e28 (🏘️)

"#]]
    );

    let linked_repo = gix::open_opts(
        repo.path()
            .parent()
            .expect("repository git dir is inside the worktree")
            .join("wt-B-inside"),
        gix::open::Options::isolated(),
    )?
    .with_object_memory();
    let ws = Workspace::from_head(
        &linked_repo,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // when the graph is built from the B linked worktree repository, the workspace remains visible but the B worktree owns the entrypoint branch
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  ·8dc508f (⌂|✓) ►main, ►origin/main <> origin/main
│ *  ·a5f94a2 (⌂|🏘)
╭─┤
│ *  👉·3e01e28 (⌂|🏘) ►B[📁wt-B-inside@repo]
├─╯
│ *  🟣197ddce ►origin/A
├─╯
*  ·081bae9 (⌂|🏘|✓) ►A, ►A-inside[📁wt-A-inside], ►A-outside[📁wt-A-outside] <> origin/A
*  🏁·3183e43 (⌂|🏘|✓)
layout:
  empty chain anchors: 081bae9
"#]]
    );

    // workspace projection should keep the linked-worktree ownership marker on the focused stack while leaving the workspace ref itself unowned
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
├── ≡📙:A <> origin/A⇣1 on 081bae9 {0}
│   └── 📙:A <> origin/A⇣1
│       └── 🟣197ddce
└── ≡👉:B[📁wt-B-inside@repo] on 081bae9
    └── 👉:B[📁wt-B-inside@repo]
        └── ·3e01e28 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn duplicate_parent_connection_from_ws_commit_to_ambiguous_branch_no_advanced_target()
-> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/duplicate-workspace-connection-no-target")?;
    // Note that HEAD isn't actually pointing at origin/main, but twice at main
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f18d244 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
* fafd9d0 (origin/main, main, B, A) init

"#]]
        .raw()
    );

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    // Our graph is incapable of showing these two connections due to traversal
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·f18d244 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►main, ►origin/main <> origin/main
layout:
  materialized parents: f18d244: fafd9d0
  empty chain anchors: fafd9d0
"#]]
    );

    // Branch should be visible in workspace once.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:A on fafd9d0 {1}
    └── 📙:A

"#]]
    );

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:A on fafd9d0 {1}
│   └── 📙:A
└── ≡📙:B on fafd9d0 {2}
    └── 📙:B

"#]]
    );

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:A on fafd9d0 {1}
    ├── 📙:A
    └── 📙:B

"#]]
    );

    Ok(())
}

#[test]
fn duplicate_parent_connection_from_ws_commit_to_ambiguous_branch() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/duplicate-workspace-connection")?;
    // Note that HEAD isn't actually pointing at origin/main, but twice at main
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f18d244 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\
| * 12b42b0 (origin/main) RM
|/  
* fafd9d0 (main, B, A) init

"#]]
        .raw()
    );

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·f18d244 (⌂|🏘)
│ *  🟣12b42b0 (✓) ►origin/main
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►A, ►B, ►main <> origin/main
layout:
  materialized parents: f18d244: fafd9d0
  empty chain anchors: fafd9d0
"#]]
    );

    // Branch should be visible in workspace once.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
└── ≡📙:A on fafd9d0 {1}
    └── 📙:A

"#]]
    );

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
├── ≡📙:A on fafd9d0 {1}
│   └── 📙:A
└── ≡📙:B on fafd9d0 {2}
    └── 📙:B

"#]]
    );

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws.redo(&repo, &*meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
└── ≡📙:A on fafd9d0 {1}
    ├── 📙:A
    └── 📙:B

"#]]
    );

    // With extra-target these cases work as well
    meta.data_mut().branches.clear();
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
├── ≡📙:A on fafd9d0 {1}
│   └── 📙:A
└── ≡📙:B on fafd9d0 {2}
    └── 📙:B

"#]]
    );

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
└── ≡📙:A on fafd9d0 {1}
    ├── 📙:A
    └── 📙:B

"#]]
    );

    Ok(())
}

mod edit_commit {
    use but_graph::Workspace;
    use but_testsupport::{graph_dag, graph_workspace, visualize_commit_graph_all};

    use super::project_meta;
    use crate::walk::{add_workspace, id_at, read_only_in_memory_scenario, standard_options};

    #[test]
    fn applied_stack_below_explicit_lower_bound() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("ws/edit-commit/simple")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* a62b0de (A) A2
* 120a217 (gitbutler/edit) A1
* fafd9d0 (origin/main, main) init

"#]]
        );

        add_workspace(&mut meta);
        let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        snapbox::assert_data_eq!(
            graph_dag(&ws),
            snapbox::str![[r#"
*  👉·3ea2742 (⌂|🏘)
*  ·a62b0de (⌂|🏘) ►A
*  ·120a217 (⌂|🏘) ►gitbutler/edit
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 3ea2742: a62b0de
"#]]
        );

        // special branch names are skipped by default and entirely invisible.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:A on fafd9d0
    └── :A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
        );

        // However, if the HEAD points to that reference…
        let (id, ref_name) = id_at(&repo, "gitbutler/edit");
        let ws = Workspace::from_tip(
            id,
            ref_name,
            &*meta,
            project_meta(&*meta),
            standard_options(),
        )?
        .validated()?;
        snapbox::assert_data_eq!(
            graph_dag(&ws),
            snapbox::str![[r#"
*  ·3ea2742 (⌂|🏘)
*  ·a62b0de (⌂|🏘) ►A
*  👉·120a217 (⌂|🏘) ►gitbutler/edit
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
"#]]
        );
        // …then the segment becomes visible.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:A on fafd9d0
    ├── :A
    │   └── ·a62b0de (🏘️)
    └── 👉:gitbutler/edit
        └── ·120a217 (🏘️)

"#]]
        );
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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]].raw()
    );

    // Add workspace with origin/main as target (not origin/main)
    add_workspace(&mut meta);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣10 on 68e62aa
└── ≡:anon: on 68e62aa
    └── :anon:
        ├── ·4eaff93 (🏘️) ►local-stack, ►reconstructed-insert-blank-commit-branch, ►reimplement-insert-blank-commit
        ├── ·d19db1d (🏘️)
        └── ·fb0a67e (🏘️)

"#]]
    );

    // Also add the local stack as a workspace stack
    add_stack_with_segments(
        &mut meta,
        0,
        "reimplement-insert-blank-commit",
        StackState::InWorkspace,
        &["reconstructed-insert-blank-commit-branch"],
    );

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣10 on 68e62aa
└── ≡📙:reimplement-insert-blank-commit on 68e62aa {0}
    ├── 📙:reimplement-insert-blank-commit
    └── 📙:reconstructed-insert-blank-commit-branch
        ├── ·4eaff93 (🏘️) ►local-stack
        ├── ·d19db1d (🏘️)
        └── ·fb0a67e (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn reproduce_12146() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/reproduce-12146")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   d77ecda (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 7163661 (B) New commit on branch B
|/  
* 81d4e38 (A) add A
* e32cf47 (origin/main, main) add M

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·d77ecda (⌂|🏘)
├─╮
│ *  ·7163661 (⌂|🏘) ►B
├─╯
*  ·81d4e38 (⌂|🏘) ►A
*  🏁·e32cf47 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: d77ecda: 7163661 81d4e38
  empty chain anchors: 81d4e38^ 7163661^
"#]]
    );

    // The sibling ID is not set, and we see only two stacks: B owns 7163661,
    // and both A and B include the shared base commit 81d4e38 (A only has 81d4e38).
    let ws = &ws;
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e32cf47
├── ≡📙:A on e32cf47 {0}
│   └── 📙:A
│       └── ·81d4e38 (🏘️)
└── ≡📙:B on e32cf47 {1}
    └── 📙:B
        ├── ·7163661 (🏘️)
        └── ·81d4e38 (🏘️)

"#]]
    );

    Ok(())
}

/// A stack where a local merge commit at the bottom is already integrated into
/// origin/main (the same PR was merged upstream). The merge commit is kept
/// because it is above the workspace target — integrated commits are only
/// pruned at or below the target.
#[test]
fn integrated_merge_at_bottom_is_kept() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/integrated-merge-at-bottom")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 0, "local-stack", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on f5f42e0
└── ≡📙:local-stack on fafd9d0 {0}
    └── 📙:local-stack
        ├── ·66ea651 (🏘️)
        ├── ·e5a88a7 (🏘️)
        └── ·0b3ccaf (🏘️)

"#]]
    );

    Ok(())
}

/// A branch that has a commit, merges main into itself, then has another commit.
/// The fork-point approach finds the original divergence point, so all branch
/// commits (including those below the merge-from-main) remain visible.
#[test]
fn merge_from_main_keeps_all_branch_commits() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/merge-from-main-in-branch")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 891e228 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* cd76046 (my-branch) branch-commit-2
*   f8ff9a3 Merge main into my-branch
|\  
| * ef56fab (origin/main, main) main-advance
* | 6f65768 branch-commit-1
|/  
* fafd9d0 init

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 0, "my-branch", StackState::InWorkspace, &[]);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·891e228 (⌂|🏘)
*  ·cd76046 (⌂|🏘) ►my-branch
*    ·f8ff9a3 (⌂|🏘)
├─╮
* │  ·6f65768 (⌂|🏘)
│ *  ·ef56fab (⌂|🏘|✓) ►main, ►origin/main <> origin/main
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓)
layout:
  materialized parents: 891e228: cd76046
  empty chain anchors: cd76046^
"#]]
    );

    // The fork-point approach correctly finds the original divergence point (fafd9d0)
    // instead of the moved merge base (ef56fab), so all 3 branch commits are visible:
    // branch-commit-2, the merge commit, and branch-commit-1.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ef56fab
└── ≡📙:my-branch on fafd9d0 {0}
    └── 📙:my-branch
        ├── ·cd76046 (🏘️)
        ├── ·f8ff9a3 (🏘️)
        └── ·6f65768 (🏘️)

"#]]
    );

    Ok(())
}

/// A branch whose commits are integrated (reachable from origin/main after
/// upstream merged them) but the workspace target hasn't advanced yet.
/// Integrated commits above the target must be kept so `integrate_upstream`
/// can detect them. Once the target advances past them, they are pruned.
#[test]
fn integrated_commits_above_target_are_kept() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/integrated-above-target")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 7786959 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| *   1af5972 (origin/main, main) Merge branch my-branch
| |\  
| |/  
|/|   
* | 312f819 (my-branch) B
* | e255adc A
|/  
* fafd9d0 init

"#]]
        .raw()
    );

    let init_id = repo.rev_parse_single("main~1")?.detach();
    add_workspace_with_target(&mut meta, init_id);
    add_stack_with_segments(&mut meta, 0, "my-branch", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    // With the target at "init", A and B are above the target and should be
    // kept even though they are marked integrated.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
└── ≡📙:my-branch on fafd9d0 {0}
    └── 📙:my-branch
        ├── ·312f819 (🏘️|✓)
        └── ·e255adc (🏘️|✓)

"#]]
    );

    // Now advance the target to origin/main (which includes the merge).
    // Both commits are at or below the new target and should be pruned,
    // but the metadata-tracked branch entry is preserved.
    let main_id = repo.rev_parse_single("main")?.detach();
    add_workspace_with_target(&mut meta, main_id);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 312f819
└── ≡📙:my-branch on 312f819 {0}
    └── 📙:my-branch

"#]]
    );

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_hard_limit(usize::MAX),
    )?
    .validated()?;
    assert!(
        !ws.hard_limit_hit(),
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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    // Stored target is 'target' (main~1); origin/main is one commit ahead at 'upstream'.
    let target_id = repo.rev_parse_single("main~1")?.detach();
    add_workspace_with_target(&mut meta, target_id);
    add_stack_with_segments(&mut meta, 0, "my-branch", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "old-branch", StackState::InWorkspace, &[]);

    // 'W' and 'O' are above/beside the target and kept; 'target' and 'base' are
    // integrated and at or below the target, so they are pruned from both stacks
    // even though origin/main has advanced past the target.
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 322cb14
├── ≡📙:my-branch on 2121f9c {0}
│   └── 📙:my-branch
│       └── ·f5055a1 (🏘️)
└── ≡📙:old-branch on 322cb14 {1}
    └── 📙:old-branch
        └── ·f458f7d (🏘️)

"#]]
    );
    Ok(())
}

/// A branch that forks below the target and catches up via `merge origin/main`, so the
/// target enters X only through the merge's second parent (off X's first-parent spine).
/// X is floored at its fork point - where its own first-parent work meets the trunk - so
/// the trunk below the fork (`c1`, `init`) is pruned, leaving X's own commits.
#[test]
fn catchup_merge_below_target_floors_at_fork() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/catchup-merge-leak")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    // Stored target is 'T' (main~2); origin/main is two commits ahead at 'U'.
    let target_id = repo.rev_parse_single("main~2")?.detach();
    add_workspace_with_target(&mut meta, target_id);
    add_stack_with_segments(&mut meta, 0, "X", StackState::InWorkspace, &[]);

    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on d263f88
└── ≡📙:X on b4bd43f {0}
    └── 📙:X
        ├── ·f210f41 (🏘️)
        ├── ·f8cd0ce (🏘️)
        └── ·4eec82a (🏘️)

"#]]
    );
    Ok(())
}

/// A non-workspace ref (tag) points at the workspace commit itself,
/// and that ref is used as the entrypoint for traversal.
/// This verifies that the entrypoint is correctly identified even when it
/// coincides with the workspace commit.
#[test]
fn entrypoint_on_workspace_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/entrypoint-on-workspace-commit")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3ea2742 (HEAD -> gitbutler/workspace, tag: my-tag) GitButler Workspace Commit
* a62b0de (A) A2
* 120a217 A1
* fafd9d0 (origin/main, main) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·3ea2742 (⌂|🏘) ►tags/my-tag
*  ·a62b0de (⌂|🏘) ►A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 3ea2742: a62b0de
"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:A on fafd9d0
    └── :A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
    );

    // Now traverse from the tag that points at the workspace commit.
    let (id, name) = id_at(&repo, "my-tag");
    let ws = Workspace::from_tip(id, name, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·3ea2742 (⌂|🏘) ►tags/my-tag
*  ·a62b0de (⌂|🏘) ►A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 3ea2742: a62b0de
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:A on fafd9d0
    └── :A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
    );
    Ok(())
}

/// A workspace where the local branch was deleted, leaving only origin/A.
/// The workspace commit still references the old branch tip as a parent.
/// This probes whether a remote-only segment at the top of a stack is handled
/// correctly (previously protected by front-pruning workaround).
#[test]
fn remote_only_stack_top() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-only-stack-top")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* a62b0de (origin/A) A2
* 120a217 A1
* fafd9d0 (origin/main, main) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  👉·3ea2742 (⌂|🏘)
*  ·a62b0de (⌂|🏘) ►origin/A
*  ·120a217 (⌂|🏘)
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 3ea2742: a62b0de
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:anon: on fafd9d0
    └── :anon:
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
    );
    Ok(())
}

/// A local branch B is stacked on top of a remote-only origin/A (no local A).
/// origin/A's commits are on the first-parent path between B and main.
/// This probes whether a remote-only segment appearing after a local segment
/// in a stack is handled correctly (previously protected by tail-pruning workaround).
#[test]
fn remote_trailing_local_stack() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-trailing-local-stack")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 5638b41 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* cb7021b (B) B2
* ce3278a B1
* a62b0de (origin/A) A2
* 120a217 A1
* fafd9d0 (origin/main, main) init

"#]]
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*  🏁·fafd9d0 (⌂|✓) ►main, ►origin/main <> origin/main
*  👉·5638b41 (⌂|🏘)
*  ·cb7021b (⌂|🏘) ►B
*  🏁·ce3278a (⌂|🏘)
layout:
  materialized parents: 5638b41: cb7021b
"#]]
    );
    // this is a weird state as the target is actually disjoint from the workspace - it appears empty now
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on cb7021b

"#]]
    );
    Ok(())
}

/// A workspace that merges a remote-only branch (origin/A) with no local counterpart.
/// Unlike `remote_only_stack_top` where the local was deleted after workspace creation,
/// here the local never existed. This tests whether the `is_pruned` check correctly
/// handles a stack that starts with a remote-only segment.
#[test]
fn remote_ref_as_stack_top() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/remote-ref-as-stack-top")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   21bff1f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * a62b0de (origin/A) A2
| * 120a217 A1
|/  
* fafd9d0 (origin/main, main) init

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
        .validated()?;
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*    👉·21bff1f (⌂|🏘)
├─╮
│ *  ·a62b0de (⌂|🏘) ►origin/A
│ *  ·120a217 (⌂|🏘)
├─╯
*  🏁·fafd9d0 (⌂|🏘|✓) ►main, ►origin/main <> origin/main
layout:
  materialized parents: 21bff1f: fafd9d0 a62b0de
"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:anon: on fafd9d0
    └── :anon:
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_stack_branch_on_same_commit() -> anyhow::Result<()> {
    // The workspace ref rests on the same commit as a metadata stack branch, without a managed
    // workspace commit. The stack branch names the traversal segment, dropping the special
    // workspace ref from the commit's refs — the build must still splice the empty workspace
    // segment (it used to skip it, making the projection fail to find the workspace upstream).
    let (repo, mut meta) = read_only_in_memory_scenario("ws/just-init-with-branches")?;
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    let ws_ref: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let ws_tip = repo.find_reference(ws_ref.as_ref())?.peel_to_id()?;
    let ws = Workspace::from_tip(
        ws_tip,
        ws_ref,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace <> ✓!
└── ≡📙:A {0}
    └── 📙:A
        └── ·fafd9d0 (🏘️) ►B, ►C, ►D, ►E, ►F, ►main[🌳]

"#]]
    );
    Ok(())
}

mod applied_main {
    //! The applied-main corner specs (see graph-unify-plan.md "MAIN AS AN ORDINARY BRANCH"):
    //! what the projection currently says when metadata declares the target's LOCAL tracking
    //! branch as a workspace stack. These renders are the baseline for lifting the
    //! target-local apply-blocker — behavior changes must show up here first.
    use super::*;

    /// (a) main rests at the workspace base and is not a workspace-commit parent:
    /// membership comes from metadata alone, via the empty-lane machinery.
    #[test]
    fn at_base() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("ws/applied-main-at-base")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   5edc691 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f57c528 (B) B1
* | 49d4b34 (A) A1
|/  
* 3183e43 (origin/main, main) M1

"#]]
            .raw()
        );

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 2, "main", StackState::InWorkspace, &[]);

        let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let ws = &ws;
        snapbox::assert_data_eq!(
            graph_workspace(ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:A on 3183e43 {0}
│   └── 📙:A
│       └── ·49d4b34 (🏘️)
├── ≡📙:B on 3183e43 {1}
│   └── 📙:B
│       └── ·f57c528 (🏘️)
└── ≡📙:main <> origin/main on 3183e43 {2}
    └── 📙:main <> origin/main

"#]]
        );
        Ok(())
    }

    /// (b) main has its own commit ahead of origin/main and is the workspace commit's first
    /// parent — a lane with commits, ahead of its remote like any branch.
    #[test]
    fn ahead_of_remote() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("ws/applied-main-ahead")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   e8484be (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 49d4b34 (A) A1
* | bce0c5e (main) M2
|/  
* 3183e43 (origin/main) M1

"#]]
            .raw()
        );

        add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

        let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let ws = &ws;
        snapbox::assert_data_eq!(
            graph_workspace(ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:main <> origin/main⇡1 on 3183e43 {0}
│   └── 📙:main <> origin/main⇡1
│       └── ·bce0c5e (🏘️)
└── ≡📙:A on 3183e43 {1}
    └── 📙:A
        └── ·49d4b34 (🏘️)

"#]]
        );
        Ok(())
    }

    /// (c) main is a workspace-commit parent at the base while origin/main moved ahead:
    /// the applied lane is behind its remote.
    #[test]
    fn behind_remote() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("ws/applied-main-behind")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   1943cdc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 49d4b34 (A) A1
|/  
| * 73c46a6 (origin/main) RM1
|/  
* 3183e43 (main) M1

"#]]
            .raw()
        );

        add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

        let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let ws = &ws;
        snapbox::assert_data_eq!(
            graph_workspace(ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡📙:main <> origin/main⇣1 on 3183e43 {0}
│   └── 📙:main <> origin/main⇣1
│       └── 🟣73c46a6 (✓)
└── ≡📙:A on 3183e43 {1}
    └── 📙:A
        └── ·49d4b34 (🏘️)

"#]]
        );
        Ok(())
    }

    /// (d) main (and its remote) advanced above A's fork point: the stale-fork corner.
    ///
    /// RULING (Mattias, 2026-07-04): the target's local is exempt from integrated pruning when
    /// metadata applies it as a lane — caught up with the target, ALL its commits are
    /// integrated by definition, so pruning would empty the lane and slide its base to the
    /// workspace lower bound. The applied lane keeps its commits: it IS the base indicator,
    /// and its base stays correct by construction.
    #[test]
    fn above_stack_fork_point() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("ws/applied-main-above-fork")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   e8484be (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 49d4b34 (A) A1
* | bce0c5e (origin/main, main) M2
|/  
* 3183e43 M1

"#]]
            .raw()
        );

        add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);

        let ws = Workspace::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let ws = &ws;
        snapbox::assert_data_eq!(
            graph_workspace(ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:main <> origin/main on 3183e43 {0}
│   └── 📙:main <> origin/main
│       └── ❄️bce0c5e (🏘️|✓)
└── ≡📙:A on 3183e43 {1}
    └── 📙:A
        └── ·49d4b34 (🏘️)

"#]]
        );
        Ok(())
    }
}
