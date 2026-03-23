//! These tests exercise the insert segment operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, mutate};
use but_testsupport::{git_status, visualize_commit_graph, visualize_commit_graph_all};

use crate::utils::{fixture_writable, standard_options};

#[test]
fn insert_single_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = "refs/heads/B".try_into()?;
    let b_selector = editor
        .select_reference(b)
        .context("Failed to find reference b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment(b_selector, delimiter, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   5ae394a (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\  
    | * 930563a (C) C: add another 10 lines to new file
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    * |   77b07be (A) A: 10 lines on top
    |\ \  
    | |/  
    |/|   
    | * a748762 (B) B: another 10 lines at the bottom
    | * 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_single_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment(b_selector, delimiter, mutate::InsertSide::Below)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   e22f751 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | 743ea2e (B) B: another 10 lines at the bottom
    |/ /  
    * |   507ce96 (A) A: 10 lines on top
    |\ \  
    | |/  
    |/|   
    | * 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_multi_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_segment(a_selector, delimiter, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   b7346f3 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\  
    | * 930563a (C) C: add another 10 lines to new file
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    * | 4670c6d (B, A) B: another 10 lines at the bottom
    * |   1470cfe B: 10 lines at the bottom
    |\ \  
    | |/  
    |/|   
    | * add59d2 A: 10 lines on top
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_multi_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_segment(a_selector, delimiter, mutate::InsertSide::Below)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   78624ea (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    * | | e4c78ba (A) A: 10 lines on top
    |/ /  
    * | a748762 (B) B: another 10 lines at the bottom
    * | 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_single_node_segment_above_with_explicit_children() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_segment_into(
        b_selector,
        delimiter,
        mutate::InsertSide::Above,
        Some(mutate::SomeSelectors::new(vec![c_selector])?),
    )?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   a14ecd6 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    | | *   53c45c8 (C) C: add another 10 lines to new file
    | | |\  
    | |_|/  
    |/| |   
    * | | 77b07be (A) A: 10 lines on top
    |\| | 
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    |/ /  
    | * 68a2fc3 C: add 10 lines to new file
    | * 984fd1c C: new file with 10 lines
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn insert_single_node_segment_below_with_explicit_parents() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "@")?, @r"
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
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_selector,
    };

    editor.insert_segment_into(
        a_selector,
        delimiter,
        mutate::InsertSide::Below,
        Some(mutate::SomeSelectors::new(vec![c_selector])?),
    )?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   9cf36b2 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \  
    * | | 37fb54d (A) A: 10 lines on top
    |\| | 
    | * | d202f84 (B) B: another 10 lines at the bottom
    | |\| 
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | |/  
    |/|   
    | * 62e05ba B: 10 lines at the bottom
    |/  
    * 8f0d338 (tag: base) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}
