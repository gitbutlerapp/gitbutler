use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
    ref_metadata::{
        ProjectMeta, StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_graph::{
    Graph, NodeGraphEntrypoint, NodeKind, ReferenceMetadata,
    init::{Overlay, Tip, TipRole},
    workspace::WorkspaceKind,
};
use but_testsupport::{InMemoryRefMetadata, graph_workspace, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::init::{
    StackState, add_stack_with_segments, add_workspace, id_at, id_by_rev,
    read_only_in_memory_scenario, standard_options,
    utils::{
        add_stack, add_workspace_with_target, add_workspace_without_target,
        named_read_only_in_memory_scenario, remove_target, standard_options_with_extra_target,
    },
};
use but_testsupport::graph_tree;

fn project_meta(meta: &impl RefMetadata) -> ProjectMeta {
    meta.workspace(WORKSPACE_REF_NAME.try_into().expect("valid workspace ref"))
        .map(|workspace| workspace.project_meta())
        .unwrap_or_default()
}

fn assert_ad_hoc_entrypoint(workspace: &but_graph::Workspace, expected_ref: Option<&str>) {
    assert!(
        matches!(&workspace.kind, WorkspaceKind::AdHoc),
        "an entrypoint outside stored target membership stays ad hoc"
    );
    assert_eq!(
        workspace.ref_name().map(ToString::to_string),
        expected_ref.map(str::to_owned),
        "the ad-hoc projection preserves the requested entrypoint"
    );
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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●    ·59a427f (⌂|🏘)
├─╮
◎ │  main <> origin/main
● │  ·0a415d8 (⌂|🏘)
│ ◎  A
│ ●  ·a62b0de (⌂|🏘)
│ ●  ·120a217 (⌂|🏘)
│ │ ◎  origin/main
│ │ ●  🟣1f5c47b
├───╯
● │  ·73ba99d (⌂|🏘)
├─╯
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    assert_eq!(
        graph.managed_workspace_commit_id(),
        Some(repo.rev_parse_single("gitbutler/workspace")?.detach()),
        "the workspace commit is recorded in construction context"
    );

    // It's perfectly valid to have the local tracking branch of our target in the workspace,
    // and the low-bound computation works as well.
    let ws = &graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on fafd9d0
├── ≡:8:main <> origin/main →:10:⇡1⇣1 on 73ba99d
│   └── :8:main <> origin/main →:10:⇡1⇣1
│       ├── 🟣1f5c47b ►origin/main
│       └── ·0a415d8 (🏘️)
└── ≡:9:A on fafd9d0
    └── :9:A
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·e5e2623 (⌂|🏘)
│ ◎  main <> origin/main
├─╯
│ ◎  origin/main
│ ●  🟣cb54dca (✓)
├─╯
●  ·0a415d8 (⌂|🏘|✓)
●  ·73ba99d (⌂|🏘|✓)
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    let ws = &graph.into_workspace()?;
    // It's notable how the local tracking branch of our target (origin/main) is ignored, it's not part of our workspace,
    // but acts as base.
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 0a415d8

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let ws = &graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:9:A on 3183e43 {1}
│   └── 📙:9:A
│       └── ·7236012 (🏘️)
└── ≡📙:8:B on 3183e43 {2}
    ├── 📙:8:B
    │   └── ·68c8a9d (🏘️)
    └── 📙:4:below

"#]]
    );

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["below"]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:9:A on 3183e43 {1}
│   ├── 📙:9:A
│   │   └── ·7236012 (🏘️)
│   └── 📙:4:below
└── ≡📙:8:B on 3183e43 {2}
    └── 📙:8:B
        └── ·68c8a9d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn workspace_projection_with_advanced_stack_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/advanced-stack-tip-outside-workspace")?;
    let stack_id = add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📙B
●  ·cc0bf57 (⌂)
│ ◎  👉📕gitbutler/workspace[🌳]
│ ●  ·2076060 (⌂|🏘)
├─╯
●  ·d69fe94 (⌂|🏘)
◎  📙A
●  ·09d8e52 (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·85efbe4 (⌂|🏘|✓)

"#]]
    );
    let ws = &graph.into_workspace()?;
    assert_eq!(
        ws.stacks.iter().map(|stack| stack.id).collect::<Vec<_>>(),
        [Some(stack_id)],
        "the advanced B ref is reconciled with its reachable A base instead of duplicated"
    );
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:8:B →:2: on 85efbe4 {1}
    ├── 📙:8:B →:2:
    │   ├── ·cc0bf57*
    │   └── ·d69fe94 (🏘️)
    └── 📙:9:A
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

    let x_stack = add_stack_with_segments(&mut meta, 1, "X", StackState::InWorkspace, &[]);
    let feat_stack = add_stack_with_segments(&mut meta, 2, "feat-2", StackState::InWorkspace, &[]);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let ws = graph.into_workspace()?;
    let projected_stack_ids = ws
        .stacks
        .iter()
        .filter_map(|stack| stack.id)
        .collect::<Vec<_>>();
    assert!(projected_stack_ids.contains(&x_stack));
    assert!(!projected_stack_ids.contains(&feat_stack));
    assert_eq!(
        ws.stacks.len(),
        2,
        "both managed commit parents are visible"
    );
    let anonymous = ws
        .stacks
        .iter()
        .find(|stack| stack.id.is_none())
        .expect("the real W parent isn't assigned stale feat-2 metadata");
    assert_eq!(
        anonymous.tip_skip_empty(),
        Some(id_by_rev(&repo, ":/W2").detach())
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:15:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 3183e43
├── ≡📙:18:X <> origin/X →:19:⇡1 on 3183e43 {1}
│   ├── 📙:18:X <> origin/X →:19:⇡1
│   │   └── ·0b203b5 (🏘️)
│   └── :19:origin/X →:18:
│       └── ❄4840f3b (🏘️)
└── ≡:3:anon on a821094
    └── :3:anon
        ├── ·835086d (🏘️) ►four, ►three
        └── ·ff310d3 (🏘️)

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  A-empty-01
├─╯
│ ◎  A-empty-02
├─╯
│ ◎  A-empty-03
├─╯
│ ◎  B-empty
│ │ ◎  ambiguous-01
│ ├─╯
│ │ ◎  👉📕gitbutler/workspace[🌳]
│ │ ●  ·20de6ee (⌂|🏘)
│ │ ◎  B <> origin/B
│ │ ●  ·70e9a36 (⌂|🏘)
│ │ │ ◎  new-A
│ │ │ │ ◎  new-B
│ │ │ ├─╯
│ │ │ │ ◎  origin/B
├───────╯
│ │ │ │ ◎  origin/main
│ │ │ │ ◎  main <> origin/main
│ │ │ ├─╯
│ │ │ │ ◎  tags/without-ref
│ │ ├───╯
│ │ ● │  ·320e105 (⌂|🏘)
│ ├─╯ │
│ ●   │  ·2a31450 (⌂|🏘)
├─╯   │
●     │  ·70bde6b (⌂|🏘)
├─────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // All non-integrated segments are visible.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:11:B <> origin/B →:19:⇡3 on fafd9d0
    └── :11:B <> origin/B →:19:⇡3
        ├── ·70e9a36 (🏘️)
        ├── ·320e105 (🏘️) ►without-ref
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B

"#]]
    );

    // There is always a segment for the entrypoint, and code working with the graph
    // deals with that naturally.
    let (without_ref_id, ref_name) = id_at(&repo, "without-ref");
    let graph = Graph::from_commit_traversal(
        without_ref_id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // See how tags ARE allowed to name a segment, at least when used as entrypoint.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  A-empty-01
├─╯
│ ◎  A-empty-02
├─╯
│ ◎  A-empty-03
├─╯
│ ◎  B-empty
│ │ ◎  ambiguous-01
│ ├─╯
│ │ ◎  📕gitbutler/workspace[🌳]
│ │ ●  ·20de6ee (⌂|🏘)
│ │ ◎  B <> origin/B
│ │ ●  ·70e9a36 (⌂|🏘)
│ │ │ ◎  new-A
│ │ │ │ ◎  new-B
│ │ │ ├─╯
│ │ │ │ ◎  origin/B
├───────╯
│ │ │ │ ◎  origin/main
│ │ │ │ ◎  main <> origin/main
│ │ │ ├─╯
│ │ │ │ ◎  👉tags/without-ref
│ │ ├───╯
│ │ ● │  ·320e105 (⌂|🏘)
│ ├─╯ │
│ ●   │  ·2a31450 (⌂|🏘)
├─╯   │
●     │  ·70bde6b (⌂|🏘)
├─────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    // Now `HEAD` is outside a workspace, which goes to single-branch mode. But it knows it's in a workspace
    // and shows the surrounding parts, while marking the segment as entrypoint.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:11:B <> origin/B →:19:⇡3 on fafd9d0
    └── :11:B <> origin/B →:19:⇡3
        ├── ·70e9a36 (🏘️)
        ├── ·320e105 (🏘️) ►without-ref
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B

"#]]
    );

    // We don't have to give it a ref-name
    let graph = Graph::from_commit_traversal(
        without_ref_id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  A-empty-01
├─╯
│ ◎  A-empty-02
├─╯
│ ◎  A-empty-03
├─╯
│ ◎  B-empty
│ │ ◎  ambiguous-01
│ ├─╯
│ │ ◎  📕gitbutler/workspace[🌳]
│ │ ●  ·20de6ee (⌂|🏘)
│ │ ◎  B <> origin/B
│ │ ●  ·70e9a36 (⌂|🏘)
│ │ │ ◎  new-A
│ │ │ │ ◎  new-B
│ │ │ ├─╯
│ │ │ │ ◎  origin/B
├───────╯
│ │ │ │ ◎  origin/main
│ │ │ │ ◎  main <> origin/main
│ │ │ ├─╯
│ │ │ │ ◎  tags/without-ref
│ │ ├───╯
│ │ ● │  👉·320e105 (⌂|🏘)
│ ├─╯ │
│ ●   │  ·2a31450 (⌂|🏘)
├─╯   │
●     │  ·70bde6b (⌂|🏘)
├─────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // Entrypoint is now unnamed (as no ref-name was provided for traversal)
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:11:B <> origin/B →:19:⇡3 on fafd9d0
    └── :11:B <> origin/B →:19:⇡3
        ├── ·70e9a36 (🏘️)
        ├── ·320e105 (🏘️) ►without-ref
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B

"#]]
    );

    // Putting the entrypoint onto a commit in an anonymous segment with ambiguous refs makes no difference.
    let (b_id_1, tag_ref_name) = id_at(&repo, "B-empty");
    let graph = Graph::from_commit_traversal(
        b_id_1,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  A-empty-01
├─╯
│ ◎  A-empty-02
├─╯
│ ◎  A-empty-03
├─╯
│ ◎  B-empty
│ │ ◎  ambiguous-01
│ ├─╯
│ │ ◎  📕gitbutler/workspace[🌳]
│ │ ●  ·20de6ee (⌂|🏘)
│ │ ◎  B <> origin/B
│ │ ●  ·70e9a36 (⌂|🏘)
│ │ │ ◎  new-A
│ │ │ │ ◎  new-B
│ │ │ ├─╯
│ │ │ │ ◎  origin/B
├───────╯
│ │ │ │ ◎  origin/main
│ │ │ │ ◎  main <> origin/main
│ │ │ ├─╯
│ │ │ │ ◎  tags/without-ref
│ │ ├───╯
│ │ ● │  ·320e105 (⌂|🏘)
│ ├─╯ │
│ ●   │  👉·2a31450 (⌂|🏘)
├─╯   │
●     │  ·70bde6b (⌂|🏘)
├─────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // Doing this is very much like edit mode, and there is always a segment starting at the entrypoint.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:11:B <> origin/B →:18:⇡3 on fafd9d0
    └── :11:B <> origin/B →:18:⇡3
        ├── ·70e9a36 (🏘️)
        ├── ·320e105 (🏘️) ►without-ref
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B

"#]]
    );

    // If we pass an entrypoint ref name, it will be used as segment name (despite being ambiguous without it)
    let graph = Graph::from_commit_traversal(
        b_id_1,
        tag_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  A-empty-01
├─╯
│ ◎  A-empty-02
├─╯
│ ◎  A-empty-03
├─╯
│ ◎  👉B-empty
│ │ ◎  ambiguous-01
│ ├─╯
│ │ ◎  📕gitbutler/workspace[🌳]
│ │ ●  ·20de6ee (⌂|🏘)
│ │ ◎  B <> origin/B
│ │ ●  ·70e9a36 (⌂|🏘)
│ │ │ ◎  new-A
│ │ │ │ ◎  new-B
│ │ │ ├─╯
│ │ │ │ ◎  origin/B
├───────╯
│ │ │ │ ◎  origin/main
│ │ │ │ ◎  main <> origin/main
│ │ │ ├─╯
│ │ │ │ ◎  tags/without-ref
│ │ ├───╯
│ │ ● │  ·320e105 (⌂|🏘)
│ ├─╯ │
│ ●   │  ·2a31450 (⌂|🏘)
├─╯   │
●     │  ·70bde6b (⌂|🏘)
├─────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:11:B <> origin/B →:18:⇡3 on fafd9d0
    └── :11:B <> origin/B →:18:⇡3
        ├── ·70e9a36 (🏘️)
        ├── ·320e105 (🏘️) ►without-ref
        ├── ·2a31450 (🏘️) ►B-empty, ►ambiguous-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A-empty-02
│ ◎  📙B <> origin/B
│ │ ◎  ambiguous-01
│ │ │ ◎    👉📕gitbutler/workspace[🌳]
│ │ │ ├─╮
│ │ │ │ ●  ·20de6ee (⌂|🏘)
│ ├─────╯
│ ● │ │  ·70e9a36 (⌂|🏘)
│ │ │ │ ◎  new-A
│ │ │ │ │ ◎  new-B
│ │ │ │ ├─╯
│ │ │ │ │ ◎  origin/B
├─────────╯
│ │ │ │ │ ◎  origin/main
│ │ │ │ │ ◎  main <> origin/main
│ │ │ │ ├─╯
│ │ │ │ │ ◎  tags/without-ref
│ ├───────╯
│ ● │ │ │  ·320e105 (⌂|🏘)
│ ├───╯ │
│ ◎ │   │  📙B-empty
│ ├─╯   │
│ ●     │  ·2a31450 (⌂|🏘)
│ ◎     │  📙A-empty-03
│ ◎     │  📙A-empty-01
│ ◎     │  📙A
├─╯     │
●       │  ·70bde6b (⌂|🏘)
├───────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // We pickup empty segments.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:11:B <> origin/B →:18:⇡2 on fafd9d0 {0}
    ├── 📙:11:B <> origin/B →:18:⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►without-ref
    ├── 📙:12:B-empty
    │   └── ·2a31450 (🏘️) ►ambiguous-01
    ├── 📙:17:A-empty-03
    ├── 📙:15:A-empty-01
    └── 📙:14:A
        └── ❄70bde6b (🏘️) ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B

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

    let graph = Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📙B <> origin/B
│ ◎  ambiguous-01
│ │ ◎    👉📕gitbutler/workspace[🌳]
│ │ ├─╮
│ │ │ ●  ·20de6ee (⌂|🏘)
├─────╯
● │ │  ·70e9a36 (⌂|🏘)
│ │ │ ◎  new-A
│ │ │ │ ◎  new-B
│ │ │ ├─╯
│ │ │ │ ◎  origin/B
│ │ │ │ │ ◎  origin/main
│ │ │ │ │ ◎  main <> origin/main
│ │ │ ├───╯
│ │ │ │ │ ◎  tags/without-ref
├─────────╯
● │ │ │ │  ·320e105 (⌂|🏘)
├───╯ │ │
◎ │   │ │  📙B-empty
├─╯   │ │
●     │ │  ·2a31450 (⌂|🏘)
◎     │ │  📙A-empty-03
◎     │ │  📙A-empty-02
◎     │ │  📙A-empty-01
◎     │ │  📙A
├───────╯
●     │  ·70bde6b (⌂|🏘)
├─────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:11:B <> origin/B →:18:⇡2 on fafd9d0 {0}
    ├── 📙:11:B <> origin/B →:18:⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►without-ref
    ├── 📙:12:B-empty
    │   └── ·2a31450 (🏘️) ►ambiguous-01
    ├── 📙:17:A-empty-03
    ├── 📙:16:A-empty-02
    ├── 📙:15:A-empty-01
    └── 📙:14:A
        └── ❄70bde6b (🏘️) ►A-empty-01, ►A-empty-02, ►A-empty-03, ►origin/B

"#]]
    );

    // Define only some of the branches, it should figure that out.
    // It respects the order of the mention in the stack, `A` before `A-empty-01`.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-empty-01"]);
    add_stack_with_segments(&mut meta, 1, "B-empty", StackState::InWorkspace, &["B"]);

    let (id, ref_name) = id_at(&repo, "A-empty-01");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A-empty-02
│ ◎  A-empty-03
├─╯
│ ◎  📙B <> origin/B
│ │ ◎  ambiguous-01
│ │ │ ◎    📕gitbutler/workspace[🌳]
│ │ │ ├─╮
│ │ │ │ ●  ·20de6ee (⌂|🏘)
│ ├─────╯
│ ● │ │  ·70e9a36 (⌂|🏘)
│ │ │ │ ◎  new-A
│ │ │ │ │ ◎  new-B
│ │ │ │ ├─╯
│ │ │ │ │ ◎  origin/B
├─────────╯
│ │ │ │ │ ◎  origin/main
│ │ │ │ │ ◎  main <> origin/main
│ │ │ │ ├─╯
│ │ │ │ │ ◎  tags/without-ref
│ ├───────╯
│ ● │ │ │  ·320e105 (⌂|🏘)
│ ├───╯ │
│ ◎ │   │  📙B-empty
│ ├─╯   │
│ ●     │  ·2a31450 (⌂|🏘)
│ ◎     │  📙A
│ ◎     │  👉📙A-empty-01
├─╯     │
●       │  ·70bde6b (⌂|🏘)
├───────╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:11:B <> origin/B →:16:⇡2 on fafd9d0 {0}
    ├── 📙:11:B <> origin/B →:16:⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►without-ref
    ├── 📙:17:B-empty
    │   └── ·2a31450 (🏘️) ►ambiguous-01
    ├── 📙:12:A
    └── 👉📙:13:A-empty-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-02, ►A-empty-03, ►origin/B

"#]]
    );

    add_stack_with_segments(&mut meta, 2, "new-A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "new-B", StackState::InWorkspace, &[]);

    let (id, ref_name) = id_at(&repo, "new-A");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?;

    // We can also summon new empty stacks from branches resting on the base, and set them
    // as entrypoint, to have two more stacks.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:11:B <> origin/B →:16:⇡2 on fafd9d0 {0}
    ├── 📙:11:B <> origin/B →:16:⇡2
    │   ├── ·70e9a36 (🏘️)
    │   └── ·320e105 (🏘️) ►without-ref
    ├── 📙:17:B-empty
    │   └── ·2a31450 (🏘️) ►ambiguous-01
    ├── 📙:12:A
    └── 📙:13:A-empty-01
        └── ❄70bde6b (🏘️) ►A, ►A-empty-02, ►A-empty-03, ►origin/B

"#]]
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·2c12d75 (⌂|🏘)
◎  B
●  ·320e105 (⌂|🏘)
◎  B-sub
●  ·2a31450 (⌂|🏘)
◎  A
●  ·70bde6b (⌂|🏘)
│ ◎  new-A
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:9:B on fafd9d0
    ├── :9:B
    │   └── ·320e105 (🏘️)
    ├── :10:B-sub
    │   └── ·2a31450 (🏘️)
    └── :11:A
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·2c12d75 (⌂|🏘)
├─╯
◎  📙B
●  ·320e105 (⌂|🏘)
◎  📙B-sub
●  ·2a31450 (⌂|🏘)
◎  📙A
●  ·70bde6b (⌂|🏘)
│ ◎  📙new-A
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:9:B on fafd9d0 {0}
    ├── 📙:9:B
    │   └── ·320e105 (🏘️)
    ├── 📙:10:B-sub
    │   └── ·2a31450 (🏘️)
    └── 📙:11:A
        └── ·70bde6b (🏘️)

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
└── ≡📙:12:C on 0cc5a6f {0}
    ├── 📙:12:C
    │   └── ·c6d714c (🏘️)
    └── 📙:7:merge

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

    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
└── ≡📙:12:C on 0cc5a6f {0}
    └── 📙:12:C
        └── ·c6d714c (🏘️)

"#]]
    );

    // Finally, when the 'merge' branch is independent, it still works as it should.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "merge", StackState::InWorkspace, &[]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.stacks
            .iter()
            .filter_map(|stack| stack.segments.first()?.ref_name())
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["refs/heads/C", "refs/heads/merge"],
        "independent empty branches at the target tip survive in metadata order"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
├── ≡📙:12:C on 0cc5a6f {0}
│   └── 📙:12:C
│       └── ·c6d714c (🏘️)
└── ≡📙:7:merge on 0cc5a6f {1}
    └── 📙:7:merge

"#]]
    );

    // The order is respected.
    add_stack_with_segments(&mut meta, 1, "C", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 0, "merge", StackState::InWorkspace, &[]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.stacks
            .iter()
            .filter_map(|stack| stack.segments.first()?.ref_name())
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["refs/heads/merge", "refs/heads/C"],
        "reversed metadata order is preserved for an empty target-tip branch"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0cc5a6f
├── ≡📙:7:merge on 0cc5a6f {0}
│   └── 📙:7:merge
└── ≡📙:12:C on 0cc5a6f {1}
    └── 📙:12:C
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉gitbutler/workspace[🌳]
●  ·47e1cf1 (⌂)
●    ·f40fb16 (⌂)
├─╮
● │  ·450c58a (⌂)
│ ●  ·c6d714c (⌂)
├─╯
●    ·0cc5a6f (⌂)
├─╮
● │  ·7fdb58d (⌂)
│ ●  ·e255adc (⌂)
├─╯
●  🏁·fafd9d0 (⌂)

"#]]
    );

    // This a very untypical setup, but it's not forbidden. Code might want to check
    // if the workspace commit is actually managed before proceeding.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:8:gitbutler/workspace <> ✓!
└── ≡:0:anon {1}
    └── :0:anon
        ├── ·47e1cf1 ►gitbutler/workspace[🌳]
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
    let graph =
        Graph::from_commit_traversal(id, name, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉entrypoint
●  ·98c5aba (⌂)
●  ·807b6ce (⌂)
●  ·6d05486 (⌂)
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·b6917c7 (⌂|🏘)
│ ◎  main
│ ●  ·f7fe830 (⌂|🏘)
├─╯
●  ·b688f2d (⌂|🏘)
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    // This is an unmanaged workspace, even though commits from a workspace flow into it.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:9:entrypoint <> ✓!
└── ≡👉:9:entrypoint {1}
    └── 👉:9:entrypoint
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  empty-1-on-merge
│ ◎  empty-2-on-merge
├─╯
│ ◎  👉gitbutler/workspace[🌳]
│ ●  ·47e1cf1 (⌂)
│ ◎  merge-2
│ ●    ·f40fb16 (⌂)
│ ├─╮
│ ◎ │  D
│ ● │  ·450c58a (⌂)
├─╯ │
│   ◎  C
│   ●  ·c6d714c (⌂)
├───╯
│ ◎  merge
├─╯
●    ·0cc5a6f (⌂)
├─╮
◎ │  B
● │  ·7fdb58d (⌂)
│ ◎  A
│ ●  ·e255adc (⌂)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂)

"#]]
    );

    // Without workspace data this becomes a single-branch workspace, with `main` as normal segment.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:8:gitbutler/workspace <> ✓!
└── ≡:0:anon {1}
    ├── :0:anon
    │   └── ·47e1cf1 ►gitbutler/workspace[🌳]
    ├── :9:merge-2
    │   └── ·f40fb16
    ├── :10:D
    │   ├── ·450c58a
    │   └── ·0cc5a6f ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    └── :15:B
        ├── ·7fdb58d
        └── ·fafd9d0 ►main, ►origin/main

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
◎ │  📙empty-2-on-merge
◎ │  📙empty-1-on-merge
◎ │  📙merge
│ ●  ·47e1cf1 (⌂|🏘)
│ ◎  merge-2
│ ●    ·f40fb16 (⌂|🏘)
│ ├─╮
│ ◎ │  D
│ ● │  ·450c58a (⌂|🏘)
├─╯ │
│   ◎  C
│   ●  ·c6d714c (⌂|🏘)
├───╯
●    ·0cc5a6f (⌂|🏘)
├─╮
◎ │  B
● │  ·7fdb58d (⌂|🏘)
│ ◎  A
│ ●  ·e255adc (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:13:empty-2-on-merge on fafd9d0 {0}
│   ├── 📙:13:empty-2-on-merge
│   ├── 📙:12:empty-1-on-merge
│   ├── 📙:14:merge
│   │   └── ·0cc5a6f (🏘️) ►empty-1-on-merge, ►empty-2-on-merge
│   └── :15:B
│       └── ·7fdb58d (🏘️)
└── ≡:11:merge-2 on fafd9d0
    ├── :11:merge-2
    │   └── ·f40fb16 (🏘️)
    ├── :17:D
    │   ├── ·450c58a (🏘️)
    │   └── ·0cc5a6f (🏘️) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    └── :15:B
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
    let graph =
        Graph::from_commit_traversal(id, name, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  empty-1-on-merge
│ ◎  empty-2-on-merge
├─╯
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·47e1cf1 (⌂|🏘)
│ ◎  merge-2
│ ●    ·f40fb16 (⌂|🏘)
│ ├─╮
│ ◎ │  D
│ ● │  ·450c58a (⌂|🏘)
├─╯ │
│   ◎  👉C
│   ●  ·c6d714c (⌂|🏘)
├───╯
│ ◎  merge
├─╯
●    ·0cc5a6f (⌂|🏘)
├─╮
◎ │  B
● │  ·7fdb58d (⌂|🏘)
│ ◎  A
│ ●  ·e255adc (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:11:merge-2 on fafd9d0
    ├── :11:merge-2
    │   └── ·f40fb16 (🏘️)
    ├── :16:D
    │   ├── ·450c58a (🏘️)
    │   └── ·0cc5a6f (🏘️) ►empty-1-on-merge, ►empty-2-on-merge, ►merge
    └── :17:B
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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        extra_target_options.clone(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
◎ │ │  📙A
├───╯
│ ◎  📙B
├─╯
│ ◎  main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    let NodeGraphEntrypoint::Node(entrypoint) = graph.entrypoint() else {
        panic!("workspace reference is born");
    };
    let entrypoint = *entrypoint;
    let NodeKind::Reference(reference) = graph.nodes()[entrypoint].kind() else {
        panic!("symbolic workspace entrypoint is represented by a reference node");
    };
    assert_eq!(
        reference.ref_info.commit_id, extra_target_options.extra_target_commit_id,
        "the virtual workspace reference keeps its resolved target"
    );
    assert!(
        graph.nodes()[entrypoint].parents().len() > 1,
        "the virtual workspace reference keeps each ambiguous path"
    );
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.id,
        Some(entrypoint),
        "workspace projection starts at the canonical entrypoint node"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on fafd9d0
├── ≡📙:1:A on fafd9d0 {1}
│   └── 📙:1:A
└── ≡📙:2:B on fafd9d0 {2}
    └── 📙:2:B

"#]]
    );

    let (id, ref_name) = id_at(&repo, "B");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        extra_target_options.clone(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      📕gitbutler/workspace[🌳]
├─┬─╮
◎ │ │  📙A
├───╯
│ ◎  👉📙B
├─╯
│ ◎  main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on fafd9d0
├── ≡📙:1:A on fafd9d0 {1}
│   └── 📙:1:A
└── ≡👉📙:2:B on fafd9d0 {2}
    └── 👉📙:2:B

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        extra_target_options,
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      📕gitbutler/workspace[🌳]
├─┬─╮
◎ │ │  👉📙A
├───╯
│ ◎  📙B
├─╯
│ ◎  main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on fafd9d0
├── ≡👉📙:1:A on fafd9d0 {1}
│   └── 👉📙:1:A
└── ≡📙:2:B on fafd9d0 {2}
    └── 📙:2:B

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  C
├─╯
│ ◎  D
├─╯
│ ◎  E
├─╯
│ ◎  F
├─╯
│ ◎  📕gitbutler/workspace
├─╯
│ ◎  origin/main
│ ◎  👉main[🌳] <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0

"#]]
    );

    let (id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
    let graph = Graph::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  C
├─╯
│ ◎  D
├─╯
│ ◎  E
├─╯
│ ◎  F
├─╯
│ ◎  👉📕gitbutler/workspace
├─╯
│ ◎  origin/main
│ ◎  main[🌳] <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    // However, when the workspace is checked out, it's at least empty.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0

"#]]
    );

    // The simplest possible setup where we can define how the workspace should look like,
    // in terms of dependent and independent virtual segments.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      📕gitbutler/workspace
├─┬─╮
◎ │ │  📙C
◎ │ │  📙B
◎ │ │  📙A
├───╯
│ ◎  📙D
│ ◎  📙E
│ ◎  📙F
├─╯
│ ◎  origin/main
│ ◎  👉main[🌳] <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    // With empty project metadata, workspace segmentation is retained around the workspace ref.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:3:C on fafd9d0 {0}
│   ├── 📙:3:C
│   ├── 📙:2:B
│   └── 📙:1:A
└── ≡📙:4:D on fafd9d0 {1}
    ├── 📙:4:D
    ├── 📙:5:E
    └── 📙:6:F

"#]]
    );

    let graph = Graph::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // Now the dependent segments are applied, and so is the separate stack.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace
├─┬─╮
◎ │ │  📙C
◎ │ │  📙B
◎ │ │  📙A
├───╯
│ ◎  📙D
│ ◎  📙E
│ ◎  📙F
├─╯
│ ◎  origin/main
│ ◎  main[🌳] <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:3:C on fafd9d0 {0}
│   ├── 📙:3:C
│   ├── 📙:2:B
│   └── 📙:1:A
└── ≡📙:4:D on fafd9d0 {1}
    ├── 📙:4:D
    ├── 📙:5:E
    └── 📙:6:F

"#]]
    );

    let graph = Graph::from_commit_traversal(
        id,
        ws_ref_name,
        &*meta,
        project_meta(&*meta),
        but_graph::init::Options {
            dangerously_skip_postprocessing_for_debugging: true,
            ..standard_options()
        },
    )?
    .validated()?;
    // Show how the lack of post-processing affects the graph - remotes are also not connected.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
●  👉🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:0:gitbutler/workspace <> ✓! on fafd9d0

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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let head_baseline_tree = graph_tree(&head_baseline).to_string();
    let head_baseline_workspace = graph_workspace(&head_baseline.into_workspace()?).to_string();

    let head_tips = vec![
        Tip::new(commit_id).with_role(TipRole::WorkspaceStackBranch {
            desired_ref_name: stack_ref("F"),
        }),
        Tip::new(commit_id)
            .with_ref_name(Some(ws_ref_name.clone()))
            .with_role(TipRole::Workspace)
            .with_metadata(ReferenceMetadata::Workspace(workspace_metadata.clone())),
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

    let workspace_baseline = Graph::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    let workspace_baseline_tree = graph_tree(&workspace_baseline).to_string();
    let workspace_baseline_workspace = graph_workspace(&workspace_baseline.into_workspace()?);
    snapbox::assert_data_eq!(
        workspace_baseline_workspace.to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:3:C on fafd9d0 {0}
│   ├── 📙:3:C
│   ├── 📙:2:B
│   └── 📙:1:A
└── ≡📙:4:D on fafd9d0 {1}
    ├── 📙:4:D
    ├── 📙:5:E
    └── 📙:6:F

"#]]
    );
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
            .with_metadata(ReferenceMetadata::Workspace(workspace_metadata))
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
    let graph = Graph::from_commit_traversal_tips(
        &repo,
        head_tips,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    assert_eq!(
        graph_tree(&graph).to_string(),
        head_baseline_tree,
        "unordered explicit tips with a reachable entrypoint should match HEAD traversal"
    );
    assert_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        head_baseline_workspace,
        "unordered explicit tips with a reachable entrypoint should match the HEAD workspace projection"
    );

    let graph = Graph::from_commit_traversal_tips(
        &repo,
        explicit_tips.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    assert_eq!(
        graph_tree(&graph).to_string(),
        workspace_baseline_tree,
        "unordered explicit tips should create the same graph as workspace metadata traversal"
    );
    let explicit_workspace = graph_workspace(&graph.into_workspace()?);
    snapbox::assert_data_eq!(
        explicit_workspace.to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:3:C on fafd9d0 {0}
│   ├── 📙:3:C
│   ├── 📙:2:B
│   └── 📙:1:A
└── ≡📙:4:D on fafd9d0 {1}
    ├── 📙:4:D
    ├── 📙:5:E
    └── 📙:6:F

"#]]
    );
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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let baseline_tree = graph_tree(&baseline).to_string();
    let baseline_workspace = graph_workspace(&baseline.into_workspace()?).to_string();

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(target_id),
    )?
    .validated()?;

    assert_eq!(
        graph_tree(&graph).to_string(),
        baseline_tree,
        "duplicated synthetic integrated tips should not change graph traversal"
    );
    assert_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let baseline_tree = graph_tree(&baseline).to_string();
    let baseline_workspace = graph_workspace(&baseline.into_workspace()?).to_string();

    add_stack_with_segments(&mut meta, 3, "B", StackState::InWorkspace, &[]);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    assert_eq!(
        graph_tree(&graph).to_string(),
        baseline_tree,
        "duplicate stack branch metadata (B) should not enqueue the same stack branch traversal twice"
    );
    assert_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
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
    let graph = Graph::from_commit_traversal(
        id,
        ws_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;

    // By default, we see both stacks as they are configured, which disambiguates them.
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.stacks[0]
            .segments
            .iter()
            .filter_map(|segment| segment.ref_name().map(ToString::to_string))
            .collect::<Vec<_>>(),
        ["refs/heads/C", "refs/heads/B", "refs/heads/A"],
        "the complete active metadata chain is retained before archive truncation"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:3:C on fafd9d0 {0}
    ├── 📙:3:C
    ├── 📙:2:B
    └── 📙:1:A

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:3:C on fafd9d0 {0}
    └── 📙:3:C

"#]]
    );

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = false;

    // Now only the first one is archived.
    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Default::default())?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:3:C on fafd9d0 {0}
    ├── 📙:3:C
    └── 📙:2:B

"#]]
    );

    let heads = &mut meta.data_mut().branches.get_mut(&stack_id).unwrap().heads;
    heads[0].archived = true;
    heads[1].archived = true;
    heads[2].archived = true;

    // Archiving everything removes the stack entirely.
    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    assert_eq!(
        graph.managed_workspace_commit_id(),
        Some(repo.rev_parse_single("gitbutler/workspace")?.detach()),
        "the managed workspace commit remains separate from stack commits"
    );
    // Without any information it looks quite barren.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  C
├─╯
│ ◎  D
│ │ ◎  E
│ ├─╯
│ │ ◎  F
│ │ │ ◎  G
│ │ ├─╯
│ │ │ ◎  S1
│ │ ├─╯
│ │ │ ◎  👉📕gitbutler/workspace[🌳]
│ │ │ ●  ·298d938 (⌂|🏘)
│ │ ├─╯
│ │ ●  ·16f132b (⌂|🏘)
│ ├─╯
│ ●  ·917b9da (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // With no workspace at all as the workspace segment isn't split.
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.commits)
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        [
            repo.rev_parse_single("S1")?.detach(),
            repo.rev_parse_single("D")?.detach(),
        ],
        "same-tip reference ownership must not hide commit-bearing workspace history"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:2:anon on fafd9d0
    └── :2:anon
        ├── ·16f132b (🏘️) ►F, ►G, ►S1
        └── ·917b9da (🏘️) ►D, ►E

"#]]
    );

    let (id, ref_name) = id_at(&repo, "S1");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // The S1 starting position is a split, so there is more.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  C
├─╯
│ ◎  D
│ │ ◎  E
│ ├─╯
│ │ ◎  F
│ │ │ ◎  G
│ │ ├─╯
│ │ │ ◎  👉S1
│ │ ├─╯
│ │ │ ◎  📕gitbutler/workspace[🌳]
│ │ │ ●  ·298d938 (⌂|🏘)
│ │ ├─╯
│ │ ●  ·16f132b (⌂|🏘)
│ ├─╯
│ ●  ·917b9da (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:2:anon on fafd9d0
    └── :2:anon
        ├── ·16f132b (🏘️) ►F, ►G, ►S1
        └── ·917b9da (🏘️) ►D, ►E

"#]]
    );

    // Define the workspace.
    add_stack_with_segments(&mut meta, 1, "C", StackState::InWorkspace, &["B"]);
    add_stack_with_segments(&mut meta, 2, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 3, "S1", StackState::InWorkspace, &["G", "F"]);
    add_stack_with_segments(&mut meta, 4, "D", StackState::InWorkspace, &["E"]);

    // We see that all segments are used: S1 C B A E D G F
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📙A
│ ◎  📙C
│ ◎  📙B
├─╯
│ ◎    👉📕gitbutler/workspace[🌳]
│ ├─╮
│ │ ●  ·298d938 (⌂|🏘)
│ ├─╯
│ ◎  📙S1
│ ◎  📙G
│ ◎  📙F
│ ●  ·16f132b (⌂|🏘)
│ ◎  📙D
│ ◎  📙E
│ ●  ·917b9da (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:12:S1 on fafd9d0 {3}
    ├── 📙:12:S1
    ├── 📙:11:G
    ├── 📙:10:F
    │   └── ·16f132b (🏘️) ►G, ►S1
    ├── 📙:13:D
    └── 📙:14:E
        └── ·917b9da (🏘️) ►D

"#]]
    );

    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // This should look the same as before, despite the starting position.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📙A
│ ◎  📙C
│ ◎  📙B
├─╯
│ ◎    📕gitbutler/workspace[🌳]
│ ├─╮
│ │ ●  ·298d938 (⌂|🏘)
│ ├─╯
│ ◎  👉📙S1
│ ◎  📙G
│ ◎  📙F
│ ●  ·16f132b (⌂|🏘)
│ ◎  📙D
│ ◎  📙E
│ ●  ·917b9da (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡👉📙:12:S1 on fafd9d0 {3}
    ├── 👉📙:12:S1
    ├── 📙:11:G
    ├── 📙:10:F
    │   └── ·16f132b (🏘️) ►G, ►S1
    ├── 📙:13:D
    └── 📙:14:E
        └── ·917b9da (🏘️) ►D

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
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎          👉📕gitbutler/workspace
├─┬─┬─┬─╮
◎ │ │ │ │  📙C
◎ │ │ │ │  📙B
├───────╯
│ ◎ │ │  📙A
├─╯ │ │
│   ◎ │  📙D
│   ◎ │  📙E
├───╯ │
│     ◎  📙F
├─────╯
│ ◎  origin/main
│ ◎  main[🌳] <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:3:C on fafd9d0 {0}
│   ├── 📙:3:C
│   └── 📙:2:B
├── ≡📙:1:A on fafd9d0 {1}
│   └── 📙:1:A
├── ≡📙:4:D on fafd9d0 {2}
│   ├── 📙:4:D
│   └── 📙:5:E
└── ≡📙:6:F on fafd9d0 {3}
    └── 📙:6:F

"#]]
    );

    let (id, ref_name) = id_at(&repo, "C");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // The entrypoint shouldn't affect the projected workspace. However, as its
    // commit is integrated, it isn't considered part of the workspace.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎          📕gitbutler/workspace
├─┬─┬─┬─╮
◎ │ │ │ │  👉📙C
◎ │ │ │ │  📙B
├───────╯
│ ◎ │ │  📙A
├─╯ │ │
│   ◎ │  📙D
│   ◎ │  📙E
├───╯ │
│     ◎  📙F
├─────╯
│ ◎  origin/main
│ ◎  main[🌳] <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // We should see the same stacks as we did before, just with a different entrypoint.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace <> ✓refs/remotes/origin/main on fafd9d0
├── ≡👉📙:3:C on fafd9d0 {0}
│   ├── 👉📙:3:C
│   └── 📙:2:B
├── ≡📙:1:A on fafd9d0 {1}
│   └── 📙:1:A
├── ≡📙:4:D on fafd9d0 {2}
│   ├── 📙:4:D
│   └── 📙:5:E
└── ≡📙:6:F on fafd9d0 {3}
    └── 📙:6:F

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·9bcd3af (⌂|🏘)
│ ◎  main <> origin/main
├─╯
│ ◎  origin/main
│ ●  🟣ca7baa7 (✓)
│ ●  🟣7ea1468 (✓)
├─╯
●  ·998eae6 (⌂|🏘|✓)
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // Everything in the workspace is integrated, thus it's empty.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 998eae6

"#]]
    );

    let (id, ref_name) = id_at(&repo, "main");
    // The integration branch can be in the workspace and be checked out.
    let graph = Graph::from_commit_traversal(
        id,
        Some(ref_name),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
●  ·9bcd3af (⌂|🏘)
│ ◎  👉main <> origin/main
├─╯
│ ◎  origin/main
│ ●  🟣ca7baa7 (✓)
│ ●  🟣7ea1468 (✓)
├─╯
●  ·998eae6 (⌂|🏘|✓)
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // If it's checked out, we must show the branch container, but it's not part of the
    // managed workspace. The target context is preserved and integrated local/base commits
    // are pruned, leaving only target-side commits ahead of the stored target.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 998eae6

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
    let graph = Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·8b39ce4 (⌂|🏘)
◎  A <> origin/A
●  ·9d34471 (⌂|🏘)
●  ·5b89c71 (⌂|🏘)
│ ◎  origin/A
│ │ ◎  push-remote/A
│ ├─╯
│ ●  🟣3ea1a8f
│ ●  🟣9c50f71
│ ●  🟣2cfbb79
╭─┤
│ ●  🟣e898cd0
├─╯
●  ·998eae6 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    // There is no target branch, so nothing is integrated, and `main` shows up.
    // It's not special.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓!
└── ≡:10:A <> origin/A →:11:⇡2⇣4
    ├── :10:A <> origin/A →:11:⇡2⇣4
    │   ├── 🟣3ea1a8f ►origin/A, ►push-remote/A
    │   ├── 🟣9c50f71
    │   ├── 🟣2cfbb79
    │   ├── 🟣e898cd0
    │   ├── ·9d34471 (🏘️)
    │   ├── ·5b89c71 (🏘️)
    │   └── ❄998eae6 (🏘️)
    └── :13:main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    let id = id_by_rev(&repo, ":/init");
    let graph =
        Graph::from_commit_traversal(id, None, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
●  ·8b39ce4 (⌂|🏘)
◎  A <> origin/A
●  ·9d34471 (⌂|🏘)
●  ·5b89c71 (⌂|🏘)
│ ◎  origin/A
│ │ ◎  push-remote/A
│ ├─╯
│ ●  🟣3ea1a8f
│ ●  🟣9c50f71
│ ●  🟣2cfbb79
╭─┤
│ ●  🟣e898cd0
├─╯
●  ·998eae6 (⌂|🏘)
◎  main
●  👉🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    // The whole workspace is visible, but it's clear where the entrypoint is.
    // As there is no target ref, `main` shows up.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓!
└── ≡:10:A <> origin/A →:12:⇡2⇣4
    ├── :10:A <> origin/A →:12:⇡2⇣4
    │   ├── 🟣3ea1a8f ►origin/A, ►push-remote/A
    │   ├── 🟣9c50f71
    │   ├── 🟣2cfbb79
    │   ├── 🟣e898cd0
    │   ├── ·9d34471 (🏘️)
    │   ├── ·5b89c71 (🏘️)
    │   └── ❄998eae6 (🏘️)
    └── :11:main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    // When the push-remote is configured, it overrides the remote we use for listing, even if a fetch remote is available.
    let mut ws = meta.workspace(WORKSPACE_REF_NAME.try_into().expect("valid workspace ref"))?;
    let mut pm = ws.project_meta();
    pm.push_remote = Some("push-remote".into());
    ws.set_project_meta(pm);
    meta.set_workspace(&ws)?;
    let graph = Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·8b39ce4 (⌂|🏘)
◎  A <> push-remote/A
●  ·9d34471 (⌂|🏘)
●  ·5b89c71 (⌂|🏘)
│ ◎  origin/A
│ │ ◎  push-remote/A
│ ├─╯
│ ●  🟣3ea1a8f
│ ●  🟣9c50f71
│ ●  🟣2cfbb79
╭─┤
│ ●  🟣e898cd0
├─╯
●  ·998eae6 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓!
└── ≡:10:A <> push-remote/A →:12:⇡2⇣4
    ├── :10:A <> push-remote/A →:12:⇡2⇣4
    │   ├── 🟣3ea1a8f ►origin/A, ►push-remote/A
    │   ├── 🟣9c50f71
    │   ├── 🟣2cfbb79
    │   ├── 🟣e898cd0
    │   ├── ·9d34471 (🏘️)
    │   ├── ·5b89c71 (🏘️)
    │   └── ❄998eae6 (🏘️)
    └── :13:main
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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·7786959 (⌂|🏘)
◎  B
●  ·312f819 (⌂|🏘)
◎  A
●  ·e255adc (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    // It's worth noting that we avoid double-listing remote commits that are also
    // directly owned by another remote segment.
    // they have to be considered as something relevant to the branch history.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:5:B on fafd9d0
    ├── :5:B
    │   └── ·312f819 (🏘️)
    └── :6:A
        └── ·e255adc (🏘️)

"#]]
    );

    // The result is the same when changing the entrypoint.
    let (id, name) = id_at(&repo, "A");
    let graph =
        Graph::from_commit_traversal(id, name, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
●  ·7786959 (⌂|🏘)
◎  B <> origin/B
●  ·312f819 (⌂|🏘)
◎  👉A <> origin/A
●  ·e255adc (⌂|🏘)
│ ◎  origin/B
│ ●  🟣682be32
│ ◎  origin/A
│ ●  🟣e29c23d
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:9:B <> origin/B →:12:⇡1⇣2 on fafd9d0
    ├── :9:B <> origin/B →:12:⇡1⇣2
    │   ├── 🟣682be32 ►origin/B
    │   ├── 🟣e29c23d ►origin/A
    │   └── ·312f819 (🏘️)
    └── 👉:10:A <> origin/A →:11:⇡1⇣1
        ├── 🟣e29c23d ►origin/A
        └── ·e255adc (🏘️)

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·dd0cca8 (⌂|🏘)
├─╯
◎  📙A
│ ◎  main <> origin/main
├─╯
●  ·e255adc (⌂|🏘)
◎  origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // The main branch is not present, as it's the target.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:3:A on fafd9d0 {1}
    └── 📙:3:A
        └── ·e255adc (🏘️) ►main

"#]]
    );

    // But mention it if it's in the workspace. It should retain order.
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["main"]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:3:A on fafd9d0 {1}
    ├── 📙:3:A
    └── 📙:4:main <> origin/main →:5:⇡1
        └── ·e255adc (🏘️) ►A

"#]]
    );

    // But mention it if it's in the workspace. It should retain order - inverting the order is fine.
    add_stack_with_segments(&mut meta, 1, "main", StackState::InWorkspace, &["A"]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:4:main <> origin/main →:5: on fafd9d0 {1}
    ├── 📙:4:main <> origin/main →:5:
    └── 📙:3:A
        └── ·e255adc (🏘️) ►main

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
    // If a remote reference leads back to its local tracking branch and the
    // commit's local name is ambiguous, use the remote-tracking relationship to
    // disambiguate the local reference.
    // Note that this is more complicated if the local tracking branch is also advanced, but
    // this is something to improve when workspace-less operation becomes a thing *and* we
    // need to get better as disambiguation.
    // The target branch is actually counted as remote, but it doesn't come through here as
    // it steals the commit from `main`. This should be fine.
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  B <> origin/B
│ ◎  ambiguous-A
│ │ ◎  ambiguous-B
├───╯
│ │ ◎  👉📕gitbutler/workspace[🌳]
│ │ ●  ·e30f90c (⌂|🏘)
│ │ │ ◎  origin/A
│ │ │ ◎  A <> origin/A
│ ├───╯
│ │ │ ◎  origin/B
│ │ │ ●  🟣ac24e74
├─────╯
│ │ │ ◎  origin/C
│ │ │ ◎  C <> origin/C
│ │ ├─╯
│ │ │ ◎  origin/ambiguous-C
│ │ │ ◎  ambiguous-C <> origin/ambiguous-C
│ │ ├─╯
│ │ ●  ·2173153 (⌂|🏘)
├───╯
● │  ·312f819 (⌂|🏘)
├─╯
●  ·e255adc (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // An anonymous segment to start with is alright, and can always happen for other situations as well.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:2:anon on fafd9d0
    └── :2:anon
        ├── ·2173153 (🏘️) ►C, ►ambiguous-C, ►origin/C, ►origin/ambiguous-C
        ├── ·312f819 (🏘️) ►B, ►ambiguous-B
        └── ·e255adc (🏘️) ►A, ►ambiguous-A, ►origin/A

"#]]
    );

    // If 'C' is in the workspace, it's naturally disambiguated.
    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &[]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  B <> origin/B
│ ◎  ambiguous-A
│ │ ◎  ambiguous-B
├───╯
│ │ ◎    👉📕gitbutler/workspace[🌳]
│ │ ├─╮
│ │ │ ●  ·e30f90c (⌂|🏘)
│ │ ├─╯
│ │ │ ◎  origin/A
│ │ │ ◎  A <> origin/A
│ ├───╯
│ │ │ ◎  origin/B
│ │ │ ●  🟣ac24e74
├─────╯
│ │ │ ◎  origin/C
│ │ ├─╯
│ │ ◎  📙C <> origin/C
│ │ │ ◎  origin/ambiguous-C
│ │ │ ◎  ambiguous-C <> origin/ambiguous-C
│ │ ├─╯
│ │ ●  ·2173153 (⌂|🏘)
├───╯
● │  ·312f819 (⌂|🏘)
├─╯
●  ·e255adc (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    // And because `C` is in the workspace data, its data is denoted.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:9:C <> origin/C →:11: on fafd9d0 {0}
    └── 📙:9:C <> origin/C →:11:
        ├── ❄2173153 (🏘️) ►ambiguous-C, ►origin/C, ►origin/ambiguous-C
        ├── ❄312f819 (🏘️) ►B, ►ambiguous-B
        └── ❄e255adc (🏘️) ►A, ►ambiguous-A, ►origin/A

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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·4077353 (⌂|🏘)
◎  B
●  ·6b1a13b (⌂|🏘)
●  ·03ad472 (⌂|🏘)
◎  A
●  ·79bbb29 (⌂|🏘)
●  ·fc98174 (⌂|🏘)
●  ·a381df5 (⌂|🏘)
●  ·777b552 (⌂|🏘)
●    ·ce4a760 (⌂|🏘)
├─╮
│ ◎  A-feat
│ ●  ·fea59b5 (⌂|🏘)
│ ●  ·4deea74 (⌂|🏘)
├─╯
●  ·01d0e1e (⌂|🏘)
◎  main
●  ·4b3e5a8 (⌂|🏘)
●  ·34d0715 (⌂|🏘)
●  🏁·eb5f731 (⌂|🏘)

"#]]
    );
    // It's true that `A` is fully integrated so it isn't displayed. so from a workspace-perspective
    // it's the right answer.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:14:gitbutler/workspace[🌳] <> ✓!
└── ≡:15:B
    ├── :15:B
    │   ├── ·6b1a13b (🏘️)
    │   └── ·03ad472 (🏘️)
    ├── :16:A
    │   ├── ·79bbb29 (🏘️)
    │   ├── ·fc98174 (🏘️)
    │   ├── ·a381df5 (🏘️)
    │   ├── ·777b552 (🏘️)
    │   ├── ·ce4a760 (🏘️)
    │   └── ·01d0e1e (🏘️)
    └── :18:main
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·4077353 (⌂|🏘)
├─╯
◎  📙B
●  ·6b1a13b (⌂|🏘)
●  ·03ad472 (⌂|🏘)
◎  📙A
│ ◎  origin/main
│ ●  🟣d0df794 (✓)
│ ●  🟣09c6e08 (✓)
│ ●  🟣7b9f260 (✓)
╭─┤
│ ◎  main <> origin/main
│ ●  🟣4b3e5a8 (✓)
│ ●  🟣34d0715 (✓)
│ ●  🏁🟣eb5f731 (✓)
●  ·79bbb29 (⌂|🏘|✓)
●  ·fc98174 (⌂|🏘|✓)
●  ·a381df5 (⌂|🏘|✓)
●  ·777b552 (⌂|🏘|✓)
●  ✂·ce4a760 (⌂|🏘|✓)

"#]]
    );
    // `A` is integrated, hence it's not shown.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:17:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
└── ≡📙:18:B on 79bbb29 {0}
    ├── 📙:18:B
    │   ├── ·6b1a13b (🏘️)
    │   └── ·03ad472 (🏘️)
    └── 📙:19:A

"#]]
    );

    // The limit is effective for integrated workspaces branches, and it doesn't unnecessarily
    // prolong the traversal once the all tips are known to be integrated.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·4077353 (⌂|🏘)
├─╯
◎  📙B
●  ·6b1a13b (⌂|🏘)
●  ·03ad472 (⌂|🏘)
◎  📙A
│ ◎  origin/main
│ ●  🟣d0df794 (✓)
│ ●  🟣09c6e08 (✓)
│ ●  🟣7b9f260 (✓)
╭─┤
│ ◎  main <> origin/main
│ ●  🟣4b3e5a8 (✓)
│ ●  🟣34d0715 (✓)
│ ●  🏁🟣eb5f731 (✓)
●  ·79bbb29 (⌂|🏘|✓)
●  ·fc98174 (⌂|🏘|✓)
●  ·a381df5 (⌂|🏘|✓)
●  ·777b552 (⌂|🏘|✓)
●  ✂·ce4a760 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:17:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
└── ≡📙:18:B on 79bbb29 {0}
    ├── 📙:18:B
    │   ├── ·6b1a13b (🏘️)
    │   └── ·03ad472 (🏘️)
    └── 📙:19:A

"#]]
    );

    meta.data_mut().branches.clear();
    add_workspace(&mut meta);
    // When looking from an integrated branch within the workspace, but without limit,
    // the (lack of) limit is respected.
    // When the entrypoint starts on an integrated commit, the 'all-tips-are-integrated' condition doesn't
    // kick in anymore.
    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉A
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·4077353 (⌂|🏘)
│ ◎  B
│ ●  ·6b1a13b (⌂|🏘)
│ ●  ·03ad472 (⌂|🏘)
├─╯
│ ◎  main <> origin/main
│ │ ◎  origin/main
│ │ ●  🟣d0df794 (✓)
│ │ ●  🟣09c6e08 (✓)
│ │ ●  🟣7b9f260 (✓)
╭─┬─╯
● │  ·79bbb29 (⌂|🏘|✓)
● │  ·fc98174 (⌂|🏘|✓)
● │  ·a381df5 (⌂|🏘|✓)
● │  ·777b552 (⌂|🏘|✓)
● │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ ◎  A-feat
│ │ ●  ·fea59b5 (⌂|🏘|✓)
│ │ ●  ·4deea74 (⌂|🏘|✓)
├───╯
● │  ·01d0e1e (⌂|🏘|✓)
├─╯
●  ·4b3e5a8 (⌂|🏘|✓)
●  ·34d0715 (⌂|🏘|✓)
●  🏁·eb5f731 (⌂|🏘|✓)

"#]]
    );
    // The entrypoint branch is downgraded to a single-branch view with target context
    // preserved. All commits on this branch are integrated, so the branch container remains
    // but its commit list is pruned.
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:18:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:19:B on 79bbb29
    └── :19:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    let graph = Graph::from_commit_traversal(
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
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉A
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·4077353 (⌂|🏘)
│ ◎  B
│ ●  ·6b1a13b (⌂|🏘)
│ ●  ·03ad472 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ●  🟣d0df794 (✓)
│ ●  🟣09c6e08 (✓)
│ ●  🟣7b9f260 (✓)
╭─┤
│ ◎  main <> origin/main
│ ●  🟣4b3e5a8 (✓)
│ ●  🟣34d0715 (✓)
│ ●  🏁🟣eb5f731 (✓)
●  ·79bbb29 (⌂|🏘|✓)
●  ·fc98174 (⌂|🏘|✓)
●  ·a381df5 (⌂|🏘|✓)
●  ·777b552 (⌂|🏘|✓)
●  ✂·ce4a760 (⌂|🏘|✓)

"#]]
    );
    // Because the branch is integrated, the surrounding workspace isn't shown. The downgraded
    // branch view keeps target context and prunes the integrated commits.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:17:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
└── ≡:18:B on 79bbb29
    └── :18:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    // See what happens with an out-of-workspace HEAD and an arbitrary extra target.
    let (id, _ref_name) = id_at(&repo, "origin/main");
    let graph = Graph::from_commit_traversal(
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
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·4077353 (⌂|🏘|✓)
│ ◎  B
│ ●  ·6b1a13b (⌂|🏘|✓)
│ ●  ·03ad472 (⌂|🏘|✓)
├─╯
│ ◎  main <> origin/main
│ │ ◎  origin/main
│ │ ●  👉·d0df794 (⌂|✓)
│ │ ●  ·09c6e08 (⌂|✓)
│ │ ●  ·7b9f260 (⌂|✓)
╭─┬─╯
● │  ·79bbb29 (⌂|🏘|✓)
● │  ·fc98174 (⌂|🏘|✓)
● │  ·a381df5 (⌂|🏘|✓)
● │  ·777b552 (⌂|🏘|✓)
● │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ ◎  A-feat
│ │ ●  ·fea59b5 (⌂|🏘|✓)
│ │ ●  ·4deea74 (⌂|🏘|✓)
├───╯
● │  ·01d0e1e (⌂|🏘|✓)
├─╯
●  ·4b3e5a8 (⌂|🏘|✓)
●  ·34d0715 (⌂|🏘|✓)
●  🏁·eb5f731 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:0:DETACHED <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡👉:0:anon on 4b3e5a8 {1}
    └── 👉:0:anon
        ├── ·d0df794 (✓) ►origin/main
        ├── ·09c6e08 (✓)
        └── ·7b9f260 (✓)

"#]]
    );

    // However, when choosing an initially unknown branch, it will get the extra target tip settings.
    let graph = Graph::from_commit_traversal(
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
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·4077353 (⌂|🏘)
│ ◎  B
│ ●  ·6b1a13b (⌂|🏘|✓)
│ ●  ·03ad472 (⌂|🏘|✓)
├─╯
│ ◎  main <> origin/main
│ │ ◎  origin/main
│ │ ●  👉·d0df794 (⌂|✓)
│ │ ●  ·09c6e08 (⌂|✓)
│ │ ●  ·7b9f260 (⌂|✓)
╭─┬─╯
● │  ·79bbb29 (⌂|🏘|✓)
● │  ·fc98174 (⌂|🏘|✓)
● │  ·a381df5 (⌂|🏘|✓)
● │  ·777b552 (⌂|🏘|✓)
● │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ ◎  A-feat
│ │ ●  ·fea59b5 (⌂|🏘|✓)
│ │ ●  ·4deea74 (⌂|🏘|✓)
├───╯
● │  ·01d0e1e (⌂|🏘|✓)
├─╯
●  ·4b3e5a8 (⌂|🏘|✓)
●  ·34d0715 (⌂|🏘|✓)
●  🏁·eb5f731 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
⌂:2:DETACHED <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡👉:2:anon on 4b3e5a8 {1}
    └── 👉:2:anon
        ├── ·d0df794 (✓) ►origin/main
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  👉📕gitbutler/workspace[🌳]
│ ●  ·4077353 (⌂|🏘)
│ ◎  B
│ ●  ·6b1a13b (⌂|🏘)
│ ●  ·03ad472 (⌂|🏘)
├─╯
│ ◎  main <> origin/main
│ │ ◎  origin/main
│ │ ●  🟣d0df794 (✓)
│ │ ●  🟣09c6e08 (✓)
│ │ ●  🟣7b9f260 (✓)
╭─┬─╯
● │  ·79bbb29 (⌂|🏘|✓)
● │  ·fc98174 (⌂|🏘|✓)
● │  ·a381df5 (⌂|🏘|✓)
● │  ·777b552 (⌂|🏘|✓)
● │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ ◎  A-feat
│ │ ●  ·fea59b5 (⌂|🏘|✓)
│ │ ●  ·4deea74 (⌂|🏘|✓)
├───╯
● │  ·01d0e1e (⌂|🏘|✓)
├─╯
●  ·4b3e5a8 (⌂|🏘|✓)
●  ·34d0715 (⌂|🏘|✓)
●  🏁·eb5f731 (⌂|🏘|✓)

"#]]
    );

    // This search discovers the whole workspace, without the integrated one.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:19:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:20:B on 79bbb29
    └── :20:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    // However, we can specify an additional/old target segment to show integrated portions as well.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:19:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣11 on 4b3e5a8
└── ≡:20:B on 4b3e5a8
    └── :20:B
        ├── ·6b1a13b (🏘️)
        ├── ·03ad472 (🏘️)
        ├── ·79bbb29 (🏘️|✓) ►A
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
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉A
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·4077353 (⌂|🏘)
│ ◎  B
│ ●  ·6b1a13b (⌂|🏘)
│ ●  ·03ad472 (⌂|🏘)
├─╯
│ ◎  main <> origin/main
│ │ ◎  origin/main
│ │ ●  🟣d0df794 (✓)
│ │ ●  🟣09c6e08 (✓)
│ │ ●  🟣7b9f260 (✓)
╭─┬─╯
● │  ·79bbb29 (⌂|🏘|✓)
● │  ·fc98174 (⌂|🏘|✓)
● │  ·a381df5 (⌂|🏘|✓)
● │  ·777b552 (⌂|🏘|✓)
● │    ·ce4a760 (⌂|🏘|✓)
├───╮
│ │ ◎  A-feat
│ │ ●  ·fea59b5 (⌂|🏘|✓)
│ │ ●  ·4deea74 (⌂|🏘|✓)
├───╯
● │  ·01d0e1e (⌂|🏘|✓)
├─╯
●  ·4b3e5a8 (⌂|🏘|✓)
●  ·34d0715 (⌂|🏘|✓)
●  🏁·eb5f731 (⌂|🏘|✓)

"#]]
    );

    // The entrypoint isn't contained in the managed workspace anymore, so it's a standalone
    // single-branch view. Target context is preserved, so integrated commits are pruned while
    // the branch container remains visible.
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:19:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:20:B on 79bbb29
    └── :20:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    // When converting to a workspace, we are still aware of the workspace membership as long as
    // the lower bound of the workspace includes it.
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:19:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣11 on 4b3e5a8
└── ≡:20:B on 4b3e5a8
    └── :20:B
        ├── ·6b1a13b (🏘️)
        ├── ·03ad472 (🏘️)
        ├── ·79bbb29 (🏘️|✓) ►A
        ├── ·fc98174 (🏘️|✓)
        ├── ·a381df5 (🏘️|✓)
        ├── ·777b552 (🏘️|✓)
        ├── ·ce4a760 (🏘️|✓)
        └── ·01d0e1e (🏘️|✓)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "main");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // When the branch is below the forkpoint, the workspace also isn't shown anymore.
    // The downgraded branch view keeps target context and prunes integrated base commits.
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:18:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:19:B on 79bbb29
    └── :19:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    let id = id_by_rev(&repo, "main~1");
    let graph =
        Graph::from_commit_traversal(id, None, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    // Detached states are also possible. They keep the anonymous container while
    // preserving target context and pruning integrated commits.
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:19:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on 79bbb29
└── ≡:20:B on 79bbb29
    └── :20:B
        ├── ·6b1a13b (🏘️)
        └── ·03ad472 (🏘️)

"#]]
    );

    // Containment follows the stored target commit, not the current target-ref status. Once A is
    // the stored boundary, neither the integrated branch at that boundary nor its named or
    // detached ancestors belong to the managed workspace.
    let (target_id, target_ref_name) = id_at(&repo, "A");
    add_workspace_with_target(&mut meta, target_id);
    let workspace = Graph::from_commit_traversal(
        target_id,
        target_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;
    assert_ad_hoc_entrypoint(&workspace, Some("refs/heads/A"));
    assert_eq!(
        workspace.stored_target_commit_id(),
        Some(target_id.detach())
    );

    let (main_id, main_ref_name) = id_at(&repo, "main");
    let workspace = Graph::from_commit_traversal(
        main_id,
        main_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;
    assert_ad_hoc_entrypoint(&workspace, Some("refs/heads/main"));

    let detached_id = id_by_rev(&repo, "main~1");
    let workspace = Graph::from_commit_traversal(
        detached_id,
        None,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;
    assert_ad_hoc_entrypoint(&workspace, None);
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

    let graph =
        Graph::from_head(&repo, &meta, project_meta(&meta), standard_options())?.validated()?;
    // Main is a normal branch, and its remote is known.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📙main[🌳] <> origin/main
│ ◎  origin/main
│ ●  ·956a3de (⌂)
│ ◎  📕gitbutler/workspace
├─╯
●  🏁·3183e43 (⌂|🏘)

"#]]
    );

    let ws = graph.into_workspace()?;
    // The workspace shows the remote commit, there is nothing special about the target.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace <> ✓!
└── ≡👉📙:3:main[🌳] <> origin/main →:4:⇣1 {0}
    └── 👉📙:3:main[🌳] <> origin/main →:4:⇣1
        ├── 🟣956a3de ►origin/main
        └── ❄3183e43 (🏘️) ►gitbutler/workspace

"#]]
    );

    // If the remote isn't setup officially, deduction still works as we find
    // symbolic remote names for deduction in workspace ref names as well.
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));
    let graph = ws
        .graph
        .redo_traversal_with_overlay(&repo, &meta, Overlay::default())?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📙main[🌳] <> origin/main
│ ◎  origin/main
│ ●  ·956a3de (⌂)
│ ◎  📕gitbutler/workspace
├─╯
●  🏁·3183e43 (⌂|🏘)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace <> ✓!
└── ≡👉📙:3:main[🌳] <> origin/main →:4:⇣1 {0}
    └── 👉📙:3:main[🌳] <> origin/main →:4:⇣1
        ├── 🟣956a3de ►origin/main
        └── ❄3183e43 (🏘️) ►gitbutler/workspace

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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(0),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ✂·4077353 (⌂|🏘)

"#]]
    );
    // The commit in the workspace branch is always ignored and is expected to be the workspace merge commit.
    // So nothing to show here.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:2:gitbutler/workspace[🌳] <> ✓!

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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(0),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  👉📕gitbutler/workspace[🌳]
│ ●  ·4077353 (⌂|🏘)
│ ◎  B
│ ●  ·6b1a13b (⌂|🏘)
│ ●  ·03ad472 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ●  🟣d0df794 (✓)
│ ●  🟣09c6e08 (✓)
│ ●  🟣7b9f260 (✓)
╭─┤
│ ◎  main <> origin/main
│ ●  🟣4b3e5a8 (✓)
│ ●  🟣34d0715 (✓)
│ ●  🏁🟣eb5f731 (✓)
●  ·79bbb29 (⌂|🏘|✓)
●  ·fc98174 (⌂|🏘|✓)
●  ✂·a381df5 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:14:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on 79bbb29
└── ≡:15:B on 79bbb29
    └── :15:B
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  dependent
│ ◎  👉📕gitbutler/workspace[🌳]
│ ●  ·f8f33a7 (⌂|🏘)
├─╯
│ ◎  lane
│ │ ◎  on-top-of-dependent
├───╯
│ │ ◎  origin/advanced-lane
│ │ ◎  advanced-lane <> origin/advanced-lane
├───╯
● │  ·cbc6713 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // By default, the advanced lane is simply frozen as its remote contains the commit.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:7:advanced-lane <> origin/advanced-lane →:10: on fafd9d0
    └── :7:advanced-lane <> origin/advanced-lane →:10:
        └── ❄cbc6713 (🏘️) ►dependent, ►on-top-of-dependent, ►origin/advanced-lane

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·f8f33a7 (⌂|🏘)
├─╯
◎  📙dependent
│ ◎  lane
│ │ ◎  on-top-of-dependent
│ │ │ ◎  origin/advanced-lane
├─────╯
◎ │ │  📙advanced-lane <> origin/advanced-lane
├───╯
● │  ·cbc6713 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // When putting the dependent branch on top as empty segment, the frozen state is retained.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:8:dependent on fafd9d0 {1}
    ├── 📙:8:dependent
    └── 📙:7:advanced-lane <> origin/advanced-lane →:10:
        └── ❄cbc6713 (🏘️) ►dependent, ►on-top-of-dependent, ►origin/advanced-lane

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  C
├─╯
│ ◎  D
├─╯
│ ◎  E
├─╯
│ ◎  F
├─╯
│ ◎  👉📕gitbutler/workspace[🌳]
├─╯
│ ◎  origin/main
├─╯
●  ·2cde30a (⌂|🏘|✓)
●  ·1c938f4 (⌂|🏘|✓)
●  ·b82769f (⌂|🏘|✓)
●  ·988032f (⌂|🏘|✓)
●  ·cd5b655 (⌂|🏘|✓)
◎  main <> origin/main
●  🏁·2be54cd (⌂|🏘|✓)

"#]]
    );
    // Workspace is empty as everything is integrated.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:13:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a

"#]]
    );

    add_stack_with_segments(&mut meta, 0, "C", StackState::InWorkspace, &["B", "A"]);
    add_stack_with_segments(&mut meta, 1, "D", StackState::InWorkspace, &["E", "F"]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
◎ │ │  📙C
◎ │ │  📙B
◎ │ │  📙A
├───╯
│ ◎  📙D
│ ◎  📙E
│ ◎  📙F
├─╯
│ ◎  origin/main
├─╯
●  ·2cde30a (⌂|🏘|✓)
●  ·1c938f4 (⌂|🏘|✓)
●  ·b82769f (⌂|🏘|✓)
●  ·988032f (⌂|🏘|✓)
●  ·cd5b655 (⌂|🏘|✓)
◎  main <> origin/main
●  🏁·2be54cd (⌂|🏘|✓)

"#]]
    );

    // Empty stack segments on top of integrated portions will show, and nothing integrated shows.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:13:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 2cde30a
├── ≡📙:9:C on 2cde30a {0}
│   ├── 📙:9:C
│   ├── 📙:8:B
│   └── 📙:7:A
└── ≡📙:10:D on 2cde30a {1}
    ├── 📙:10:D
    ├── 📙:11:E
    └── 📙:12:F

"#]]
    );

    // However, when passing an additional old position of the target, we can show the now-integrated parts.
    // The stacks will always be created on top of the integrated segments as that's where their references are
    // (these segments are never conjured up out of thin air).
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:13:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣5 on 2be54cd
├── ≡📙:9:C on 2be54cd {0}
│   ├── 📙:9:C
│   ├── 📙:8:B
│   └── 📙:7:A
│       ├── ❄2cde30a (🏘️|✓) ►B, ►C, ►D, ►E, ►F, ►gitbutler/workspace[🌳], ►origin/main
│       ├── ❄1c938f4 (🏘️|✓)
│       ├── ❄b82769f (🏘️|✓)
│       ├── ❄988032f (🏘️|✓)
│       └── ❄cd5b655 (🏘️|✓)
└── ≡📙:10:D on 2be54cd {1}
    ├── 📙:10:D
    ├── 📙:11:E
    └── 📙:12:F
        ├── ❄2cde30a (🏘️|✓) ►A, ►B, ►C, ►D, ►E, ►gitbutler/workspace[🌳], ►origin/main
        ├── ❄1c938f4 (🏘️|✓)
        ├── ❄b82769f (🏘️|✓)
        ├── ❄988032f (🏘️|✓)
        └── ❄cd5b655 (🏘️|✓)

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
    let graph = Graph::from_commit_traversal(
        main_id,
        main_ref_name.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
●  ·41ed0e4 (⌂|🏘)
│ ◎  workspace
├─╯
│ ◎  origin/main
│ ●    🟣232ed06 (✓)
│ ├─╮
│ ◎ │  workspace-to-target
│ ● │  🟣abcfd9a (✓)
│ ● │  🟣bc86eba (✓)
│ ● │  🟣c7ae303 (✓)
├─╯ │
│   ◎  long-workspace-to-target
│   ●  🟣9e2a79e (✓)
│   ●  🟣fdeaa43 (✓)
│   ●  🟣30565ee (✓)
│   ●  🟣0c1c23a (✓)
│   ●  🟣56d152c (✓)
│   ●  🟣e6e1360 (✓)
│   ●  🟣1a22a39 (✓)
├───╯
●    ·9730cbf (⌂|🏘|✓)
├─╮
◎ │  main-to-workspace
● │  ·dc7ab57 (⌂|🏘|✓)
│ ◎  long-main-to-workspace
│ ●  ·77f31a0 (⌂|🏘|✓)
│ ●  ·eb17e31 (⌂|🏘|✓)
│ ●  ·fe2046b (⌂|🏘|✓)
│ ●  ·5532ef5 (⌂|🏘|✓)
│ ◎  👉main <> origin/main
│ ●  ·2438292 (⌂|🏘|✓)
├─╯
●  ·c056b75 (⌂|🏘|✓)
●  ·f49c977 (⌂|🏘|✓)
●  ·7b7ebb2 (⌂|🏘|✓)
●  ·dca4960 (⌂|🏘|✓)
●  ·11c29b8 (⌂|🏘|✓)
●  ·c32dd03 (⌂|🏘|✓)
●  ·b625665 (⌂|🏘|✓)
●  ·a821094 (⌂|🏘|✓)
●  ·bce0c5e (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );
    // Entrypoint is outside of the managed workspace, so it is projected as a
    // single-branch view. Target context is preserved and integrated commits below
    // the target trunk are pruned.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:32:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on dc7ab57

"#]]
    );

    // When setting a limit when traversing 'main', it is respected.
    // We still want it to be found and connected though, and it's notable that the limit kicks in
    // once everything reconciled.
    let graph = Graph::from_commit_traversal(
        main_id,
        main_ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options().with_limit_hint(1),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
●  ·41ed0e4 (⌂|🏘)
│ ◎  workspace
├─╯
│ ◎  origin/main
│ ●    🟣232ed06 (✓)
│ ├─╮
│ ◎ │  workspace-to-target
│ ● │  🟣abcfd9a (✓)
│ ● │  🟣bc86eba (✓)
│ ● │  🟣c7ae303 (✓)
├─╯ │
│   ◎  long-workspace-to-target
│   ●  🟣9e2a79e (✓)
│   ●  🟣fdeaa43 (✓)
│   ●  🟣30565ee (✓)
│   ●  🟣0c1c23a (✓)
│   ●  🟣56d152c (✓)
│   ●  🟣e6e1360 (✓)
│   ●  🟣1a22a39 (✓)
├───╯
●    ·9730cbf (⌂|🏘|✓)
├─╮
◎ │  main-to-workspace
● │  ·dc7ab57 (⌂|🏘|✓)
│ ◎  long-main-to-workspace
│ ●  ·77f31a0 (⌂|🏘|✓)
│ ●  ·eb17e31 (⌂|🏘|✓)
│ ●  ·fe2046b (⌂|🏘|✓)
│ ●  ·5532ef5 (⌂|🏘|✓)
│ ◎  👉main <> origin/main
│ ●  ·2438292 (⌂|🏘|✓)
├─╯
●  ·c056b75 (⌂|🏘|✓)
●  ·f49c977 (⌂|🏘|✓)
●  ·7b7ebb2 (⌂|🏘|✓)
●  ✂·dca4960 (⌂|🏘|✓)

"#]]
    );
    // The limit is visible as well. Target context is preserved in the downgraded
    // branch view, so integrated local/base commits are pruned.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:27:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on dc7ab57

"#]]
    );

    // From the workspace, even without limit, we don't traverse all of 'main' as it's uninteresting.
    // However, we wait for the target to be fully reconciled to get the proper workspace configuration.
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·41ed0e4 (⌂|🏘)
│ ◎  workspace
├─╯
│ ◎  origin/main
│ ●    🟣232ed06 (✓)
│ ├─╮
│ ◎ │  workspace-to-target
│ ● │  🟣abcfd9a (✓)
│ ● │  🟣bc86eba (✓)
│ ● │  🟣c7ae303 (✓)
├─╯ │
│   ◎  long-workspace-to-target
│   ●  🟣9e2a79e (✓)
│   ●  🟣fdeaa43 (✓)
│   ●  🟣30565ee (✓)
│   ●  🟣0c1c23a (✓)
│   ●  🟣56d152c (✓)
│   ●  🟣e6e1360 (✓)
│   ●  🟣1a22a39 (✓)
├───╯
●    ·9730cbf (⌂|🏘|✓)
├─╮
◎ │  main-to-workspace
● │  ·dc7ab57 (⌂|🏘|✓)
│ ◎  long-main-to-workspace
│ ●  ·77f31a0 (⌂|🏘|✓)
│ ●  ·eb17e31 (⌂|🏘|✓)
│ ●  ·fe2046b (⌂|🏘|✓)
│ ●  ·5532ef5 (⌂|🏘|✓)
│ ◎  main <> origin/main
│ ●  ·2438292 (⌂|🏘|✓)
├─╯
●  ·c056b75 (⌂|🏘|✓)
●  ·f49c977 (⌂|🏘|✓)
●  ·7b7ebb2 (⌂|🏘|✓)
●  ·dca4960 (⌂|🏘|✓)
●  ·11c29b8 (⌂|🏘|✓)
●  ✂·c32dd03 (⌂|🏘|✓)

"#]]
    );

    // Everything is integrated, nothing to see here.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:29:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on dc7ab57

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
    let graph = Graph::from_head(
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
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·9412ebd (⌂|🏘)
◎  A <> origin/A
●  ·8407093 (⌂|🏘)
●  ·7dfaa0c (⌂|🏘)
●  ·544e458 (⌂|🏘)
│ ◎  origin/A
│ ●  🟣975754f
│ ●  🟣f48ff69
│ │ ◎  origin/main
├───╯
◎ │  main <> origin/main
● │  ·685d644 (⌂|🏘|✓)
● │  ·cafdb27 (⌂|🏘|✓)
● │  ·c056b75 (⌂|🏘|✓)
● │  ·f49c977 (⌂|🏘|✓)
● │  ·7b7ebb2 (⌂|🏘|✓)
● │  ·dca4960 (⌂|🏘|✓)
● │  ·11c29b8 (⌂|🏘|✓)
● │  ·c32dd03 (⌂|🏘|✓)
● │  ·b625665 (⌂|🏘|✓)
● │  ·a821094 (⌂|🏘|✓)
● │  ·bce0c5e (⌂|🏘|✓)
├─╯
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:20:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 685d644
└── ≡:21:A <> origin/A →:22:⇡3⇣2 on 685d644
    └── :21:A <> origin/A →:22:⇡3⇣2
        ├── 🟣975754f ►origin/A
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
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·f514495 (⌂|🏘)
│ │ ◎  long-workspace-to-target
│ │ │ ◎  main-to-workspace
│ │ │ │ ◎  workspace
│ ├─────╯
│ │ │ │ ◎  origin/main
│ │ ├───╯
│ │ ● │  🟣024f837 (✓)
│ │ ● │  🟣64a8284 (✓)
│ │ ● │  🟣b72938c (✓)
│ │ ● │  🟣9ccbf6f (✓)
│ │ ● │  🟣5fa4905 (✓)
│ │ ● │  🟣43074d3 (✓)
│ │ ● │  🟣800d4a9 (✓)
│ │ ● │  🟣742c068 (✓)
│ │ ● │  🟣fe06afd (✓)
│ │ ● │    🟣3027746 (✓)
│ │ ├───╮
│ │ ● │ │  🟣f0d2a35 (✓)
│ ├─╯ │ │
│ ●   │ │  ·c9120f1 (⌂|🏘|✓)
│ ├───╮ │
│ ◎   │ │  long-main-to-workspace
│ ●   │ │  ·b39c7ec (⌂|🏘|✓)
│ ●   │ │  ·2983a97 (⌂|🏘|✓)
│ ●   │ │  ·144ea85 (⌂|🏘|✓)
│ ●   │ │  ·5aecfd2 (⌂|🏘|✓)
│ ◎   │ │  👉main <> origin/main
│ ●   │ │  ·bce0c5e (⌂|🏘|✓)
├─╯   │ │
│     │ ◎  longer-workspace-to-target
│     │ ●  🟣edf041f (✓)
│     │ ●  🟣d9f03f6 (✓)
│     │ ●  🟣8d1d264 (✓)
│     │ ●  🟣fa7ceae (✓)
│     │ ●  🟣95bdbf1 (✓)
│     │ ●  🟣5bac978 (✓)
│     ├─╯
│     ●  ·1126587 (⌂|🏘|✓)
├─────╯
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );
    // `main` is integrated, but it is the entrypoint, so the branch container is shown.
    // With preserved target context, integrated commits below the target trunk are pruned.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:28:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1

"#]]
    );

    // Now the target looks for the entrypoint, which is the workspace, something it can do more easily.
    // We wait for targets to fully reconcile as well.
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  B
├─╯
│ ◎  👉📕gitbutler/workspace[🌳]
│ ●  ·f514495 (⌂|🏘)
│ │ ◎  long-workspace-to-target
│ │ │ ◎  main-to-workspace
│ │ │ │ ◎  workspace
│ ├─────╯
│ │ │ │ ◎  origin/main
│ │ ├───╯
│ │ ● │  🟣024f837 (✓)
│ │ ● │  🟣64a8284 (✓)
│ │ ● │  🟣b72938c (✓)
│ │ ● │  🟣9ccbf6f (✓)
│ │ ● │  🟣5fa4905 (✓)
│ │ ● │  🟣43074d3 (✓)
│ │ ● │  🟣800d4a9 (✓)
│ │ ● │  🟣742c068 (✓)
│ │ ● │  🟣fe06afd (✓)
│ │ ● │    🟣3027746 (✓)
│ │ ├───╮
│ │ ● │ │  🟣f0d2a35 (✓)
│ ├─╯ │ │
│ ●   │ │  ·c9120f1 (⌂|🏘|✓)
│ ├───╮ │
│ ◎   │ │  long-main-to-workspace
│ ●   │ │  ·b39c7ec (⌂|🏘|✓)
│ ●   │ │  ·2983a97 (⌂|🏘|✓)
│ ●   │ │  ·144ea85 (⌂|🏘|✓)
│ ●   │ │  ·5aecfd2 (⌂|🏘|✓)
│ ◎   │ │  main <> origin/main
│ ●   │ │  ·bce0c5e (⌂|🏘|✓)
├─╯   │ │
│     │ ◎  longer-workspace-to-target
│     │ ●  🟣edf041f (✓)
│     │ ●  🟣d9f03f6 (✓)
│     │ ●  🟣8d1d264 (✓)
│     │ ●  🟣fa7ceae (✓)
│     │ ●  🟣95bdbf1 (✓)
│     │ ●  🟣5bac978 (✓)
│     ├─╯
│     ●  ·1126587 (⌂|🏘|✓)
├─────╯
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    let ws = graph.into_workspace()?;
    // Everything is integrated.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:28:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣17 on c9120f1

"#]]
    );

    // With a lower base for the target, we see more.
    let target_commit_id = repo.rev_parse_single("3183e43")?.detach();
    add_workspace_with_target(&mut meta, target_commit_id);

    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:28:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣24 on 3183e43
└── ≡:29:workspace on 3183e43
    └── :29:workspace
        ├── ·c9120f1 (🏘️|✓)
        └── ·1126587 (🏘️|✓) ►main-to-workspace

"#]]
    );

    // We can also add independent virtual branches to that new base.
    add_stack(&mut meta, 3, "A", StackState::InWorkspace);
    add_stack(&mut meta, 4, "B", StackState::InWorkspace);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:28:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣24 on 3183e43
└── ≡:29:workspace on 3183e43
    └── :29:workspace
        ├── ·c9120f1 (🏘️|✓)
        └── ·1126587 (🏘️|✓) ►main-to-workspace

"#]]
    );

    // We can also add stacked virtual branches to that new base.
    meta.data_mut().branches.clear();
    add_workspace_with_target(&mut meta, target_commit_id);
    add_stack_with_segments(&mut meta, 3, "A", StackState::InWorkspace, &["B"]);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:28:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣24 on 3183e43
└── ≡:29:workspace on 3183e43
    └── :29:workspace
        ├── ·c9120f1 (🏘️|✓)
        └── ·1126587 (🏘️|✓) ►main-to-workspace

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

"#]]
        .raw()
    );

    add_workspace(&mut meta);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A
│ ◎  👉📕gitbutler/workspace[🌳]
│ ●    ·2b30d94 (⌂|🏘)
╭─┼─╮
│ ◎ │  D
│ ● │  ·9895054 (⌂|🏘)
│ ◎ │  C
│ ● │  ·de625cc (⌂|🏘)
│ ● │  ·23419f8 (⌂|🏘)
│ ● │  ·5dc4389 (⌂|🏘)
│ │ ◎  B
│ │ ●  ·acdc49a (⌂|🏘)
│ │ ●  ·f0117e0 (⌂|🏘)
│ ├─╯
│ │ ◎  main <> origin/main
│ │ │ ◎  shared
│ ├───╯
│ │ │ ◎  origin/main
│ │ │ ●  🟣c08dc6b (✓)
╭───┬─╯
● │ │  ·0bad3af (⌂|🏘|✓)
├─╯ │
●   │  ·d4f537e (⌂|🏘|✓)
●   │  ·b448757 (⌂|🏘|✓)
●   │  ·e9a378d (⌂|🏘|✓)
├───╯
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    // A is still shown despite it being fully integrated, as it's still enclosed by the
    // workspace tip and the fork-point, at least when we provide the previous known location of the target.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:16:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣5 on 3183e43
├── ≡:17:D on 3183e43
│   ├── :17:D
│   │   └── ·9895054 (🏘️)
│   └── :20:C
│       ├── ·de625cc (🏘️)
│       ├── ·23419f8 (🏘️)
│       ├── ·5dc4389 (🏘️)
│       ├── ·d4f537e (🏘️|✓) ►shared
│       ├── ·b448757 (🏘️|✓)
│       └── ·e9a378d (🏘️|✓)
├── ≡:15:A on 3183e43
│   └── :15:A
│       ├── ·0bad3af (🏘️|✓)
│       ├── ·d4f537e (🏘️|✓) ►shared
│       ├── ·b448757 (🏘️|✓)
│       └── ·e9a378d (🏘️|✓)
└── ≡:18:B on 3183e43
    └── :18:B
        ├── ·acdc49a (🏘️)
        ├── ·f0117e0 (🏘️)
        ├── ·d4f537e (🏘️|✓) ►shared
        ├── ·b448757 (🏘️|✓)
        └── ·e9a378d (🏘️|✓)

"#]]
    );

    // If we do not, integrated portions are removed.
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:16:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on d4f537e
├── ≡:17:D on d4f537e
│   ├── :17:D
│   │   └── ·9895054 (🏘️)
│   └── :20:C
│       ├── ·de625cc (🏘️)
│       ├── ·23419f8 (🏘️)
│       └── ·5dc4389 (🏘️)
└── ≡:18:B on d4f537e
    └── :18:B
        ├── ·acdc49a (🏘️)
        └── ·f0117e0 (🏘️)

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

"#]]
        .raw()
    );

    add_workspace(&mut meta);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●      ·2b30d94 (⌂|🏘)
├─┬─╮
◎ │ │  D
● │ │  ·9895054 (⌂|🏘)
◎ │ │  C
● │ │  ·de625cc (⌂|🏘)
● │ │  ·23419f8 (⌂|🏘)
● │ │  ·5dc4389 (⌂|🏘)
│ ◎ │  A
│ ● │  ·0bad3af (⌂|🏘)
├─╯ │
│   ◎  B
│   ●  ·acdc49a (⌂|🏘)
│   ●  ·f0117e0 (⌂|🏘)
├───╯
│ ◎  main <> origin/main
│ │ ◎  shared
├───╯
● │  ·d4f537e (⌂|🏘)
● │  ·b448757 (⌂|🏘)
● │  ·e9a378d (⌂|🏘)
├─╯
│ ◎  origin/main
│ ●  🟣bce0c5e (✓)
├─╯
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    // Segments can definitely repeat
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:15:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡:16:D on 3183e43
│   ├── :16:D
│   │   └── ·9895054 (🏘️)
│   └── :19:C
│       ├── ·de625cc (🏘️)
│       ├── ·23419f8 (🏘️)
│       ├── ·5dc4389 (🏘️)
│       ├── ·d4f537e (🏘️) ►shared
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
├── ≡:17:A on 3183e43
│   └── :17:A
│       ├── ·0bad3af (🏘️)
│       ├── ·d4f537e (🏘️) ►shared
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
└── ≡:18:B on 3183e43
    └── :18:B
        ├── ·acdc49a (🏘️)
        ├── ·f0117e0 (🏘️)
        ├── ·d4f537e (🏘️) ►shared
        ├── ·b448757 (🏘️)
        └── ·e9a378d (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        Some(ref_name),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // Checking out anything inside the workspace yields the same result.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:15:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡:16:D on 3183e43
│   ├── :16:D
│   │   └── ·9895054 (🏘️)
│   └── :20:C
│       ├── ·de625cc (🏘️)
│       ├── ·23419f8 (🏘️)
│       ├── ·5dc4389 (🏘️)
│       ├── ·d4f537e (🏘️) ►shared
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
├── ≡👉:17:A on 3183e43
│   └── 👉:17:A
│       ├── ·0bad3af (🏘️)
│       ├── ·d4f537e (🏘️) ►shared
│       ├── ·b448757 (🏘️)
│       └── ·e9a378d (🏘️)
└── ≡:18:B on 3183e43
    └── :18:B
        ├── ·acdc49a (🏘️)
        ├── ·f0117e0 (🏘️)
        ├── ·d4f537e (🏘️) ►shared
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let target_ref = super::ref_name("refs/remotes/origin/main");
    let target_nodes = graph
        .nodes()
        .iter()
        .filter(|node| {
            matches!(
                node.kind(),
                NodeKind::Reference(reference)
                    if reference.ref_info.ref_name.as_ref() == target_ref.as_ref()
            )
        })
        .count();
    assert_eq!(
        target_nodes, 1,
        "the target ref is represented exactly once"
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·335d6f2 (⌂|🏘)
╭─┤
◎ │  📙dependent
│ │ ◎  lane
│ ├─╯
│ │ ◎  origin/advanced-lane
├───╯
◎ │  📙advanced-lane <> origin/advanced-lane
● │  ·cbc6713 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // The dependent branch is empty and on top of the one with the remote
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:8:dependent on fafd9d0 {1}
    ├── 📙:8:dependent
    └── 📙:7:advanced-lane <> origin/advanced-lane →:9:
        └── ❄cbc6713 (🏘️) ►dependent, ►origin/advanced-lane

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·335d6f2 (⌂|🏘)
╭─┤
│ │ ◎  lane
│ ├─╯
│ │ ◎  origin/advanced-lane
├───╯
◎ │  📙advanced-lane <> origin/advanced-lane
◎ │  📙dependent
● │  ·cbc6713 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // Having done something unusual, which is to put the dependent branch
    // underneath the other already pushed, it creates a different view of ownership.
    // It's probably OK to leave it like this for now, and instead allow users to reorder
    // these more easily.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:7:advanced-lane <> origin/advanced-lane →:9: on fafd9d0 {1}
    ├── 📙:7:advanced-lane <> origin/advanced-lane →:9:
    └── 📙:8:dependent
        └── ❄cbc6713 (🏘️) ►advanced-lane, ►origin/advanced-lane

"#]]
    );

    let (id, ref_name) = id_at(&repo, "advanced-lane");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡👉📙:7:advanced-lane <> origin/advanced-lane →:9: on fafd9d0 {1}
    ├── 👉📙:7:advanced-lane <> origin/advanced-lane →:9:
    └── 📙:8:dependent
        └── ❄cbc6713 (🏘️) ►advanced-lane, ►origin/advanced-lane

"#]]
    );

    let (id, ref_name) = id_at(&repo, "dependent");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:7:advanced-lane <> origin/advanced-lane →:9: on fafd9d0 {1}
    ├── 📙:7:advanced-lane <> origin/advanced-lane →:9:
    └── 👉📙:8:dependent
        └── ❄cbc6713 (🏘️) ►advanced-lane, ►origin/advanced-lane

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
*   e982e8a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * aff8449 (B-on-A) B-on-A
* | 4f1bb32 (C-on-A) C-on-A
|/  
| * b627ca7 (origin/A) A-on-remote
|/  
* e255adc (A) A
* fafd9d0 (origin/main, main) init

"#]]
        .raw()
    );

    add_stack_with_segments(&mut meta, 1, "C-on-A", StackState::InWorkspace, &[]);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A <> origin/A
│ ◎    👉📕gitbutler/workspace[🌳]
│ ├─╮
│ │ ●  ·e982e8a (⌂|🏘)
│ ╭─┤
│ ◎ │  📙C-on-A
│ ● │  ·4f1bb32 (⌂|🏘)
├─╯ │
│   ◎  B-on-A
│   ●  ·aff8449 (⌂|🏘)
├───╯
│ ◎  origin/A
│ ●  🟣b627ca7
├─╯
●  ·e255adc (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:9:C-on-A on fafd9d0 {1}
│   └── 📙:9:C-on-A
│       ├── ·4f1bb32 (🏘️)
│       └── ·e255adc (🏘️) ►A
└── ≡:10:B-on-A on fafd9d0
    └── :10:B-on-A
        ├── ·aff8449 (🏘️)
        └── ·e255adc (🏘️) ►A

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·873d056 (⌂|🏘)
╭─┤
◎ │  📙advanced-lane
● │  ·cbc6713 (⌂|🏘)
│ ◎  📙lane
├─╯
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘)
◎  origin/main
●  🏁🟣da83717 (✓)

"#]]
    );

    // Since `lane` is connected directly, no segment has to be created.
    // However, as nothing is integrated, it really is another name for `main` now,
    // `main` is nothing special.
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1
├── ≡📙:6:advanced-lane {0}
│   └── 📙:6:advanced-lane
│       ├── ·cbc6713 (🏘️)
│       └── ·fafd9d0 (🏘️) ►lane, ►main
└── ≡📙:7:lane {1}
    └── 📙:7:lane
        └── ·fafd9d0 (🏘️) ►main

"#]]
    );

    // Reverse the order of stacks in the worktree data.
    for (idx, name) in lanes.into_iter().rev().enumerate() {
        add_stack_with_segments(&mut meta, idx, name, StackState::InWorkspace, &[]);
    }
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·873d056 (⌂|🏘)
╭─┤
◎ │  📙advanced-lane
● │  ·cbc6713 (⌂|🏘)
│ ◎  📙lane
├─╯
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘)
◎  origin/main
●  🏁🟣da83717 (✓)

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1
├── ≡📙:6:advanced-lane {1}
│   └── 📙:6:advanced-lane
│       ├── ·cbc6713 (🏘️)
│       └── ·fafd9d0 (🏘️) ►lane, ►main
└── ≡📙:7:lane {0}
    └── 📙:7:lane
        └── ·fafd9d0 (🏘️) ►main

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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·a221221 (⌂|🏘)
├─╯
◎  📙A <> origin/A
●  ·aadad9d (⌂|🏘)
◎  origin/main
●  ·96a2408 (⌂|🏘|✓)
│ ◎  integrated
├─╯
│ ◎  origin/A
│ ●  🟣2b1808c
├─╯
●  ·f15ca75 (⌂|🏘|✓)
●  ·9456d79 (⌂|🏘|✓)
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // Remote tracking branches we just want to aggregate, just like anonymous segments,
    // but only when another target is provided (the old position, `main`).
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on fafd9d0
└── ≡📙:11:A <> origin/A →:12:⇡1⇣1 on fafd9d0 {1}
    ├── 📙:11:A <> origin/A →:12:⇡1⇣1
    │   ├── 🟣2b1808c ►origin/A
    │   └── ·aadad9d (🏘️)
    └── :8:origin/main →:7:
        ├── ❄96a2408 (🏘️|✓)
        ├── ❄f15ca75 (🏘️|✓) ►integrated
        └── ❄9456d79 (🏘️|✓)

"#]]
    );

    // Otherwise, nothing that's integrated is shown. Note how 96a2408 seems missing,
    // but it's skipped because it's actually part of an integrated otherwise ignored segment.
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.commits_on_remote)
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        [id_by_rev(&repo, "origin/A").detach()],
        "history reachable from the local stack or its base is not remote-only"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 96a2408
└── ≡📙:11:A <> origin/A →:12:⇡1⇣1 on 96a2408 {1}
    └── 📙:11:A <> origin/A →:12:⇡1⇣1
        ├── 🟣2b1808c ►origin/A
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·4f08b8d (⌂|🏘)
├─╯
◎  📙B <> origin/B
●  ·da597e8 (⌂|🏘)
◎  📙A <> origin/A
│ ◎  main <> origin/main
│ │ ◎  origin/B
│ │ ●  🟣e0bd0a7
│ │ ◎  origin/A
│ │ ●  🟣0b6b861
│ ├─╯
│ │ ◎  origin/main
│ │ ●  🟣b694668 (✓)
╭─┬─╯
● │  ·1818c17 (⌂|🏘|✓)
├─╯
●  🏁·281456a (⌂|🏘|✓)

"#]]
    );

    // This is the default as it includes both the integrated and non-integrated segment.
    // Note how there is no expensive computation to see if remote commits are the same,
    // it's all ID-based.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 281456a
└── ≡📙:11:B <> origin/B →:12:⇡1⇣2 on 281456a {0}
    ├── 📙:11:B <> origin/B →:12:⇡1⇣2
    │   ├── 🟣e0bd0a7 ►origin/B
    │   ├── 🟣0b6b861 ►origin/A
    │   └── ·da597e8 (🏘️)
    └── 📙:9:A <> origin/A →:13:⇣1
        ├── 🟣0b6b861 ►origin/A
        └── ·1818c17 (🏘️|✓)

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "A"),
    )?
    .validated()?;
    // Pretending we are rebased onto A still shows the same remote commits.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1818c17
└── ≡📙:11:B <> origin/B →:13:⇡1⇣2 on 1818c17 {0}
    ├── 📙:11:B <> origin/B →:13:⇡1⇣2
    │   ├── 🟣e0bd0a7 ►origin/B
    │   ├── 🟣0b6b861 ►origin/A
    │   └── ·da597e8 (🏘️)
    └── 📙:7:A <> origin/A →:12:⇣1
        └── 🟣0b6b861 ►origin/A

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:7:top on fafd9d0 {0}
    ├── 📙:7:top
    │   └── ❄bfbff44 (🏘️) ►origin/bottom
    └── 📙:9:bottom <> origin/bottom →:8:⇣1
        ├── 🟣bfbff44 (🏘️) ►top, ►origin/bottom
        └── ❄7fdb58d (🏘️)

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·1109eb2 (⌂|🏘)
├─╯
◎  📙D <> origin/D
●  ·624e118 (⌂|🏘)
│ ◎  origin/A
│ │ ◎  origin/B
│ ├─╯
│ │ ◎  origin/C
│ ├─╯
│ │ ◎  origin/D
│ │ ●  🟣3045ea6
│ ├─╯
│ ●  🟣1818c17
│ │ ◎  origin/main
├───╯
◎ │  main <> origin/main
● │  ·0b6b861 (⌂|🏘|✓)
├─╯
●  🏁·281456a (⌂|🏘|✓)

"#]]
    );

    let ambiguous_remote_tip = repo.rev_parse_single("origin/A")?.detach();
    for remote_ref in [
        "refs/remotes/origin/A",
        "refs/remotes/origin/B",
        "refs/remotes/origin/C",
    ] {
        let remote_ref = super::ref_name(remote_ref);
        let (_, remote_node) = graph
            .node_by_ref_name(remote_ref.as_ref())
            .expect("remote tracking reference should be present");
        assert_eq!(
            remote_node.ref_info.commit_id,
            Some(ambiguous_remote_tip),
            "{remote_ref} should resolve to the commit its Git ref points to, showing that something special happened here"
        );
    }

    // only one remote commit as unrelated remotes split a linear segment
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
└── ≡📙:9:D <> origin/D →:10:⇡1⇣2 on 0b6b861 {0}
    └── 📙:9:D <> origin/D →:10:⇡1⇣2
        ├── 🟣3045ea6 ►origin/D
        ├── 🟣1818c17 ►origin/A, ►origin/B, ►origin/C
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·deeae50 (⌂|🏘)
├─╯
◎  📙D <> origin/D
●  ·353471f (⌂|🏘)
●  ·8a4b945 (⌂|🏘)
●  ·e0bd0a7 (⌂|🏘)
│ ◎  origin/D
│ ●  🟣bbd4ff6
│ ◎  origin/C
│ ●  🟣e5f5a87
│ ◎  origin/B
│ ●  🟣da597e8
│ ◎  origin/A
│ ●  🟣1818c17
│ │ ◎  origin/main
├───╯
◎ │  main <> origin/main
● │  ·0b6b861 (⌂|🏘|✓)
├─╯
●  🏁·281456a (⌂|🏘|✓)

"#]]
    );

    // We let each remote on the path down own a commit so we only see one remote commit here,
    // the one belonging to the last remaining associated remote tracking branch of D.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:12:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 0b6b861
└── ≡📙:13:D <> origin/D →:14:⇡3⇣4 on 0b6b861 {0}
    └── 📙:13:D <> origin/D →:14:⇡3⇣4
        ├── 🟣bbd4ff6 ►origin/D
        ├── 🟣e5f5a87 ►origin/C
        ├── 🟣da597e8 ►origin/B
        ├── 🟣1818c17 ►origin/A
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
│ ◎  origin/A
│ ●  🟣4fe5a6f
│ ◎  A <> origin/A
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓!
└── ≡:4:A <> origin/A →:7:⇣1
    ├── :4:A <> origin/A →:7:⇣1
    │   ├── 🟣4fe5a6f ►origin/A
    │   ├── ❄a62b0de (🏘️) ►gitbutler/workspace[🌳]
    │   └── ❄120a217 (🏘️)
    └── :6:main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
│ ◎  origin/A
│ ●  🟣4fe5a6f
│ ◎  👉A <> origin/A
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    // Main can be a normal segment if there is no target ref.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:4:A <> origin/A →:7:⇣1
    ├── 👉:4:A <> origin/A →:7:⇣1
    │   ├── 🟣4fe5a6f ►origin/A
    │   ├── ❄a62b0de (🏘️) ►gitbutler/workspace[🌳]
    │   └── ❄120a217 (🏘️)
    └── :6:main
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A <> origin/A
│ ◎  B
├─╯
│ ◎  origin/A
│ ●  🟣4fe5a6f
│ ◎  👉📕gitbutler/workspace[🌳]
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:6:gitbutler/workspace[🌳] <> ✓!
└── ≡:0:anon
    ├── :0:anon
    │   ├── ·a62b0de (🏘️) ►A, ►B, ►gitbutler/workspace[🌳]
    │   └── ·120a217 (🏘️)
    └── :7:main
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // We can help it by adding metadata.
    // Note how the selection still manages to hold on to the `A` which now gets its very own
    // empty segment.
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let (id, a_ref) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  origin/A
●  🟣4fe5a6f
◎    📕gitbutler/workspace[🌳]
├─╮
◎ │  👉A <> origin/A
◎ │  📙B
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace.lower_bound,
        Some(id_by_rev(&repo, "A").detach()),
        "the computed common ancestor remains workspace display context without becoming a target"
    );
    assert_eq!(
        workspace.stacks[0]
            .segments
            .iter()
            .flat_map(|segment| segment.commits.iter().map(|commit| commit.id))
            .collect::<Vec<_>>(),
        [
            id_by_rev(&repo, "A").detach(),
            id_by_rev(&repo, "A~1").detach(),
            id_by_rev(&repo, "main").detach(),
        ],
        "a target-free stack retains ordinary history through main"
    );
    assert_eq!(
        workspace.stacks[0]
            .segments
            .last()
            .and_then(|segment| segment.ref_name())
            .map(ToString::to_string),
        Some("refs/heads/main".into()),
        "the target-free stack retains its named main root"
    );

    // Main can be a normal segment if there is no target ref.
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:6:gitbutler/workspace[🌳] <> ✓! on a62b0de
└── ≡👉:4:A <> origin/A →:8:⇣1 {1}
    ├── 👉:4:A <> origin/A →:8:⇣1
    │   └── 🟣4fe5a6f ►origin/A
    ├── 📙:5:B
    │   ├── ❄a62b0de (🏘️) ►A, ►gitbutler/workspace[🌳]
    │   └── ❄120a217 (🏘️)
    └── :7:main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    // Finally, show the normal version with just disambiguated 'B".
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
◎ │  📙B
├─╯
│ ◎  origin/A
│ ●  🟣4fe5a6f
│ ◎  A <> origin/A
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:6:gitbutler/workspace[🌳] <> ✓! on a62b0de
└── ≡📙:5:B {1}
    ├── 📙:5:B
    │   ├── ·a62b0de (🏘️) ►A, ►gitbutler/workspace[🌳]
    │   └── ·120a217 (🏘️)
    └── :7:main
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // Order is respected
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);
    let graph = Graph::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // The remote tracking branch must remain linked.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:6:gitbutler/workspace[🌳] <> ✓! on a62b0de
└── ≡📙:5:B {1}
    ├── 📙:5:B
    ├── 👉📙:4:A <> origin/A →:8:⇣1
    │   ├── 🟣4fe5a6f ►origin/A
    │   ├── ❄a62b0de (🏘️) ►B, ►gitbutler/workspace[🌳]
    │   └── ❄120a217 (🏘️)
    └── :7:main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    // Order is respected, vice-versa
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let graph =
        Graph::from_commit_traversal(id, a_ref, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:6:gitbutler/workspace[🌳] <> ✓! on a62b0de
└── ≡👉📙:4:A <> origin/A →:8:⇣1 {1}
    ├── 👉📙:4:A <> origin/A →:8:⇣1
    │   └── 🟣4fe5a6f ►origin/A
    ├── 📙:5:B
    │   ├── ❄a62b0de (🏘️) ►A, ►gitbutler/workspace[🌳]
    │   └── ❄120a217 (🏘️)
    └── :7:main
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
│ ◎  origin/A
│ ◎  A <> origin/A
├─╯
│ ◎  origin/B
│ ◎  B <> origin/B
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓!
└── ≡:0:anon
    ├── :0:anon
    │   ├── ·a62b0de (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►origin/A, ►origin/B
    │   └── ·120a217 (🏘️)
    └── :8:main <> origin/main⇡1
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // Remote handling is still happening when A is disambiguated by entrypoint.
    let (id, a_ref) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
│ ◎  origin/A
│ ◎  👉A <> origin/A
├─╯
│ ◎  origin/B
│ ◎  B <> origin/B
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓!
└── ≡:0:anon
    ├── :0:anon
    │   ├── ·a62b0de (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►origin/A, ►origin/B
    │   └── ·120a217 (🏘️)
    └── :8:main <> origin/main⇡1
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // The same is true when starting at a different ref.
    let (id, b_ref) = id_at(&repo, "B");
    let graph =
        Graph::from_commit_traversal(id, b_ref, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓!
└── ≡:0:anon
    ├── :0:anon
    │   ├── ·a62b0de (🏘️) ►A, ►B, ►gitbutler/workspace[🌳], ►origin/A, ►origin/B
    │   └── ·120a217 (🏘️)
    └── :8:main <> origin/main⇡1
        └── ·fafd9d0 (🏘️)

"#]]
    );

    // If disambiguation happens through the workspace, 'A' still shows the right remote, and 'B' as well
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);
    let graph = Graph::from_commit_traversal(
        id,
        a_ref.clone(),
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    📕gitbutler/workspace[🌳]
├─╮
│ │ ◎  origin/A
├───╯
◎ │  👉A <> origin/A
│ │ ◎  origin/B
├───╯
◎ │  📙B <> origin/B
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓! on a62b0de
└── ≡👉:3:A <> origin/A →:6: {1}
    ├── 👉:3:A <> origin/A →:6:
    ├── 📙:4:B <> origin/B →:7:
    │   ├── ❄a62b0de (🏘️) ►A, ►gitbutler/workspace[🌳], ►origin/A, ►origin/B
    │   └── ❄120a217 (🏘️)
    └── :8:main <> origin/main
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A <> origin/A
│ ◎  👉📕gitbutler/workspace[🌳]
│ ●  ·3ea2742 (⌂|🏘)
├─╯
│ ◎  origin/A
│ ●  🟣4fe5a6f
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    // TODO: add more stacks.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓!
└── ≡:6:A <> origin/A →:8:⇣1
    ├── :6:A <> origin/A →:8:⇣1
    │   ├── 🟣4fe5a6f ►origin/A
    │   ├── ❄a62b0de (🏘️)
    │   └── ❄120a217 (🏘️)
    └── :7:main
        └── ❄fafd9d0 (🏘️)

"#]]
    );

    let (id, ref_name) = id_at(&repo, "A");
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉A <> origin/A
│ ◎  📕gitbutler/workspace[🌳]
│ ●  ·3ea2742 (⌂|🏘)
├─╯
│ ◎  origin/A
│ ●  🟣4fe5a6f
├─╯
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓!
└── ≡👉:6:A <> origin/A →:8:⇣1
    ├── 👉:6:A <> origin/A →:8:⇣1
    │   ├── 🟣4fe5a6f ►origin/A
    │   ├── ❄a62b0de (🏘️)
    │   └── ❄120a217 (🏘️)
    └── :7:main
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
│ ◎  origin/main
├─╯
●  ·8ee08de (⌂|🏘|✓)
◎  A
●  ·120a217 (⌂|🏘|✓)
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:3:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 120a217

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    assert!(
        graph.managed_workspace_commit_id().is_none(),
        "a workspace reference without a managed workspace commit must not absorb its own target commit"
    );
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·dca94a4 (⌂|🏘)
◎  A
●  ·120a217 (⌂|🏘)
◎  main
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    let workspace = graph.into_workspace()?;
    assert!(
        matches!(
            workspace.kind,
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. }
        ),
        "an unmanaged commit under the workspace ref must preserve the missing-workspace warning"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓!
└── ≡:0:anon
    ├── :0:anon
    │   └── ·dca94a4 (🏘️) ►gitbutler/workspace[🌳]
    ├── :4:A
    │   └── ·120a217 (🏘️)
    └── :5:main
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    // Notably we also pick up 'lane' which sits on the base.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
◎ │ │  📙lane
├───╯
● │  ·cbc6713 (⌂|🏘)
◎ │  📙lane-segment-01
◎ │  📙lane-segment-02
│ ◎  📙lane-2
│ ◎  📙lane-2-segment-01
│ ◎  📙lane-2-segment-02
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:10:lane on fafd9d0 {0}
│   ├── 📙:10:lane
│   │   └── ·cbc6713 (🏘️) ►gitbutler/workspace[🌳]
│   ├── 📙:5:lane-segment-01
│   └── 📙:6:lane-segment-02
└── ≡📙:2:lane-2 on fafd9d0 {1}
    ├── 📙:2:lane-2
    ├── 📙:3:lane-2-segment-01
    └── 📙:4:lane-2-segment-02

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    // the order is maintained as provided in the workspace.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
│ ◎ │  📙lane
│ ├─╯
│ ●  ·cbc6713 (⌂|🏘)
├─╯
◎  📙lane-2
◎  📙lane-2-segment-01
◎  📙lane-2-segment-02
│ ◎  📙lane-segment-01
│ ◎  📙lane-segment-02
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:2:lane-2 on fafd9d0 {0}
│   ├── 📙:2:lane-2
│   ├── 📙:3:lane-2-segment-01
│   └── 📙:4:lane-2-segment-02
└── ≡📙:10:lane on fafd9d0 {0}
    ├── 📙:10:lane
    │   └── ·cbc6713 (🏘️) ►gitbutler/workspace[🌳]
    ├── 📙:2:lane-2
    ├── 📙:3:lane-2-segment-01
    └── 📙:4:lane-2-segment-02

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·4f08b8d (⌂|🏘)
◎  B
●  ·da597e8 (⌂|🏘)
◎  A <> origin/A
●  ·1818c17 (⌂|🏘)
│ ◎  main <> origin/main
├─╯
│ ◎  origin/A
│ │ ◎  origin/main
│ ├─╯
│ ●  🟣0b6b861 (✓)
├─╯
●  🏁·281456a (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 281456a
└── ≡:9:B on 281456a
    ├── :9:B
    │   └── ·da597e8 (🏘️)
    └── :10:A <> origin/A →:6:⇡1⇣1
        ├── 🟣0b6b861 (✓) ►origin/A, ►origin/main
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    // Standard handling after traversal and post-processing.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·8926b15 (⌂|🏘)
◎  main
●  ·3686017 (⌂|🏘)
◎  gitbutler/edit
●  ·9725482 (⌂|🏘)
◎  gitbutler/target
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );

    // But special handling for workspace views.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:4:gitbutler/workspace[🌳] <> ✓!
└── ≡:5:main
    └── :5:main
        ├── ·3686017 (🏘️)
        ├── ·9725482 (🏘️) ►gitbutler/edit
        └── ·fafd9d0 (🏘️) ►gitbutler/target

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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        md.project_meta(),
        // standard_options_with_extra_target(&repo, "gitbutler/target"),
        standard_options(),
    )?
    .validated()?;
    // Standard handling after traversal and post-processing.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·270738b (⌂|🏘)
◎  A
●  ·c59457b (⌂|🏘)
◎  gitbutler/edit
●  ·e146f13 (⌂|🏘)
│ ◎  origin/gitbutler/target
│ │ ◎  origin/main
├───╯
◎ │  main <> origin/main
● │  ·971953d (⌂|🏘)
├─╯
◎  gitbutler/target <> origin/gitbutler/target
●  ·ce09734 (⌂|🏘|✓)
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // But special handling for workspace views. Note how we don't overshoot
    // and stop exactly where we have to, magically even.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/gitbutler/target on ce09734
└── ≡:9:A
    ├── :9:A
    │   ├── ·c59457b (🏘️)
    │   └── ·e146f13 (🏘️) ►gitbutler/edit
    └── :11:main <> origin/main →:12:
        └── ❄971953d (🏘️) ►origin/main

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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●        ·fe6ba62 (⌂|🏘)
├─┬─┬─╮
│ ◎ │ │  B
│ ● │ │  ·2f8f06d (⌂|🏘)
│ │ ◎ │  C
│ │ ● │  ·3f7c4e6 (⌂|🏘)
│ │ ● │  ·b6895d7 (⌂|🏘)
│ │ │ ◎  new-name-for-D
│ │ │ ●  ·ed36e3b (⌂|🏘)
│ │ ├─╯
│ │ │ ◎  origin/B-middle
│ ├───╯
│ │ │ ◎  origin/main
│ │ │ ◎  main <> origin/main
│ │ │ ●  ·867927f (⌂|✓)
│ ╭───┤
│ │ │ ●  ·6e03461 (⌂|✓)
╭───┬─╯
● │ │  ·a62b0de (⌂|🏘|✓)
● │ │  ·120a217 (⌂|🏘|✓)
├───╯
│ ●  ·91bc3fc (⌂|🏘|✓)
│ ●  ·cf9330f (⌂|🏘|✓)
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // If it doesn't know how the workspace should be looking like, i.e. which branches are contained,
    // nothing special happens.
    // The branches that are outside the workspace don't exist and segments are flattened.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:15:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on fafd9d0
├── ≡:16:B on 91bc3fc
│   └── :16:B
│       └── ·2f8f06d (🏘️)
├── ≡:17:C on fafd9d0
│   └── :17:C
│       ├── ·3f7c4e6 (🏘️)
│       └── ·b6895d7 (🏘️)
└── ≡:18:new-name-for-D on fafd9d0
    └── :18:new-name-for-D
        └── ·ed36e3b (🏘️)

"#]]
    );

    // However, when the desired workspace is set up, the traversal will include these extra tips.
    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &["A-middle"]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["B-middle"]);
    add_stack_with_segments(&mut meta, 2, "C", StackState::InWorkspace, &["C-bottom"]);
    add_stack_with_segments(&mut meta, 3, "D", StackState::InWorkspace, &[]);

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, ":/init"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📙A
●  ·c83f258 (⌂)
│ ◎  📙B-middle <> origin/B-middle
│ ●  ·c8f73c7 (⌂)
│ ◎  intermediate-branch
│ ●  ·ff75b80 (⌂)
│ │ ◎  📙C-bottom
│ │ ●    ·790a17d (⌂)
│ │ ├─╮
│ │ ● │  ·969aaec (⌂)
│ │ │ ◎  tmp
│ │ │ ●  ·631be19 (⌂)
│ │ ├─╯
│ │ │ ◎  📙D
│ │ │ ●  ·71dad1a (⌂)
│ │ │ │ ◎      👉📕gitbutler/workspace[🌳]
│ │ │ │ ├─┬─╮
│ │ │ │ │ │ ●  ·fe6ba62 (⌂|🏘)
╭─────┬─┬─┬─╯
│ │ │ │ ◎ │  📙B
│ │ │ │ ● │  ·2f8f06d (⌂|🏘)
│ ├─────╯ │
│ │ │ │   ◎  📙C
│ │ │ │   ●  ·3f7c4e6 (⌂|🏘)
│ │ ├─────╯
│ │ ● │  ·b6895d7 (⌂|🏘)
│ │ │ │ ◎  new-name-for-D
│ │ │ ├─╯
│ │ │ ●  ·ed36e3b (⌂|🏘)
│ │ ├─╯
│ │ │ ◎  origin/A-middle
│ │ │ ◎  📙A-middle <> origin/A-middle
│ │ │ ●  ·27c2545 (⌂)
│ │ │ │ ◎  origin/B-middle
│ ├─────╯
│ │ │ │ ◎  origin/main
│ │ │ │ ◎  main <> origin/main
│ │ │ │ ●  ·867927f (⌂|✓)
│ ╭─────┤
│ │ │ │ ●  ·6e03461 (⌂|✓)
╭───┬───╯
● │ │ │  ·a62b0de (⌂|🏘|✓)
├─────╯
● │ │  ·120a217 (⌂|🏘|✓)
├───╯
│ ●  ·91bc3fc (⌂|🏘|✓)
│ ●  ·cf9330f (⌂|🏘|✓)
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

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
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:23:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣6 on fafd9d0
├── ≡📙:24:B on fafd9d0 {1}
│   └── 📙:24:B
│       ├── ·2f8f06d (🏘️)
│       ├── ·91bc3fc (🏘️|✓) ►origin/B-middle
│       └── ·cf9330f (🏘️|✓)
├── ≡📙:25:C on fafd9d0 {2}
│   └── 📙:25:C
│       ├── ·3f7c4e6 (🏘️)
│       └── ·b6895d7 (🏘️)
├── ≡:5:anon on fafd9d0
│   └── :5:anon
│       ├── ·a62b0de (🏘️|✓)
│       └── ·120a217 (🏘️|✓)
└── ≡:26:new-name-for-D on fafd9d0
    └── :26:new-name-for-D
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
    let graph = Graph::from_commit_traversal(
        id,
        ref_name,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  ·873d056 (⌂|🏘)
╭─┬─╯
│ ◎  📙advanced-lane
│ ●  ·cbc6713 (⌂|🏘)
◎ │  👉📙lane
├─╯
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)
◎  origin/main
●  🏁🟣da83717 (✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
├── ≡👉📙:7:lane on fafd9d0 {0}
│   └── 👉📙:7:lane
└── ≡📙:6:advanced-lane on fafd9d0 {1}
    └── 📙:6:advanced-lane
        └── ·cbc6713 (🏘️)

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  ·873d056 (⌂|🏘)
╭─┬─╯
│ ◎  📙advanced-lane
│ ●  ·cbc6713 (⌂|🏘)
◎ │  📙lane
├─╯
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)
◎  origin/main
●  🏁🟣da83717 (✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
├── ≡📙:4:lane on fafd9d0 {0}
│   └── 📙:4:lane
└── ≡📙:8:advanced-lane on fafd9d0 {1}
    └── 📙:8:advanced-lane
        └── ·cbc6713 (🏘️)

"#]]
    );

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·873d056 (⌂|🏘)
╭─┤
◎ │  📙advanced-lane
● │  ·cbc6713 (⌂|🏘)
│ ◎  📙lane
├─╯
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘)
◎  origin/main
●  🏁🟣da83717 (✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1
├── ≡📙:6:advanced-lane {1}
│   └── 📙:6:advanced-lane
│       ├── ·cbc6713 (🏘️)
│       └── ·fafd9d0 (🏘️) ►lane, ►main
└── ≡📙:7:lane {0}
    └── 📙:7:lane
        └── ·fafd9d0 (🏘️) ►main

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  ·a7131b1 (⌂|🏘)
│ │ ◎  intermediate-ref
│ │ ●  ·4d3831e (⌂|🏘)
│ │ ●    ·468357f (⌂|🏘)
│ │ ├─╮
│ │ │ ◎  branch-on-top
│ │ │ ●  ·d3166f7 (⌂|🏘)
│ │ ├─╯
│ │ ●  ·118ddbb (⌂|🏘)
│ │ ●  ·619d548 (⌂|🏘)
╭─┬─╯
│ ◎  📙B
│ ●  ·8a352d5 (⌂|🏘)
◎ │  📙A
● │  ·6fdab32 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  ·bce0c5e (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    // We show the original 'native' configuration without pruning anything, even though
    // it contains the workspace commit 619d548.
    // It's up to the caller to deal with this situation as the workspace now is marked differently.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:12:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:14:A on bce0c5e {0}
│   └── 📙:14:A
│       └── ·6fdab32 (🏘️)
└── ≡📙:15:B on bce0c5e {1}
    └── 📙:15:B
        └── ·8a352d5 (🏘️)

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?;
    // The extra-target as would happen in the typical case would change nothing though.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  ·a7131b1 (⌂|🏘)
│ │ ◎  intermediate-ref
│ │ ●  ·4d3831e (⌂|🏘)
│ │ ●    ·468357f (⌂|🏘)
│ │ ├─╮
│ │ │ ◎  branch-on-top
│ │ │ ●  ·d3166f7 (⌂|🏘)
│ │ ├─╯
│ │ ●  ·118ddbb (⌂|🏘)
│ │ ●  ·619d548 (⌂|🏘)
╭─┬─╯
│ ◎  📙B
│ ●  ·8a352d5 (⌂|🏘)
◎ │  📙A
● │  ·6fdab32 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  ·bce0c5e (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:12:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:14:A on bce0c5e {0}
│   └── 📙:14:A
│       └── ·6fdab32 (🏘️)
└── ≡📙:15:B on bce0c5e {1}
    └── 📙:15:B
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·da912a8 (⌂|🏘)
│ ◎  intermediate-ref
│ ●  ·198eaf8 (⌂|🏘)
│ ●    ·3147997 (⌂|🏘)
│ ├─╮
│ │ ◎  branch-on-top
│ │ ●  ·dd7bb9a (⌂|🏘)
│ ├─╯
│ ●  ·9785229 (⌂|🏘)
│ ●  ·c58f157 (⌂|🏘)
├─╯
◎  📙A
●  ·6fdab32 (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  ·bce0c5e (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    // Here we'd show what happens if the workspace commit is somewhere in the middle
    // of the segment. This is relevant for code trying to find it, which isn't done here.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:13:A on bce0c5e {0}
    └── 📙:13:A
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  origin/A
│ ◎  origin/gitbutler/workspace
│ ◎  👉📕gitbutler/workspace[🌳] <> origin/gitbutler/workspace
╭─┤
│ ●  ·00e1860 (⌂|🏘)
├─╯
◎  📙A <> origin/A
●  ·6507810 (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  ·b625665 (⌂|🏘|✓)
●  ·a821094 (⌂|🏘|✓)
●  ⛰·bce0c5e (⌂|🏘|✓|⛰)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on b625665
└── ≡📙:10:A <> origin/A →:11: on b625665 {1}
    └── 📙:10:A <> origin/A →:11:
        └── ❄6507810 (🏘️) ►origin/A

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  origin/gitbutler/workspace
◎    👉📕gitbutler/workspace[🌳] <> origin/gitbutler/workspace
├─╮
│ ●  ·00e1860 (⌂|🏘)
├─╯
◎  📙A
●  ⛰·6507810 (⌂|🏘|⛰)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:3:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:5:A on b625665 {1}
    └── 📙:5:A
        └── ✂️·6507810 (🏘️|⛰)

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●    ·e82dfab (⌂|🏘)
├─╮
◎ │  B
● │  ·78b1b59 (⌂|🏘)
● │  ·f52fcec (⌂|🏘)
│ ◎  A
│ ●  ·6fdab32 (⌂|🏘)
├─╯
●  ·bce0c5e (⌂|🏘)
●  🏁·3183e43 (⌂|🏘)

"#]]
    );

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓! on bce0c5e
├── ≡:7:B
│   └── :7:B
│       ├── ·78b1b59 (🏘️)
│       ├── ·f52fcec (🏘️)
│       ├── ·bce0c5e (🏘️)
│       └── ·3183e43 (🏘️)
└── ≡:8:A
    └── :8:A
        ├── ·6fdab32 (🏘️)
        ├── ·bce0c5e (🏘️)
        └── ·3183e43 (🏘️)

"#]]
    );

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    // The same is true if stacks are known in workspace metadata.
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  ·e82dfab (⌂|🏘)
╭─┬─╯
│ ◎  📙B
│ ●  ·78b1b59 (⌂|🏘)
◎ │  📙A
● │  ·6fdab32 (⌂|🏘)
│ │ ◎  origin/main
│ │ ◎  main <> origin/main
│ │ ●  ·938e6f2 (⌂|✓)
│ ├─╯
│ ●  ·f52fcec (⌂|🏘|✓)
├─╯
●  ·bce0c5e (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on bce0c5e
├── ≡📙:11:A on bce0c5e {0}
│   └── 📙:11:A
│       └── ·6fdab32 (🏘️)
└── ≡📙:10:B on f52fcec {1}
    └── 📙:10:B
        └── ·78b1b59 (🏘️)

"#]]
    );

    // Finally, if the extra-target, indicating an old stored base that isn't valid anymore.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, ":/M3"),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  ·e82dfab (⌂|🏘)
╭─┬─╯
│ ◎  📙B
│ ●  ·78b1b59 (⌂|🏘)
◎ │  📙A
● │  ·6fdab32 (⌂|🏘)
│ │ ◎  origin/main
│ │ ◎  main <> origin/main
│ │ ●  ·938e6f2 (⌂|✓)
│ ├─╯
│ ●  ·f52fcec (⌂|🏘|✓)
├─╯
●  ·bce0c5e (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    // The base is still adjusted so it matches the actual stacks. With the extra-target
    // resolved as the target commit, the integrated `f52fcec` is at the target and is
    // pruned - consistent with the no-extra-target case above.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on bce0c5e
├── ≡📙:11:A on bce0c5e {0}
│   └── 📙:11:A
│       └── ·6fdab32 (🏘️)
└── ≡📙:10:B on f52fcec {1}
    └── 📙:10:B
        └── ·78b1b59 (🏘️)

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●    ·c5587c9 (⌂|🏘)
├─╮
◎ │  B
● │  ·ce25240 (⌂|🏘)
│ ◎  A
│ ●  ·de6d39c (⌂|🏘)
│ │ ◎  origin/main
│ ├─╯
│ ◎  main <> origin/main
│ ●  ·a821094 (⌂|🏘)
├─╯
●  ·bce0c5e (⌂|🏘)
●  🏁·3183e43 (⌂|🏘)

"#]]
    );

    // The base is automatically set to the lowest one that includes both branches, despite the target.
    // Interestingly, A now gets to see integrated parts of the target branch.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓! on bce0c5e
├── ≡:7:B
│   └── :7:B
│       ├── ·ce25240 (🏘️)
│       ├── ·bce0c5e (🏘️)
│       └── ·3183e43 (🏘️)
└── ≡:8:A
    ├── :8:A
    │   └── ·de6d39c (🏘️)
    └── :9:main <> origin/main →:10:
        ├── ❄a821094 (🏘️) ►origin/main
        ├── ❄bce0c5e (🏘️)
        └── ❄3183e43 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn dependent_branch_on_base() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/dependent-branch-on-base")?;
    snapbox::assert_data_eq!(visualize_commit_graph_all(&repo)?, snapbox::str![[r#"
*-.   a0385a8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 49d4b34 (A) A1
| |/  
|/|   
| * f9e2cb7 (C2-3, C2-2, C2-1, C) C2
| * aaa195b (C1-3, C1-2, C1-1) C1
|/  
* 3183e43 (origin/main, main, below-below-C, below-below-B, below-below-A, below-C, below-B, below-A, B) M1

"#]].raw());

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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎        👉📕gitbutler/workspace[🌳]
├─┬─┬─╮
│ │ │ ●  ·a0385a8 (⌂|🏘)
╭─┬─┬─╯
│ ◎ │  📙B
│ ◎ │  📙below-B
│ ◎ │  📙below-below-B
│ │ ◎  📙C
│ │ ◎  📙C2-1
│ │ ◎  📙C2-2
│ │ ◎  📙C2-3
│ │ ●  ·f9e2cb7 (⌂|🏘)
│ │ ◎  📙C1-3
│ │ ◎  📙C1-2
│ │ ◎  📙C1-1
│ │ ●  ·aaa195b (⌂|🏘)
│ │ ◎  📙below-C
│ │ ◎  📙below-below-C
│ ├─╯
◎ │  📙A
● │  ·49d4b34 (⌂|🏘)
◎ │  📙below-A
◎ │  📙below-below-A
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    // Both stacks will look the same, with the dependent branch inserted at the very bottom.
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:14:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:19:A on 3183e43 {1}
│   ├── 📙:19:A
│   │   └── ·49d4b34 (🏘️)
│   ├── 📙:6:below-A
│   └── 📙:9:below-below-A
├── ≡📙:5:B on 3183e43 {2}
│   ├── 📙:5:B
│   ├── 📙:7:below-B
│   └── 📙:10:below-below-B
└── ≡📙:15:C on 3183e43 {3}
    ├── 📙:15:C
    ├── 📙:16:C2-1
    ├── 📙:17:C2-2
    ├── 📙:18:C2-3
    │   └── ·f9e2cb7 (🏘️) ►C, ►C2-1, ►C2-2
    ├── 📙:22:C1-3
    ├── 📙:21:C1-2
    ├── 📙:20:C1-1
    │   └── ·aaa195b (🏘️) ►C1-2, ►C1-3
    ├── 📙:8:below-C
    └── 📙:11:below-below-C

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
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    // The stack-id could still be found, even though `A` is wrongly marked as outside the workspace.
    // Below A doesn't apply as it's marked inactive.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:14:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:5:B on 3183e43 {2}
│   ├── 📙:5:B
│   ├── 📙:7:below-B
│   └── 📙:10:below-below-B
├── ≡📙:15:C on 3183e43 {3}
│   ├── 📙:15:C
│   ├── 📙:16:C2-1
│   ├── 📙:17:C2-2
│   ├── 📙:18:C2-3
│   │   └── ·f9e2cb7 (🏘️) ►C, ►C2-1, ►C2-2
│   ├── 📙:22:C1-3
│   ├── 📙:21:C1-2
│   ├── 📙:20:C1-1
│   │   └── ·aaa195b (🏘️) ►C1-2, ►C1-3
│   ├── 📙:8:below-C
│   └── 📙:11:below-below-C
└── ≡📙:19:A on 3183e43 {1}
    └── 📙:19:A
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
└── ≡📙:10:A <> origin/A →:13:⇣1 on 1ee1e34 {1}
    └── 📙:10:A <> origin/A →:13:⇣1
        └── 🟣2181501 ►origin/A

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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
└── ≡📙:7:A <> origin/A →:9:⇣1 on 081bae9 {1}
    └── 📙:7:A <> origin/A →:9:⇣1
        └── 🟣197ddce ►origin/A

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
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_extra_target_commit_id(repo.rev_parse_single("origin/main")?),
    )?
    .validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 1ee1e34
└── ≡📙:12:A <> origin/A →:14:⇡1⇣1 on 1ee1e34 {1}
    └── 📙:12:A <> origin/A →:14:⇡1⇣1
        ├── 🟣2181501 ►origin/A
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·a26ae77 (⌂|🏘)
│ ◎  unapplied
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // if the branch was never seen, it's not visible as one would expect.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0

"#]]
    );

    // An applied branch would be present, but has no commit.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:3:unapplied on fafd9d0 {1}
    └── 📙:3:unapplied

"#]]
    );

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;

    // This will be an empty workspace.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  base-peer
│ ◎  base-peer-1
├─╯
│ ◎  base-peer-2
├─╯
│ ◎  base-peer-3
├─╯
│ ◎  base-peer-4
├─╯
│ ◎  base-peer-5
├─╯
│ ◎  base-peer-6
├─╯
│ ◎  base-peer-7
├─╯
│ ◎  base-peer-8
├─╯
│ ◎    👉📕gitbutler/workspace[🌳]
│ ├─╮
│ │ ●  ·20f65b7 (⌂|🏘)
│ ├─╯
│ ◎  📙survivor
│ ●  ·4ca0966 (⌂|🏘)
│ ●  ·a3b180e (⌂|🏘)
├─╯
│ ◎  📙unapplied
├─╯
│ ◎  origin/HEAD
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  ·ce09734 (⌂|🏘|✓)
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    let target_node = graph
        .node_by_ref_name(target_ref.as_ref())
        .map(|(_, reference)| reference)
        .unwrap_or_else(|| {
            panic!(
                "expected exact target reference node for existing ref {target_ref}, graph was:\n{}",
                graph_tree(&graph)
            )
        });
    assert_eq!(
        target_node.ref_info.commit_id,
        Some(repo.rev_parse_single(target_ref.as_bstr())?.detach()),
        "the exact target reference keeps its resolved commit"
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:18:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ce09734
└── ≡📙:19:survivor on ce09734 {1}
    └── 📙:19:survivor
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
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:18:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ce09734
└── ≡📙:19:survivor on ce09734 {1}
    └── 📙:19:survivor
        ├── ·4ca0966 (🏘️)
        └── ·a3b180e (🏘️)

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·a26ae77 (⌂|🏘)
│ ◎  unapplied
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘)

"#]]
    );
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace.stacks[0].ref_name().map(ToString::to_string),
        Some("refs/heads/main".into()),
        "the only local branch disambiguated by a remote owns the same-tip history"
    );

    // the main branch is disambiguated by its remote reference.
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:2:gitbutler/workspace[🌳] <> ✓!
└── ≡:3:main <> origin/main →:5:
    └── :3:main <> origin/main →:5:
        └── ❄fafd9d0 (🏘️) ►unapplied, ►origin/main

"#]]
    );

    // The 'unapplied' branch can be added on top of that, and we make clear we want `main` as well.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "main", StackState::InWorkspace, &[]);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace
            .stacks
            .iter()
            .map(|stack| {
                (
                    stack.id,
                    stack.ref_name().map(ToString::to_string),
                    stack.tip_skip_empty(),
                    stack.base(),
                )
            })
            .collect::<Vec<_>>(),
        [
            (
                Some(StackId::from_number_for_testing(1)),
                Some("refs/heads/unapplied".into()),
                None,
                Some(id_by_rev(&repo, "main").detach()),
            ),
            (
                Some(StackId::from_number_for_testing(2)),
                Some("refs/heads/main".into()),
                None,
                Some(id_by_rev(&repo, "main").detach()),
            ),
        ],
        "different metadata stack identities preserve independent empty same-tip roots"
    );
    snapbox::assert_data_eq!(
        graph_tree(&workspace.graph).to_string(),
        snapbox::str![[r#"
◎      👉📕gitbutler/workspace[🌳]
├─┬─╮
│ │ ●  ·a26ae77 (⌂|🏘)
├───╯
◎ │  📙unapplied
│ │ ◎  origin/main
│ ├─╯
│ ◎  📙main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:3:unapplied on fafd9d0 {1}
│   └── 📙:3:unapplied
└── ≡📙:2:main <> origin/main →:4: on fafd9d0 {2}
    └── 📙:2:main <> origin/main →:4:

"#]]
    );

    // We simulate an unapplied branch on the base by giving it branch metadata, but not listing
    // it in the workspace.
    add_stack_with_segments(&mut meta, 1, "unapplied", StackState::Inactive, &[]);
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;

    // Now only `main` shows up.
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:2:main <> origin/main →:4: on fafd9d0 {2}
    └── 📙:2:main <> origin/main →:4:

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    // notably the target ref and local tracking branch have sibling links setup
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  B
│ ◎    👉📕gitbutler/workspace[🌳]
╭─┼─╮
│ │ ◎  📙A
├───╯
│ │ ◎  origin/main
│ ├─╯
│ ◎  📙main <> origin/main
├─╯
●  ✂·bce0c5e (⌂|🏘|✓)

"#]]
    );
    // sibling links between origin/main and main are also set
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:4:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:5:main <> origin/main →:6: on bce0c5e {0}
│   └── 📙:5:main <> origin/main →:6:
└── ≡📙:2:A on bce0c5e {1}
    └── 📙:2:A

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A-inside[📁wt-A-inside]
│ ◎  A-outside[📁wt-A-outside]
├─╯
│ ◎    👉📕gitbutler/workspace[🌳@repo]
│ ├─╮
│ │ ●  ·a5f94a2 (⌂|🏘)
│ ╭─┤
│ ◎ │  📙A <> origin/A
├─╯ │
│   ◎  B[📁wt-B-inside]
│   ●  ·3e01e28 (⌂|🏘)
├───╯
│ ◎  origin/A
│ ●  🟣197ddce
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
│ ●  ·8dc508f (⌂|✓)
├─╯
●  ·081bae9 (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳@repo] <> ✓refs/remotes/origin/main⇣1 on 081bae9
├── ≡📙:8:A <> origin/A →:13:⇣1 on 081bae9 {0}
│   └── 📙:8:A <> origin/A →:13:⇣1
│       └── 🟣197ddce ►origin/A
└── ≡:12:B[📁wt-B-inside] on 081bae9
    └── :12:B[📁wt-B-inside]
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
    let graph = Graph::from_head(
        &linked_repo,
        &*meta,
        project_meta(&*meta),
        standard_options(),
    )?
    .validated()?;
    // when the graph is built from the B linked worktree repository, the workspace remains visible but the B worktree owns the entrypoint branch
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  A-inside[📁wt-A-inside]
│ ◎  A-outside[📁wt-A-outside]
├─╯
│ ◎    📕gitbutler/workspace[🌳]
│ ├─╮
│ │ ●  ·a5f94a2 (⌂|🏘)
│ ╭─┤
│ ◎ │  📙A <> origin/A
├─╯ │
│   ◎  👉B[📁wt-B-inside@repo]
│   ●  ·3e01e28 (⌂|🏘)
├───╯
│ ◎  origin/A
│ ●  🟣197ddce
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
│ ●  ·8dc508f (⌂|✓)
├─╯
●  ·081bae9 (⌂|🏘|✓)
●  🏁·3183e43 (⌂|🏘|✓)

"#]]
    );

    // workspace projection should keep the linked-worktree ownership marker on the focused stack while leaving the workspace ref itself unowned
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:11:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 081bae9
├── ≡📙:8:A <> origin/A →:13:⇣1 on 081bae9 {0}
│   └── 📙:8:A <> origin/A →:13:⇣1
│       └── 🟣197ddce ►origin/A
└── ≡👉:12:B[📁wt-B-inside@repo] on 081bae9
    └── 👉:12:B[📁wt-B-inside@repo]
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  B
│ ◎    👉📕gitbutler/workspace[🌳]
│ ├─╮
│ │ ●  ·f18d244 (⌂|🏘)
╭─┬─╯
│ ◎  📙A
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // Branch should be visible in workspace once.
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:2:A on fafd9d0 {1}
    └── 📙:2:A

"#]]
    );

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
├── ≡📙:2:A on fafd9d0 {1}
│   └── 📙:2:A
└── ≡📙:3:B on fafd9d0 {2}
    └── 📙:3:B

"#]]
    );

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡📙:2:A on fafd9d0 {1}
    ├── 📙:2:A
    └── 📙:3:B

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  B
│ ◎    👉📕gitbutler/workspace[🌳]
│ ├─╮
│ │ ●  ·f18d244 (⌂|🏘)
╭─┬─╯
│ ◎  📙A
├─╯
│ ◎  main <> origin/main
├─╯
│ ◎  origin/main
│ ●  🟣12b42b0 (✓)
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // Branch should be visible in workspace once.
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
└── ≡📙:3:A on fafd9d0 {1}
    └── 📙:3:A

"#]]
    );

    // 'create' a new branch by metadata
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
├── ≡📙:3:A on fafd9d0 {1}
│   └── 📙:3:A
└── ≡📙:4:B on fafd9d0 {2}
    └── 📙:4:B

"#]]
    );

    // Now pretend it's stacked.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let ws = ws
        .graph
        .redo_traversal_with_overlay(&repo, &*meta, Overlay::default())?
        .into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
└── ≡📙:3:A on fafd9d0 {1}
    ├── 📙:3:A
    └── 📙:4:B

"#]]
    );

    // With extra-target these cases work as well
    meta.data_mut().branches.clear();
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
├── ≡📙:3:A on fafd9d0 {1}
│   └── 📙:3:A
└── ≡📙:4:B on fafd9d0 {2}
    └── 📙:4:B

"#]]
    );

    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["B"]);
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options_with_extra_target(&repo, "main"),
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on fafd9d0
└── ≡📙:3:A on fafd9d0 {1}
    ├── 📙:3:A
    └── 📙:4:B

"#]]
    );

    Ok(())
}

mod edit_commit {
    use but_graph::Graph;
    use but_testsupport::{graph_workspace, visualize_commit_graph_all};

    use but_testsupport::graph_tree;

    use super::project_meta;
    use crate::init::{add_workspace, id_at, read_only_in_memory_scenario, standard_options};

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
        let graph = Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        snapbox::assert_data_eq!(
            graph_tree(&graph).to_string(),
            snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·3ea2742 (⌂|🏘)
◎  A
●  ·a62b0de (⌂|🏘)
◎  gitbutler/edit
●  ·120a217 (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
        );

        // special branch names are skipped by default and entirely invisible.
        snapbox::assert_data_eq!(
            graph_workspace(&graph.into_workspace()?).to_string(),
            snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:7:A on fafd9d0
    └── :7:A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️) ►gitbutler/edit

"#]]
        );

        // However, if the HEAD points to that reference…
        let (id, ref_name) = id_at(&repo, "gitbutler/edit");
        let graph = Graph::from_commit_traversal(
            id,
            ref_name,
            &*meta,
            project_meta(&*meta),
            standard_options(),
        )?
        .validated()?;
        snapbox::assert_data_eq!(
            graph_tree(&graph).to_string(),
            snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
●  ·3ea2742 (⌂|🏘)
◎  A
●  ·a62b0de (⌂|🏘)
◎  👉gitbutler/edit
●  ·120a217 (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
        );
        // …then the segment becomes visible.
        snapbox::assert_data_eq!(
            graph_workspace(&graph.into_workspace()?).to_string(),
            snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:7:A on fafd9d0
    └── :7:A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️) ►gitbutler/edit

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
    snapbox::assert_data_eq!(visualize_commit_graph_all(&repo)?, snapbox::str![[r#"
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

"#]].raw());

    // Add workspace with origin/main as target (not origin/main)
    add_workspace(&mut meta);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    assert_eq!(
        graph.managed_workspace_commit_id(),
        Some(repo.rev_parse_single("gitbutler/workspace")?.detach()),
        "the managed workspace commit remains separate from the local stack"
    );
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.commits)
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        [
            repo.rev_parse_single("local-stack")?.detach(),
            repo.rev_parse_single("local-stack~1")?.detach(),
            repo.rev_parse_single("local-stack~2")?.detach(),
        ],
        "the local stack below the managed workspace commit must remain visible"
    );
    assert_eq!(
        workspace
            .target_ref
            .as_ref()
            .map(|target| target.commits_ahead),
        Some(10),
        "target status excludes every commit reachable from the workspace lower bound"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:22:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣10 on 68e62aa
└── ≡:4:anon on 68e62aa
    └── :4:anon
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:22:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣10 on 68e62aa
└── ≡📙:25:reimplement-insert-blank-commit on 68e62aa {0}
    ├── 📙:25:reimplement-insert-blank-commit
    └── 📙:24:reconstructed-insert-blank-commit-branch
        ├── ·4eaff93 (🏘️) ►local-stack, ►reimplement-insert-blank-commit
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·d77ecda (⌂|🏘)
╭─┤
│ ◎  📙A
◎ │  📙B
● │  ·7163661 (⌂|🏘)
├─╯
●  ·81d4e38 (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·e32cf47 (⌂|🏘|✓)

"#]]
    );

    // The sibling ID is not set, and we see only two stacks: B owns 7163661,
    // and both A and B include the shared base commit 81d4e38 (A only has 81d4e38).
    let ws = &graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on e32cf47
├── ≡📙:8:B on e32cf47 {1}
│   └── 📙:8:B
│       ├── ·7163661 (🏘️)
│       └── ·81d4e38 (🏘️) ►A
└── ≡📙:7:A on e32cf47 {0}
    └── 📙:7:A
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace
            .target_ref
            .as_ref()
            .map(|target| target.commits_ahead),
        Some(1),
        "the sibling ancestry of the target merge is already reachable from the lower bound"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:10:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on f5f42e0
└── ≡📙:11:local-stack on fafd9d0 {0}
    └── 📙:11:local-stack
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎    👉📕gitbutler/workspace[🌳]
├─╮
│ ●  ·891e228 (⌂|🏘)
├─╯
◎  📙my-branch
●  ·cd76046 (⌂|🏘)
●    ·f8ff9a3 (⌂|🏘)
├─╮
● │  ·6f65768 (⌂|🏘)
│ │ ◎  origin/main
│ ├─╯
│ ◎  main <> origin/main
│ ●  ·ef56fab (⌂|🏘|✓)
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    // The fork-point approach correctly finds the original divergence point (fafd9d0)
    // instead of the moved merge base (ef56fab), so all 3 branch commits are visible:
    // branch-commit-2, the merge commit, and branch-commit-1.
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.stacks[0]
            .segments
            .iter()
            .flat_map(|segment| segment.commits.iter().map(|commit| commit.id))
            .collect::<Vec<_>>(),
        [
            id_by_rev(&repo, "my-branch").detach(),
            id_by_rev(&repo, "my-branch~1").detach(),
            id_by_rev(&repo, "my-branch~2").detach(),
        ],
        "the stack contains only its first-parent work above the target fork"
    );
    assert_eq!(
        ws.stacks[0].base(),
        Some(id_by_rev(&repo, "main~1").detach()),
        "the stack stops at its first-parent fork with target history"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on ef56fab
└── ≡📙:9:my-branch on fafd9d0 {0}
    └── 📙:9:my-branch
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    // With the target at "init", A and B are above the target and should be
    // kept even though they are marked integrated.
    let workspace = graph.into_workspace()?;
    assert_eq!(
        workspace
            .target_ref
            .as_ref()
            .map(|target| target.commits_ahead),
        Some(3),
        "the target merge and its two side commits are all ahead of init"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣3 on fafd9d0
└── ≡📙:7:my-branch on fafd9d0 {0}
    └── 📙:7:my-branch
        ├── ·312f819 (🏘️|✓)
        └── ·e255adc (🏘️|✓)

"#]]
    );

    // Now advance the target to origin/main (which includes the merge).
    // Both commits are at or below the new target and should be pruned,
    // but the metadata-tracked branch entry is preserved.
    let main_id = repo.rev_parse_single("main")?.detach();
    add_workspace_with_target(&mut meta, main_id);

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let workspace = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:8:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 312f819
└── ≡📙:7:my-branch on 312f819 {0}
    └── 📙:7:my-branch

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        standard_options().with_hard_limit(usize::MAX),
    )?
    .validated()?;
    assert!(
        !graph.hard_limit_hit(),
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let ws = graph.into_workspace()?;
    let my_branch_ref: gix::refs::FullName = "refs/heads/my-branch".try_into()?;
    let my_branch = ws
        .stacks
        .iter()
        .find(|stack| stack.ref_name() == Some(my_branch_ref.as_ref()))
        .expect("my-branch stack is projected");
    assert_eq!(
        my_branch
            .segments
            .iter()
            .flat_map(|segment| segment.commits.iter().map(|commit| commit.id))
            .collect::<Vec<_>>(),
        [id_by_rev(&repo, "my-branch").detach()],
        "a stack reaching the stored target stops before target history"
    );
    assert_eq!(
        my_branch.base(),
        Some(target_id),
        "the reachable stored target is this stack's delimiter"
    );
    let old_branch_ref: gix::refs::FullName = "refs/heads/old-branch".try_into()?;
    let old_branch = ws
        .stacks
        .iter()
        .find(|stack| stack.ref_name() == Some(old_branch_ref.as_ref()))
        .expect("old-branch stack is projected");
    assert_eq!(
        old_branch
            .segments
            .iter()
            .flat_map(|segment| segment.commits.iter().map(|commit| commit.id))
            .collect::<Vec<_>>(),
        [id_by_rev(&repo, "old-branch").detach()],
        "a stack forked below the stored target excludes integrated trunk commits"
    );
    assert_eq!(
        old_branch.base(),
        Some(id_by_rev(&repo, "main~2").detach()),
        "the divergent stack stops at its own target-history fork"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:9:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on 322cb14
├── ≡📙:10:my-branch on 2121f9c {0}
│   └── 📙:10:my-branch
│       └── ·f5055a1 (🏘️)
└── ≡📙:11:old-branch on 322cb14 {1}
    └── 📙:11:old-branch
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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.stacks[0]
            .segments
            .iter()
            .flat_map(|segment| segment.commits.iter().map(|commit| commit.id))
            .collect::<Vec<_>>(),
        [
            id_by_rev(&repo, "X").detach(),
            id_by_rev(&repo, "X~1").detach(),
            id_by_rev(&repo, "X~2").detach(),
        ],
        "a catch-up merge keeps only the stack's first-parent work"
    );
    assert_eq!(
        ws.stacks[0].base(),
        Some(id_by_rev(&repo, "X~3").detach()),
        "a catch-up merge stops at the first-parent fork with target history"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:12:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣2 on d263f88
└── ≡📙:13:X on b4bd43f {0}
    └── 📙:13:X
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
│ ◎  origin/main
│ │ ◎  tags/my-tag
├───╯
● │  ·3ea2742 (⌂|🏘)
◎ │  A
● │  ·a62b0de (⌂|🏘)
● │  ·120a217 (⌂|🏘)
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:8:A on fafd9d0
    └── :8:A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
    );

    // Now traverse from the tag that points at the workspace commit.
    let (id, name) = id_at(&repo, "my-tag");
    let graph =
        Graph::from_commit_traversal(id, name, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  📕gitbutler/workspace[🌳]
│ ◎  origin/main
│ │ ◎  👉tags/my-tag
├───╯
● │  ·3ea2742 (⌂|🏘)
◎ │  A
● │  ·a62b0de (⌂|🏘)
● │  ·120a217 (⌂|🏘)
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    let workspace = graph.into_workspace()?;
    let WorkspaceKind::Managed { ref_info } = &workspace.kind else {
        panic!("a sibling ref at the workspace commit remains in the managed workspace");
    };
    assert_eq!(
        ref_info.ref_name.as_bstr(),
        b"refs/heads/gitbutler/workspace",
        "the containing managed workspace ref is preserved"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&workspace).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:8:A on fafd9d0
    └── :8:A
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·3ea2742 (⌂|🏘)
◎  origin/A
●  ·a62b0de (⌂|🏘)
●  ·120a217 (⌂|🏘)
│ ◎  origin/main
├─╯
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:7:origin/A on fafd9d0
    └── :7:origin/A
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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●  ·5638b41 (⌂|🏘)
◎  B
●  ·cb7021b (⌂|🏘)
●  🏁·ce3278a (⌂|🏘)
◎  origin/main
◎  main <> origin/main
●  🏁·fafd9d0 (⌂|✓)

"#]]
    );
    // this is a weird state as the target is actually disjoint from the workspace - it appears empty now
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1
└── ≡:7:B
    └── :7:B
        ├── ·cb7021b (🏘️)
        └── ·ce3278a (🏘️)

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
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    snapbox::assert_data_eq!(
        graph_tree(&graph).to_string(),
        snapbox::str![[r#"
◎  👉📕gitbutler/workspace[🌳]
●    ·21bff1f (⌂|🏘)
├─╮
│ ◎  origin/A
│ ●  ·a62b0de (⌂|🏘)
│ ●  ·120a217 (⌂|🏘)
├─╯
│ ◎  origin/main
│ ◎  main <> origin/main
├─╯
●  🏁·fafd9d0 (⌂|🏘|✓)

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&graph.into_workspace()?).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on fafd9d0
└── ≡:7:origin/A on fafd9d0
    └── :7:origin/A
        ├── ·a62b0de (🏘️)
        └── ·120a217 (🏘️)

"#]]
    );
    Ok(())
}
