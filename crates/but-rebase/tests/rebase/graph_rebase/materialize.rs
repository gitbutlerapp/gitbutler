//! Tests for `materialize` vs `materialize_without_checkout` behavior differences
use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step};
use but_testsupport::{
    StackState, graph_tree, visualize_commit_graph_all, visualize_disk_tree_skip_dot_git,
};

use crate::{
    graph_rebase::add_stack_with_segments,
    utils::{fixture_writable, standard_options},
};

fn project_meta(meta: &impl but_core::RefMetadata) -> but_core::ref_metadata::ProjectMeta {
    meta.workspace(
        but_core::WORKSPACE_REF_NAME
            .try_into()
            .expect("valid workspace ref"),
    )
    .map(|workspace| workspace.project_meta())
    .unwrap_or_default()
}

#[test]
fn materialize_removes_dropped_commit_changes_from_worktree() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
    let worktree = repo.workdir().unwrap();

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(worktree)?, @"
    .
    ├── .git:40755
    ├── a:100644
    ├── b:100644
    ├── base:100644
    └── c:100644
    ");

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Drop the 'c' commit (HEAD)
    let c = repo.rev_parse_single("HEAD")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.replace(c_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1)
        └── 🏁·35b8235 (⌂|1)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    // After materialize, file 'c' should be GONE from worktree
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(worktree)?, @"
    .
    ├── .git:40755
    ├── a:100644
    ├── b:100644
    └── base:100644
    ");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a96434e (HEAD -> main) b
    * d591dfe a
    * 35b8235 base
    ");

    Ok(())
}

#[test]
fn materialize_without_checkout_preserves_dropped_commit_changes_in_worktree() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
    let worktree = repo.workdir().unwrap();

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(worktree)?, @"
    .
    ├── .git:40755
    ├── a:100644
    ├── b:100644
    ├── base:100644
    └── c:100644
    ");

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Drop the 'c' commit (HEAD)
    let c = repo.rev_parse_single("HEAD")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.replace(c_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1)
        └── 🏁·35b8235 (⌂|1)
    ");
    let outcome = outcome.materialize_without_checkout()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    // After materialize_without_checkout, file 'c' should STILL exist in worktree
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(worktree)?, @"
    .
    ├── .git:40755
    ├── a:100644
    ├── b:100644
    ├── base:100644
    └── c:100644
    ");

    // But the commit graph should still be updated (refs moved)
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a96434e (HEAD -> main) b
    * d591dfe a
    * 35b8235 base
    ");

    Ok(())
}

#[test]
fn both_methods_update_references_identically() -> Result<()> {
    // Test with materialize
    let (ref_after_materialize, overlayed_materialize) = {
        let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

        let graph = Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let mut ws = graph.into_workspace()?;
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

        let c = repo.rev_parse_single("HEAD")?;
        let c_sel = editor.select_commit(c.detach())?;
        editor.replace(c_sel, Step::None)?;

        let outcome = editor.rebase()?;
        let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
        let outcome = outcome.materialize()?;
        assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

        (
            repo.rev_parse_single("main")?.detach().to_string(),
            overlayed,
        )
    };

    // Test with materialize_without_checkout
    let (ref_after_materialize_without_checkout, overlayed_without_checkout) = {
        let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

        let graph = Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?
            .validated()?;
        let mut ws = graph.into_workspace()?;
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

        let c = repo.rev_parse_single("HEAD")?;
        let c_sel = editor.select_commit(c.detach())?;
        editor.replace(c_sel, Step::None)?;

        let outcome = editor.rebase()?;
        let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
        let outcome = outcome.materialize_without_checkout()?;
        assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

        (
            repo.rev_parse_single("main")?.detach().to_string(),
            overlayed,
        )
    };

    insta::assert_snapshot!(overlayed_materialize, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1)
        └── 🏁·35b8235 (⌂|1)
    ");
    assert_eq!(overlayed_materialize, overlayed_without_checkout);

    // Both should update 'main' to the same commit
    assert_eq!(
        ref_after_materialize, ref_after_materialize_without_checkout,
        "Both methods should update references identically"
    );

    insta::assert_snapshot!(ref_after_materialize, @"a96434e2505c2ea0896cf4f58fec0778e074d3da");

    Ok(())
}

#[test]
fn materialize_repoints_head_when_checkout_reference_is_replaced() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
    let replacement_ref = gix::refs::FullName::try_from("refs/heads/replacement")?;
    let head_before = repo.rev_parse_single("HEAD")?.detach();

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let main_selector = editor.select_reference("refs/heads/main".try_into()?)?;
    editor.replace(main_selector, Step::new_reference(replacement_ref.clone()))?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:replacement[🌳]
        ├── ·120e3a9 (⌂|1)
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1)
        └── 🏁·35b8235 (⌂|1)
    ");
    assert_eq!(
        repo.head_name()?,
        Some(gix::refs::FullName::try_from("refs/heads/main")?),
        "overlay preview should not repoint HEAD before materialization"
    );

    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());
    assert_eq!(
        repo.head_name()?,
        Some(replacement_ref.clone()),
        "materialize should keep HEAD attached to the replacement checkout reference"
    );
    assert_eq!(
        repo.find_reference(replacement_ref.as_ref())?.id(),
        head_before,
        "replacement branch should point at the previous checkout commit"
    );
    assert!(
        repo.try_find_reference("refs/heads/main")?.is_none(),
        "replaced checkout branch should be deleted"
    );

    Ok(())
}

#[test]
fn materialize_without_checkout_does_not_repoint_head_when_checkout_reference_is_replaced()
-> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
    let replacement_ref = gix::refs::FullName::try_from("refs/heads/replacement")?;

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let main_selector = editor.select_reference("refs/heads/main".try_into()?)?;
    editor.replace(main_selector, Step::new_reference(replacement_ref.clone()))?;

    let outcome = editor.rebase()?;
    outcome.materialize_without_checkout()?;

    assert_eq!(
        repo.head_name()?,
        Some(gix::refs::FullName::try_from("refs/heads/main")?),
        "materialize_without_checkout should leave the symbolic HEAD target untouched"
    );
    assert!(
        repo.try_find_reference(replacement_ref.as_ref())?.is_some(),
        "reference edits should still create the replacement branch"
    );
    assert!(
        repo.try_find_reference("refs/heads/main")?.is_none(),
        "reference edits should still delete the replaced branch"
    );

    Ok(())
}

#[test]
fn materialize_keeps_immutable_refs_unchanged_while_updating_local_refs() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-with-empty-stack")?;
    add_stack_with_segments(&mut meta, 1, "stack-1", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-2", StackState::InWorkspace, &[]);
    let main_before = repo.rev_parse_single("main")?.detach();

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   74bcc92 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | 2169646 (stack-1) Commit D
    * | 46ef828 Commit C
    |/  
    | * a0f2ac5 (origin/main, main) Commit X
    |/  
    * f555940 (stack-2) Commit A
    * d664be0 Commit B
    * fafd9d0 init
    ");

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let stack_tip = repo.rev_parse_single("stack-2")?.detach();
    let stack_tip_sel = editor.select_commit(stack_tip)?;
    editor.replace(stack_tip_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   3cc8b6f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | c869f24 (stack-1) Commit D
    * | 07a9b49 Commit C
    |/  
    | * a0f2ac5 (origin/main, main) Commit X
    | * f555940 Commit A
    |/  
    * d664be0 (stack-2) Commit B
    * fafd9d0 init
    ");

    assert_eq!(repo.rev_parse_single("main")?.detach(), main_before);

    Ok(())
}

#[test]
fn materialize_does_not_delete_immutable_refs_removed_from_graph() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("workspace-with-empty-stack")?;
    add_stack_with_segments(&mut meta, 1, "stack-1", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-2", StackState::InWorkspace, &[]);
    let main_ref = gix::refs::FullName::try_from("refs/heads/main")?;
    let main_before = repo.rev_parse_single("main")?.detach();

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let main_sel = editor.select_reference(main_ref.as_ref())?;
    editor.replace(main_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    assert_eq!(repo.rev_parse_single("main")?.detach(), main_before);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   74bcc92 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | 2169646 (stack-1) Commit D
    * | 46ef828 Commit C
    |/  
    | * a0f2ac5 (origin/main, main) Commit X
    |/  
    * f555940 (stack-2) Commit A
    * d664be0 Commit B
    * fafd9d0 init
    ");

    Ok(())
}
