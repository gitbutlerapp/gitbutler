//! Exercises the step option for whether a step should be allowed to enter a conflicted state.

use crate::utils::{fixture_writable, standard_options};
use anyhow::{Result, bail};
use but_graph::Graph;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{GraphExt as _, LookupStep, Step, mutate::InsertSide},
};
use but_testsupport::{cat_commit, visualize_commit_graph_all};

#[test]
fn by_default_conflicts_are_allowed() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits-one-file")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f37690f (HEAD -> main, c) c
    * 3b3bd41 (b) b
    * 5e0ba46 (a) a
    * 6155f21 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // Replacing b with none will cause c to conflict
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    // We expect to see conflicted headers
    insta::assert_snapshot!(cat_commit(&repo, "c")?, @r"
    tree 7c4363d235e51107d74c858038cfab0d192db092
    parent 5e0ba4636be91de6216903697b269915d3db6c53
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000000001
    gitbutler-conflicted 1

    c
    ");

    Ok(())
}

#[test]
fn if_a_commit_has_been_configured_not_to_conflict_but_ends_up_conflicted_an_error_is_raised()
-> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits-one-file")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f37690f (HEAD -> main, c) c
    * 3b3bd41 (b) b
    * 5e0ba46 (a) a
    * 6155f21 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // Replacing b with none will cause c to conflict
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    // Set c to disallow conflicts
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let Step::Pick(mut c_pick) = editor.lookup_step(c_sel)? else {
        bail!("c_sel should be a pick");
    };
    c_pick.conflictable = false;
    editor.replace(c_sel, Step::Pick(c_pick))?;

    // We should see an error given saying C ended up being conflicted
    insta::assert_debug_snapshot!(editor.rebase(), @r#"
    Err(
        "Commit f37690fa0ac6f48391974bb0a7cdc4c8a6c6fe7a was marked as not conflictable, but resulted in a conflicted state",
    )
    "#);

    Ok(())
}

#[test]
fn if_a_commit_has_been_configured_not_to_conflict_and_doesnt_end_up_conflicted_result_is_ok()
-> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits-one-file")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f37690f (HEAD -> main, c) c
    * 3b3bd41 (b) b
    * 5e0ba46 (a) a
    * 6155f21 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // Insert an empty commit above b to cause c to get cherry picked with out a conflict
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    let mut empty = editor.empty_commit()?;
    empty.message = b"I'm a new commit! Hello there".into();
    let empty_id = editor.new_commit(empty, DateMode::CommitterKeepAuthorKeep)?;
    editor.insert(b_sel, Step::new_pick(empty_id), InsertSide::Above)?;

    // Set c to disallow conflicts
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let Step::Pick(mut c_pick) = editor.lookup_step(c_sel)? else {
        bail!("c_sel should be a pick");
    };
    c_pick.conflictable = false;
    editor.replace(c_sel, Step::Pick(c_pick))?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    // The rebase is successful because `c` remained unconflicted
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3ac884c (HEAD -> main, c) c
    * 80401d2 (b) I'm a new commit! Hello there
    * 3b3bd41 b
    * 5e0ba46 (a) a
    * 6155f21 (base) base
    ");

    Ok(())
}
