use anyhow::Result;
use but_core::RefMetadata;
use but_graph::init::Overlay;
use but_rebase::graph_rebase::Editor;
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::reword;

use crate::ref_info::with_workspace_commit::utils::{
    named_writable_scenario_with_description,
    named_writable_scenario_with_description_and_graph as writable_scenario,
};

#[test]
fn reword_head_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
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
    let id = repo.rev_parse_single("three")?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bee1f03 (HEAD -> three) New name
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_middle_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
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
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 555bf78 (HEAD -> three) commit three
* 5608218 (two) New name
| * 16fd221 (origin/two) commit two
|/
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_base_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
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
    let id = repo.rev_parse_single("one")?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize()?;

    // We end up with two divergent histories here. This is to be expected if we
    // rewrite the very bottom commit in a repository.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 93151df (HEAD -> three) commit three
* fc0e8de (two) commit two
* f1db5b0 (one) New name
* 16fd221 (origin/two) commit two
* 8b426d0 commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_base_moves_local_aliases_but_not_immutable_aliases() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("reword-three-commits")?;
    let old_base = repo.rev_parse_single("one")?.detach();
    let local_alias: gix::refs::FullName = "refs/heads/local-alias".try_into()?;
    let remote_alias: gix::refs::FullName = "refs/remotes/origin/remote-alias".try_into()?;
    let tag_alias: gix::refs::FullName = "refs/tags/tag-alias".try_into()?;
    for name in [&local_alias, &remote_alias, &tag_alias] {
        repo.reference(
            name.clone(),
            old_base,
            gix::refs::transaction::PreviousValue::Any,
            "same-tip convergence alias test setup",
        )?;
    }

    let mut project_meta = meta
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?
        .project_meta();
    project_meta.target_commit_id = repo.rev_parse_single("main").ok().map(|id| id.detach());
    let graph = but_graph::Graph::from_repo(&repo, &meta, project_meta, Overlay::default())?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    reword(editor, old_base, b"New name".into())?
        .0
        .materialize()?;

    let new_base = repo.rev_parse_single("one")?.detach();
    assert_ne!(new_base, old_base);
    assert_eq!(
        repo.rev_parse_single(local_alias.as_bstr())?.detach(),
        new_base
    );
    for name in [&remote_alias, &tag_alias] {
        assert_eq!(repo.rev_parse_single(name.as_bstr())?.detach(), old_base);
    }

    Ok(())
}
