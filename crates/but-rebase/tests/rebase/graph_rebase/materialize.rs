//! Tests for `materialize` vs `materialize_without_checkout` behavior differences
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step};
use but_testsupport::{
    StackState, git_status, graph_tree, visualize_commit_graph_all,
    visualize_disk_tree_skip_dot_git,
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
)> {
    let (repo, tmp) = but_testsupport::writable_scenario_slow(name);
    let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path()
            .join(".git")
            .join("should-never-be-written.toml"),
    )?;
    Ok((repo, tmp, std::mem::ManuallyDrop::new(meta)))
}

fn worktree_tip(repo: &gix::Repository, name: &str) -> Result<but_graph::init::WorktreeTip> {
    let proxy = repo
        .worktrees()?
        .into_iter()
        .find(|proxy| proxy.id() == name)
        .with_context(|| format!("missing worktree {name}"))?;
    let name = proxy.id().to_owned();
    let worktree_repo = proxy.into_repo()?;
    let mut head = worktree_repo.head()?;
    let ref_name = head.referent_name().map(ToOwned::to_owned);
    let id = head.peel_to_commit()?.id;
    Ok(but_graph::init::WorktreeTip { name, ref_name, id })
}

fn options_with_worktrees(
    repo: &gix::Repository,
    names: &[&str],
) -> Result<but_graph::init::Options> {
    let mut options = standard_options();
    options.worktree_tips = names
        .iter()
        .map(|name| worktree_tip(repo, name))
        .collect::<Result<Vec<_>>>()?;
    Ok(options)
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
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
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
    ├── ·a96434e (⌂|1)
    ├── ·d591dfe (⌂|1)
    └── 🏁·35b8235 (⌂|1)

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
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
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
    ├── ·a96434e (⌂|1)
    ├── ·d591dfe (⌂|1)
    └── 🏁·35b8235 (⌂|1)

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
    let (ref_after_materialize, overlayed_materialize) = {
        let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

        let graph =
            Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
        let mut ws = graph.into_workspace()?;
        let mut db = but_testsupport::in_memory_db();
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

        let c = repo.rev_parse_single("HEAD")?;
        let c_sel = editor.select_commit(c.detach())?;
        editor.replace(c_sel, Step::None)?;

        let outcome = editor.rebase()?;
        let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
        let outcome = outcome.materialize(Default::default())?;
        assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

        (
            repo.rev_parse_single("main")?.detach().to_string(),
            overlayed,
        )
    };

    // Test with materialize_without_checkout
    let (ref_after_materialize_without_checkout, overlayed_without_checkout) = {
        let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

        let graph =
            Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
        let mut ws = graph.into_workspace()?;
        let mut db = but_testsupport::in_memory_db();
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

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

    snapbox::assert_data_eq!(
        &overlayed_materialize,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·a96434e (⌂|1)
    ├── ·d591dfe (⌂|1)
    └── 🏁·35b8235 (⌂|1)

"#]]
    );
    assert_eq!(overlayed_materialize, overlayed_without_checkout);

    // Both should update 'main' to the same commit
    assert_eq!(
        ref_after_materialize, ref_after_materialize_without_checkout,
        "Both methods should update references identically"
    );

    snapbox::assert_data_eq!(
        ref_after_materialize,
        snapbox::str!["a96434e2505c2ea0896cf4f58fec0778e074d3da"]
    );

    Ok(())
}

#[test]
fn materialize_repoints_head_when_checkout_reference_is_replaced() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;
    let replacement_ref = gix::refs::FullName::try_from("refs/heads/replacement")?;
    let head_before = repo.rev_parse_single("HEAD")?.detach();

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let main_selector = editor.select_reference("refs/heads/main".try_into()?)?;
    editor.replace(main_selector, Step::new_reference(replacement_ref.clone()))?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:replacement[🌳]
    ├── ·120e3a9 (⌂|1)
    ├── ·a96434e (⌂|1)
    ├── ·d591dfe (⌂|1)
    └── 🏁·35b8235 (⌂|1)

"#]]
    );
    assert_eq!(
        repo.head_name()?,
        Some(gix::refs::FullName::try_from("refs/heads/main")?),
        "overlay preview should not repoint HEAD before materialization"
    );

    let outcome = outcome.materialize(Default::default())?;
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
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

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

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
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

    let graph = Graph::from_head(&repo, &*meta, target_meta(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let main_sel = editor.select_reference(main_ref.as_ref())?;
    editor.replace(main_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize(Default::default())?;

    assert_eq!(repo.rev_parse_single("main")?.detach(), main_before);

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
    let (repo, _tmpdir, mut meta) = worktree_fixture("worktree-checkout-heads")?;
    let old_middle = repo.rev_parse_single("middle")?.detach();
    let attached_dir = repo.workdir().unwrap().join("wt");
    let detached_dir = repo.workdir().unwrap().join("wt-detached");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        options_with_worktrees(&repo, &["wt", "wt-detached"])?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let mut replacement = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    let a = repo.rev_parse_single("middle:a")?.detach();
    let mut tree = repo.edit_tree(replacement.tree)?;
    tree.remove("a")?;
    tree.upsert("a-renamed", gix::objs::tree::EntryKind::Blob, a)?;
    replacement.tree = tree.write()?.detach();
    replacement.message = "a rewritten".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    let old_middle_selector = editor.select_commit(old_middle)?;
    editor.replace(old_middle_selector, Step::new_pick(replacement))?;
    editor.rebase()?.materialize(Default::default())?;

    let new_middle = repo.rev_parse_single("middle")?.detach();
    assert_ne!(new_middle, old_middle);

    let attached = gix::open(&attached_dir)?;
    assert_eq!(
        std::fs::read_to_string(attached.git_dir().join("HEAD"))?,
        "ref: refs/heads/middle\n"
    );
    assert_eq!(
        attached.head_name()?,
        Some("refs/heads/middle".try_into()?),
        "the branch-backed worktree stays attached"
    );
    assert_eq!(attached.head_id()?.detach(), new_middle);
    assert!(
        !attached_dir.join("a").exists(),
        "the attached worktree removes the rename source"
    );
    assert_eq!(
        std::fs::read_to_string(attached_dir.join("a-renamed"))?,
        "a\n",
        "the attached worktree writes the rename target"
    );
    // The attached worktree's index and files match its rewritten branch.
    snapbox::assert_data_eq!(git_status(&attached)?, snapbox::str![""]);

    let detached = gix::open(&detached_dir)?;
    assert_eq!(
        detached.head_name()?,
        None,
        "the detached worktree stays detached"
    );
    assert_eq!(detached.head_id()?.detach(), new_middle);
    assert_eq!(
        std::fs::read_to_string(detached.git_dir().join("HEAD"))?,
        format!("{new_middle}\n")
    );
    assert!(
        !detached_dir.join("a").exists(),
        "the detached worktree removes the rename source"
    );
    assert_eq!(
        std::fs::read_to_string(detached_dir.join("a-renamed"))?,
        "a\n",
        "the detached worktree writes the rename target"
    );
    // The detached worktree's index and files match its rewritten HEAD.
    snapbox::assert_data_eq!(git_status(&detached)?, snapbox::str![""]);
    Ok(())
}

#[test]
fn references_checked_out_in_linked_worktrees_are_not_deleted() -> Result<()> {
    let (repo, _tmpdir, mut meta) = worktree_fixture("worktree-checkout-heads")?;
    let middle = repo.rev_parse_single("middle")?.detach();
    repo.reference(
        "refs/heads/doomed",
        middle,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "test setup",
    )?;

    let graph =
        Graph::from_head(&repo, &*meta, Default::default(), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    for refname in ["refs/heads/middle", "refs/heads/doomed"] {
        let selector = editor.select_reference(refname.try_into()?)?;
        editor.replace(selector, Step::None)?;
    }
    editor.rebase()?.materialize(Default::default())?;

    assert!(
        repo.try_find_reference("middle")?.is_some(),
        "a checked-out branch must not be deleted, even when the worktree is not visible"
    );
    assert!(
        repo.try_find_reference("doomed")?.is_none(),
        "an otherwise-identical unchecked-out branch is deleted"
    );
    Ok(())
}

#[test]
fn changes_consumed_from_a_linked_worktree_cancel_during_its_checkout() -> Result<()> {
    let (repo, _tmpdir, mut meta) = worktree_fixture("worktree-partial-amend")?;
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

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        options_with_worktrees(&repo, &["wt"])?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let middle_selector = editor.select_commit(middle)?;
    editor.replace(middle_selector, Step::new_pick(amended))?;
    editor.set_worktree_merge_base_override(gix::bstr::BStr::new("wt"), consumed_tree)?;
    editor.rebase()?.materialize(Default::default())?;

    assert_eq!(repo.rev_parse_single("middle")?.detach(), amended);
    assert_eq!(
        std::fs::read_to_string(worktree_dir.join("test.txt"))?,
        "line 1\nline 1.1\nline 1.2\nline 2\nline 3\n",
        "only the hunk that wasn't consumed is left in the worktree - \
         without the merge-base override the consumed one is duplicated"
    );
    snapbox::assert_data_eq!(
        git_status(&linked_repo(&repo, "wt")?)?,
        snapbox::str![[r#"
 M test.txt

"#]]
    );
    Ok(())
}

#[test]
fn a_merge_base_override_for_an_unknown_worktree_is_rejected() -> Result<()> {
    let (repo, _tmpdir, mut meta) = worktree_fixture("worktree-partial-amend")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        options_with_worktrees(&repo, &["wt"])?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let tree = repo.rev_parse_single("middle^{tree}")?.detach();

    let err = editor
        .set_worktree_merge_base_override(gix::bstr::BStr::new("nope"), tree)
        .expect_err("callers must be able to bail before mutating the step graph");
    assert!(
        format!("{err:#}").contains("no checkout recorded"),
        "{err:#}"
    );
    Ok(())
}

#[test]
fn materialize_without_checkout_moves_detached_worktree_heads_only() -> Result<()> {
    let (repo, _tmpdir, mut meta) = worktree_fixture("worktree-checkout-heads")?;
    let detached_dir = repo.workdir().unwrap().join("wt-detached");
    let old_middle = repo.rev_parse_single("middle")?.detach();
    let files_before = visualize_disk_tree_skip_dot_git(&detached_dir)?.to_string();

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        options_with_worktrees(&repo, &["wt", "wt-detached"])?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let mut replacement = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    replacement.message = "a rewritten".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    let selector = editor.select_commit(old_middle)?;
    editor.replace(selector, Step::new_pick(replacement))?;
    editor.rebase()?.materialize_without_checkout()?;

    let new_middle = repo.rev_parse_single("middle")?.detach();
    assert_ne!(new_middle, old_middle);

    let detached = linked_repo(&repo, "wt-detached")?;
    assert_eq!(
        detached.head_id()?.detach(),
        new_middle,
        "the detached worktree's HEAD follows the rewrite through the ref transaction"
    );
    assert_eq!(detached.head_name()?, None, "and stays detached");
    assert_eq!(
        visualize_disk_tree_skip_dot_git(&detached_dir)?.to_string(),
        files_before,
        "while its checkout is left exactly as it was"
    );
    Ok(())
}

#[test]
fn a_detached_worktree_that_moved_since_editor_creation_is_rejected() -> Result<()> {
    let (repo, _tmpdir, mut meta) = worktree_fixture("worktree-checkout-heads")?;
    let old_middle = repo.rev_parse_single("middle")?.detach();

    let graph = Graph::from_head(
        &repo,
        &*meta,
        Default::default(),
        options_with_worktrees(&repo, &["wt", "wt-detached"])?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut db = but_testsupport::in_memory_db();
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let mut replacement = but_core::Commit::from_id(repo.rev_parse_single("middle")?)?;
    replacement.message = "a rewritten".into();
    let replacement = repo.write_object(replacement.inner)?.detach();
    let selector = editor.select_commit(old_middle)?;
    editor.replace(selector, Step::new_pick(replacement))?;
    let outcome = editor.rebase()?;

    // Someone checks the detached worktree out somewhere else in the meantime.
    let detached = linked_repo(&repo, "wt-detached")?;
    let elsewhere = repo.rev_parse_single("main")?.detach();
    but_core::worktree::safe_checkout_from_head(elsewhere, &detached, Default::default())?;
    assert_eq!(detached.head_id()?.detach(), elsewhere);

    let err = outcome
        .materialize_without_checkout()
        .expect_err("the transaction must not move a HEAD it never looked at");
    assert!(
        format!("{err:#}").contains("worktrees/wt-detached/HEAD"),
        "{err:#}"
    );
    assert_eq!(
        linked_repo(&repo, "wt-detached")?.head_id()?.detach(),
        elsewhere,
        "the worktree keeps what someone else put there"
    );
    Ok(())
}
