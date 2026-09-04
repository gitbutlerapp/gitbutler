use anyhow::Result;
use but_rebase::graph_rebase::{Editor, anchor::Anchor, mutate::InsertSide};
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::insert_blank_commit;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn insert_below_commit() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("two")?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    insert_blank_commit(editor, Anchor::Commit(id.detach()), InsertSide::Below)?
        .0
        .materialize()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bdea705 (HEAD -> three) commit three
* b7af41f (two) commit two
* b216146 
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_commit() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("two")?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    insert_blank_commit(editor, Anchor::Commit(id.detach()), InsertSide::Above)?
        .0
        .materialize()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f6276fd (HEAD -> three) commit three
* a2b712e (two) 
* 16fd221 (origin/two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_below_reference() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    insert_blank_commit(
        editor,
        Anchor::Reference(reference.name().to_owned()),
        InsertSide::Below,
    )?
    .0
    .materialize()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f6276fd (HEAD -> three) commit three
* a2b712e (two) 
* 16fd221 (origin/two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_reference() -> Result<()> {
    let (_tmp, ws, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut _meta, &repo)?;
    insert_blank_commit(
        editor,
        Anchor::Reference(reference.name().to_owned()),
        InsertSide::Above,
    )?
    .0
    .materialize()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f6276fd (HEAD -> three) commit three
* a2b712e 
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}
