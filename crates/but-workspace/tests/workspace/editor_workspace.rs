use but_core::RefMetadata;
use but_graph::init::Options;
use but_testsupport::{CommandExt as _, graph_workspace, visualize_commit_graph_all};

use crate::ref_info::with_workspace_commit::utils::{named_writable_scenario, project_meta};

fn switch_to_workspace(
    repo: &gix::Repository,
    meta: &mut impl RefMetadata,
    db: &mut but_db::DbHandle,
) -> anyhow::Result<but_graph::Workspace> {
    let graph =
        but_graph::Graph::from_head(repo, &*meta, project_meta(repo)?, db, Options::limited())?;
    let mut ws = graph.into_workspace()?;
    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, meta, repo, db)?;
    but_workspace::editor_workspace::switch_to_workspace(editor)?
        .0
        .materialize(Default::default())?;
    Ok(ws)
}

#[test]
fn switch_from_plain_branch() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) =
        named_writable_scenario("no-ws-ref-no-ws-commit-two-branches")?;

    let ws = switch_to_workspace(&repo, &mut meta, &mut db)?;

    // A workspace commit now sits on main, checked out through the workspace reference.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 700abf0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* e5d0542 (origin/main, main, B, A) A

"#]]
    );
    // A and B merely rest on the target with nothing ordering them, so no stack holds them.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on e5d0542

"#]]
    );
    Ok(())
}

#[test]
fn switch_from_branch_replaces_existing_workspace_reference() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    but_testsupport::git(&repo).args(["checkout", "A"]).run();

    let ws = switch_to_workspace(&repo, &mut meta, &mut db)?;

    // The workspace reference left B's workspace commit for a new one on A.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d69fe94 (B) B
| * f772ad8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 85efbe4
└── ≡📙:A on 85efbe4 {[..]}
    └── 📙:A
        └── ·09d8e52 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn switch_from_removed_head_reference() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
    but_testsupport::git(&repo).args(["checkout", "A"]).run();
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&repo)?,
        &mut db,
        Options::limited(),
    )?;
    let mut ws = graph.into_workspace()?;
    let mut editor = but_rebase::graph_rebase::Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let head = editor.head().expect("HEAD is on A");
    editor.replace_with_none(head)?;

    but_workspace::editor_workspace::switch_to_workspace(editor)?
        .0
        .materialize(Default::default())?;

    // A is gone, and the workspace commit sits on the commit A pointed to.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d69fe94 (B) B
| * f772ad8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* 09d8e52 A
* 85efbe4 (origin/main, main) M

"#]]
    );
    Ok(())
}

#[test]
fn switch_on_workspace_reference_fails() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;

    let err = switch_to_workspace(&repo, &mut meta, &mut db).unwrap_err();
    assert_eq!(
        err.to_string(),
        "HEAD is already on the workspace reference",
        "a workspace can only be switched to from outside of it"
    );
    Ok(())
}
