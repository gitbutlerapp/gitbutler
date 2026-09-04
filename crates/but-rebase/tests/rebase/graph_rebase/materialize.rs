//! Tests for `materialize` vs `materialize_without_checkout` behavior differences
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step};
use but_testsupport::{
    StackState, git_status_at_dir, graph_tree, visualize_commit_graph_all,
    visualize_commit_graph_all_from_dir, visualize_disk_tree_skip_dot_git,
};
use snapbox::IntoData;

use crate::{
    graph_rebase::add_stack_with_segments,
    utils::{fixture_writable, standard_options, target_meta},
};

fn worktree_fixture(
    name: &str,
) -> Result<(
    gix::Repository,
    tempfile::TempDir,
    std::mem::ManuallyDrop<but_meta::VirtualBranchesTomlMetadata>,
    but_db::DbHandle,
)> {
    let (repo, tmp) = but_testsupport::writable_scenario_slow(name);
    let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path()
            .join(".git")
            .join("should-never-be-written.toml"),
    )?;
    // Adoption already ran, so the fixture's worktrees count as active.
    let mut db = but_testsupport::project_db(&repo)?;
    db.worktree_meta_mut().mark_adopted()?;
    Ok((repo, tmp, std::mem::ManuallyDrop::new(meta), db))
}

/// Build the graph with all of the fixture's worktrees discovered from `db`.
fn graph_with_worktrees(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    db: &mut but_db::DbHandle,
) -> Result<Graph> {
    let options = but_graph::init::Options {
        worktrees: true,
        ..standard_options()
    };
    Graph::from_head(repo, meta, Default::default(), db, options)
}

fn linked_repo(repo: &gix::Repository, name: &str) -> Result<gix::Repository> {
    repo.worktrees()?
        .into_iter()
        .find(|proxy| proxy.id() == name)
        .with_context(|| format!("missing worktree {name}"))?
        .into_repo()
        .map_err(Into::into)
}

#[test]
fn materialize_removes_dropped_commit_changes_from_worktree() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;
    let worktree = repo.workdir().unwrap();

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(worktree)?.to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── a:100644
├── b:100644
├── base:100644
└── c:100644

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Drop the 'c' commit (HEAD)
    let c = repo.rev_parse_single("HEAD")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.replace(c_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·a96434e (⌂)
    ├── ·d591dfe (⌂)
    └── 🏁·35b8235 (⌂)

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    // After materialize, file 'c' should be GONE from worktree
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(worktree)?.to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── a:100644
├── b:100644
└── base:100644

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe a
* 35b8235 base

"#]]
    );

    Ok(())
}

#[test]
fn materialize_without_checkout_preserves_dropped_commit_changes_in_worktree() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;
    let worktree = repo.workdir().unwrap();

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(worktree)?.to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── a:100644
├── b:100644
├── base:100644
└── c:100644

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Drop the 'c' commit (HEAD)
    let c = repo.rev_parse_single("HEAD")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.replace(c_sel, Step::None)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·a96434e (⌂)
    ├── ·d591dfe (⌂)
    └── 🏁·35b8235 (⌂)

"#]]
    );
    let outcome = outcome.materialize_without_checkout()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    // After materialize_without_checkout, file 'c' should STILL exist in worktree
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(worktree)?.to_string(),
        snapbox::str![[r#"
.
├── .git:40755
├── a:100644
├── b:100644
├── base:100644
└── c:100644

"#]]
    );

    // But the commit graph should still be updated (refs moved)
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe a
* 35b8235 base

"#]]
    );

    Ok(())
}

#[test]
fn both_methods_update_references_identically() -> Result<()> {
    // Test with materialize
    {
        let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;

        let graph = Graph::from_head(
            &repo,
            &*meta,
            Default::default(),
            &mut db,
            standard_options(),
        )?
        .validated()?;
        let mut ws = graph.into_workspace()?;
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

        let c = repo.rev_parse_single("HEAD")?;
        let c_sel = editor.select_commit(c.detach())?;
        editor.replace(c_sel, Step::None)?;

        let outcome = editor.rebase()?;
        let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
        let outcome = outcome.materialize(Default::default())?;
        assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

        snapbox::assert_data_eq!(
            &overlayed,
            snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·a96434e (⌂)
    ├── ·d591dfe (⌂)
    └── 🏁·35b8235 (⌂)

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe a
* 35b8235 base

"#]]
        );
    }

    // Test with materialize_without_checkout
    {
        let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;

        let graph = Graph::from_head(
            &repo,
            &*meta,
            Default::default(),
            &mut db,
            standard_options(),
        )?
        .validated()?;
        let mut ws = graph.into_workspace()?;
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

        let c = repo.rev_parse_single("HEAD")?;
        let c_sel = editor.select_commit(c.detach())?;
        editor.replace(c_sel, Step::None)?;

        let outcome = editor.rebase()?;
        let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
        let outcome = outcome.materialize_without_checkout()?;
        assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

        snapbox::assert_data_eq!(
            &overlayed,
            snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·a96434e (⌂)
    ├── ·d591dfe (⌂)
    └── 🏁·35b8235 (⌂)

"#]]
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe a
* 35b8235 base

"#]]
        );
    }

    Ok(())
}

#[test]
fn materialize_repoints_head_when_checkout_reference_is_replaced() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let main_selector = editor.select_reference("refs/heads/main".try_into()?)?;
    editor.replace(
        main_selector,
        Step::new_reference("refs/heads/replacement".try_into()?),
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:replacement[🌳]
    ├── ·120e3a9 (⌂)
    ├── ·a96434e (⌂)
    ├── ·d591dfe (⌂)
    └── 🏁·35b8235 (⌂)

"#]]
    );
    // The overlay is only a preview: HEAD is still on `main`.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());
    // HEAD stays attached to the checkout reference under its new name.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> replacement) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );

    Ok(())
}

#[test]
fn materialize_without_checkout_does_not_repoint_head_when_checkout_reference_is_replaced()
-> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let main_selector = editor.select_reference("refs/heads/main".try_into()?)?;
    editor.replace(
        main_selector,
        Step::new_reference("refs/heads/replacement".try_into()?),
    )?;

    let outcome = editor.rebase()?;
    outcome.materialize_without_checkout()?;

    // The reference edits still apply, but nothing points HEAD at the replacement.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (replacement) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );
    // HEAD still names the deleted `main`.
    snapbox::assert_data_eq!(
        format!("{:?}", repo.head_name()?),
        snapbox::str![[r#"Some(FullName("refs/heads/main"))"#]]
    );

    Ok(())
}

#[test]
fn materialize_keeps_immutable_refs_unchanged_while_updating_local_refs() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("workspace-with-empty-stack")?;
    add_stack_with_segments(&mut meta, 1, "stack-1", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-2", StackState::InWorkspace, &[]);
    let main_before = repo.rev_parse_single("main")?.detach();

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let stack_tip = repo.rev_parse_single("stack-2")?.detach();
    let stack_tip_sel = editor.select_commit(stack_tip)?;
    editor.replace(stack_tip_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    assert_eq!(repo.rev_parse_single("main")?, main_before);

    Ok(())
}

#[test]
fn materialize_does_not_delete_immutable_refs_removed_from_graph() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("workspace-with-empty-stack")?;
    add_stack_with_segments(&mut meta, 1, "stack-1", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "stack-2", StackState::InWorkspace, &[]);
    let main_ref = gix::refs::FullName::try_from("refs/heads/main")?;
    let main_before = repo.rev_parse_single("main")?.detach();

    let graph =
        Graph::from_head(&repo, &*meta, target_meta(), &mut db, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let main_sel = editor.select_reference(main_ref.as_ref())?;
    editor.replace(main_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize(Default::default())?;

    assert_eq!(repo.rev_parse_single("main")?, main_before);

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn visible_attached_and_detached_worktrees_follow_a_rewritten_commit() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = worktree_fixture("worktree-checkout-heads")?;
    let attached_dir = repo.workdir().unwrap().join("wt");
    let detached_dir = repo.workdir().unwrap().join("wt-detached");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe (middle) a
* 35b8235 base

"#]]
    );
    let graph = graph_with_worktrees(&repo, &*meta, &mut db)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let mut replacement = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    let a = repo.rev_parse_single("middle:a")?.detach();
    let mut tree = repo.edit_tree(replacement.tree)?;
    tree.remove("a")?;
    tree.upsert("a-renamed", gix::objs::tree::EntryKind::Blob, a)?;
    replacement.tree = tree.write()?.detach();
    replacement.message = "a rewritten".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    let middle_selector = editor.select_commit(repo.rev_parse_single("middle")?.detach())?;
    editor.replace(middle_selector, Step::new_pick(replacement))?;
    editor.rebase()?.materialize(Default::default())?;

    // The branch-backed worktree stays attached to the rewritten `middle`.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all_from_dir(&attached_dir)?,
        snapbox::str![[r#"
* 5f2f07e (main) b
* aa3594b (HEAD -> middle) a rewritten
* 35b8235 base

"#]]
    );
    // Its checkout carries the rename, and index and files match the rewritten branch.
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&attached_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
├── a-renamed:100644
└── base:100644

"#]]
    );
    snapbox::assert_data_eq!(git_status_at_dir(&attached_dir)?, snapbox::str![""]);

    // The detached worktree's HEAD moves to the rewritten commit and stays detached.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all_from_dir(&detached_dir)?,
        snapbox::str![[r#"
* 5f2f07e (main) b
* aa3594b (HEAD, middle) a rewritten
* 35b8235 base

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&detached_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
├── a-renamed:100644
└── base:100644

"#]]
    );
    snapbox::assert_data_eq!(git_status_at_dir(&detached_dir)?, snapbox::str![""]);
    Ok(())
}

#[test]
fn references_checked_out_in_linked_worktrees_are_not_deleted() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = worktree_fixture("worktree-checkout-heads")?;
    repo.reference(
        "refs/heads/doomed",
        repo.rev_parse_single("middle")?.detach(),
        gix::refs::transaction::PreviousValue::MustNotExist,
        "test setup",
    )?;
    // `doomed` only differs from `middle` in not being checked out anywhere.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe (middle, doomed) a
* 35b8235 base

"#]]
    );

    // Without worktree discovery the editor cannot see `middle`'s checkout.
    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    for refname in ["refs/heads/middle", "refs/heads/doomed"] {
        let selector = editor.select_reference(refname.try_into()?)?;
        editor.replace(selector, Step::None)?;
    }
    editor.rebase()?.materialize(Default::default())?;

    // Only the branch no worktree has checked out is deleted.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe (middle) a
* 35b8235 base

"#]]
    );
    Ok(())
}

#[test]
fn changes_consumed_from_a_linked_worktree_cancel_during_its_checkout() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = worktree_fixture("worktree-partial-amend")?;
    let worktree_dir = repo.workdir().unwrap().join("wt");
    let middle = repo.rev_parse_single("middle")?.detach();

    // Stand in for `commit_amend_from_worktree`: bake the worktree's first hunk into
    // its branch and hand the checkout the matching additive merge base.
    let mut amended = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    let blob = repo
        .write_blob("line 1\nline 1.1\nline 2\nline 3\n")?
        .detach();
    let mut tree = repo.edit_tree(amended.tree)?;
    tree.upsert("test.txt", gix::objs::tree::EntryKind::Blob, blob)?;
    let consumed_tree = tree.write()?.detach();
    amended.tree = consumed_tree;
    amended.message = "base, with line 1.1".into();
    let amended = repo.write_object(amended.inner)?.detach();
    let graph = graph_with_worktrees(&repo, &*meta, &mut db)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let middle_selector = editor.select_commit(middle)?;
    editor.replace(middle_selector, Step::new_pick(amended))?;
    editor.set_worktree_merge_base_override(gix::bstr::BStr::new("wt"), consumed_tree)?;
    editor.rebase()?.materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 60e1c0f (HEAD -> main) main
* b257c51 (middle) base, with line 1.1

"#]]
    );
    // Only the hunk that wasn't consumed is left in the worktree - without the
    // merge-base override the consumed one would be duplicated.
    snapbox::assert_data_eq!(
        std::fs::read_to_string(worktree_dir.join("test.txt"))?,
        snapbox::str![[r#"
line 1
line 1.1
line 1.2
line 2
line 3

"#]]
    );
    snapbox::assert_data_eq!(
        git_status_at_dir(&worktree_dir)?,
        snapbox::str![[r#"
 M test.txt

"#]]
    );
    Ok(())
}

#[test]
fn a_merge_base_override_for_an_unknown_worktree_is_rejected() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = worktree_fixture("worktree-partial-amend")?;
    let graph = graph_with_worktrees(&repo, &*meta, &mut db)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let tree = repo.rev_parse_single("middle^{tree}")?.detach();

    let err = editor
        .set_worktree_merge_base_override(gix::bstr::BStr::new("nope"), tree)
        .expect_err("callers must be able to bail before mutating the step graph");
    snapbox::assert_data_eq!(
        format!("{err:#}"),
        snapbox::str!["Worktree nope has no checkout recorded in the editor"]
    );
    Ok(())
}

#[test]
fn materialize_without_checkout_moves_detached_worktree_heads_only() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = worktree_fixture("worktree-checkout-heads")?;
    let detached_dir = repo.workdir().unwrap().join("wt-detached");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all_from_dir(&detached_dir)?,
        snapbox::str![[r#"
* a96434e (main) b
* d591dfe (HEAD, middle) a
* 35b8235 base

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&detached_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
├── a:100644
└── base:100644

"#]]
    );
    let graph = graph_with_worktrees(&repo, &*meta, &mut db)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let mut replacement = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    let a = repo.rev_parse_single("middle:a")?.detach();
    let mut tree = repo.edit_tree(replacement.tree)?;
    tree.remove("a")?;
    tree.upsert("a-renamed", gix::objs::tree::EntryKind::Blob, a)?;
    replacement.tree = tree.write()?.detach();
    replacement.message = "a rewritten".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    let selector = editor.select_commit(repo.rev_parse_single("middle")?.detach())?;
    editor.replace(selector, Step::new_pick(replacement))?;
    editor.rebase()?.materialize_without_checkout()?;

    // The detached HEAD follows the rewrite through the ref transaction and stays detached.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all_from_dir(&detached_dir)?,
        snapbox::str![[r#"
* 5f2f07e (main) b
* aa3594b (HEAD, middle) a rewritten
* 35b8235 base

"#]]
    );
    // Its checkout is left as it was; the rename shows only as a staged change against HEAD.
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&detached_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
├── a:100644
└── base:100644

"#]]
    );
    snapbox::assert_data_eq!(
        git_status_at_dir(&detached_dir)?,
        snapbox::str![[r#"
R  a-renamed -> a

"#]]
    );
    Ok(())
}

#[test]
fn a_detached_worktree_that_moved_since_editor_creation_is_rejected() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = worktree_fixture("worktree-checkout-heads")?;
    let detached_dir = repo.workdir().unwrap().join("wt-detached");
    let graph = graph_with_worktrees(&repo, &*meta, &mut db)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let mut replacement = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    replacement.message = "a rewritten".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    let selector = editor.select_commit(repo.rev_parse_single("middle")?.detach())?;
    editor.replace(selector, Step::new_pick(replacement))?;
    let outcome = editor.rebase()?;

    // Someone checks the detached worktree out somewhere else in the meantime.
    let detached = linked_repo(&repo, "wt-detached")?;
    let elsewhere = repo.rev_parse_single("main")?.detach();
    but_core::worktree::safe_checkout_from_head(elsewhere, &detached, Default::default())?;

    let err = outcome
        .materialize_without_checkout()
        .expect_err("the transaction must not move a HEAD it never looked at");
    snapbox::assert_data_eq!(
        format!("{err:#}"),
        snapbox::str![[
            r#"The reference "worktrees/wt-detached/HEAD" should have content d591dfed1777b8f00f5b7b6f427537eeb5878178, actual content was a96434e2505c2ea0896cf4f58fec0778e074d3da"#
        ]]
    );
    // The worktree keeps what someone else put there.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all_from_dir(&detached_dir)?,
        snapbox::str![[r#"
* a96434e (HEAD, main) b
* d591dfe (middle) a
* 35b8235 base

"#]]
    );
    Ok(())
}

/// Inserting below a worktree-checked-out branch moves only that branch:
/// the lane its commit belongs to is not rebased, because the branch is
/// represented as a fork directly onto the commit it points at.
#[test]
fn insert_below_worktree_ref_moves_only_that_ref() -> Result<()> {
    use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};

    let (repo, _tmpdir, mut meta, mut db) = worktree_fixture("worktree-checkout-heads")?;
    let attached_dir = repo.workdir().unwrap().join("wt");
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a96434e (HEAD -> main) b
* d591dfe (middle) a
* 35b8235 base

"#]]
    );

    let graph = graph_with_worktrees(&repo, &*meta, &mut db)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // A commit to insert, with the same tree the worktree already has.
    let mut template = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    template.message = "only for the worktree branch".into();
    template.parents = vec![].into();
    let new_commit = repo.write_object(template.inner)?.detach();

    editor.insert(
        RelativeTo::Reference("refs/heads/middle".try_into()?),
        Step::new_pick(new_commit),
        InsertSide::Below,
    )?;
    editor.rebase()?.materialize(Default::default())?;

    // `main` is untouched; the inserted commit sits directly on the commit `middle` pointed at.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a96434e (HEAD -> main) b
| * 3c608ef (middle) only for the worktree branch
|/  
* d591dfe a
* 35b8235 base

"#]]
        .raw()
    );

    // The linked checkout stays attached to `middle`, followed it, and is clean.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all_from_dir(&attached_dir)?,
        snapbox::str![[r#"
* a96434e (main) b
| * 3c608ef (HEAD -> middle) only for the worktree branch
|/  
* d591dfe a
* 35b8235 base

"#]]
    );
    snapbox::assert_data_eq!(git_status_at_dir(&attached_dir)?, snapbox::str![""]);
    Ok(())
}
