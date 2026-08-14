use but_core::Commit;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use but_testsupport::visualize_commit_graph_all;
use gix::prelude::ObjectIdExt as _;
use snapbox::IntoData;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn insert_below_commit() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let one = repo.rev_parse_single("one")?.detach();
    let two = repo.rev_parse_single("two")?.detach();

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;
    but_workspace::commit::cherry_pick_commits(
        editor,
        [one],
        RelativeTo::Commit(two),
        InsertSide::Below,
    )?
    .0
    .materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 68995ae (HEAD -> three) commit three
* 75334f1 (two) commit two
* 50680ef commit one
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );

    Ok(())
}

#[test]
fn insert_above_commit() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let one = repo.rev_parse_single("one")?.detach();
    let two = repo.rev_parse_single("two")?.detach();

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;
    but_workspace::commit::cherry_pick_commits(
        editor,
        [one],
        RelativeTo::Commit(two),
        InsertSide::Above,
    )?
    .0
    .materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b4ca6cc (HEAD -> three) commit three
* 5ad6169 (two) commit one
* 16fd221 (origin/two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    Ok(())
}

#[test]
fn insert_below_reference() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let one = repo.rev_parse_single("one")?.detach();
    let two_ref: gix::refs::FullName = "refs/heads/two".try_into()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;
    but_workspace::commit::cherry_pick_commits(
        editor,
        [one],
        RelativeTo::Reference(two_ref),
        InsertSide::Below,
    )?
    .0
    .materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b4ca6cc (HEAD -> three) commit three
* 5ad6169 (two) commit one
* 16fd221 (origin/two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    Ok(())
}

#[test]
fn sources_are_applied_in_the_order_given() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-single-stack-double-stack", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let b = repo.rev_parse_single("B")?.detach();
    let c = repo.rev_parse_single("C")?.detach();
    let a_ref: gix::refs::FullName = "refs/heads/A".try_into()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;
    let (rebase, _) = but_workspace::commit::cherry_pick_commits(
        editor,
        [b, c],
        RelativeTo::Reference(a_ref),
        InsertSide::Below,
    )?;
    rebase.materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   ce4b2e2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f603807 (A) C
| * 698ccd3 B
| * 09d8e52 A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn sources_are_deduped() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-single-stack-double-stack", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let b = repo.rev_parse_single("B")?.detach();
    let a_ref: gix::refs::FullName = "refs/heads/A".try_into()?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;
    let (rebase, inserted_selectors) = but_workspace::commit::cherry_pick_commits(
        editor,
        [b, b],
        RelativeTo::Reference(a_ref),
        InsertSide::Below,
    )?;

    assert_eq!(
        inserted_selectors.len(),
        1,
        "duplicate B should produce only one copy"
    );
    rebase.materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   ec1bb42 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 698ccd3 (A) B
| * 09d8e52 A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn copies_get_new_change_ids() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-single-stack-double-stack", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let source = repo.rev_parse_single("B")?.detach();
    let target_ref: gix::refs::FullName = "refs/heads/A".try_into()?;
    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;

    let (rebase, inserted_selectors) = but_workspace::commit::cherry_pick_commits(
        editor,
        [source],
        RelativeTo::Reference(target_ref),
        InsertSide::Below,
    )?;
    let copy = rebase.lookup_pick(inserted_selectors[0])?;
    rebase.materialize(Default::default())?;

    assert_ne!(
        Commit::from_id(source.attach(&repo))?.change_id(),
        Commit::from_id(copy.attach(&repo))?.change_id(),
        "a copied commit should have a new change ID"
    );

    Ok(())
}

#[test]
fn copies_commit_contents() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-single-stack-double-stack-files", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let source = repo.rev_parse_single("B")?.detach();
    let target_ref: gix::refs::FullName = "refs/heads/A".try_into()?;
    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;

    let (rebase, inserted_selectors) = but_workspace::commit::cherry_pick_commits(
        editor,
        [source],
        RelativeTo::Reference(target_ref),
        InsertSide::Below,
    )?;
    let copy = rebase.lookup_pick(inserted_selectors[0])?;
    rebase.materialize(Default::default())?;

    assert_eq!(
        repo.find_commit(copy)?.message_raw()?,
        repo.find_commit(source)?.message_raw()?,
        "the copy should retain the source commit message"
    );
    assert_eq!(
        repo.rev_parse_single(format!("{copy}:file-a").as_str())?
            .object()?
            .data,
        b"a\n",
        "the copy should retain the destination contents"
    );
    assert_eq!(
        repo.rev_parse_single(format!("{copy}:file-b").as_str())?
            .object()?
            .data,
        b"b\n",
        "the copy should apply the source changes"
    );

    Ok(())
}

#[test]
fn rebased_children_keep_contents() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-single-stack-double-stack-files", |_| {})?;
    let mut workspace = graph.into_workspace()?;
    let source = repo.rev_parse_single("B")?.detach();
    let target = repo.rev_parse_single("A")?.detach();
    let mut db = but_testsupport::in_memory_db();
    let editor = Editor::create(&mut workspace, &mut meta, &repo, &mut db)?;

    but_workspace::commit::cherry_pick_commits(
        editor,
        [source],
        RelativeTo::Commit(target),
        InsertSide::Below,
    )?
    .0
    .materialize(Default::default())?;

    let rebased_target = repo.rev_parse_single("A")?.detach();
    assert_eq!(
        repo.find_commit(rebased_target)?.message_raw()?,
        repo.find_commit(target)?.message_raw()?,
        "the rebased child should retain its message"
    );
    assert_eq!(
        repo.rev_parse_single(format!("{rebased_target}:file-a").as_str())?
            .object()?
            .data,
        b"a\n",
        "the rebased child should retain its changes"
    );
    assert_eq!(
        repo.rev_parse_single(format!("{rebased_target}:file-b").as_str())?
            .object()?
            .data,
        b"b\n",
        "the rebased child should include the cherry-picked changes"
    );

    Ok(())
}
