//! Tests for `materialize` vs `materialize_without_checkout` behavior differences
use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{
    Editor, Step,
    mutate::{SegmentDelimiter, SelectorSet},
};
use but_testsupport::{
    StackState, graph_tree, visualize_commit_graph_all, visualize_disk_tree_skip_dot_git,
};
use snapbox::IntoData;

use crate::{
    graph_rebase::add_stack_with_segments,
    utils::{fixture_writable, fixture_writable_slow, standard_options},
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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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
    let outcome = outcome.materialize()?;
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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

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
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let stack_tip = repo.rev_parse_single("stack-2")?.detach();
    let stack_tip_sel = editor.select_commit(stack_tip)?;
    editor.replace(stack_tip_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

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

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let main_sel = editor.select_reference(main_ref.as_ref())?;
    editor.replace(main_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

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

/// Build the graph over `repo` with `middle` (checked out in the linked worktree
/// named `wt`) as a worktree tip, so the editor records a worktree checkout.
fn graph_options_with_worktree_tip(repo: &gix::Repository) -> Result<but_graph::init::Options> {
    let mut options = standard_options();
    options.worktree_tips = vec![but_graph::init::WorktreeTip {
        name: "wt".into(),
        ref_name: Some("refs/heads/middle".try_into()?),
        id: repo.find_reference("middle")?.peel_to_id()?.detach(),
    }];
    Ok(options)
}

fn repoint_reference(
    editor: &mut Editor<'_, '_, impl but_core::RefMetadata>,
    refname: &str,
    target: gix::ObjectId,
) -> Result<()> {
    let reference = editor.select_reference(refname.try_into()?)?;
    let target = editor.select_commit(target)?;
    editor.disconnect_segment_from(
        SegmentDelimiter {
            child: reference,
            parent: reference,
        },
        SelectorSet::All,
        SelectorSet::All,
        false,
    )?;
    editor.add_edge(reference, target, 0)
}

fn repoint_middle(
    editor: &mut Editor<'_, '_, impl but_core::RefMetadata>,
    target: gix::ObjectId,
) -> Result<()> {
    repoint_reference(editor, "refs/heads/middle", target)
}

fn linked_worktree_index(worktree_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    Ok(gix::open(worktree_dir)?.path().join("index"))
}

#[test]
fn linked_worktree_non_overlapping_dirt_survives_inbound_and_outbound_rewrites() -> Result<()> {
    for target in ["main", "main~2"] {
        let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout-dirt")?;
        let worktree_dir = repo.workdir().unwrap().join("wt");
        std::fs::write(worktree_dir.join("unrelated"), "dirty but unrelated\n")?;

        let graph = Graph::from_head(
            &repo,
            &*meta,
            project_meta(&*meta),
            graph_options_with_worktree_tip(&repo)?,
        )?
        .validated()?;
        let mut ws = graph.into_workspace()?;
        let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;
        let new_tip = repo.rev_parse_single(target)?.detach();
        repoint_middle(&mut editor, new_tip)?;

        editor.rebase()?.materialize()?;

        assert_eq!(
            repo.rev_parse_single("middle")?.detach(),
            new_tip,
            "the linked-worktree branch reaches the requested inbound/outbound tip"
        );
        assert_eq!(
            std::fs::read(worktree_dir.join("unrelated"))?,
            b"dirty but unrelated\n",
            "unrelated linked-worktree dirt survives the history rewrite"
        );
        assert!(
            but_testsupport::git_status_at_dir(&worktree_dir)?.contains("unrelated"),
            "the preserved edit remains uncommitted"
        );
    }
    Ok(())
}

#[test]
fn overlapping_linked_worktree_dirt_blocks_inbound_rewrite_before_ref_or_index_moves() -> Result<()>
{
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout-dirt")?;
    let worktree_dir = repo.workdir().unwrap().join("wt");
    std::fs::write(worktree_dir.join("main-only"), "local collision\n")?;
    let middle_before = repo.rev_parse_single("middle")?.detach();
    let index_path = linked_worktree_index(&worktree_dir)?;
    let index_before = std::fs::read(&index_path)?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        graph_options_with_worktree_tip(&repo)?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let main = repo.rev_parse_single("main")?.detach();
    repoint_middle(&mut editor, main)?;

    let err = editor
        .rebase()?
        .materialize()
        .expect_err("overlapping dirt must block the linked-worktree checkout");
    assert!(
        format!("{err:#}").contains("Uncommitted files would be overwritten by checkout"),
        "the checkout conflict is surfaced: {err:#}"
    );
    assert_eq!(repo.rev_parse_single("middle")?.detach(), middle_before);
    assert_eq!(
        std::fs::read(worktree_dir.join("main-only"))?,
        b"local collision\n"
    );
    assert_eq!(std::fs::read(index_path)?, index_before);
    Ok(())
}

#[test]
fn overlapping_linked_worktree_dirt_blocks_outbound_rewrite_before_ref_or_index_moves() -> Result<()>
{
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout-dirt")?;
    let worktree_dir = repo.workdir().unwrap().join("wt");
    std::fs::write(worktree_dir.join("shared"), "local collision\n")?;
    let middle_before = repo.rev_parse_single("middle")?.detach();
    let index_path = linked_worktree_index(&worktree_dir)?;
    let index_before = std::fs::read(&index_path)?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        graph_options_with_worktree_tip(&repo)?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let base = repo.rev_parse_single("main~2")?.detach();
    repoint_middle(&mut editor, base)?;

    let err = editor
        .rebase()?
        .materialize()
        .expect_err("overlapping dirt must block the linked-worktree checkout");
    assert!(
        format!("{err:#}").contains("Uncommitted files would be overwritten by checkout"),
        "the checkout conflict is surfaced: {err:#}"
    );
    assert_eq!(repo.rev_parse_single("middle")?.detach(), middle_before);
    assert_eq!(
        std::fs::read(worktree_dir.join("shared"))?,
        b"local collision\n"
    );
    assert_eq!(std::fs::read(index_path)?, index_before);
    Ok(())
}

#[test]
fn all_linked_worktrees_are_preflighted_before_the_first_checkout_changes() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout-dirt")?;
    let worktree = repo.workdir().unwrap().join("wt");
    let conflicting_worktree = repo.workdir().unwrap().join("wt2");
    std::fs::write(conflicting_worktree.join("main-only"), "local collision\n")?;
    let middle_before = repo.rev_parse_single("middle")?.detach();
    let second_before = repo.rev_parse_single("second")?.detach();
    let worktree_index = linked_worktree_index(&worktree)?;
    let worktree_index_before = std::fs::read(&worktree_index)?;
    let conflicting_index = linked_worktree_index(&conflicting_worktree)?;
    let conflicting_index_before = std::fs::read(&conflicting_index)?;

    let mut options = standard_options();
    options.worktree_tips = [
        (&worktree, "wt", "middle"),
        (&conflicting_worktree, "wt2", "second"),
    ]
    .into_iter()
    .map(|(_, name, branch)| {
        Ok(but_graph::init::WorktreeTip {
            name: name.into(),
            ref_name: Some(format!("refs/heads/{branch}").try_into()?),
            id: repo.find_reference(branch)?.peel_to_id()?.detach(),
        })
    })
    .collect::<Result<Vec<_>>>()?;
    let graph = Graph::from_head(&repo, &*meta, project_meta(&*meta), options)?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let base = repo.rev_parse_single("main~2")?.detach();
    let main = repo.rev_parse_single("main")?.detach();
    repoint_middle(&mut editor, base)?;
    repoint_reference(&mut editor, "refs/heads/second", main)?;

    editor
        .rebase()?
        .materialize()
        .expect_err("the second linked-worktree conflict must abort every checkout");

    assert_eq!(repo.rev_parse_single("middle")?.detach(), middle_before);
    assert_eq!(repo.rev_parse_single("second")?.detach(), second_before);
    assert_eq!(
        std::fs::read(worktree.join("shared"))?,
        b"middle\n",
        "the first checkout was not changed before the later conflict was found"
    );
    assert!(worktree.join("middle-only").exists());
    assert_eq!(std::fs::read(worktree_index)?, worktree_index_before);
    assert_eq!(
        std::fs::read(conflicting_worktree.join("main-only"))?,
        b"local collision\n"
    );
    assert_eq!(std::fs::read(conflicting_index)?, conflicting_index_before);
    Ok(())
}

#[test]
fn materialize_checks_out_linked_worktrees_seeded_into_the_graph() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout")?;
    let worktree_dir = repo.workdir().unwrap().join("wt");

    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&worktree_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
├── a:100644
└── base:100644

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        graph_options_with_worktree_tip(&repo)?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Drop the 'a' commit that 'middle' (checked out in the worktree) points to.
    let a = repo.rev_parse_single("middle")?;
    let a_sel = editor.select_commit(a.detach())?;
    editor.replace(a_sel, Step::None)?;

    editor.rebase()?.materialize()?;

    // The worktree's branch moved to 'base', and its checkout followed.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d862b68 (HEAD -> main) b
* 35b8235 (middle) base

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&worktree_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
└── base:100644

"#]]
    );

    Ok(())
}

#[test]
fn materialize_without_checkout_still_checks_out_linked_worktrees() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout")?;
    let worktree_dir = repo.workdir().unwrap().join("wt");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        graph_options_with_worktree_tip(&repo)?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    // Drop the 'a' commit that 'middle' (checked out in the worktree) points to.
    let a = repo.rev_parse_single("middle")?;
    let a_sel = editor.select_commit(a.detach())?;
    editor.replace(a_sel, Step::None)?;

    editor.rebase()?.materialize_without_checkout()?;

    // The refs moved just like with `materialize`...
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d862b68 (HEAD -> main) b
* 35b8235 (middle) base

"#]]
    );
    // ...the editor's own (HEAD) worktree was not checked out, so the dropped
    // commit's file survives there as an uncommitted change...
    assert!(
        repo.workdir().unwrap().join("a").exists(),
        "the HEAD checkout is skipped - that is this variant's contract"
    );
    // ...but the linked worktree still followed its moved branch, instead of
    // being left stale on the old tree.
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&worktree_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
└── base:100644

"#]]
    );

    Ok(())
}

#[test]
fn materialize_leaves_linked_worktrees_alone_without_worktree_tips() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout")?;
    let worktree_dir = repo.workdir().unwrap().join("wt");

    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("middle")?;
    let a_sel = editor.select_commit(a.detach())?;
    editor.replace(a_sel, Step::None)?;

    editor.rebase()?.materialize()?;

    // The branch still moves, but the worktree checkout is left stale -
    // today's behavior when the feature flag is off.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d862b68 (HEAD -> main) b
* 35b8235 (middle) base

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_disk_tree_skip_dot_git(&worktree_dir)?.to_string(),
        snapbox::str![[r#"
.
├── .git:100644
├── a:100644
└── base:100644

"#]]
    );

    Ok(())
}

#[test]
fn set_worktree_merge_base_override_needs_a_matching_worktree_checkout() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout")?;
    let tree_id = repo.rev_parse_single("HEAD^{tree}")?.detach();

    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta(&*meta),
        graph_options_with_worktree_tip(&repo)?,
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    editor
        .set_worktree_merge_base_override("wt".into(), tree_id)
        .expect("the worktree seeded into the graph has a checkout");
    let err = editor
        .set_worktree_merge_base_override("not-a-worktree".into(), tree_id)
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("No checkout is recorded for a linked worktree named not-a-worktree"),
        "unknown names fail fast: {err}"
    );
    Ok(())
}

#[test]
fn set_worktree_merge_base_override_errors_without_worktree_tips() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout")?;
    let tree_id = repo.rev_parse_single("HEAD^{tree}")?.detach();

    // No worktree tips seeded (feature flag off): even the existing 'wt' has
    // no checkout in the editor.
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let err = editor
        .set_worktree_merge_base_override("wt".into(), tree_id)
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("No checkout is recorded for a linked worktree named wt"),
        "worktrees not seeded into the graph fail like unknown ones: {err}"
    );
    Ok(())
}

#[test]
fn rebase_never_deletes_refs_checked_out_in_worktrees() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable_slow("worktree-checkout")?;
    // A sibling branch on the same commit as 'middle' that no worktree checks out.
    repo.reference(
        "refs/heads/doomed",
        repo.rev_parse_single("middle")?.detach(),
        gix::refs::transaction::PreviousValue::MustNotExist,
        "test setup",
    )?;

    // No worktree tips: the deletion guard is independent of the feature flag.
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    for refname in ["refs/heads/middle", "refs/heads/doomed"] {
        let selector = editor.select_reference(refname.try_into()?)?;
        editor.replace(selector, Step::None)?;
    }
    editor.rebase()?.materialize()?;

    assert!(
        repo.try_find_reference("doomed")?.is_none(),
        "an unchecked-out branch removed from the step graph is deleted"
    );
    assert!(
        repo.try_find_reference("middle")?.is_some(),
        "a branch checked out in a linked worktree survives - deleting it would dangle that worktree's HEAD"
    );

    Ok(())
}
