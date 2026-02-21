use anyhow::{Result, bail};
use but_rebase::graph_rebase::cherry_pick::{CherryPickOutcome, cherry_pick};
use but_testsupport::{visualize_commit_graph_all, visualize_tree};
use gix::prelude::ObjectIdExt;

use crate::utils::fixture_writable;

fn get_parents(id: &gix::Id) -> Result<Vec<gix::ObjectId>> {
    Ok(id
        .object()?
        .peel_to_commit()?
        .parent_ids()
        .map(|i| i.detach())
        .collect())
}

#[test]
fn basic_cherry_pick_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("single-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(023c575a8c22020139844490ba2e8f333fcec85c),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    96a9057
    ├── base-f:100644:7898192 "a\n"
    ├── clean-commit-f:100644:20a3acd "clean-commit\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}
// Basic cherry pick - conflicting
#[test]
fn basic_cherry_pick_cp_conflicts() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("single-conflicting-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(e9ee7b59aff786fc970c30f6965d1de1913c7ec4),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    0367fb7
    ├── .auto-resolution:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-base-0:45eb973 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── clean-f:100644:8312630 "clean\n"
    ├── .conflict-files:100644:68fb397 "ancestorEntries = []\nourEntries = [\"target-f\"]\ntheirEntries = [\"target-f\"]\n"
    ├── .conflict-side-0:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-side-1:c1a7ba6 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── clean-f:100644:8312630 "clean\n"
    │   └── target-f:100644:9b1719f "conflict\n"
    ├── CONFLICT-README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    ├── base-f:100644:7898192 "a\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}
// Basic cherry pick - identity
#[test]
fn basic_cherry_pick_identity() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("single-conflicting-commit")?;
    let parents = get_parents(&target)?;
    let result = cherry_pick(&repo, target.detach(), &parents, true)?;

    insta::assert_debug_snapshot!(result, @"
    Identity(
        Sha1(b23d933c3781f649b740445e5337362d74b9103e),
    )
    ");

    Ok(())
}
// single parent to multiple parents - clean... this one is SFW
#[test]
fn single_parent_to_multiple_parents_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("single-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(39763a0f3cb7ca0f3eac78368173aa6367aeffcf),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    2d609c4
    ├── base-f:100644:7898192 "a\n"
    ├── clean-commit-f:100644:20a3acd "clean-commit\n"
    ├── target-2-f:100644:caac8f9 "target 2\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

// single parent to multiple parents - cp conflicts
#[test]
fn single_parent_to_multiple_parents_cp_conflicts() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("single-conflicting-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(0fcbe01202743fa55f1a1e07342ad26f2e7a0abe),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    1804f3d
    ├── .auto-resolution:744efa9 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── target-2-f:100644:caac8f9 "target 2\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-base-0:45eb973 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── clean-f:100644:8312630 "clean\n"
    ├── .conflict-files:100644:68fb397 "ancestorEntries = []\nourEntries = [\"target-f\"]\ntheirEntries = [\"target-f\"]\n"
    ├── .conflict-side-0:744efa9 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── target-2-f:100644:caac8f9 "target 2\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-side-1:c1a7ba6 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── clean-f:100644:8312630 "clean\n"
    │   └── target-f:100644:9b1719f "conflict\n"
    ├── CONFLICT-README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    ├── base-f:100644:7898192 "a\n"
    ├── target-2-f:100644:caac8f9 "target 2\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

// single parent to multiple parents - parents conflict
#[test]
fn single_parent_to_multiple_parents_parents_conflict() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("single-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-conflicting-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    FailedToMergeBases {
        base_merge_failed: false,
        bases: None,
        onto_merge_failed: true,
        ontos: Some(
            [
                Sha1(cc8998caa25bc039884eb893ec89b4880c6bd232),
                Sha1(16cfd2c3707e064337a80ba66fc0f6d2171d6ddb),
            ],
        ),
    }
    ");

    Ok(())
}

// multiple parent to single parent - clean
#[test]
fn multiple_parents_to_single_parent_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(1327f60f892048e2dd2c96c639e6b6aa750bdbe3),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    96a9057
    ├── base-f:100644:7898192 "a\n"
    ├── clean-commit-f:100644:20a3acd "clean-commit\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

// multiple parent to single parent - cp conflicts
#[test]
fn multiple_parents_to_single_parent_cp_conflicts() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-conflicting-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(28fa7c91af8652f4e69c1e3184f92569a3468a34),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    91fe014
    ├── .auto-resolution:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-base-0:4acd705 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── clean-2-f:100644:13e9394 "clean 2\n"
    │   └── clean-f:100644:8312630 "clean\n"
    ├── .conflict-files:100644:68fb397 "ancestorEntries = []\nourEntries = [\"target-f\"]\ntheirEntries = [\"target-f\"]\n"
    ├── .conflict-side-0:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-side-1:09af0e9 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── clean-2-f:100644:13e9394 "clean 2\n"
    │   ├── clean-f:100644:8312630 "clean\n"
    │   └── target-f:100644:9b1719f "conflict\n"
    ├── CONFLICT-README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    ├── base-f:100644:7898192 "a\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

// multiple parent to single parent - parents conflict
#[test]
fn multiple_parents_to_single_parent_parents_conflict() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo
        .rev_parse_single("merge-clean-commit-conflicting-parents")?
        .detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @"
    FailedToMergeBases {
        base_merge_failed: true,
        bases: Some(
            [
                Sha1(5183ac6942d0fc2fc0cca84e1a8ad06370f2952c),
                Sha1(a3e84fbc36af1f7f48f9b8e3c61f71db360d6b7c),
            ],
        ),
        onto_merge_failed: false,
        ontos: None,
    }
    ");

    Ok(())
}

// multiple parents to multiple parents - clean
#[test]
fn multiple_parents_to_multiple_parents_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(819265d9df0efd0ecba56b9f930f16eea335d329),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    2d609c4
    ├── base-f:100644:7898192 "a\n"
    ├── clean-commit-f:100644:20a3acd "clean-commit\n"
    ├── target-2-f:100644:caac8f9 "target 2\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

// multiple parents to multiple parents - cp conflicts
#[test]
fn multiple_parents_to_multiple_parents_cp_conflicts() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-conflicting-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(2e6cb06fe98780bb8c7a301a522edd98805d1499),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    0aeaf79
    ├── .auto-resolution:744efa9 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── target-2-f:100644:caac8f9 "target 2\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-base-0:4acd705 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── clean-2-f:100644:13e9394 "clean 2\n"
    │   └── clean-f:100644:8312630 "clean\n"
    ├── .conflict-files:100644:68fb397 "ancestorEntries = []\nourEntries = [\"target-f\"]\ntheirEntries = [\"target-f\"]\n"
    ├── .conflict-side-0:744efa9 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── target-2-f:100644:caac8f9 "target 2\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-side-1:09af0e9 
    │   ├── base-f:100644:7898192 "a\n"
    │   ├── clean-2-f:100644:13e9394 "clean 2\n"
    │   ├── clean-f:100644:8312630 "clean\n"
    │   └── target-f:100644:9b1719f "conflict\n"
    ├── CONFLICT-README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    ├── base-f:100644:7898192 "a\n"
    ├── target-2-f:100644:caac8f9 "target 2\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

// multiple parents to multiple parents - parents conflict
#[test]
fn multiple_parents_to_multiple_parents_base_parents_conflict() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo
        .rev_parse_single("merge-clean-commit-conflicting-parents")?
        .detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    FailedToMergeBases {
        base_merge_failed: true,
        bases: Some(
            [
                Sha1(5183ac6942d0fc2fc0cca84e1a8ad06370f2952c),
                Sha1(a3e84fbc36af1f7f48f9b8e3c61f71db360d6b7c),
            ],
        ),
        onto_merge_failed: false,
        ontos: None,
    }
    ");

    Ok(())
}

#[test]
fn multiple_parents_to_multiple_parents_target_parents_conflict() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-conflicting-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    FailedToMergeBases {
        base_merge_failed: false,
        bases: None,
        onto_merge_failed: true,
        ontos: Some(
            [
                Sha1(cc8998caa25bc039884eb893ec89b4880c6bd232),
                Sha1(16cfd2c3707e064337a80ba66fc0f6d2171d6ddb),
            ],
        ),
    }
    ");

    Ok(())
}

// multiple parents to multiple parents - identity
#[test]
fn multiple_parents_to_multiple_parents_identity() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?;
    let parents = get_parents(&target)?;

    let result = cherry_pick(&repo, target.detach(), &parents, true)?;

    insta::assert_debug_snapshot!(result, @"
    Identity(
        Sha1(bec85a3ab113b86032660cac3d09afb4d342e135),
    )
    ");

    Ok(())
}

// no parents cherry pick - is identity
#[test]
fn no_parents_identity() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("base")?;

    let result = cherry_pick(&repo, target.detach(), &[], true)?;

    insta::assert_debug_snapshot!(result, @"
    Identity(
        Sha1(7a749663ddce268238da073e025f30a281120ef5),
    )
    ");

    Ok(())
}

// single parent to no parents - clean
#[test]
fn single_parent_to_no_parents_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("single-clean-commit")?.detach();

    let result = cherry_pick(&repo, target, &[], true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(756d7a456e069d4553d52d339158135390d3780e),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    assert!(&get_parents(&id.attach(&repo))?.is_empty());

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    3b64efb
    └── clean-commit-f:100644:20a3acd "clean-commit\n"
    "#);

    Ok(())
}

// no parents to single parent - clean
#[test]
fn no_parents_to_single_parent_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("base")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(f3555f184de4e805e12a6ee83e406f3a39eb2091),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    aa3d213
    ├── base-f:100644:7898192 "a\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

// no parents to single parent - cp conflicts
#[test]
fn no_parents_to_single_parent_cp_conflicts() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("base-conflicting")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(28f862257bff139659b763a2c873b8d3f0f780b0),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    1267a55
    ├── .auto-resolution:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-base-0:4b825dc 
    ├── .conflict-files:100644:68fb397 "ancestorEntries = []\nourEntries = [\"target-f\"]\ntheirEntries = [\"target-f\"]\n"
    ├── .conflict-side-0:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-side-1:144e5f5 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:9b1719f "conflict\n"
    ├── CONFLICT-README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
    ├── base-f:100644:7898192 "a\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

#[test]
fn cherry_pick_back_to_original_parents_unconflicts() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-conflicting-commit")?;
    let parents = get_parents(&target)?;
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-target")?.detach();

    let result = cherry_pick(&repo, target.detach(), &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(2e6cb06fe98780bb8c7a301a522edd98805d1499),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    let result = cherry_pick(&repo, id, &parents, true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(3d7dfa09a071658d3b84eb1ee195ea0ebfeb601f),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    09af0e9
    ├── base-f:100644:7898192 "a\n"
    ├── clean-2-f:100644:13e9394 "clean 2\n"
    ├── clean-f:100644:8312630 "clean\n"
    └── target-f:100644:9b1719f "conflict\n"
    "#);

    Ok(())
}

#[test]
fn cherry_pick_recursive_merge() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick-recursive-merge")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * aae265d (first-parent) first-parent
    | * 7d9539e (second-parent) second-parent
    |/  
    * 486af94 (b) b
    | * 7ae309b (third-parent) third-parent
    |/  
    | * bdbf7b1 (HEAD -> to-pick) to-pick
    |/  
    * 3ba97d6 (a) a
    * dd35aa7 (base) base
    ");

    let target = repo.rev_parse_single("to-pick")?;
    let onto = repo.rev_parse_single("first-parent")?.detach();
    let onto2 = repo.rev_parse_single("second-parent")?.detach();
    let onto3 = repo.rev_parse_single("third-parent")?.detach();

    let result = cherry_pick(&repo, target.detach(), &[onto, onto2, onto3], true)?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(cd1d00c1d637d5567f7a0739d1aa9ca3e65b990e),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2, onto3]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    4f825d9
    ├── base-f:100644:718e7e9 "a\nx\nc\nd\n"
    └── foo-f:100644:2d07937 "1\nx\n2\n"
    "#);

    Ok(())
}
