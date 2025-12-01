/// These tests demonstrate that if none of the steps are changed, the same
/// graphs are returned.
use anyhow::{Result, bail};
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, rebase::RebaseOutcome};
use but_testsupport::visualize_commit_graph_all;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn four_commits() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * 120e3a9 (HEAD -> main) c
    * a96434e b
    * d591dfe a
    * 35b8235 base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.create_editor()?;
    let outcome = editor.rebase(&repo)?;
    let RebaseOutcome::Success(outcome) = outcome else {
        bail!("Rebase failed");
    };
    outcome.materialize(&repo)?;

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn merge_in_the_middle() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-in-the-middle")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.create_editor()?;
    let outcome = editor.rebase(&repo)?;
    let RebaseOutcome::Success(outcome) = outcome else {
        bail!("Rebase failed");
    };
    outcome.materialize(&repo)?;

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn three_branches_merged() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("three-branches-merged")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    *-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/  
    * / add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let editor = graph.create_editor()?;
    let outcome = editor.rebase(&repo)?;
    let RebaseOutcome::Success(outcome) = outcome else {
        bail!("Rebase failed");
    };
    outcome.materialize(&repo)?;

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}
