use but_core::RefMetadata;
use but_core::ref_metadata::StackKind;
use but_graph::walk::Options;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_workspace, invoke_bash, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description,
    named_writable_scenario_with_description_and_graph,
};

#[test]
fn move_top_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
    // Put C on top of A
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/C".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   bdcbf64 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * c813d8d (B) B
* | f2cc60d (C) C
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
├── ≡📙:C on 85efbe4 {1}
│   ├── 📙:C
│   │   └── ·f2cc60d (🏘️)
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {2}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn moving_branch_onto_itself_fails_without_changing_workspace() -> anyhow::Result<()> {
    let (_tmp, ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;

    let before = graph_workspace(&ws).to_string();
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;

    let err = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/C".try_into()?,
        "refs/heads/C".try_into()?,
    )
    .expect_err("moving a branch onto itself should fail before graph mutation");

    assert_eq!(
        err.to_string(),
        "Cannot move branch refs/heads/C onto itself"
    );
    assert_eq!(
        graph_workspace(&ws).to_string(),
        before,
        "workspace projection should stay unchanged after rejected self-move"
    );

    Ok(())
}

#[test]
fn move_bottom_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   9c6a201 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f9061ed (B) B
| * 09d8e52 (A) A
* | 8e00332 (C) C
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
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    │   └── ·f9061ed (🏘️)
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_single_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
    // Put A on top of C
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        "refs/heads/C".try_into()?,
    )?;

    // Materialize the operation
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4c58dd4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 148f8f3 (A) A
* 09bc93e (C) C
* c813d8d (B) B
* 85efbe4 (origin/main, main) M

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:A on 85efbe4 {2}
    ├── 📙:A
    │   └── ·148f8f3 (🏘️)
    ├── 📙:C
    │   └── ·09bc93e (🏘️)
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn reorder_branch_in_stack() -> anyhow::Result<()> {
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
    // Put B on top of C
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        "refs/heads/C".try_into()?,
    )?;

    // Materialize the operation
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   c6b8b22 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | de0581e (B) B
* | 8e00332 (C) C
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
│   ├── 📙:B
│   │   └── ·de0581e (🏘️)
│   └── 📙:C
│       └── ·8e00332 (🏘️)
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn insert_branch_in_the_middle_of_a_stack() -> anyhow::Result<()> {
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
    // Put A on top of B, and below C
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    // Materialize the operation
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0c5cde5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 3e7ff55 (C) C
* 4dfe841 (A) A
* c813d8d (B) B
* 85efbe4 (origin/main, main) M

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:C on 85efbe4 {2}
    ├── 📙:C
    │   └── ·3e7ff55 (🏘️)
    ├── 📙:A
    │   └── ·4dfe841 (🏘️)
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_empty_branch() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
├── ≡📙:B on 85efbe4 {2}
│   └── 📙:B
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Put B on top of A
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
    Ok(())
}

#[test]
fn move_branch_on_top_of_empty_branch() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
├── ≡📙:B on 85efbe4 {2}
│   └── 📙:B
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Put A on top of B
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    // Materialize the operation
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 09d8e52 (A) A
* 85efbe4 (origin/main, main, B) M

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:A on 85efbe4 {2}
    ├── 📙:A
    │   └── ·09d8e52 (🏘️)
    └── 📙:B

"#]]
    );
    Ok(())
}

#[test]
fn move_empty_branch_on_top_of_empty_branch_in_same_stack() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("empty-workspace-target-advanced")?;
    invoke_bash(
        "git branch A gitbutler/target\ngit branch B gitbutler/target\n",
        &repo,
    );
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let mut ws = but_graph::Workspace::from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
        Options {
            extra_target_commit_id: repo
                .rev_parse_single("gitbutler/target")
                .ok()
                .map(|id| id.detach()),
            ..Options::limited()
        },
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡📙:B on 3183e43 {1}
    ├── 📙:B
    └── 📙:A

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡📙:A on 3183e43 {1}
    ├── 📙:A
    └── 📙:B

"#]]
    );

    Ok(())
}

#[test]
fn move_empty_branch_on_top_of_empty_branch_across_stacks() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("empty-workspace-target-advanced")?;
    invoke_bash(
        "git branch A gitbutler/target\ngit branch B gitbutler/target\n",
        &repo,
    );
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let mut ws = but_graph::Workspace::from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
        Options {
            extra_target_commit_id: repo
                .rev_parse_single("gitbutler/target")
                .ok()
                .map(|id| id.detach()),
            ..Options::limited()
        },
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡📙:A on 3183e43 {1}
│   └── 📙:A
└── ≡📙:B on 3183e43 {2}
    └── 📙:B

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡📙:A on 3183e43 {2}
    ├── 📙:A
    └── 📙:B

"#]]
    );

    Ok(())
}

#[test]
fn non_empty_move_display_order_follows_workspace_parents() -> anyhow::Result<()> {
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
    let before_display_order = stack_display_order(&ws);
    let before_metadata_order = metadata_stack_order(&ws);
    // Display order is the workspace commit's parent array; the fixture's declared metadata
    // order lags behind it until the next metadata write.
    assert_eq!(
        before_display_order,
        ["refs/heads/C", "refs/heads/A"].map(str::to_owned)
    );

    // Move non-empty C on top of non-empty A.
    // This rewrites metadata and keeps display + metadata aligned.
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/C".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    let updated_metadata_order = ws_meta
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::Applied))
        .unwrap_or_default();

    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;

    // before refreshing `ws` the pure-virtual change isn't visible (should be fixed once meta is in db!)
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:C on 85efbe4 {2}
│   ├── 📙:C
│   │   └── ·f2cc60d (🏘️)
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {2}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;
    // after the refresh the workspace is finally uptodate (this will probably be an issue unless callers know that)
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:C on 85efbe4 {1}
│   ├── 📙:C
│   │   └── ·f2cc60d (🏘️)
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {2}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let after_display_order = stack_display_order(&ws);

    // The move changes both the stored metadata order and the displayed order.
    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);
    // Stack order is taken from the workspace-commit parent array (the source of
    // truth), not metadata order: after the move the parents are [B, C] while
    // metadata is [C, B]. The order snapshots below capture the parent-array order.

    snapbox::assert_data_eq!(
        format!("{before_display_order:#?}"),
        snapbox::str![[r#"
[
    "refs/heads/C",
    "refs/heads/A",
]
"#]]
    );

    snapbox::assert_data_eq!(
        format!("{after_display_order:#?}"),
        snapbox::str![[r#"
[
    "refs/heads/C",
    "refs/heads/B",
]
"#]]
    );

    Ok(())
}

#[test]
fn empty_move_display_order_follows_workspace_parents() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
        .raw()
    );

    let before_display_order = stack_display_order(&ws);
    let before_metadata_order = metadata_stack_order(&ws);
    // Display order is the workspace commit's parent array; the fixture's declared metadata
    // order lags behind it until the next metadata write.
    assert_eq!(
        before_display_order,
        ["refs/heads/B", "refs/heads/A"].map(str::to_owned)
    );

    // Move empty B on top of non-empty A.
    // This path rewrites metadata and keeps display + metadata aligned.
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    let updated_metadata_order = ws_meta
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::AppliedAndUnapplied))
        .unwrap_or_default();

    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    let after_display_order = stack_display_order(&ws);

    // The move changes both the stored metadata order and the displayed order; the
    // displayed order is taken from the workspace-commit parent array (see snapshot).
    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);

    snapbox::assert_data_eq!(
        format!("{before_display_order:#?}"),
        snapbox::str![[r#"
[
    "refs/heads/B",
    "refs/heads/A",
]
"#]]
    );

    snapbox::assert_data_eq!(
        format!("{after_display_order:#?}"),
        snapbox::str![[r#"
[
    "refs/heads/B",
]
"#]]
    );

    Ok(())
}

#[test]
fn move_branch_when_base_segment_has_no_ref_name() -> anyhow::Result<()> {
    // When origin/main advances past the fork point, the old fork commit becomes
    // an unnamed base segment. Moving a branch should still work by falling back
    // to selecting by the segment's tip commit.
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks-advanced-remote",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   a236c53 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * c813d8d (B) B
* | 09d8e52 (A) A
|/  
| * 148c87a (origin/main) M2
|/  
* 85efbe4 (main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
├── ≡📙:A on 85efbe4 {1}
│   └── 📙:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:B on 85efbe4 {2}
    └── 📙:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Move B on top of A — the base segment at the old fork point has no ref name.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 148c87a (origin/main) M2
| * 0db3c2f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * f9061ed (B) B
| * 09d8e52 (A) A
|/  
* 85efbe4 (main) M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    │   └── ·f9061ed (🏘️)
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_empty_branch_onto_non_empty_branch_with_advanced_target() -> anyhow::Result<()> {
    // Regression: when the target branch (local `main`/`origin/main`) is ahead of the workspace
    // base, the merge-base segment is represented in the editor graph by the `gitbutler/target`
    // reference node sitting above the base commit. Selecting the base by commit would point one
    // hop too far and fail the direct-parent check. Moving the empty branch onto the non-empty one
    // must still succeed.
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-with-empty-stack-target-advanced",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
|/  
| * e1bbad3 (origin/main, main) add X
|/  
* 85efbe4 (gitbutler/target, B) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
├── ≡📙:B on 85efbe4 {2}
│   └── 📙:B
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Put empty B on top of non-empty A.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 09d8e52 (B, A) A
| * e1bbad3 (origin/main, main) add X
|/  
* 85efbe4 (gitbutler/target) M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_non_empty_branch_onto_empty_branch_with_advanced_target() -> anyhow::Result<()> {
    // Same setup as the empty-onto-non-empty regression, but the subject is the non-empty branch
    // and the target is the empty one. Both directions must succeed when the target is ahead.
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-with-empty-stack-target-advanced",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
|/  
| * e1bbad3 (origin/main, main) add X
|/  
* 85efbe4 (gitbutler/target, B) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
├── ≡📙:B on 85efbe4 {2}
│   └── 📙:B
└── ≡📙:A on 85efbe4 {1}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    // Put non-empty A on top of empty B.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        &ws,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 09d8e52 (A) A
| * e1bbad3 (origin/main, main) add X
|/  
* 85efbe4 (gitbutler/target, B) M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:A on 85efbe4 {2}
    ├── 📙:A
    │   └── ·09d8e52 (🏘️)
    └── 📙:B

"#]]
    );

    Ok(())
}

fn stack_display_order(ws: &but_graph::Workspace) -> Vec<String> {
    ws.display_stacks()
        .expect("displayable")
        .iter()
        .filter_map(|stack| stack.ref_name())
        .map(|name| name.to_string())
        .collect()
}

fn metadata_stack_order(ws: &but_graph::Workspace) -> Vec<String> {
    ws.metadata
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::Applied))
        .unwrap_or_default()
}

fn workspace_metadata_stack_order(
    ws_meta: &but_core::ref_metadata::Workspace,
    kind: StackKind,
) -> Vec<String> {
    ws_meta
        .stacks(kind)
        .filter_map(|stack| stack.name())
        .map(|name| name.to_string())
        .collect()
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

/// The managed fuzzer's ws-with-empty-stack seed-0 finding: moving a commit-owning branch
/// above the empty sitting on its own tip ran the generic pick surgery — a degenerate
/// self-move that disconnected the range from the workspace commit without reconnecting it,
/// expelling the subject's commits from the workspace (the ref survived on disk). The
/// crossed empty now re-anchors below the subject's unchanged range instead.
#[test]
fn repro_managed_empty_shuffle_projection_drop() -> anyhow::Result<()> {
    use but_meta::VirtualBranchesTomlMetadata;

    fn stack_and_empty(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", stack_and_empty)?;

    let attempts = [
        ("refs/heads/A", "refs/heads/B"),
        ("refs/heads/B", "refs/heads/A"),
        ("refs/heads/B", "refs/heads/A"),
        ("refs/heads/B", "refs/heads/A"),
        ("refs/heads/A", "refs/heads/B"),
    ];
    for (i, (subject, target)) in attempts.iter().enumerate() {
        eprintln!("step {i}: move {subject} -> {target}");
        match managed_move_and_apply(
            &repo,
            &mut meta,
            &mut ws,
            (*subject).try_into()?,
            (*target).try_into()?,
        ) {
            Ok(()) => eprintln!(
                "  ok\n--- commits ---\n{}--- workspace ---\n{}",
                visualize_commit_graph_all(&repo)?,
                graph_workspace(&ws)
            ),
            Err(e) => {
                let msg = format!("{e:?}");
                eprintln!("  err: {msg}");
                assert!(!msg.contains("BUG:"), "collision at step {i}: {msg}");
            }
        }
    }
    let projected: Vec<_> = ws
        .display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|seg| seg.ref_name().map(|n| n.as_bstr().to_string()))
        .collect();
    assert!(
        projected.contains(&"refs/heads/A".to_string()),
        "A dropped from the projection: {projected:?}"
    );
    Ok(())
}

/// Moving a commit-owning branch onto a ref with empties riding above it (found by fuzzing
/// the empties-between-owners fixture): the riders re-keyed onto the subject's tip carrying
/// their edge statements verbatim, but the workspace-commit edge was never redirected
/// through the inserted range — the subject's commits silently left the workspace. The
/// redirect now always follows the group's edges, and the riders chain above the subject's
/// own ref instead of standing as a colliding twin group.
#[test]
fn repro_managed_move_onto_ref_with_riders() -> anyhow::Result<()> {
    use but_meta::VirtualBranchesTomlMetadata;

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(
            meta,
            1,
            "e3",
            StackState::InWorkspace,
            &["e2", "B", "e1", "A"],
        );
        add_stack_with_segments(meta, 2, "F", StackState::InWorkspace, &[]);
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-empties-between-owners",
            setup,
        )?;

    let attempts = [
        ("refs/heads/A", "refs/heads/e1"),
        ("refs/heads/A", "refs/heads/F"),
        ("refs/heads/B", "refs/heads/e2"),
        ("refs/heads/e1", "refs/heads/F"),
        // The finding: A (owns a commit) onto B, whose group carries e3 above it.
        ("refs/heads/A", "refs/heads/B"),
    ];
    for (i, (subject, target)) in attempts.iter().enumerate() {
        if let Err(e) = managed_move_and_apply(
            &repo,
            &mut meta,
            &mut ws,
            (*subject).try_into()?,
            (*target).try_into()?,
        ) {
            let msg = format!("{e:?}");
            assert!(!msg.contains("BUG:"), "collision at step {i}: {msg}");
        }
    }
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 893d602
├── ≡📙:e3 on 893d602 {1}
│   ├── 📙:e3
│   ├── 📙:A
│   │   └── ·cc8f743 (🏘️)
│   ├── 📙:B
│   │   └── ·f0058fc (🏘️)
│   └── 📙:e2
└── ≡📙:e1 on 893d602 {2}
    ├── 📙:e1
    └── 📙:F
        └── ·9920891 (🏘️)

"#]]
    );
    Ok(())
}

/// Moving an empty branch onto an empty branch that rests on a DIFFERENT commit (found by
/// fuzzing the empties-between-owners fixture): the metadata-only arm treated every
/// empty-onto-empty move as a pure display reorder, so the subject's ref never re-pointed
/// and the projection reconciled by dropping it. Different-commit empties now take the
/// graph path and physically re-point.
#[test]
fn repro_managed_empty_move_across_commits() -> anyhow::Result<()> {
    use but_meta::VirtualBranchesTomlMetadata;

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(
            meta,
            1,
            "e3",
            StackState::InWorkspace,
            &["e2", "B", "e1", "A"],
        );
        add_stack_with_segments(meta, 2, "F", StackState::InWorkspace, &[]);
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-empties-between-owners",
            setup,
        )?;

    // e1 rests on A's tip; e3 rests on B's tip — same stack, different commits.
    managed_move_and_apply(
        &repo,
        &mut meta,
        &mut ws,
        "refs/heads/e1".try_into()?,
        "refs/heads/e3".try_into()?,
    )?;
    assert_eq!(
        repo.rev_parse_single("e1")?.detach(),
        repo.rev_parse_single("e3")?.detach(),
        "e1 must physically re-point onto e3's commit"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 893d602
├── ≡📙:e1 on 893d602 {1}
│   ├── 📙:e1
│   ├── 📙:e3
│   ├── 📙:e2
│   ├── 📙:B
│   │   └── ·406dba0 (🏘️)
│   └── 📙:A
│       └── ·26e45af (🏘️)
└── ≡📙:F on 893d602 {2}
    └── 📙:F
        └── ·9920891 (🏘️)

"#]]
    );
    Ok(())
}

/// Deleting a branch label whose commits stay as an anonymous segment (found by fuzzing
/// mixed ops): the stack's remaining chain has no positioned branch on the run's START, and
/// the run must still be bound to it — otherwise its same-commit empty members fall out of
/// the projection while their refs remain intact on disk. Binding an anonymous run therefore
/// has to walk the leg below the start rather than only look at the start itself.
#[test]
fn repro_managed_remove_owner_keeps_chain_empties() -> anyhow::Result<()> {
    use but_core::ref_metadata::StackId;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_workspace::branch::create_reference::{Anchor, Position};

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
    }
    fn test_stack_id(rn: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(rn.as_bstr().iter().map(|&b| b as u128).sum())
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            setup,
        )?;

    // Empty B by squashing its commit away, fold A into the stack, then hang two fresh
    // empties off it — and delete C, the only ref left on the stack's tip commit.
    let b_commit = repo.rev_parse_single("B")?.detach();
    let a_commit = repo.rev_parse_single("A")?.detach();
    let outcome = but_workspace::commit::squash_commits(
        Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?,
        vec![b_commit],
        a_commit,
        but_workspace::commit::squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;
    let (graph, _) = outcome.rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;
    managed_move_and_apply(
        &repo,
        &mut meta,
        &mut ws,
        r_ref("refs/heads/A"),
        r_ref("refs/heads/B"),
    )?;
    for (name, anchor) in [
        ("refs/heads/fz-2", None),
        (
            "refs/heads/fz-3",
            Some(Anchor::AtSegment {
                ref_name: std::borrow::Cow::Owned("refs/heads/B".try_into()?),
                position: Position::Below,
            }),
        ),
    ] {
        ws = but_workspace::branch::create_reference(
            gix::refs::FullName::try_from(name)?.as_ref(),
            anchor,
            &repo,
            &ws,
            &mut meta,
            test_stack_id,
            None,
        )?
        .expect("the workspace changed");
    }
    but_workspace::branch::remove_reference(
        r_ref("refs/heads/C"),
        &repo,
        &ws,
        &mut meta,
        Default::default(),
    )?
    .expect("C is deleted");
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;

    let projected: Vec<_> = ws
        .display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|seg| seg.ref_name().map(|n| n.shorten().to_string()))
        .collect();
    for name in ["A", "B", "fz-3", "fz-2"] {
        assert!(
            projected.contains(&name.to_string()),
            "{name} dropped from the projection: {projected:?}"
        );
    }
    Ok(())
}

/// Moving a branch out of a chain listed in not-yet-persisted metadata (found by fuzzing
/// mixed ops): the mid-flight projection saw the moved branch as another claimed chain's
/// territory MID-LEG and ended the walk there, vanishing the segments below — which trips
/// the applied-stacks-projected sanity check. Foreign territory now only ends a walk at
/// another run's own start.
#[test]
fn repro_managed_move_into_anonymous_tip_stack() -> anyhow::Result<()> {
    use but_meta::VirtualBranchesTomlMetadata;

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "D", StackState::InWorkspace, &["A"]);
        add_stack_with_segments(meta, 2, "E", StackState::InWorkspace, &["C", "B"]);
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-double-stack-triple-stack-files",
            setup,
        )?;

    // Anonymize stack 1's tip by deleting D, then move B (mid-stack owner from stack 2)
    // onto A. The move's intermediate projection runs against metadata that still lists
    // B in stack 2.
    but_workspace::branch::remove_reference(
        r_ref("refs/heads/D"),
        &repo,
        &ws,
        &mut meta,
        Default::default(),
    )?
    .expect("D is deleted");
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;
    managed_move_and_apply(
        &repo,
        &mut meta,
        &mut ws,
        r_ref("refs/heads/B"),
        r_ref("refs/heads/A"),
    )?;

    let projected: Vec<_> = ws
        .display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|seg| seg.ref_name().map(|n| n.shorten().to_string()))
        .collect();
    for name in ["A", "B", "C", "E"] {
        assert!(
            projected.contains(&name.to_string()),
            "{name} dropped from the projection: {projected:?}"
        );
    }
    Ok(())
}

/// An all-empty stack must keep the commit it rests on as its base (found by fuzzing the
/// advanced-remote scenario): pruning the bound's integrated `main` segment from a stack
/// of only empties discarded the base stored on it, and the next anchored
/// `create_reference` hit `try_branch_resting_commit_id`'s "impossible" no-base case.
/// The old bottom's base now carries over when empty bottoms are removed.
#[test]
fn repro_managed_all_empty_stack_keeps_base() -> anyhow::Result<()> {
    use but_core::ref_metadata::StackId;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_workspace::branch::create_reference::{Anchor, Position};

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
    }
    fn test_stack_id(rn: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(rn.as_bstr().iter().map(|&b| b as u128).sum())
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks-advanced-remote",
            setup,
        )?;

    // Empty B onto the anonymous fork commit, then grow its all-empty stack around it.
    let b_commit = repo.rev_parse_single("B")?.detach();
    let a_commit = repo.rev_parse_single("A")?.detach();
    let outcome = but_workspace::commit::squash_commits(
        Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?,
        vec![b_commit],
        a_commit,
        but_workspace::commit::squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;
    let (graph, _) = outcome.rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;

    for (name, anchor_name, position) in [
        ("refs/heads/fz-1", "refs/heads/B", Position::Below),
        ("refs/heads/fz-3", "refs/heads/fz-1", Position::Above),
    ] {
        ws = but_workspace::branch::create_reference(
            gix::refs::FullName::try_from(name)?.as_ref(),
            Some(Anchor::AtSegment {
                ref_name: std::borrow::Cow::Owned(anchor_name.try_into()?),
                position,
            }),
            &repo,
            &ws,
            &mut meta,
            test_stack_id,
            None,
        )?
        .expect("the workspace changed");
        let pm = ws.project_meta().clone();
        ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;
    }
    let display_stacks = ws.display_stacks()?;
    let all_empty_stack = display_stacks
        .iter()
        .find(|stack| {
            stack
                .segments
                .iter()
                .any(|seg| seg.ref_name().is_some_and(|n| n.shorten() == "B"))
        })
        .expect("B's stack is projected");
    assert!(
        all_empty_stack
            .segments
            .iter()
            .all(|seg| seg.commits.is_empty()),
        "the stack holds only empty branches"
    );
    assert!(
        all_empty_stack.base().is_some(),
        "an all-empty stack keeps the commit it rests on as its base"
    );
    Ok(())
}

/// Creating a reference BELOW an empty anchor placed it at the anchor's resting commit's
/// PARENT (found by fuzzing): the resting commit of an empty belongs to the segment below
/// it, so stepping to the parent skipped that segment's whole territory and scrambled the
/// stack. Below an empty now shares its resting commit; only the order differs.
#[test]
fn repro_managed_create_below_empty_anchor() -> anyhow::Result<()> {
    use but_core::ref_metadata::StackId;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_workspace::branch::create_reference::{Anchor, Position};

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["A"]);
    }
    fn test_stack_id(rn: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(rn.as_bstr().iter().map(|&b| b as u128).sum())
    }
    // One stack: B (empty) on top of A (owns one commit).
    let (_tmp, ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-with-empty-top-branch",
            setup,
        )?;

    let mut ws = but_workspace::branch::create_reference(
        r_ref("refs/heads/below-B"),
        Some(Anchor::AtSegment {
            ref_name: std::borrow::Cow::Owned("refs/heads/B".try_into()?),
            position: Position::Below,
        }),
        &repo,
        &ws,
        &mut meta,
        test_stack_id,
        None,
    )?
    .expect("the workspace changed");
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;

    assert_eq!(
        repo.rev_parse_single("below-B")?.detach(),
        repo.rev_parse_single("A")?.detach(),
        "below an empty anchor shares its resting commit (A's tip), not A's parent"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    ├── 📙:below-B
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );
    Ok(())
}

/// Moving a same-commit empty branch ACROSS stacks took the metadata-only arm (found by
/// fuzzing): the subject's old chain stayed in the layout, so the next materialize built a
/// stale workspace leg from it and the projection dropped the target branch. Cross-stack
/// empty moves now go through the editor, which merges the chains before materializing.
#[test]
fn repro_managed_empty_move_across_stacks_same_commit() -> anyhow::Result<()> {
    use but_core::ref_metadata::StackId;
    use but_meta::VirtualBranchesTomlMetadata;

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
    }
    fn test_stack_id(rn: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(rn.as_bstr().iter().map(|&b| b as u128).sum())
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks-advanced-remote",
            setup,
        )?;

    // Stack A's commit relocates under B's, emptying A onto the fork commit.
    managed_move_and_apply(
        &repo,
        &mut meta,
        &mut ws,
        r_ref("refs/heads/A"),
        r_ref("refs/heads/B"),
    )?;
    let commits: Vec<_> = ws
        .display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .flat_map(|seg| {
            let name = seg.ref_name().map(|n| n.shorten().to_string());
            seg.commits.iter().map(move |c| (name.clone(), c.id))
        })
        .collect();
    let a_commit = commits
        .iter()
        .find_map(|(n, id)| (n.as_deref() == Some("A")).then_some(*id))
        .expect("A owns a commit");
    let b_commit = commits
        .iter()
        .find_map(|(n, id)| (n.as_deref() == Some("B")).then_some(*id))
        .expect("B owns a commit");
    let outcome = but_workspace::commit::move_commits(
        Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?,
        [b_commit],
        but_rebase::graph_rebase::anchor::Anchor::Commit(a_commit),
        but_rebase::graph_rebase::mutate::InsertSide::Below,
    )?;
    let (graph, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;

    // A fresh never-materialized chain moves onto B (empty, same commit, another stack).
    ws = but_workspace::branch::create_reference(
        r_ref("refs/heads/fz-4"),
        None,
        &repo,
        &ws,
        &mut meta,
        test_stack_id,
        None,
    )?
    .expect("the workspace changed");
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;
    managed_move_and_apply(
        &repo,
        &mut meta,
        &mut ws,
        r_ref("refs/heads/fz-4"),
        r_ref("refs/heads/B"),
    )?;

    let projected: Vec<_> = ws
        .display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|seg| seg.ref_name().map(|n| n.shorten().to_string()))
        .collect();
    for name in ["A", "fz-4", "B"] {
        assert!(
            projected.contains(&name.to_string()),
            "{name} dropped from the projection: {projected:?}"
        );
    }
    Ok(())
}

/// A chain member naming the workspace lower bound vanished from the stacks (found by
/// fuzzing): after A's commit re-homed under B and A emptied onto the bound, the builder
/// let the chain's bottom empty name the bound commit — a segment below the workspace that
/// the stacks never display. Chain members now always float above the bound as empties.
#[test]
fn repro_managed_chain_empty_never_names_the_bound() -> anyhow::Result<()> {
    use but_core::ref_metadata::StackId;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_workspace::branch::create_reference::{Anchor, Position};

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
    }
    fn test_stack_id(rn: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(rn.as_bstr().iter().map(|&b| b as u128).sum())
    }
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks-advanced-remote",
            setup,
        )?;

    // fz-0 below owner A lands at the bound; A's commit then re-homes under B,
    // emptying A onto the bound as well; finally B moves above A.
    ws = but_workspace::branch::create_reference(
        r_ref("refs/heads/fz-0"),
        Some(Anchor::AtSegment {
            ref_name: std::borrow::Cow::Owned("refs/heads/A".try_into()?),
            position: Position::Below,
        }),
        &repo,
        &ws,
        &mut meta,
        test_stack_id,
        None,
    )?
    .expect("the workspace changed");
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;

    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let outcome = but_workspace::commit::move_commits(
        Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?,
        [a_commit],
        but_rebase::graph_rebase::anchor::Anchor::Commit(b_commit),
        but_rebase::graph_rebase::mutate::InsertSide::Below,
    )?;
    let (graph, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm, &mut but_testsupport::in_memory_db())?;
    managed_move_and_apply(
        &repo,
        &mut meta,
        &mut ws,
        r_ref("refs/heads/B"),
        r_ref("refs/heads/A"),
    )?;

    let projected: Vec<_> = ws
        .display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|seg| seg.ref_name().map(|n| n.shorten().to_string()))
        .collect();
    for name in ["B", "A", "fz-0"] {
        assert!(
            projected.contains(&name.to_string()),
            "{name} dropped from the projection: {projected:?}"
        );
    }
    Ok(())
}

/// An applied chain with nothing above the workspace lower bound — one branch on an
/// unreachable side leg, the other resting below the bound on integrated territory —
/// was claimed by no run and vanished from the projection entirely (found by fuzzing
/// builder configurations): the next metadata write would have silently unapplied it.
/// Such chains now surface post-collect, once branch absorption by other stacks is known.
#[test]
fn repro_managed_chain_below_bound_surfaces() -> anyhow::Result<()> {
    use but_meta::VirtualBranchesTomlMetadata;

    fn setup(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &["D"]);
    }
    let (_tmp, ws, _repo, _meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-chain-below-bound",
            setup,
        )?;

    let projected: Vec<_> = ws
        .display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|seg| seg.ref_name().map(|n| n.shorten().to_string()))
        .collect();
    assert!(
        projected.contains(&"D".to_string()),
        "the chain's below-bound branch surfaces: {projected:?}"
    );
    assert!(
        ws.display_stacks()
            .expect("displayable")
            .iter()
            .any(|stack| stack.id
                == Some(but_core::ref_metadata::StackId::from_number_for_testing(2))),
        "the chain keeps its stack identity"
    );
    Ok(())
}

/// An applied EMPTY branch resting exactly at the target's unpulled tip (fuzz seed 145):
/// its chain must not leg the materialized workspace merge into target territory —
/// re-merging there silently adopts the unpulled commits and projects the target's
/// local (`main`) as an applied stack.
#[test]
fn repro_managed_empty_at_unpulled_target_keeps_ws_parents() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-empty-at-unpulled-target",
            |meta| {
                add_stack_with_segments(meta, 1, "E", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 7edd22c (origin/main, main, E) advanced
| * 04cdd5e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 7d7d38f (B) B1
|/  
* 85efbe4 M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    │   └── ·7d7d38f (🏘️)
    └── 📙:E

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    let (graph, _) = editor.rebase()?.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    // The round trip re-merges the workspace commit over B alone; E stays put at the
    // unpulled tip and settles as the chain's empty — no stack tipped by `main`.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 7edd22c (origin/main, main, E) advanced
| * 04cdd5e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 7d7d38f (B) B1
|/  
* 85efbe4 M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:B on 85efbe4 {1}
    ├── 📙:B
    │   └── ·7d7d38f (🏘️)
    └── 📙:E

"#]]
    );
    Ok(())
}

/// The target region's shape boundary lands MID-RUN of an advanced-outside branch's
/// segment (fuzz seed 378): re-minting the run's tail materialized those commits twice,
/// and the refs riding them tripped the one-name-one-reference layout assert.
#[test]
fn repro_target_region_boundary_mid_outside_run() -> anyhow::Result<()> {
    let (_tmp, ws, _repo, _meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-region-boundary-mid-outside-run",
            |meta| {
                add_stack_with_segments(meta, 1, "W", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 3, "D", StackState::InWorkspace, &["C"]);
            },
        )?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣4 on 85efbe4
├── ≡📙:W on 85efbe4 {1}
│   └── 📙:W
└── ≡:D {3}
    └── :D

"#]]
    );
    Ok(())
}

/// True-empty applied chain members resting on integrated target-only territory must
/// stay projected (builder-config fuzz seed 0): the stack bottom wears the walk's stop
/// base, and `prune_out_of_cone_empties` keyed on that anchor alone — so chain position,
/// not territory, decided which branch vanished (A dropped while C and D survived, all
/// resting on the same integrated commits; the vanished applied branch trips the
/// projection's display-completeness assert). B and E rest on unincorporated side legs
/// and stay hidden.
#[test]
fn repro_true_empty_chain_on_target_tip_stays_projected() -> anyhow::Result<()> {
    let (_tmp, ws, repo, _meta, _description) = named_writable_scenario_with_description_and_graph(
        "ws-ref-ws-commit-true-empties-on-target-tip",
        |meta| {
            add_stack_with_segments(meta, 1, "D", StackState::InWorkspace, &["B", "C", "E", "A"]);
        },
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* ad48b0b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| *   0257dd1 (origin/main, main, C, A) T
| |/  
| | * fd2df6a (D) m3
| | | * 82142e7 (E) m6
| |_|/  
|/| |   
| | | * 94b2d6c (B) m4
| | |/  
| | * 03bb4ef m2
| |/  
| * 6bbb057 m1
|/  
* c04932f M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣4 on c04932f
└── ≡📙:D on 0257dd1 {1}
    ├── 📙:D
    ├── 📙:C
    └── 📙:A

"#]]
    );
    Ok(())
}

/// A chain with content only OUTSIDE the workspace (A advanced-outside) and an empty
/// member (D) resting INSIDE the anonymous lane the chain claims (fuzz seed 523): the
/// splice must interpose on the lane's own edge, not leg the workspace merge onto D's
/// anchor — that grew the merge a redundant parent and reshuffled stack identities on
/// the next materialize. Pure-empty chains keep their fresh workspace leg.
#[test]
fn repro_managed_empty_inside_anonymous_lane_keeps_ws_parents() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-empty-inside-anonymous-lane",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &["D"]);
            },
        )?;
    let ws_commit_before = repo.rev_parse_single("gitbutler/workspace")?.detach();
    let projected_before = graph_workspace(&ws).to_string();
    snapbox::assert_data_eq!(
        &*projected_before,
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on f090f6e
├── ≡:anon: on f090f6e
│   └── :anon:
│       └── ·9f9f57e (🏘️)
└── ≡📙:D on f090f6e {1}
    └── 📙:D

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    let (graph, _) = editor.rebase()?.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    assert_eq!(
        repo.rev_parse_single("gitbutler/workspace")?.detach(),
        ws_commit_before,
        "a no-op round trip keeps the workspace merge bit for bit"
    );
    assert_eq!(
        graph_workspace(&ws).to_string(),
        projected_before,
        "the projection survives the round trip unchanged"
    );
    Ok(())
}

/// The member-carrier twin (fuzz seed 668): the chain's empty rests on its OWN
/// content's leg base (A empty at E's leg base = the target tip). The splice
/// interposes on the chain member's edge — a fresh workspace leg would re-merge the
/// target tip in as a redundant parent.
#[test]
fn repro_managed_empty_at_own_leg_base_keeps_ws_parents() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-empty-at-own-leg-base",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &["E"]);
            },
        )?;
    let ws_commit_before = repo.rev_parse_single("gitbutler/workspace")?.detach();
    let projected_before = graph_workspace(&ws).to_string();
    snapbox::assert_data_eq!(
        &*projected_before,
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 29ab206
└── ≡📙:E on 29ab206 {1}
    ├── 📙:E
    │   └── ·ef6688b (🏘️)
    └── 📙:A

"#]]
    );

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
    let (graph, _) = editor.rebase()?.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &meta, &mut but_testsupport::in_memory_db())?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;

    assert_eq!(
        repo.rev_parse_single("gitbutler/workspace")?.detach(),
        ws_commit_before,
        "a no-op round trip keeps the workspace merge bit for bit"
    );
    assert_eq!(
        graph_workspace(&ws).to_string(),
        projected_before,
        "the projection survives the round trip unchanged"
    );
    Ok(())
}

fn r_ref(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically valid ref name")
}

fn managed_move_and_apply(
    repo: &gix::Repository,
    meta: &mut but_meta::VirtualBranchesTomlMetadata,
    ws: &mut but_graph::Workspace,
    subject: &gix::refs::FullNameRef,
    target: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), meta, repo)?;
    but_workspace::branch::move_branch(editor, ws, subject, target)?.apply(
        ws,
        repo,
        &mut but_testsupport::in_memory_db(),
    )?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(
        repo,
        meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
    )?;
    Ok(())
}

/// Tests for `move_branch` in single-branch (ad-hoc) mode, where `HEAD` is on a plain local branch
/// (no `gitbutler/workspace` commit) and the tip-to-base order of same-commit empty branches lives
/// in the `branch_order` metadata table rather than in `Workspace` metadata.
mod single_branch_mode {
    use std::collections::HashMap;

    use but_core::RefMetadata;
    use but_core::ref_metadata::StackId;
    use but_graph::walk::Options;
    use but_meta::BranchOrderMetadata;
    use but_rebase::graph_rebase::Editor;
    use but_testsupport::{graph_workspace, invoke_bash};
    use but_workspace::branch::create_reference::{Anchor, Position};

    use crate::ref_info::with_workspace_commit::utils::named_writable_scenario;
    use crate::utils::r;

    fn stack_id_for_name(rn: &gix::refs::FullNameRef) -> StackId {
        use bstr::ByteSlice;
        StackId::from_number_for_testing(rn.shorten().chars().map(|c| c as u128).sum())
    }

    fn branch_order_meta(repo: &gix::Repository) -> anyhow::Result<BranchOrderMetadata> {
        BranchOrderMetadata::from_paths(repo.path().join("virtual-branches.toml"), repo.path())
    }

    fn project_meta(repo: &gix::Repository) -> but_core::ref_metadata::ProjectMeta {
        but_core::ref_metadata::ProjectMeta::resolve(repo).unwrap_or_default()
    }

    fn ad_hoc_workspace_with_three_non_empty_branches(
        head: &str,
    ) -> anyhow::Result<(
        tempfile::TempDir,
        gix::Repository,
        BranchOrderMetadata,
        but_core::ref_metadata::ProjectMeta,
    )> {
        let (tmp, repo, _legacy_meta) =
            named_writable_scenario("single-branch-three-branch-stack")?;
        if head != "C" {
            invoke_bash(&format!("git checkout {head}\n"), &repo);
        }
        let mut meta = branch_order_meta(&repo)?;
        meta.set_branch_stack_order(&[
            r("refs/heads/C").to_owned(),
            r("refs/heads/B").to_owned(),
            r("refs/heads/A").to_owned(),
            r("refs/heads/main").to_owned(),
        ])?;
        let project_meta = project_meta(&repo);
        Ok((tmp, repo, meta, project_meta))
    }

    /// `move_branch` returns the reordered chain instead of persisting it (so callers can skip
    /// persistence for dry runs); persist it here to mimic a real, non-dry-run caller.
    fn persist_order(
        meta: &mut BranchOrderMetadata,
        order: &Option<Vec<gix::refs::FullName>>,
    ) -> anyhow::Result<()> {
        if let Some(order) = order {
            meta.set_branch_stack_order(order)?;
        }
        Ok(())
    }

    fn move_branch_and_apply(
        repo: &gix::Repository,
        meta: &mut BranchOrderMetadata,
        project_meta: but_core::ref_metadata::ProjectMeta,
        subject: &gix::refs::FullNameRef,
        target: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<Vec<gix::refs::FullName>>> {
        let ws = but_graph::Workspace::from_head(
            repo,
            meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), meta, repo)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            ws_meta,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(editor, &ws, subject, target)?;
        assert!(
            ws_meta.is_none(),
            "ad-hoc reorder lives in branch_order, not workspace metadata"
        );
        rebase.materialize()?;
        persist_order(meta, &branch_stack_order)?;
        if let Some(new_tip) = new_tip {
            invoke_bash(&format!("git checkout {}\n", new_tip.shorten()), repo);
        }
        Ok(branch_stack_order)
    }

    fn assert_head(repo: &gix::Repository, branch_name: &str) {
        let actual = repo
            .head_name()
            .expect("HEAD can be read")
            .expect("HEAD points to a branch")
            .to_string();
        assert_eq!(actual, format!("refs/heads/{branch_name}"));
    }

    fn branch_tip(repo: &gix::Repository, branch_name: &str) -> gix::ObjectId {
        repo.rev_parse_single(branch_name)
            .expect("branch exists")
            .detach()
    }

    fn normalized_graph_snapshot(repo: &gix::Repository) -> anyhow::Result<String> {
        let rendered = but_testsupport::visualize_commit_graph_all(repo)?;
        let mut labels = HashMap::new();
        Ok(normalize_graph(&rendered, &mut labels)
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n"))
    }

    fn normalize_graph(graph: &str, labels: &mut HashMap<String, String>) -> String {
        let mut out = String::new();
        let mut token = String::new();
        for ch in graph.chars() {
            if ch.is_ascii_hexdigit() {
                token.push(ch);
            } else {
                push_normalized_token(&mut out, &mut token, labels);
                out.push(ch);
            }
        }
        push_normalized_token(&mut out, &mut token, labels);
        out
    }

    fn push_normalized_token(
        out: &mut String,
        token: &mut String,
        labels: &mut HashMap<String, String>,
    ) {
        if (7..=40).contains(&token.len()) && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
            let next = labels.len() + 1;
            let label = labels
                .entry(std::mem::take(token))
                .or_insert_with(|| format!("[C{next}]"));
            out.push_str(label);
        } else {
            out.push_str(token);
            token.clear();
        }
    }

    /// Build a single-branch (ad-hoc) workspace on `main` (3 commits) with two empty dependent
    /// branches `empty-top` and `empty-bottom` stacked above the commit-owning base branch.
    ///
    /// The tip-to-base branch order ends up as `[main, empty-top, empty-bottom, base]`, so both
    /// `empty-top` and `empty-bottom` are empty segments that can be reordered by metadata alone.
    fn ad_hoc_workspace_with_two_empty_branches() -> anyhow::Result<(
        tempfile::TempDir,
        gix::Repository,
        BranchOrderMetadata,
        but_core::ref_metadata::ProjectMeta,
    )> {
        let (tmp, repo, _legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
        let project_meta = crate::ref_info::with_workspace_commit::utils::project_meta(&repo)?;
        let mut meta = branch_order_meta(&repo)?;

        let main_ref = r("refs/heads/main");
        let mut ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta.clone(),
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;

        // Each branch is inserted directly below `main`, so creating them in this order yields the
        // chain [main, empty-top, empty-bottom, base] (tip to base).
        for name in [
            "refs/heads/base",
            "refs/heads/empty-bottom",
            "refs/heads/empty-top",
        ] {
            ws = but_workspace::branch::create_reference(
                r(name),
                Anchor::at_reference(main_ref, Position::Below),
                &repo,
                &ws,
                &mut meta,
                stack_id_for_name,
                None,
            )?
            .expect("the workspace changed");
        }

        Ok((tmp, repo, meta, project_meta))
    }

    /// Moving a branch on top of the checked-out tip reports it as `new_tip` so the caller can check
    /// it out; the operation itself does not move `HEAD`.
    #[test]
    fn reorder_above_checked_out_tip_returns_new_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) = ad_hoc_workspace_with_two_empty_branches()?;
        let main_ref = r("refs/heads/main");

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        // `main` is the checked-out entrypoint (the projected tip).
        assert_eq!(ws.ref_name(), Some(main_ref));

        // Move empty `empty-bottom` on top of the checked-out `main`, which makes it the new tip.
        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            &ws,
            r("refs/heads/empty-bottom"),
            main_ref,
        )?;
        rebase.materialize()?;

        // The subject is reported as the new tip so the caller can check it out.
        assert_eq!(
            new_tip.as_ref().map(|n| n.as_ref()),
            Some(r("refs/heads/empty-bottom"))
        );
        // The reordered chain is returned (for the caller to persist), placing the subject on top.
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/empty-bottom").to_owned(),
                r("refs/heads/main").to_owned(),
                r("refs/heads/empty-top").to_owned(),
                r("refs/heads/base").to_owned(),
            ]),
        );
        Ok(())
    }

    /// A reorder that does not touch the tip leaves `new_tip` unset.
    #[test]
    fn reorder_below_tip_has_no_new_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) = ad_hoc_workspace_with_two_empty_branches()?;

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;

        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            &ws,
            r("refs/heads/empty-bottom"),
            r("refs/heads/empty-top"),
        )?;
        rebase.materialize()?;

        assert_eq!(
            new_tip, None,
            "target isn't the tip, so the tip is unchanged"
        );
        assert!(
            branch_stack_order.is_some(),
            "the reorder is still computed and returned"
        );
        Ok(())
    }

    /// `move_branch` reorders two empty branches in single-branch (ad-hoc) mode by rewriting the
    /// `branch_order` metadata, without any graph rewrite.
    #[test]
    fn reorder_empty_branches_updates_branch_order() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) = ad_hoc_workspace_with_two_empty_branches()?;
        let main_ref = r("refs/heads/main");

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta.clone(),
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        // Single-branch (ad-hoc) workspace: `HEAD` is on `main` directly, no `gitbutler/workspace`
        // commit. `empty-top`/`empty-bottom` are empty segments; `base` owns the commits.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    ├── :main[🌳]
    ├── 📙:empty-top
    ├── 📙:empty-bottom
    └── 📙:base
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
        );
        assert_eq!(
            meta.branch_stack_order(main_ref)?,
            Some(vec![
                r("refs/heads/main").to_owned(),
                r("refs/heads/empty-top").to_owned(),
                r("refs/heads/empty-bottom").to_owned(),
                r("refs/heads/base").to_owned(),
            ]),
        );

        // Move `empty-bottom` on top of `empty-top` (both empty) - a pure metadata reorder.
        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            ws_meta,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            &ws,
            r("refs/heads/empty-bottom"),
            r("refs/heads/empty-top"),
        )?;
        assert!(
            ws_meta.is_none(),
            "ad-hoc reorder lives in branch_order, not workspace metadata"
        );
        rebase.materialize()?;
        // A real (non-dry-run) caller persists the returned order.
        persist_order(&mut meta, &branch_stack_order)?;

        // The ad-hoc order is updated: `empty-bottom` now sits above `empty-top`.
        assert_eq!(
            meta.branch_stack_order(main_ref)?,
            Some(vec![
                r("refs/heads/main").to_owned(),
                r("refs/heads/empty-bottom").to_owned(),
                r("refs/heads/empty-top").to_owned(),
                r("refs/heads/base").to_owned(),
            ]),
        );

        // Re-projecting from the reloaded metadata reflects the new order, and no commit was moved.
        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    ├── :main[🌳]
    ├── 📙:empty-bottom
    ├── 📙:empty-top
    └── 📙:base
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
        );

        Ok(())
    }

    /// Repro of `moveBranch.spec.ts` "keeps empty dependent branches when moving their
    /// commit-owning branch to the top": a commit-owning branch with two empty branches on its
    /// tip, moved to the top. The commit must ride up and the empties must drop to the base.
    #[test]
    fn move_commit_owning_branch_to_top_past_empties() -> anyhow::Result<()> {
        let (_tmp, repo, _legacy_meta) =
            named_writable_scenario("single-branch-commit-with-empties")?;
        let project_meta = project_meta(&repo);
        let mut meta = branch_order_meta(&repo)?;
        meta.set_branch_stack_order(&[
            r("refs/heads/empty-top").to_owned(),
            r("refs/heads/empty-low").to_owned(),
            r("refs/heads/commit-branch").to_owned(),
            r("refs/heads/single-branch-fixture").to_owned(),
        ])?;

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta.clone(),
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:empty-top[🌳] <> ✓!
└── ≡:empty-top[🌳] {1}
    ├── :empty-top[🌳]
    ├── :empty-low
    ├── :commit-branch
    │   └── ·cfb7163
    └── :single-branch-fixture
        └── ·563a7fc

"#]]
        );

        let branch_stack_order = move_branch_and_apply(
            &repo,
            &mut meta,
            project_meta.clone(),
            r("refs/heads/commit-branch"),
            r("refs/heads/empty-top"),
        )?;
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/commit-branch").to_owned(),
                r("refs/heads/empty-top").to_owned(),
                r("refs/heads/empty-low").to_owned(),
                r("refs/heads/single-branch-fixture").to_owned(),
            ]),
            "commit-branch reorders to the top; empties keep their relative order below it"
        );

        // The empties re-point onto the base on disk; commit-branch keeps its commit.
        let base_tip = branch_tip(&repo, "single-branch-fixture");
        assert_eq!(branch_tip(&repo, "empty-top"), base_tip);
        assert_eq!(branch_tip(&repo, "empty-low"), base_tip);
        assert_ne!(branch_tip(&repo, "commit-branch"), base_tip);

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:commit-branch[🌳] <> ✓!
└── ≡:commit-branch[🌳] {1}
    ├── :commit-branch[🌳]
    │   └── ·cfb7163
    ├── :empty-top
    ├── :empty-low
    └── :single-branch-fixture
        └── ·563a7fc

"#]]
        );
        Ok(())
    }

    #[test]
    fn move_middle_non_empty_branch_to_top_checks_out_subject() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) =
            ad_hoc_workspace_with_three_non_empty_branches("C")?;

        snapbox::assert_data_eq!(
            normalized_graph_snapshot(&repo)?,
            snapbox::str![[r#"
* [C1] (HEAD -> C) add c
* [C2] (B) add b
* [C3] (A) add a
* [C4] (main) add main"#]]
        );

        let branch_stack_order = move_branch_and_apply(
            &repo,
            &mut meta,
            project_meta,
            r("refs/heads/B"),
            r("refs/heads/C"),
        )?;

        assert_head(&repo, "B");
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/B").to_owned(),
                r("refs/heads/C").to_owned(),
                r("refs/heads/A").to_owned(),
                r("refs/heads/main").to_owned(),
            ])
        );
        snapbox::assert_data_eq!(
            normalized_graph_snapshot(&repo)?,
            snapbox::str![[r#"
* [C1] (HEAD -> B) add b
* [C2] (C) add c
* [C3] (A) add a
* [C4] (main) add main
"#]]
        );

        Ok(())
    }

    #[test]
    fn move_bottom_non_empty_branch_to_top_checks_out_subject() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) =
            ad_hoc_workspace_with_three_non_empty_branches("C")?;

        snapbox::assert_data_eq!(
            normalized_graph_snapshot(&repo)?,
            snapbox::str![[r#"
* [C1] (HEAD -> C) add c
* [C2] (B) add b
* [C3] (A) add a
* [C4] (main) add main"#]]
        );

        let branch_stack_order = move_branch_and_apply(
            &repo,
            &mut meta,
            project_meta,
            r("refs/heads/A"),
            r("refs/heads/C"),
        )?;

        assert_head(&repo, "A");
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/A").to_owned(),
                r("refs/heads/C").to_owned(),
                r("refs/heads/B").to_owned(),
                r("refs/heads/main").to_owned(),
            ])
        );
        snapbox::assert_data_eq!(
            normalized_graph_snapshot(&repo)?,
            snapbox::str![[r#"
* [C1] (HEAD -> A) add a
* [C2] (C) add c
* [C3] (B) add b
* [C4] (main) add main
"#]]
        );

        Ok(())
    }

    #[test]
    fn move_top_non_empty_branch_down_checks_out_new_top() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) =
            ad_hoc_workspace_with_three_non_empty_branches("C")?;

        let branch_stack_order = move_branch_and_apply(
            &repo,
            &mut meta,
            project_meta,
            r("refs/heads/C"),
            r("refs/heads/A"),
        )?;

        assert_head(&repo, "B");
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/B").to_owned(),
                r("refs/heads/C").to_owned(),
                r("refs/heads/A").to_owned(),
                r("refs/heads/main").to_owned(),
            ]),
            "moving the checked-out tip down should make the branch above it the new tip"
        );
        // The same commits are reordered to match the branch order, with the new tip checked out.
        snapbox::assert_data_eq!(
            normalized_graph_snapshot(&repo)?,
            snapbox::str![[r#"
* [C1] (HEAD -> B) add b
* [C2] (C) add c
* [C3] (A) add a
* [C4] (main) add main
"#]]
        );

        Ok(())
    }

    #[test]
    fn move_top_non_empty_branch_above_current_parent_is_a_noop() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) =
            ad_hoc_workspace_with_three_non_empty_branches("C")?;
        let tips_before = ["A", "B", "C"].map(|branch| branch_tip(&repo, branch));

        let branch_stack_order = move_branch_and_apply(
            &repo,
            &mut meta,
            project_meta,
            r("refs/heads/C"),
            r("refs/heads/B"),
        )?;

        assert_head(&repo, "C");
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/C").to_owned(),
                r("refs/heads/B").to_owned(),
                r("refs/heads/A").to_owned(),
                r("refs/heads/main").to_owned(),
            ]),
            "placing the tip above its current parent should preserve branch order"
        );
        assert_eq!(
            ["A", "B", "C"].map(|branch| branch_tip(&repo, branch)),
            tips_before,
            "a no-op move should not rewrite commits"
        );

        Ok(())
    }

    #[test]
    fn move_bottom_branch_above_checked_out_middle_leaves_top_branch_untouched()
    -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) =
            ad_hoc_workspace_with_three_non_empty_branches("B")?;
        let c_tip_before = branch_tip(&repo, "C");

        snapbox::assert_data_eq!(
            normalized_graph_snapshot(&repo)?,
            snapbox::str![[r#"
* [C1] (C) add c
* [C2] (HEAD -> B) add b
* [C3] (A) add a
* [C4] (main) add main"#]]
        );

        let branch_stack_order = move_branch_and_apply(
            &repo,
            &mut meta,
            project_meta,
            r("refs/heads/A"),
            r("refs/heads/B"),
        )?;

        assert_head(&repo, "A");
        assert_eq!(
            branch_tip(&repo, "C"),
            c_tip_before,
            "C should stay untouched when it is above the checked-out entrypoint"
        );
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/C").to_owned(),
                r("refs/heads/A").to_owned(),
                r("refs/heads/B").to_owned(),
                r("refs/heads/main").to_owned(),
            ])
        );
        snapbox::assert_data_eq!(
            normalized_graph_snapshot(&repo)?,
            snapbox::str![[r#"
* [C1] (HEAD -> A) add a
* [C2] (B) add b
| * [C3] (C) add c
| * [C4] add b
| * [C5] add a
|/
* [C6] (main) add main
"#]]
        );

        Ok(())
    }

    /// Moving an *empty* branch onto the commit-owning base is a metadata-only reorder and must be
    /// allowed - only a non-empty *subject* needs a real rebase, so a non-empty *target* is fine.
    #[test]
    fn reorder_empty_branch_onto_commit_owning_base() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) = ad_hoc_workspace_with_two_empty_branches()?;

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;

        // `base` owns the stack's commits; moving the empty `empty-top` on top of it is still just a
        // metadata reorder and must succeed (previously rejected because the target owns commits).
        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            &ws,
            r("refs/heads/empty-top"),
            r("refs/heads/base"),
        )?;
        rebase.materialize()?;

        assert_eq!(new_tip, None, "base isn't the checked-out tip");
        // `empty-top` is placed directly above `base`; the rest of the order is preserved.
        assert_eq!(
            branch_stack_order,
            Some(vec![
                r("refs/heads/main").to_owned(),
                r("refs/heads/empty-bottom").to_owned(),
                r("refs/heads/empty-top").to_owned(),
                r("refs/heads/base").to_owned(),
            ]),
        );
        Ok(())
    }

    /// Regression for the "clobbering" concern (#4): a branch is only projected as a *movable*
    /// segment in ad-hoc mode when it's already part of `branch_order`. Refs that aren't tracked
    /// there (e.g. stale/partial metadata, or refs created outside GitButler) are not projected as
    /// segments, so `move_branch` fails to find them *before* reaching the reorder - it can never
    /// overwrite the persisted order down to just untracked refs. This documents why the
    /// "neither ref is tracked" path is unreachable in practice.
    #[test]
    fn untracked_refs_are_not_movable_and_never_clobber_order() -> anyhow::Result<()> {
        use gix::refs::transaction::PreviousValue;

        let (_tmp, repo, mut meta, project_meta) = ad_hoc_workspace_with_two_empty_branches()?;
        let main_ref = r("refs/heads/main");
        let order_before = meta.branch_stack_order(main_ref)?;
        let tip = repo.find_reference(main_ref)?.peel_to_id()?.detach();

        // Two refs at the tip that were never added to `branch_order`. They show up only as commit
        // decorations, not as ordered stack segments.
        repo.reference(r("refs/heads/x"), tip, PreviousValue::Any, "test")?;
        repo.reference(r("refs/heads/y"), tip, PreviousValue::Any, "test")?;

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    ├── :main[🌳]
    ├── 📙:empty-top
    ├── 📙:empty-bottom
    └── 📙:base
        ├── ·281da94 ►x, ►y
        ├── ·12995d7
        └── ·3d57fc1

"#]]
        );

        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
        let err = match but_workspace::branch::move_branch(
            editor,
            &ws,
            r("refs/heads/x"),
            r("refs/heads/y"),
        ) {
            Ok(_) => panic!("untracked refs must not be movable in single-branch mode"),
            Err(err) => err,
        };
        assert_eq!(
            err.to_string(),
            "Couldn't find branch to move in workspace with reference name: refs/heads/x"
        );
        assert_eq!(
            meta.branch_stack_order(main_ref)?,
            order_before,
            "the branch order must be untouched"
        );
        Ok(())
    }

    /// `move_branch` computes the reorder but must not persist it on its own: the caller applies it,
    /// which is what lets the API skip persistence for dry-run previews without corrupting metadata.
    #[test]
    fn move_branch_does_not_persist_branch_order() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) = ad_hoc_workspace_with_two_empty_branches()?;
        let main_ref = r("refs/heads/main");
        let order_before = meta.branch_stack_order(main_ref)?;

        let ws = but_graph::Workspace::from_head(
            &repo,
            &meta,
            project_meta,
            &mut but_testsupport::in_memory_db(),
            Options::limited(),
        )?;
        let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            &ws,
            r("refs/heads/empty-bottom"),
            r("refs/heads/empty-top"),
        )?;
        rebase.materialize()?;

        // A reorder is computed and returned...
        assert!(branch_stack_order.is_some());
        // ...but nothing is written to metadata until the caller persists it.
        assert_eq!(
            meta.branch_stack_order(main_ref)?,
            order_before,
            "move_branch must not persist branch order on its own"
        );
        Ok(())
    }

    /// Replay a fuzzer-found move sequence over the commit-with-empties fixture, printing the
    /// workspace order before each move; fails on any `BUG:`-classed error (an editor
    /// well-formedness violation).
    fn replay_move_sequence(attempts: &[(&str, &str)]) -> anyhow::Result<()> {
        let (_tmp, repo, _legacy_meta) =
            named_writable_scenario("single-branch-commit-with-empties")?;
        let project_meta = project_meta(&repo);
        let mut meta = branch_order_meta(&repo)?;
        meta.set_branch_stack_order(&[
            r("refs/heads/empty-top").to_owned(),
            r("refs/heads/empty-low").to_owned(),
            r("refs/heads/commit-branch").to_owned(),
            r("refs/heads/single-branch-fixture").to_owned(),
        ])?;

        for (i, (subject, target)) in attempts.iter().enumerate() {
            let ws = but_graph::Workspace::from_head(
                &repo,
                &meta,
                project_meta.clone(),
                &mut but_testsupport::in_memory_db(),
                Options::limited(),
            )?;
            let order: Vec<_> = ws
                .display_stacks()
                .expect("displayable")
                .iter()
                .flat_map(|stack| &stack.segments)
                .filter_map(|seg| seg.ref_name().map(|n| n.shorten().to_string()))
                .collect();
            eprintln!("step {i}: order before = {order:?}, move {subject} -> {target}");
            match move_branch_and_apply(
                &repo,
                &mut meta,
                project_meta.clone(),
                r(subject),
                r(target),
            ) {
                Ok(order) => eprintln!("  ok, new order = {order:?}"),
                Err(e) => {
                    let msg = format!("{e:?}");
                    eprintln!("  err: {msg}");
                    assert!(!msg.contains("BUG:"), "collision at step {i}: {msg}");
                }
            }
        }
        Ok(())
    }

    /// The fuzzer's seed-3 finding: a commit-owning branch moving down past an empty left the
    /// crossed empty carrying the re-added parent edge in a parallel group — a rank-0 collision.
    #[test]
    fn repro_seed3_ref_position_collision() -> anyhow::Result<()> {
        replay_move_sequence(&[
            ("refs/heads/commit-branch", "refs/heads/empty-top"),
            ("refs/heads/single-branch-fixture", "refs/heads/empty-top"),
            ("refs/heads/commit-branch", "refs/heads/empty-low"),
            ("refs/heads/commit-branch", "refs/heads/empty-low"),
            ("refs/heads/empty-top", "refs/heads/commit-branch"),
            (
                "refs/heads/commit-branch",
                "refs/heads/single-branch-fixture",
            ),
        ])?;
        // The last move ([empty-top, commit-branch, empty-low, base] with commit-branch moved
        // above the base) lifts both empties above commit-branch; they must share its tip.
        Ok(())
    }

    /// The fuzzer's seed-120 finding: moving the base branch above everything let the empties
    /// ride its relocated commit while the persisted order said otherwise; the diverged state
    /// then collided on a later move. The unrepresentable step now refuses cleanly (an empty
    /// may not sit below the bottom branch) and the rest reconcile.
    #[test]
    fn repro_seed120_ref_position_collision() -> anyhow::Result<()> {
        replay_move_sequence(&[
            ("refs/heads/empty-low", "refs/heads/empty-top"),
            ("refs/heads/commit-branch", "refs/heads/empty-low"),
            ("refs/heads/empty-low", "refs/heads/single-branch-fixture"),
            (
                "refs/heads/single-branch-fixture",
                "refs/heads/commit-branch",
            ),
            ("refs/heads/empty-top", "refs/heads/empty-low"),
            ("refs/heads/empty-low", "refs/heads/commit-branch"),
        ])
    }
}
