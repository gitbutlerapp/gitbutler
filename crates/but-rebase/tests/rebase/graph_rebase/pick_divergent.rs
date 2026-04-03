//! Tests for `PickDivergent` execution in the graph rebase engine.

use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step};
use but_testsupport::{visualize_commit_graph_all, visualize_tree};
use gix::prelude::ObjectIdExt as _;

use crate::utils::{fixture_writable, standard_options};

/// Single divergent member where local and remote touch different lines.
///
/// ```text
/// divergent.txt at ancestor:
///   1
///   2
///   3
///   4
///
/// local (rewrites line 2):      remote (rewrites line 4):
///   1                             1
///   local-2                       2
///   3                             3
///   4                             remote-4
///
/// Graph before:
///   * local-change-line-two          (local)
///   | * tip-after-remote             (HEAD -> main)
///   | * remote-change-line-four      (remote)
///   |/
///   * divergence-base                (ancestor)
///
/// Expected after PickDivergent(remote, local=[local], ancestor=ancestor):
///   - Remote commit is kept in place, graph shape unchanged.
///   - Output tree merges both rewrites:
///       divergent.txt = "1\nlocal-2\n3\nremote-4\n"
///   - tip-after-remote is rebased on top.
/// ```
#[test]
fn single_member_happy_path_merges_local_change_into_remote_shape() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("pick-divergent-single-member-happy-path")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 530051c (local) local-change-line-two
    | * 5d75912 (HEAD -> main) tip-after-remote
    | * c133905 (remote) remote-change-line-four
    |/  
    * 070b254 (ancestor) divergence-base
    ");

    let ancestor = repo.rev_parse_single("ancestor")?.detach();
    let local = repo.rev_parse_single("local")?.detach();
    let remote = repo.rev_parse_single("remote")?.detach();

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let remote_selector = editor.select_commit(remote)?;
    editor.replace(
        remote_selector,
        Step::new_pick_divergent(vec![local], Some(ancestor), remote, [0x11; 20]),
    )?;

    let outcome = editor.rebase()?;
    let _outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 530051c (local) local-change-line-two
    | * 74d3ff6 (HEAD -> main) tip-after-remote
    | * 916c82c (remote) remote-change-line-four
    |/  
    * 070b254 (ancestor) divergence-base
    ");

    let remote_out = repo.rev_parse_single("remote")?;
    insta::assert_snapshot!(visualize_tree(remote_out).to_string(), @r#"
    4b1e8a9
    └── divergent.txt:100644:4b1ac72 "1\nlocal-2\n3\nremote-4\n"
    "#);

    let head = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(visualize_tree(head).to_string(), @r#"
    54ce68c
    ├── divergent.txt:100644:4b1ac72 "1\nlocal-2\n3\nremote-4\n"
    └── tip.txt:100644:b218ebd "tip\n"
    "#);

    Ok(())
}

/// Two-member remote family with one local commit, all touching different
/// regions of the file. No conflicts expected.
///
/// ```text
/// file.txt at ancestor:
///   1
///   2
///   3
///   4
///   5
///   6
///   7
///   8
///
/// local-end (appends after line 8):
///   1
///   2
///   3
///   4
///   5
///   6
///   7
///   8
///   local-end
///
/// remote-1 (inserts after line 1):
///   1
///   remote-1
///   2
///   3  ...
///
/// remote-2 (inserts after line 3, i.e. between original 3 and 4):
///   1
///   remote-1
///   2
///   3
///   remote-2
///   4  ...
///
/// Graph before:
///   * local-end              (local-end, local)
///   | * remote-2             (HEAD -> main, remote-two)
///   | * remote-1             (remote-one)
///   |/
///   * divergence-base        (ancestor)
///
/// Expected after PickDivergent family (both remotes, locals=[local-end]):
///   - Graph shape preserved: two remote commits in sequence.
///   - Neither remote overlaps local-end, so no conflicts.
///   - local-end carries forward to remote-2 (final member) and is merged.
///   - Per-commit output trees:
///       remote-1: file.txt = "1\nremote-1\n2\n3\n4\n5\n6\n7\n8\n"
///       remote-2: file.txt = "1\nremote-1\n2\n3\nremote-2\n4\n5\n6\n7\n8\nlocal-end\n"
/// ```
#[test]
fn multi_member_happy_path_no_conflicts() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("pick-divergent-multi-member-happy-path")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * b67c6e2 (local-end, local) local-end
    | * 36f1b48 (HEAD -> main, remote-two) remote-2
    | * 58679a8 (remote-one) remote-1
    |/  
    * 62402e9 (ancestor) divergence-base
    ");

    let ancestor = repo.rev_parse_single("ancestor")?.detach();
    let local_end = repo.rev_parse_single("local-end")?.detach();
    let remote_one = repo.rev_parse_single("remote-one")?.detach();
    let remote_two = repo.rev_parse_single("remote-two")?.detach();

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let family_id = [0x55; 20];
    editor.replace(
        editor.select_commit(remote_one)?,
        Step::new_pick_divergent(vec![local_end], Some(ancestor), remote_one, family_id),
    )?;
    editor.replace(
        editor.select_commit(remote_two)?,
        Step::new_pick_divergent(vec![local_end], Some(ancestor), remote_two, family_id),
    )?;

    let outcome = editor.rebase()?;
    let _outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * b67c6e2 (local-end, local) local-end
    | * 6f82cbf (HEAD -> main, remote-two) remote-2
    | * 58679a8 (remote-one) remote-1
    |/  
    * 62402e9 (ancestor) divergence-base
    ");

    let remote_one_out = repo.rev_parse_single("remote-one")?;
    insta::assert_snapshot!(visualize_tree(remote_one_out).to_string(), @r#"
    2bb7f47
    └── file.txt:100644:79132d8 "1\nremote-1\n2\n3\n4\n5\n6\n7\n8\n"
    "#);

    let remote_two_out = repo.rev_parse_single("remote-two")?;
    insta::assert_snapshot!(visualize_tree(remote_two_out).to_string(), @r#"
    75911e9
    └── file.txt:100644:c4f6b31 "1\nremote-1\n2\n3\nremote-2\n4\n5\n6\n7\n8\nlocal-end\n"
    "#);

    Ok(())
}

/// Three-member remote family with two local commits. Earlier members claim
/// overlapping hunks first; the remainder carries forward to the final member.
///
/// ```text
/// file.txt at ancestor:
///   1
///   2
///   3
///   4
///   5
///   6
///   7
///   8
///
/// local-top (inserts after line 2):   local-bottom (appends after line 8):
///   1                                   1
///   2                                   2
///   local-top                           local-top
///   3                                   3
///   4                                   4
///   5                                   5
///   6                                   6
///   7                                   7
///   8                                   8
///                                       local-bottom
///
/// remote-1 (inserts after line 2):
///   1
///   2
///   remote-1
///   3  ...
///
/// remote-2 (inserts after line 4):
///   1
///   2
///   remote-1
///   3
///   4
///   remote-2
///   5  ...
///
/// remote-3: adds extra.txt = "remote-extra\n" (no file.txt change)
///
/// Graph before:
///   * local-bottom          (local-bottom, local)
///   * local-top             (local-top)
///   | * remote-3            (HEAD -> main, remote-three)
///   | * remote-2            (remote-two)
///   | * remote-1            (remote-one)
///   |/
///   * divergence-base       (ancestor)
///
/// Expected after PickDivergent family (all 3 remotes, locals=[local-top, local-bottom]):
///   - Graph shape preserved: three remote commits in sequence.
///   - remote-1 overlaps local-top (both insert at line 2) → conflicted commit.
///   - remote-2 has no remaining local overlap → clean commit.
///   - local-bottom carries forward to remote-3 (final member) → merged in.
///   - Per-commit output trees:
///       remote-1: conflicted (remote-1 vs local-top at line 2)
///       remote-2: file.txt = "1\n2\nremote-1\n3\n4\nremote-2\n5\n6\n7\n8\n"
///       remote-3: file.txt = "1\n2\nremote-1\n3\n4\nremote-2\n5\n6\n7\n8\nlocal-bottom\n"
///                 extra.txt = "remote-extra\n"
/// ```
#[test]
fn multi_member_carry_forward_preserves_remote_family_shape() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("pick-divergent-multi-member-carry-forward")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 39e1d31 (local-bottom, local) local-bottom
    * aa816ab (local-top) local-top
    | * a36fb98 (HEAD -> main, remote-three) remote-3
    | * 98c0547 (remote-two) remote-2
    | * 10ab6f8 (remote-one) remote-1
    |/  
    * 62402e9 (ancestor) divergence-base
    ");

    let ancestor = repo.rev_parse_single("ancestor")?.detach();
    let local_top = repo.rev_parse_single("local-top")?.detach();
    let local_bottom = repo.rev_parse_single("local-bottom")?.detach();
    let remote_one = repo.rev_parse_single("remote-one")?.detach();
    let remote_two = repo.rev_parse_single("remote-two")?.detach();
    let remote_three = repo.rev_parse_single("remote-three")?.detach();

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let family_id = [0x22; 20];
    editor.replace(
        editor.select_commit(remote_one)?,
        Step::new_pick_divergent(
            vec![local_top, local_bottom],
            Some(ancestor),
            remote_one,
            family_id,
        ),
    )?;
    editor.replace(
        editor.select_commit(remote_two)?,
        Step::new_pick_divergent(
            vec![local_top, local_bottom],
            Some(ancestor),
            remote_two,
            family_id,
        ),
    )?;
    editor.replace(
        editor.select_commit(remote_three)?,
        Step::new_pick_divergent(
            vec![local_top, local_bottom],
            Some(ancestor),
            remote_three,
            family_id,
        ),
    )?;

    let outcome = editor.rebase()?;
    let _outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 39e1d31 (local-bottom, local) local-bottom
    * aa816ab (local-top) local-top
    | * d52b0f1 (HEAD -> main, remote-three) remote-3
    | * 5b5bc58 (remote-two) remote-2
    | * 027d16b (remote-one) remote-1
    |/  
    * 62402e9 (ancestor) divergence-base
    ");

    let remote_one_out = repo.rev_parse_single("remote-one")?;
    insta::assert_snapshot!(visualize_tree(remote_one_out).to_string(), @r#"
    5e960c0
    ├── .auto-resolution:34a3ba0 
    │   └── file.txt:100644:0616b2f "1\n2\nremote-1\n3\n4\n5\n6\n7\n8\n"
    ├── .conflict-base-0:83d31c1 
    │   └── file.txt:100644:535d2b0 "1\n2\n3\n4\n5\n6\n7\n8\n"
    ├── .conflict-files:100644:2ea32f0 "ancestorEntries = [\"file.txt\"]\nourEntries = [\"file.txt\"]\ntheirEntries = [\"file.txt\"]\n"
    ├── .conflict-side-0:34a3ba0 
    │   └── file.txt:100644:0616b2f "1\n2\nremote-1\n3\n4\n5\n6\n7\n8\n"
    ├── .conflict-side-1:3d2fb31 
    │   └── file.txt:100644:81b972a "1\n2\nlocal-top\n3\n4\n5\n6\n7\n8\n"
    ├── CONFLICT-README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    └── file.txt:100644:0616b2f "1\n2\nremote-1\n3\n4\n5\n6\n7\n8\n"
    "#);

    let remote_two_out = repo.rev_parse_single("remote-two")?;
    insta::assert_snapshot!(visualize_tree(remote_two_out).to_string(), @r#"
    871cb84
    └── file.txt:100644:e2b4d17 "1\n2\nremote-1\n3\n4\nremote-2\n5\n6\n7\n8\n"
    "#);

    let remote_three_out = repo.rev_parse_single("remote-three")?;
    insta::assert_snapshot!(visualize_tree(remote_three_out).to_string(), @r#"
    1d9458c
    ├── extra.txt:100644:ff56302 "remote-extra\n"
    └── file.txt:100644:6c65f68 "1\n2\nremote-1\n3\n4\nremote-2\n5\n6\n7\n8\nlocal-bottom\n"
    "#);

    Ok(())
}

/// When `ancestor` is `None`, the output parent is used as the merge base
/// instead. The merge should still combine non-overlapping local and remote
/// changes correctly.
///
/// ```text
/// divergent.txt at shared-base (onto):
///   shared-1
///   shared-2
///   shared-3
///
/// local (rewrites line 3):        remote (rewrites line 2):
///   shared-1                        shared-1
///   shared-2                        remote-2
///   local-3                         shared-3
///
/// Graph before:
///   * local-change-line-three    (local)
///   | * remote-change-line-two   (HEAD -> main, remote)
///   |/
///   * shared-base                (onto)
///
/// Expected after PickDivergent(remote, local=[local], ancestor=None):
///   - No explicit ancestor, so the output parent (onto) is the merge base.
///   - Output tree merges both rewrites:
///       divergent.txt = "shared-1\nremote-2\nlocal-3\n"
/// ```
#[test]
fn no_ancestor_falls_back_to_output_parent_as_merge_base() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("pick-divergent-no-ancestor-fallback")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 6442b34 (local) local-change-line-three
    | * 44b08a5 (HEAD -> main, remote) remote-change-line-two
    |/  
    * 642085f (onto) shared-base
    ");

    let local = repo.rev_parse_single("local")?.detach();
    let remote = repo.rev_parse_single("remote")?.detach();

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    editor.replace(
        editor.select_commit(remote)?,
        Step::new_pick_divergent(vec![local], None, remote, [0x33; 20]),
    )?;

    let outcome = editor.rebase()?;
    let _outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 6442b34 (local) local-change-line-three
    | * 35304df (HEAD -> main, remote) remote-change-line-two
    |/  
    * 642085f (onto) shared-base
    ");

    let head = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(visualize_tree(head).to_string(), @r#"
    79bf5b2
    └── divergent.txt:100644:4a5c370 "shared-1\nremote-2\nlocal-3\n"
    "#);

    Ok(())
}

/// When local and remote both rewrite the same line to different values,
/// the result should be a conflicted commit preserving the remote position.
///
/// ```text
/// divergent.txt at ancestor:
///   shared-1
///   shared-2
///   shared-3
///
/// local (rewrites line 2):        remote (rewrites line 2):
///   shared-1                        shared-1
///   local-2                         remote-2
///   shared-3                        shared-3
///
/// Graph before:
///   * local-change-middle-line   (local)
///   | * remote-change-middle-line (HEAD -> main, remote)
///   |/
///   * divergence-base            (ancestor)
///
/// Expected after PickDivergent(remote, local=[local], ancestor=ancestor):
///   - Remote commit kept in place, graph shape unchanged.
///   - Output commit is conflicted (both sides rewrite line 2).
///   - conflict-side-0 (remote): "shared-1\nremote-2\nshared-3\n"
///   - conflict-side-1 (local):  "shared-1\nlocal-2\nshared-3\n"
///   - auto-resolution favors remote: "shared-1\nremote-2\nshared-3\n"
/// ```
#[test]
fn single_member_conflicted_rewrite_materializes_conflicted_remote_commit() -> Result<()> {
    let (repo, _tmpdir, mut meta) =
        fixture_writable("pick-divergent-single-member-conflicted-rewrite")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 5c7ceac (local) local-change-middle-line
    | * 1c1a8ed (HEAD -> main, remote) remote-change-middle-line
    |/  
    * cb3180d (ancestor) divergence-base
    ");

    let ancestor = repo.rev_parse_single("ancestor")?.detach();
    let local = repo.rev_parse_single("local")?.detach();
    let remote = repo.rev_parse_single("remote")?.detach();

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    editor.replace(
        editor.select_commit(remote)?,
        Step::new_pick_divergent(vec![local], Some(ancestor), remote, [0x44; 20]),
    )?;

    let outcome = editor.rebase()?;
    let _outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 5c7ceac (local) local-change-middle-line
    | * 650eec9 (HEAD -> main, remote) remote-change-middle-line
    |/  
    * cb3180d (ancestor) divergence-base
    ");

    let head = repo.rev_parse_single("HEAD")?;
    assert!(but_core::Commit::from_id(head.detach().attach(&repo))?.is_conflicted());

    insta::assert_snapshot!(visualize_tree(head).to_string(), @r#"
    d1bd82d
    ├── .auto-resolution:1cfe446 
    │   └── divergent.txt:100644:18807f6 "shared-1\nremote-2\nshared-3\n"
    ├── .conflict-base-0:0372008 
    │   └── divergent.txt:100644:306f78e "shared-1\nshared-2\nshared-3\n"
    ├── .conflict-files:100644:60e6e3f "ancestorEntries = [\"divergent.txt\"]\nourEntries = [\"divergent.txt\"]\ntheirEntries = [\"divergent.txt\"]\n"
    ├── .conflict-side-0:1cfe446 
    │   └── divergent.txt:100644:18807f6 "shared-1\nremote-2\nshared-3\n"
    ├── .conflict-side-1:6088436 
    │   └── divergent.txt:100644:bd896ff "shared-1\nlocal-2\nshared-3\n"
    ├── CONFLICT-README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    └── divergent.txt:100644:18807f6 "shared-1\nremote-2\nshared-3\n"
    "#);

    Ok(())
}
