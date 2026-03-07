//! These tests exercise the disconnect operation.
use std::collections::HashSet;

use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, Step, mutate};
use but_testsupport::{git_status, visualize_commit_graph_all};
use gix::prelude::ObjectIdExt;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn disconnect_and_remove_middle_commit_in_linear_history() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
	* 120e3a9 (HEAD -> main) c
	* a96434e b
	* d591dfe a
	* 35b8235 base
	");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let b = repo.rev_parse_single("HEAD~")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;

    let target = mutate::SegmentDelimiter {
        child: b_selector,
        parent: b_selector,
    };

    editor.disconnect_segment_from(
        target,
        mutate::SelectorSet::All,
        mutate::SelectorSet::All,
        false,
    )?;
    editor.replace(b_selector, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
	* 4de0144 (HEAD -> main) c
	* d591dfe a
	* 35b8235 base
	");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn disconnect_and_remove_two_middle_commits_in_linear_history() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("four-commits")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
	* 120e3a9 (HEAD -> main) c
	* a96434e b
	* d591dfe a
	* 35b8235 base
	");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let b = repo.rev_parse_single("HEAD~")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: b_selector,
        parent: a_selector,
    };

    editor.disconnect_segment_from(
        delimiter,
        mutate::SelectorSet::All,
        mutate::SelectorSet::All,
        false,
    )?;
    editor.replace(b_selector, Step::None)?;
    editor.replace(a_selector, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * f55e07c (HEAD -> main) c
    * 35b8235 base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn disconnect_and_remove_commit_in_merge_history_rewires_children() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-in-the-middle")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: a_selector,
        parent: a_selector,
    };

    editor.disconnect_segment_from(
        delimiter,
        mutate::SelectorSet::All,
        mutate::SelectorSet::All,
        false,
    )?;
    editor.replace(a_selector, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    let a_now = repo.rev_parse_single("A")?.detach();
    let base = repo.rev_parse_single("base")?.detach();
    assert_eq!(a_now, base, "A should now point to base after disconnect");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * dde6cc8 (HEAD -> with-inner-merge) on top of inner merge
    *   5f962e2 Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    |/  
    * 8f0d338 (tag: base, main, A) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn disconnect_and_remove_merge_with_two_parents_and_two_children() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-with-two-children")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   d1cc4c7 (HEAD -> with-two-children) tip
    |\  
    | * ce6aca9 (C2) C2: second child
    * | f94f259 (C1) C1: first child
    |/  
    *   c5d1178 (M) M: merge two parents
    |\  
    | * 392a8f8 (P2) P2: second merge parent
    * | bc0e772 (P1) P1: first merge parent
    |/  
    * 7674a5e (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let merge = repo.rev_parse_single("M")?.detach();
    let merge_selector = editor
        .select_commit(merge)
        .context("Failed to find merge commit M in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: merge_selector,
        parent: merge_selector,
    };

    editor.disconnect_segment_from(
        delimiter,
        mutate::SelectorSet::All,
        mutate::SelectorSet::All,
        false,
    )?;
    editor.replace(merge_selector, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    let p1 = repo.rev_parse_single("P1")?.detach();
    let p2 = repo.rev_parse_single("P2")?.detach();
    let expected_parents = HashSet::from([p1, p2]);

    let c1 = repo.rev_parse_single("C1")?.detach();
    let c1_commit = but_core::Commit::from_id(c1.attach(&repo))?;
    let c1_parents = c1_commit
        .inner
        .parents
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        c1_parents, expected_parents,
        "C1 should have both merge parents after removing M"
    );

    let c2 = repo.rev_parse_single("C2")?.detach();
    let c2_commit = but_core::Commit::from_id(c2.attach(&repo))?;
    let c2_parents = c2_commit
        .inner
        .parents
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        c2_parents, expected_parents,
        "C2 should have both merge parents after removing M"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f914957 (HEAD -> with-two-children) tip
    |\  
    | *   72b8072 (C2) C2: second child
    | |\  
    * | \   d8cc9ec (C1) C1: first child
    |\ \ \  
    | |/ /  
    |/| /   
    | |/    
    | * 392a8f8 (P2) P2: second merge parent
    * | bc0e772 (P1, M) P1: first merge parent
    |/  
    * 7674a5e (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn disconnect_and_remove_merge_with_two_parents_and_two_children_from_one_side() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-with-two-children")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   d1cc4c7 (HEAD -> with-two-children) tip
    |\  
    | * ce6aca9 (C2) C2: second child
    * | f94f259 (C1) C1: first child
    |/  
    *   c5d1178 (M) M: merge two parents
    |\  
    | * 392a8f8 (P2) P2: second merge parent
    * | bc0e772 (P1) P1: first merge parent
    |/  
    * 7674a5e (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let merge = repo.rev_parse_single("M")?.detach();
    let m_reference = "refs/heads/M".try_into()?;
    let child_one = repo.rev_parse_single("C1")?.detach();
    let parent_one = "refs/heads/P1".try_into()?;
    let m_reference_selector = editor
        .select_reference(m_reference)
        .context("Failed to find P1 reference in editor graph")?;
    let merge_commit_selector = editor
        .select_commit(merge)
        .context("Failed to find merge commit M in editor graph")?;
    let child_one_selector = editor
        .select_commit(child_one)
        .context("Failed to find C1 referent in editor graph")?;
    let parent_one_selector = editor
        .select_reference(parent_one)
        .context("Failed to find P1 reference in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: m_reference_selector,
        parent: merge_commit_selector,
    };

    editor.disconnect_segment_from(
        delimiter,
        mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![child_one_selector])?),
        mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![parent_one_selector])?),
        false,
    )?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    let p1 = repo.rev_parse_single("P1")?.detach();
    let m = repo.rev_parse_single("M")?.detach();
    let c1_expected_parents = HashSet::from([p1]);
    let c2_expected_parents = HashSet::from([m]);

    let c1 = repo.rev_parse_single("C1")?.detach();
    let c1_commit = but_core::Commit::from_id(c1.attach(&repo))?;
    let c1_parents = c1_commit
        .inner
        .parents
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        c1_parents, c1_expected_parents,
        "C1 should have both merge parents after removing M"
    );

    let c2 = repo.rev_parse_single("C2")?.detach();
    let c2_commit = but_core::Commit::from_id(c2.attach(&repo))?;
    let c2_parents = c2_commit
        .inner
        .parents
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        c2_parents, c2_expected_parents,
        "C2 should have both merge parents after removing M"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   3305e26 (HEAD -> with-two-children) tip
    |\  
    | * 0e87cd3 (C2) C2: second child
    | * 3089592 (M) M: merge two parents
    | * 392a8f8 (P2) P2: second merge parent
    * | f928700 (C1) C1: first child
    * | bc0e772 (P1) P1: first merge parent
    |/  
    * 7674a5e (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}
#[test]
fn disconnect_remove_merge_with_two_parents_and_two_children_children_only() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-with-two-children")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   d1cc4c7 (HEAD -> with-two-children) tip
    |\  
    | * ce6aca9 (C2) C2: second child
    * | f94f259 (C1) C1: first child
    |/  
    *   c5d1178 (M) M: merge two parents
    |\  
    | * 392a8f8 (P2) P2: second merge parent
    * | bc0e772 (P1) P1: first merge parent
    |/  
    * 7674a5e (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let merge = repo.rev_parse_single("M")?.detach();
    let m_reference = "refs/heads/M".try_into()?;
    let parent_one = "refs/heads/P1".try_into()?;
    let m_reference_selector = editor
        .select_reference(m_reference)
        .context("Failed to find P1 reference in editor graph")?;
    let merge_commit_selector = editor
        .select_commit(merge)
        .context("Failed to find merge commit M in editor graph")?;
    let parent_one_selector = editor
        .select_reference(parent_one)
        .context("Failed to find P1 reference in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: m_reference_selector,
        parent: merge_commit_selector,
    };

    editor.disconnect_segment_from(
        delimiter,
        mutate::SelectorSet::None,
        mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![parent_one_selector])?),
        false,
    )?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    let p1 = repo.rev_parse_single("P1")?.detach();
    let p2 = repo.rev_parse_single("P2")?.detach();
    let m = repo.rev_parse_single("M")?.detach();
    let c1 = repo.rev_parse_single("C1")?.detach();
    let c2 = repo.rev_parse_single("C2")?.detach();

    let c1_commit = but_core::Commit::from_id(c1.attach(&repo))?;
    let c1_parents = c1_commit
        .inner
        .parents
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        c1_parents,
        HashSet::from([m]),
        "C1 should have M as its only parent"
    );

    let c2_commit = but_core::Commit::from_id(c2.attach(&repo))?;
    let c2_parents = c2_commit
        .inner
        .parents
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        c2_parents,
        HashSet::from([m]),
        "C2 should have M as its only parent"
    );

    let m_commit = but_core::Commit::from_id(m.attach(&repo))?;
    let m_parents = m_commit
        .inner
        .parents
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(
        m_parents,
        HashSet::from([p2]),
        "M should have P2 as its only parent"
    );

    let refs_to_check = ["with-two-children", "C1", "C2", "M", "P2", "P1", "base"];
    for reference in refs_to_check {
        let commit_id = repo.rev_parse_single(reference)?.detach();
        let commit = but_core::Commit::from_id(commit_id.attach(&repo))?;
        assert!(
            !commit.inner.parents.contains(&p1),
            "{reference} should not list P1 as a parent"
        );
    }

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * bc0e772 (P1) P1: first merge parent
    | *   2eac185 (HEAD -> with-two-children) tip
    | |\  
    | | * 0e87cd3 (C2) C2: second child
    | * | 76e6d3c (C1) C1: first child
    | |/  
    | * 3089592 (M) M: merge two parents
    | * 392a8f8 (P2) P2: second merge parent
    |/  
    * 7674a5e (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
fn disconnect_fails_when_parents_to_disconnect_is_none() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-with-two-children")?;

    let before = visualize_commit_graph_all(&repo)?;

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let merge = repo.rev_parse_single("M")?.detach();
    let m_reference = "refs/heads/M".try_into()?;
    let child_one = repo.rev_parse_single("C1")?.detach();
    let m_reference_selector = editor
        .select_reference(m_reference)
        .context("Failed to find M reference in editor graph")?;
    let merge_commit_selector = editor
        .select_commit(merge)
        .context("Failed to find merge commit M in editor graph")?;
    let child_one_selector = editor
        .select_commit(child_one)
        .context("Failed to find C1 referent in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: m_reference_selector,
        parent: merge_commit_selector,
    };

    let err = editor
        .disconnect_segment_from(
            delimiter,
            mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![child_one_selector])?),
            mutate::SelectorSet::None,
            false,
        )
        .expect_err("expected disconnect to fail for parents=SelectorSet::None");
    assert!(
        err.to_string()
            .contains("Invalid parents to disconnect: SelectorSet::None is not allowed"),
        "unexpected error: {err:#}"
    );

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    let after = visualize_commit_graph_all(&repo)?;
    assert_eq!(before, after, "graph should remain unchanged on failure");

    Ok(())
}

#[test]
fn disconnect_fails_fast_if_parent_to_disconnect_is_not_direct_parent() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-with-two-children")?;

    let before = visualize_commit_graph_all(&repo)?;

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let merge = repo.rev_parse_single("M")?.detach();
    let m_reference = "refs/heads/M".try_into()?;
    let child_one = repo.rev_parse_single("C1")?.detach();
    let m_reference_selector = editor
        .select_reference(m_reference)
        .context("Failed to find M reference in editor graph")?;
    let merge_commit_selector = editor
        .select_commit(merge)
        .context("Failed to find merge commit M in editor graph")?;
    let child_one_selector = editor
        .select_commit(child_one)
        .context("Failed to find C1 referent in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: m_reference_selector,
        parent: merge_commit_selector,
    };

    let err = editor
        .disconnect_segment_from(
            delimiter,
            mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![child_one_selector])?),
            mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![child_one_selector])?),
            false,
        )
        .expect_err("expected disconnect to fail for non-parent selector");
    assert!(
        err.to_string()
            .contains("requested parent is not a direct parent"),
        "unexpected error: {err:#}"
    );

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    let after = visualize_commit_graph_all(&repo)?;
    assert_eq!(before, after, "graph should remain unchanged on failure");

    Ok(())
}

#[test]
fn disconnect_fails_fast_if_child_to_disconnect_is_not_direct_child() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-with-two-children")?;

    let before = visualize_commit_graph_all(&repo)?;

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    let merge = repo.rev_parse_single("M")?.detach();
    let m_reference = "refs/heads/M".try_into()?;
    let parent_one = "refs/heads/P1".try_into()?;
    let m_reference_selector = editor
        .select_reference(m_reference)
        .context("Failed to find M reference in editor graph")?;
    let merge_commit_selector = editor
        .select_commit(merge)
        .context("Failed to find merge commit M in editor graph")?;
    let parent_one_selector = editor
        .select_reference(parent_one)
        .context("Failed to find P1 reference in editor graph")?;

    let delimiter = mutate::SegmentDelimiter {
        child: m_reference_selector,
        parent: merge_commit_selector,
    };

    let err = editor
        .disconnect_segment_from(
            delimiter,
            mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![parent_one_selector])?),
            mutate::SelectorSet::Some(mutate::SomeSelectors::new(vec![parent_one_selector])?),
            false,
        )
        .expect_err("expected disconnect to fail for non-child selector");
    assert!(
        err.to_string()
            .contains("requested child is not a direct parent"),
        "unexpected error: {err:#}"
    );

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    let after = visualize_commit_graph_all(&repo)?;
    assert_eq!(before, after, "graph should remain unchanged on failure");

    Ok(())
}
