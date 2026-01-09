//! Exercises the step option for whether a step's parents must all be references.

use crate::utils::{fixture_writable, standard_options};
use anyhow::{Result, bail};
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt as _, LookupStep, Step};
use but_testsupport::visualize_commit_graph_all;

#[test]
fn by_default_parents_can_be_picks() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.to_editor(&repo)?;

    // By default, picks can have other picks as parents
    let outcome = editor.rebase()?;
    outcome.materialize()?;

    // The graph should remain unchanged since we made no modifications
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    Ok(())
}

#[test]
fn if_a_commit_requires_reference_parents_but_has_pick_parent_an_error_is_raised() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits-one-file")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f37690f (HEAD -> main, c) c
    * 3b3bd41 (b) b
    * 5e0ba46 (a) a
    * 6155f21 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // Set c to require reference parents
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let Step::Pick(mut c_pick) = editor.lookup_step(c_sel)? else {
        bail!("c_sel should be a pick");
    };
    c_pick.parents_must_be_references = true;
    editor.replace(c_sel, Step::Pick(c_pick))?;

    // Replace the "b" reference with Step::None so that c's parent path
    // goes through None and hits Pick(b) before finding a Reference
    let b_ref = editor.select_reference("refs/heads/b".try_into()?)?;
    editor.replace(b_ref, Step::None)?;

    // We should see an error saying c has parents that are not references
    insta::assert_debug_snapshot!(editor.rebase(), @r#"
    Err(
        "Commit f37690fa0ac6f48391974bb0a7cdc4c8a6c6fe7a has parents that are not referenced",
    )
    "#);

    Ok(())
}

#[test]
fn if_a_commit_requires_reference_parents_and_has_reference_parent_result_is_ok() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits-one-file")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f37690f (HEAD -> main, c) c
    * 3b3bd41 (b) b
    * 5e0ba46 (a) a
    * 6155f21 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // Set "a" to require reference parents - "a"'s parent is "base" which has
    // a reference pointing to it
    let a = repo.rev_parse_single("a")?;
    let a_sel = editor.select_commit(a.detach())?;
    let Step::Pick(mut a_pick) = editor.lookup_step(a_sel)? else {
        bail!("a_sel should be a pick");
    };
    a_pick.parents_must_be_references = true;
    editor.replace(a_sel, Step::Pick(a_pick))?;

    // The rebase should succeed because "a"'s parent is "base" which has a reference
    let outcome = editor.rebase()?;
    outcome.materialize()?;

    // The graph should remain unchanged since we made no content modifications
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f37690f (HEAD -> main, c) c
    * 3b3bd41 (b) b
    * 5e0ba46 (a) a
    * 6155f21 (base) base
    ");

    Ok(())
}
