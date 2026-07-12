//! These tests exercise the add_step, insert_parent and detach operations.

use anyhow::Result;
use but_core::Commit;
use but_graph::Workspace;
use but_rebase::graph_rebase::{CommitSpec, Editor, testing::Testing as _};
use gix::prelude::ObjectIdExt;

use crate::utils::{fixture, fixture_writable, standard_options};

#[test]
fn adding_a_step_returns_a_selector_that_can_be_connected_into_the_graph() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let c = repo.rev_parse_single("HEAD")?.detach();
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let c_handle = editor.select_commit(c)?;
    let a_handle = editor.select_commit(a)?;

    let mut commit = Commit::from_id(a.attach(&repo))?;
    commit.message = "synthetic parent for c".into();
    commit.parents = vec![].into();
    let new_commit = repo.write_object(commit.inner)?.detach();

    let new_handle = editor.add_commit(CommitSpec::new(new_commit))?;
    editor.insert_parent(c_handle, new_handle, 1)?;
    editor.insert_parent(new_handle, a_handle, 0)?;

    let steps_ascii = editor
        .steps_ascii()
        .replace(&new_commit.to_hex_with_len(7).to_string(), "[new]");

    snapbox::assert_data_eq!(
        steps_ascii,
        snapbox::str![[r#"
◎  refs/heads/main
●    120e3a9 c
├─╮
● │  a96434e b
│ ●  [new] synthetic parent for c
├─╯
●  d591dfe a
●  35b8235 base
"#]]
    );

    Ok(())
}

#[test]
fn inserting_at_an_occupied_parent_number_shifts_existing_parents() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let c = repo.rev_parse_single("HEAD")?.detach();
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let c_handle = editor.select_commit(c)?;
    let a_handle = editor.select_commit(a)?;

    // `c`'s only parent is `b` at parent number 0; inserting `a` there makes `a` the first
    // parent and shifts `b` to the merge side.
    editor.insert_parent(c_handle, a_handle, 0)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
●    120e3a9 c
├─╮
│ ●  a96434e b
├─╯
●  d591dfe a
●  35b8235 base
"#]]
    );

    Ok(())
}

#[test]
fn adding_a_parent_that_introduces_a_cycle_causes_an_error() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let c = repo.rev_parse_single("HEAD")?.detach();
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let c_handle = editor.select_commit(c)?;
    let a_handle = editor.select_commit(a)?;

    let err = editor
        .insert_parent(a_handle, c_handle, 1)
        .expect_err("adding a descendant as a parent should fail");

    assert_eq!(
        err.to_string(),
        "BUG: this parent would make the child its own ancestor"
    );

    Ok(())
}

#[test]
fn adding_a_valid_parent_is_successful() -> Result<()> {
    let (repo, mut meta) = fixture("merge-in-the-middle")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let b = repo.rev_parse_single("B")?.detach();
    let a_handle = editor.select_commit(a)?;
    let b_handle = editor.select_commit(b)?;

    // Addressing the COMMIT builds on it directly: the new entry bypasses the ref
    // group standing there. Addressing the REFERENCE instead (below) enters through
    // its group — the caller picks the meaning by what they anchor to.
    editor.insert_parent(a_handle, b_handle, 1)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/with-inner-merge
●  e8ee978 on top of inner merge
●    2fc288c Merge branch 'B' into with-inner-merge
├─╮
◎ │  refs/heads/A
● │    add59d2 A: 10 lines on top
├───╮
│ ◎ │  refs/heads/B
│ ├─╯
│ ●  984fd1c C: new file with 10 lines
├─╯
◎  refs/heads/main
◎  refs/tags/base
●  8f0d338 base
"#]]
    );

    Ok(())
}

#[test]
fn adding_a_parent_through_a_reference_enters_its_group() -> Result<()> {
    let (repo, mut meta) = fixture("merge-in-the-middle")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_handle = editor.select_commit(a)?;
    let b_ref = editor.select_reference("refs/heads/B".try_into()?)?;

    // The sibling of the commit-addressed case above: anchoring the REFERENCE makes
    // the new entry enter through its group.
    editor.insert_parent(a_handle, b_ref, 1)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/with-inner-merge
●  e8ee978 on top of inner merge
●    2fc288c Merge branch 'B' into with-inner-merge
├─╮
◎ │  refs/heads/A
● │  add59d2 A: 10 lines on top
├─╮
│ ◎  refs/heads/B
│ ●  984fd1c C: new file with 10 lines
├─╯
◎  refs/heads/main
◎  refs/tags/base
●  8f0d338 base
"#]]
    );

    Ok(())
}

#[test]
fn remove_edge_returns_no_orders_when_no_edges_found() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let c = repo.rev_parse_single("HEAD")?.detach();
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let c_handle = editor.select_commit(c)?;
    let a_handle = editor.select_commit(a)?;

    editor.detach(c_handle, a_handle)?;

    Ok(())
}

#[test]
fn removing_an_existing_edge_returns_its_order_and_allows_readding_it() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let b = repo.rev_parse_single("HEAD~")?.detach();
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let b_handle = editor.select_commit(b)?;
    let a_handle = editor.select_commit(a)?;

    assert_eq!(editor.detach(b_handle, a_handle)?, vec![0]);
    editor.insert_parent(b_handle, a_handle, 0)?;

    snapbox::assert_data_eq!(
        editor.steps_ascii(),
        snapbox::str![[r#"
◎  refs/heads/main
●  120e3a9 c
●  a96434e b
●  d591dfe a
●  35b8235 base
"#]]
    );

    Ok(())
}
