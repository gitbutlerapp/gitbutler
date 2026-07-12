use but_core::{RefMetadata, ref_metadata::StackId};
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn tear_off_top_most_branch() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:C on 85efbe4 {2}
│   ├── 📙:C
│   │   └── ·09bc93e (🏘️)
│   └── 📙:B
│       └── ·c813d8d (🏘️)
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Tear off C from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        &ws,
        "refs/heads/C".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   16e2eb1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 8e00332 (C) C
| * | c813d8d (B) B
| |/  
* / 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:A on 85efbe4 {1}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
├── ≡📙:B on 85efbe4 {2}
│   └── 📙:B
│       └── ·c813d8d (🏘️)
└── ≡📙:C on 85efbe4 {3}
    └── 📙:C
        └── ·8e00332 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_bottom_most_branch() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:C on 85efbe4 {2}
│   ├── 📙:C
│   │   └── ·09bc93e (🏘️)
│   └── 📙:B
│       └── ·c813d8d (🏘️)
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   7e46497 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * c813d8d (B) B
| * | 09d8e52 (A) A
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
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:C on 85efbe4 {2}
│   └── 📙:C
│       └── ·8e00332 (🏘️)
├── ≡📙:A on 85efbe4 {1}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {3}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_only_branch_in_stack() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:C on 85efbe4 {2}
│   ├── 📙:C
│   │   └── ·09bc93e (🏘️)
│   └── 📙:B
│       └── ·c813d8d (🏘️)
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Tear off A from the stack. Should be a no-op.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:C on 85efbe4 {2}
│   ├── 📙:C
│   │   └── ·09bc93e (🏘️)
│   └── 📙:B
│       └── ·c813d8d (🏘️)
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_from_single_stack_in_ws_top() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:B on 85efbe4 {2}
    ├── 📙:B
    │   └── ·d69fe94 (🏘️)
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:A on 85efbe4 {1}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {2}
    └── 📙:B
        └── ·1273ba9 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_from_single_stack_in_ws_bottom() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:B on 85efbe4 {2}
    ├── 📙:B
    │   └── ·d69fe94 (🏘️)
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Tear off A from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:B on 85efbe4 {2}
│   └── 📙:B
│       └── ·1273ba9 (🏘️)
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_empty_branch() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:A on 85efbe4 {1}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {3}
    └── 📙:B

"#]]
    );

    Ok(())
}

#[test]
fn tear_off_non_empty_branch() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Tear off A from the stack.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::tear_off_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        Some(StackId::from_number_for_testing(3)),
    )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:B on 85efbe4 {1}
│   └── 📙:B
└── ≡📙:A on 85efbe4 {3}
    └── 📙:A
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
