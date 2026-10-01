//! Exercises the pick option that leaves out a commit whose pick changes nothing.
use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Pick, Step, mutate::InsertSide};
use but_testsupport::visualize_commit_graph_all;
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

fn editor_over_four_commits() -> Result<(
    gix::Repository,
    tempfile::TempDir,
    std::mem::ManuallyDrop<but_meta::VirtualBranchesTomlMetadata>,
    but_db::DbHandle,
    but_graph::Workspace,
)> {
    let (repo, tmpdir, meta, mut db) = fixture_writable("four-commits")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
        .raw()
    );
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let ws = graph.into_workspace()?;
    Ok((repo, tmpdir, meta, db, ws))
}

fn drop_if_empty_pick(id: gix::ObjectId) -> Step {
    let mut pick = Pick::new_pick(id);
    pick.drop_if_empty = true;
    Step::Pick(pick)
}

#[test]
fn a_pick_that_changes_nothing_is_left_out() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Picking `a` again right above `a` has nothing left to add.
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let a_sel = editor.select_commit(a)?;
    editor.insert(a_sel, drop_if_empty_pick(a), InsertSide::Above)?;

    let outcome = editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        outcome.history.commit_mappings().to_debug(),
        snapbox::str![[r#"
{}

"#]]
    );
    Ok(())
}

#[test]
fn without_the_option_the_empty_pick_stays() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let a_sel = editor.select_commit(a)?;
    editor.insert(a_sel, Step::new_pick(a), InsertSide::Above)?;

    editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0519734 (HEAD -> main) c
* 6c60706 b
* 1b3bde6 a
* d591dfe a
* 35b8235 base

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn a_root_commit_that_changes_nothing_is_left_out() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // A root has changes against the empty tree; picked onto `base`, which
    // already has them, nothing is left.
    let base = repo.rev_parse_single("HEAD~3")?.detach();
    let base_sel = editor.select_commit(base)?;
    editor.insert(base_sel, drop_if_empty_pick(base), InsertSide::Above)?;

    editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn a_pick_onto_several_parents_is_kept() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // `a` again, onto `a` and `base`: the result adds nothing to `a`, but the
    // pick joins two parents and stays for that.
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let base = repo.rev_parse_single("HEAD~3")?.detach();
    let a_sel = editor.select_commit(a)?;
    let again = editor.insert(a_sel, drop_if_empty_pick(a), InsertSide::Above)?;
    editor.add_edge(again, base, 1)?;

    editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a6c0412 (HEAD -> main) c
* 498423e b
*   78295f2 a
|\  
* | d591dfe a
|/  
* 35b8235 base

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn a_merge_of_a_dropped_pick_and_its_parent_keeps_one_edge() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // `b` merges the dropped duplicate of `a` with `a` itself; once the
    // duplicate maps to `a`, `b` must not end up with `a` twice.
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let b = repo.rev_parse_single("HEAD~1")?.detach();
    let a_sel = editor.select_commit(a)?;
    editor.insert(a_sel, drop_if_empty_pick(a), InsertSide::Above)?;
    editor.add_edge(editor.select_commit(b)?, a_sel, 1)?;

    editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn a_collapsed_parent_keeps_its_place_among_the_others() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // `c` gets the parents `[b, a, D]` with `D` a dropped duplicate of `b`.
    // Once `D` collapses onto `b`, `b` must stay the first parent.
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let b = repo.rev_parse_single("HEAD~1")?.detach();
    let c = repo.rev_parse_single("HEAD")?.detach();
    let (a_sel, b_sel, c_sel) = (
        editor.select_commit(a)?,
        editor.select_commit(b)?,
        editor.select_commit(c)?,
    );
    let dup = editor.add_step(drop_if_empty_pick(b))?;
    editor.add_edge(dup, b_sel, 0)?;
    editor.add_edge(c_sel, a_sel, 1)?;
    editor.add_edge(c_sel, dup, 2)?;

    editor.rebase()?.materialize(Default::default())?;
    let parents = repo
        .head_commit()?
        .parent_ids()
        .map(|id| id.detach())
        .collect::<Vec<_>>();
    assert_eq!(parents, [b, a]);
    Ok(())
}

#[test]
fn a_pick_that_becomes_an_empty_root_is_kept() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // A commit deleting everything, picked as the new root: its tree is empty,
    // but with no parent there is nothing to be empty against.
    let c = repo.rev_parse_single("HEAD")?.detach();
    let base = repo.rev_parse_single("HEAD~3")?.detach();
    let mut wipe = but_core::Commit::from_id(c.attach(&repo))?;
    wipe.message = "delete everything".into();
    wipe.tree = repo.empty_tree().id;
    let wipe = repo.write_object(wipe.inner)?.detach();

    let base_sel = editor.select_commit(base)?;
    editor.insert(base_sel, drop_if_empty_pick(wipe), InsertSide::Below)?;

    editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 1ad07ef (HEAD -> main) c
* e38cdcc b
* d4004db a
* bc5e6a8 base
* 59a55c9 delete everything

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn a_conflicting_pick_is_kept() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits-one-file")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f37690f (HEAD -> main, c) c
* 3b3bd41 (b) b
* 5e0ba46 (a) a
* 6155f21 (base) base

"#]]
        .raw()
    );
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Without `b`, `c` conflicts: a conflicted pick is never empty.
    let b = repo.rev_parse_single("b")?.detach();
    let c = repo.rev_parse_single("c")?.detach();
    editor.replace(editor.select_commit(b)?, Step::None)?;
    editor.replace(editor.select_commit(c)?, drop_if_empty_pick(c))?;

    editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 04d1892 (HEAD -> main, c) [conflict] c
* 5e0ba46 (b, a) a
* 6155f21 (base) base

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn a_commit_that_was_empty_already_is_kept() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db, mut ws) = editor_over_four_commits()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Same tree as its parent `a`: empty on purpose, so the pick keeps it.
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let mut empty = but_core::Commit::from_id(a.attach(&repo))?;
    empty.message = "intentionally empty".into();
    empty.parents = vec![a].into();
    let empty = repo.write_object(empty.inner)?.detach();

    let a_sel = editor.select_commit(a)?;
    editor.insert(a_sel, drop_if_empty_pick(empty), InsertSide::Above)?;

    editor.rebase()?.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 5974f87 (HEAD -> main) c
* a425254 b
* 2f80bf2 intentionally empty
* d591dfe a
* 35b8235 base

"#]]
        .raw()
    );
    Ok(())
}
