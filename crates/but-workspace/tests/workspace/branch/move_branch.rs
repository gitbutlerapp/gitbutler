use but_core::RefMetadata;
use but_core::ref_metadata::StackKind;
use but_graph::init::Options;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_workspace, invoke_bash, visualize_commit_graph_all};
use snapbox::IntoData;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description,
    named_writable_scenario_with_description_and_graph, project_meta,
};

#[test]
fn move_top_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put C on top of A
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/C".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   0ffeac6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f2cc60d (C) C
| * 09d8e52 (A) A
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:C on 85efbe4 {1}
│   ├── 📙:3:C
│   │   └── ·f2cc60d (🏘️)
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:5:B on 85efbe4 {2}
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn moving_branch_onto_itself_fails_without_changing_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;

    let mut ws = graph.into_workspace()?;
    let before = graph_workspace(&ws).to_string();
    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;

    let err = but_workspace::branch::move_branch(
        editor,
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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:B on 85efbe4 {1}
│   ├── 📙:3:B
│   │   └── ·f9061ed (🏘️)
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:5:C on 85efbe4 {2}
    └── 📙:5:C
        └── ·8e00332 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_single_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put A on top of C
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/C".try_into()?,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:3:A on 85efbe4 {2}
    ├── 📙:3:A
    │   └── ·148f8f3 (🏘️)
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn reorder_branch_in_stack() -> anyhow::Result<()> {
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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put B on top of C
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/C".try_into()?,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:B on 85efbe4 {2}
    ├── 📙:4:B
    │   └── ·de0581e (🏘️)
    └── 📙:5:C
        └── ·8e00332 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn insert_branch_in_the_middle_of_a_stack() -> anyhow::Result<()> {
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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put A on top of B, and below C
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:3:C on 85efbe4 {2}
    ├── 📙:3:C
    │   └── ·3e7ff55 (🏘️)
    ├── 📙:4:A
    │   └── ·4dfe841 (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:B on 85efbe4 {2}
    └── 📙:4:B

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put B on top of A
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:4:B on 85efbe4 {1}
    ├── 📙:4:B
    └── 📙:5:A
        └── ·09d8e52 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn move_branch_on_top_of_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:B on 85efbe4 {2}
    └── 📙:4:B

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put A on top of B
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
└── ≡📙:3:A on 85efbe4 {2}
    ├── 📙:3:A
    │   └── ·09d8e52 (🏘️)
    └── 📙:4:B

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

    let project_meta = project_meta(&repo)?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta,
        Options {
            extra_target_commit_id: repo
                .rev_parse_single("gitbutler/target")
                .ok()
                .map(|id| id.detach()),
            ..Options::limited()
        },
    )?;

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡📙:4:B on 3183e43 {1}
    ├── 📙:4:B
    └── 📙:5:A

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡📙:4:A on 3183e43 {1}
    ├── 📙:4:A
    └── 📙:5:B

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

    let project_meta = project_meta(&repo)?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta,
        Options {
            extra_target_commit_id: repo
                .rev_parse_single("gitbutler/target")
                .ok()
                .map(|id| id.detach()),
            ..Options::limited()
        },
    )?;

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡📙:4:A on 3183e43 {1}
│   └── 📙:4:A
└── ≡📙:5:B on 3183e43 {2}
    └── 📙:5:B

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡📙:4:A on 3183e43 {2}
    ├── 📙:4:A
    └── 📙:5:B

"#]]
    );

    Ok(())
}

#[test]
fn non_empty_move_updates_metadata_and_keeps_display_order_aligned() -> anyhow::Result<()> {
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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );
    let before_display_order = stack_display_order(&ws);
    let before_metadata_order = metadata_stack_order(&ws);
    assert_eq!(
        before_display_order, before_metadata_order,
        "workspace projection order should match metadata before moving now that stack order is no longer reversed downstream"
    );

    // Move non-empty C on top of non-empty A.
    // This rewrites metadata and keeps display + metadata aligned.
    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/C".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    let updated_metadata_order = ws_meta
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::Applied))
        .unwrap_or_default();

    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;

    // before refreshing `ws` the pure-virtual change isn't visible (should be fixed once meta is in db!)
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:5:B on 85efbe4 {2}
│   └── 📙:5:B
│       └── ·c813d8d (🏘️)
└── ≡📙:4:C on 85efbe4 {1}
    ├── 📙:4:C
    │   └── ·f2cc60d (🏘️)
    └── 📙:3:A
        └── ·09d8e52 (🏘️)

"#]]
    );
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;
    // after the refresh the workspace is finally uptodate (this will probably be an issue unless callers know that)
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:C on 85efbe4 {1}
│   ├── 📙:3:C
│   │   └── ·f2cc60d (🏘️)
│   └── 📙:4:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:5:B on 85efbe4 {2}
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let after_display_order = stack_display_order(&ws);

    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);
    assert_eq!(
        after_display_order, updated_metadata_order,
        "workspace projection order should match metadata after moving now that stack order is no longer reversed downstream"
    );

    snapbox::assert_data_eq!(
        format!("{before_display_order:#?}"),
        snapbox::str![[r#"
[
    "refs/heads/A",
    "refs/heads/C",
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
fn empty_move_keeps_display_order_aligned_with_metadata() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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

    let mut ws = graph.into_workspace()?;
    let before_display_order = stack_display_order(&ws);
    let before_metadata_order = metadata_stack_order(&ws);
    assert_eq!(before_display_order, before_metadata_order);

    // Move empty B on top of non-empty A.
    // This path rewrites metadata and keeps display + metadata aligned.
    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    let updated_metadata_order = ws_meta
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::AppliedAndUnapplied))
        .unwrap_or_default();

    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    let after_display_order = stack_display_order(&ws);

    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);
    assert_eq!(after_display_order, updated_metadata_order);

    snapbox::assert_data_eq!(
        format!("{before_display_order:#?}"),
        snapbox::str![[r#"
[
    "refs/heads/A",
    "refs/heads/B",
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
    let (_tmp, graph, repo, mut meta, _description) =
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

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:B on 85efbe4 {2}
    └── 📙:4:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Move B on top of A — the base segment at the old fork point has no ref name.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:3:B on 85efbe4 {1}
    ├── 📙:3:B
    │   └── ·f9061ed (🏘️)
    └── 📙:4:A
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
    let (_tmp, graph, repo, mut meta, _description) =
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

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:5:B on 85efbe4 {2}
    └── 📙:5:B

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put empty B on top of non-empty A.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:5:B on 85efbe4 {1}
    ├── 📙:5:B
    └── 📙:6:A
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_non_empty_branch_onto_empty_branch_with_advanced_target() -> anyhow::Result<()> {
    // Same setup as the empty-onto-non-empty regression, but the subject is the non-empty branch
    // and the target is the empty one. Both directions must succeed when the target is ahead.
    let (_tmp, graph, repo, mut meta, _description) =
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

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:5:B on 85efbe4 {2}
    └── 📙:5:B

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    // Put non-empty A on top of empty B.
    let but_workspace::branch::move_branch::Outcome {
        rebase, ws_meta, ..
    } = but_workspace::branch::move_branch(
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    rebase.materialize(Default::default())?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
└── ≡📙:3:A on 85efbe4 {2}
    ├── 📙:3:A
    │   └── ·09d8e52 (🏘️)
    └── 📙:5:B

"#]]
    );

    Ok(())
}

fn stack_display_order(ws: &but_graph::Workspace) -> Vec<String> {
    ws.stacks
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

/// Tests for `move_branch` in single-branch (ad-hoc) mode, where `HEAD` is on a plain local branch
/// (no `gitbutler/workspace` commit) and the tip-to-base order of same-commit empty branches lives
/// in the `branch_order` metadata table rather than in `Workspace` metadata.
mod single_branch_mode {
    use std::collections::HashMap;

    use but_core::RefMetadata;
    use but_core::ref_metadata::StackId;
    use but_graph::init::Options;
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
        let mut ws = but_graph::Graph::from_head(repo, meta, project_meta, Options::limited())?
            .into_workspace()?;
        let mut db = but_testsupport::in_memory_db();
        let editor = Editor::create(&mut ws, meta, repo, &mut db)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            ws_meta,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(editor, subject, target)?;
        assert!(
            ws_meta.is_none(),
            "ad-hoc reorder lives in branch_order, not workspace metadata"
        );
        rebase.materialize(Default::default())?;
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
        let mut ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta.clone(), Options::limited())?
                .into_workspace()?;

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
            .into_owned();
        }

        Ok((tmp, repo, meta, project_meta))
    }

    /// Moving a branch on top of the checked-out tip reports it as `new_tip` so the caller can check
    /// it out; the operation itself does not move `HEAD`.
    #[test]
    fn reorder_above_checked_out_tip_returns_new_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta) = ad_hoc_workspace_with_two_empty_branches()?;
        let main_ref = r("refs/heads/main");

        let mut ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        // `main` is the checked-out entrypoint (the projected tip).
        assert_eq!(ws.ref_name(), Some(main_ref));

        // Move empty `empty-bottom` on top of the checked-out `main`, which makes it the new tip.
        let mut db = but_testsupport::in_memory_db();
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(editor, r("refs/heads/empty-bottom"), main_ref)?;
        rebase.materialize(Default::default())?;

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

        let mut ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;

        let mut db = but_testsupport::in_memory_db();
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            r("refs/heads/empty-bottom"),
            r("refs/heads/empty-top"),
        )?;
        rebase.materialize(Default::default())?;

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

        let mut ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta.clone(), Options::limited())?
                .into_workspace()?;
        // Single-branch (ad-hoc) workspace: `HEAD` is on `main` directly, no `gitbutler/workspace`
        // commit. `empty-top`/`empty-bottom` are empty segments; `base` owns the commits.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:[..]:main[🌳] <> ✓! on 281da94
└── ≡:[..]:main[🌳] {1}
    ├── :[..]:main[🌳]
    ├── 📙:[..]:empty-top
    ├── 📙:[..]:empty-bottom
    └── 📙:[..]:base
        ├── ·281da94 (✓)
        ├── ·12995d7 (✓)
        └── ·3d57fc1 (✓)

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
        let mut db = but_testsupport::in_memory_db();
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            ws_meta,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            r("refs/heads/empty-bottom"),
            r("refs/heads/empty-top"),
        )?;
        assert!(
            ws_meta.is_none(),
            "ad-hoc reorder lives in branch_order, not workspace metadata"
        );
        rebase.materialize(Default::default())?;
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
        let ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:[..]:main[🌳] <> ✓! on 281da94
└── ≡:[..]:main[🌳] {1}
    ├── :[..]:main[🌳]
    ├── 📙:[..]:empty-bottom
    ├── 📙:[..]:empty-top
    └── 📙:[..]:base
        ├── ·281da94 (✓)
        ├── ·12995d7 (✓)
        └── ·3d57fc1 (✓)

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

        let mut ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;

        // `base` owns the stack's commits; moving the empty `empty-top` on top of it is still just a
        // metadata reorder and must succeed (previously rejected because the target owns commits).
        let mut db = but_testsupport::in_memory_db();
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            new_tip,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            r("refs/heads/empty-top"),
            r("refs/heads/base"),
        )?;
        rebase.materialize(Default::default())?;

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

        let mut ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:[..]:main[🌳] <> ✓! on 281da94
└── ≡:[..]:main[🌳] {1}
    ├── :[..]:main[🌳]
    ├── 📙:[..]:empty-top
    ├── 📙:[..]:empty-bottom
    └── 📙:[..]:base
        ├── ·281da94 (✓) ►x, ►y
        ├── ·12995d7 (✓)
        └── ·3d57fc1 (✓)

"#]]
        );

        let mut db = but_testsupport::in_memory_db();
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let err = match but_workspace::branch::move_branch(
            editor,
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

        let mut ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        let mut db = but_testsupport::in_memory_db();
        let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
        let but_workspace::branch::move_branch::Outcome {
            rebase,
            branch_stack_order,
            ..
        } = but_workspace::branch::move_branch(
            editor,
            r("refs/heads/empty-bottom"),
            r("refs/heads/empty-top"),
        )?;
        rebase.materialize(Default::default())?;

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
}
