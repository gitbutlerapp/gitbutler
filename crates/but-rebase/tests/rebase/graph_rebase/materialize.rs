//! Tests for `materialize` vs `materialize_without_checkout` behavior differences
use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step};
use but_testsupport::{graph_tree, visualize_commit_graph_all, visualize_disk_tree_skip_dot_git};

use crate::utils::{fixture_writable, standard_options};

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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
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
        └── ·35b8235 (⌂|1)
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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
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
        └── ·35b8235 (⌂|1)
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

        let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
        let mut ws = graph.into_workspace()?;
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

        let c = repo.rev_parse_single("HEAD")?;
        let c_sel = editor.select_commit(c.detach())?;
        editor.replace(c_sel, Step::None)?;

        let outcome = editor.rebase()?;
        let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
        let outcome = outcome.materialize()?;
        assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

        (repo.rev_parse_single("main")?.detach().to_string(), overlayed)
    };

    // Test with materialize_without_checkout
    let (ref_after_materialize_without_checkout, overlayed_without_checkout) = {
        let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

        let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
        let mut ws = graph.into_workspace()?;
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

        let c = repo.rev_parse_single("HEAD")?;
        let c_sel = editor.select_commit(c.detach())?;
        editor.replace(c_sel, Step::None)?;

        let outcome = editor.rebase()?;
        let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
        let outcome = outcome.materialize_without_checkout()?;
        assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

        (repo.rev_parse_single("main")?.detach().to_string(), overlayed)
    };

    insta::assert_snapshot!(overlayed_materialize, @"

    └── 👉►:0[0]:main[🌳]
        ├── ·a96434e (⌂|1)
        ├── ·d591dfe (⌂|1)
        └── ·35b8235 (⌂|1)
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
