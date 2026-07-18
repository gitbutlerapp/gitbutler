use but_core::{RefMetadata, ref_metadata::StackId};
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use snapbox::IntoData;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn tear_off_top_most_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {1}
│   └── 📙:9:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:8:C on 85efbe4 {2}
    ├── 📙:8:C
    │   └── ·09bc93e (🏘️)
    └── 📙:10:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Tear off C from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        "refs/heads/C".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta().clone();
    crate::utils::refresh_workspace_from_head(&mut ws, &repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   efd284c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 09d8e52 (A) A
| * | c813d8d (B) B
| |/  
* / 8e00332 (C) C
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:10:A on 85efbe4 {1}
│   └── 📙:10:A
│       └── ·09d8e52 (🏘️)
├── ≡📙:9:B on 85efbe4 {2}
│   └── 📙:9:B
│       └── ·c813d8d (🏘️)
└── ≡📙:8:C on 85efbe4 {3}
    └── 📙:8:C
        └── ·8e00332 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_bottom_most_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {1}
│   └── 📙:9:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:8:C on 85efbe4 {2}
    ├── 📙:8:C
    │   └── ·09bc93e (🏘️)
    └── 📙:10:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        "refs/heads/B".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta().clone();
    crate::utils::refresh_workspace_from_head(&mut ws, &repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   a3c9e85 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 09d8e52 (A) A
| * | 8e00332 (C) C
| |/  
* / c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:10:A on 85efbe4 {1}
│   └── 📙:10:A
│       └── ·09d8e52 (🏘️)
├── ≡📙:9:C on 85efbe4 {2}
│   └── 📙:9:C
│       └── ·8e00332 (🏘️)
└── ≡📙:8:B on 85efbe4 {3}
    └── 📙:8:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_only_branch_in_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {1}
│   └── 📙:9:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:8:C on 85efbe4 {2}
    ├── 📙:8:C
    │   └── ·09bc93e (🏘️)
    └── 📙:10:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Tear off A from the stack. Should be a no-op.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        "refs/heads/A".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta().clone();
    crate::utils::refresh_workspace_from_head(&mut ws, &repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:9:A on 85efbe4 {1}
│   └── 📙:9:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:8:C on 85efbe4 {2}
    ├── 📙:8:C
    │   └── ·09bc93e (🏘️)
    └── 📙:10:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_from_single_stack_in_ws_top() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-ref-ws-commit-one-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* d69fe94 (B) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:7:B on 85efbe4 {1}
    ├── 📙:7:B
    │   └── ·d69fe94 (🏘️)
    └── 📙:8:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        "refs/heads/B".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta().clone();
    crate::utils::refresh_workspace_from_head(&mut ws, &repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e2d89a5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 1273ba9 (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:8:A on 85efbe4 {1}
│   └── 📙:8:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:7:B on 85efbe4 {2}
    └── 📙:7:B
        └── ·1273ba9 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_from_single_stack_in_ws_bottom() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-ref-ws-commit-one-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* d69fe94 (B) B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:7:B on 85efbe4 {1}
    ├── 📙:7:B
    │   └── ·d69fe94 (🏘️)
    └── 📙:8:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Tear off A from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        "refs/heads/A".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta().clone();
    crate::utils::refresh_workspace_from_head(&mut ws, &repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   828af37 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 1273ba9 (B) B
* | 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:8:B on 85efbe4 {2}
│   └── 📙:8:B
│       └── ·1273ba9 (🏘️)
└── ≡📙:7:A on 85efbe4 {1}
    └── 📙:7:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-with-empty-top-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["A"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d990875 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 09d8e52 (B, A) A
* 85efbe4 (origin/main, main) M

"#]]
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:7:B on 85efbe4 {1}
    ├── 📙:7:B
    └── 📙:6:A
        └── ·09d8e52 (🏘️) ►B

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        "refs/heads/B".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta().clone();
    crate::utils::refresh_workspace_from_head(&mut ws, &repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   d744692 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:7:A on 85efbe4 {1}
│   └── 📙:7:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:3:B on 85efbe4 {3}
    └── 📙:3:B

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_non_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-with-empty-top-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["A"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d990875 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 09d8e52 (B, A) A
* 85efbe4 (origin/main, main) M

"#]]
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:7:B on 85efbe4 {1}
    ├── 📙:7:B
    └── 📙:6:A
        └── ·09d8e52 (🏘️) ►B

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Tear off A from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        "refs/heads/A".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta().clone();
    crate::utils::refresh_workspace_from_head(&mut ws, &repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   b1314f4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:6:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:B on 85efbe4 {1}
│   └── 📙:3:B
└── ≡📙:7:A on 85efbe4 {3}
    └── 📙:7:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

fn set_workspace_metadata(
    meta: &mut impl RefMetadata,
    ws: &but_graph::Workspace,
    ws_meta: Option<but_core::ref_metadata::Workspace>,
) -> anyhow::Result<()> {
    if let Some((ws_meta, ref_name)) = ws_meta.zip(ws.ref_name()) {
        let mut md = meta.workspace(ref_name)?;
        *md = ws_meta;
        meta.set_workspace(&md)?;
    }
    Ok(())
}
