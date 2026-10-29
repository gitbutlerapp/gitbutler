use anyhow::Result;
use but_rebase::graph_rebase::{
    Editor,
    mutate::{InsertSide, RelativeToRef},
};
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::insert_blank_commit;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn insert_below_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
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

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    insert_blank_commit(
        editor,
        InsertSide::Below,
        RelativeToRef::Commit(id.detach()),
    )?
    .0
    .materialize(Default::default())?;

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
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
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

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    insert_blank_commit(
        editor,
        InsertSide::Above,
        RelativeToRef::Commit(id.detach()),
    )?
    .0
    .materialize(Default::default())?;

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
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
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

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    insert_blank_commit(
        editor,
        InsertSide::Below,
        RelativeToRef::Reference(reference.name()),
    )?
    .0
    .materialize(Default::default())?;

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
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
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

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    insert_blank_commit(
        editor,
        InsertSide::Above,
        RelativeToRef::Reference(reference.name()),
    )?
    .0
    .materialize(Default::default())?;

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
