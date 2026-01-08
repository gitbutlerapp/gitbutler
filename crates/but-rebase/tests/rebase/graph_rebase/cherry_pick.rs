use anyhow::{Result, bail};
use but_rebase::graph_rebase::cherry_pick::{CherryPickOutcome, cherry_pick};
use but_testsupport::visualize_tree;
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

    insta::assert_debug_snapshot!(result, @r"
    Commit(
        Sha1(54848c290a08fe833a4cbc6a58f0fcaf3bbf6e08),
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

    insta::assert_debug_snapshot!(result, @r"
    ConflictedCommit(
        Sha1(8ae3b596cae568f23d89dab540bfc8e97d6beb70),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    3417b4c
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
    └── README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
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

    insta::assert_debug_snapshot!(result, @r"
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

    insta::assert_debug_snapshot!(result, @r"
    Commit(
        Sha1(042e97dd9f0cace4fd03a8876a0ec05da996f15f),
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

    insta::assert_debug_snapshot!(result, @r"
    ConflictedCommit(
        Sha1(65ebb926505ce670f16d179118f006b5e140aa44),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    75fdd2c
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
    └── README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
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

    insta::assert_debug_snapshot!(result, @"FailedToMergeBases");

    Ok(())
}

// multiple parent to single parent - clean
#[test]
fn multiple_parents_to_single_parent_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto], true)?;

    insta::assert_debug_snapshot!(result, @r"
    Commit(
        Sha1(f402cc4dedc672129e7265fa8d19e0b55e5c6918),
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

    insta::assert_debug_snapshot!(result, @r"
    ConflictedCommit(
        Sha1(b03f2d364398cad85e2a518a10d5acdde44ca96e),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    fde5970
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
    └── README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
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

    insta::assert_debug_snapshot!(result, @"FailedToMergeBases");

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

    insta::assert_debug_snapshot!(result, @r"
    Commit(
        Sha1(6de4f47ea11937579c6dab87cf53821a05fd171a),
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

    insta::assert_debug_snapshot!(result, @r"
    ConflictedCommit(
        Sha1(219e13bf40e6451d281759652bfd2c4c665ea717),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    acdd833
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
    └── README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
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

    insta::assert_debug_snapshot!(result, @"FailedToMergeBases");

    Ok(())
}

#[test]
fn multiple_parents_to_multiple_parents_target_parents_conflict() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-conflicting-target")?.detach();

    let result = cherry_pick(&repo, target, &[onto, onto2], true)?;

    insta::assert_debug_snapshot!(result, @"FailedToMergeBases");

    Ok(())
}

// multiple parents to multiple parents - identity
#[test]
fn multiple_parents_to_multiple_parents_identity() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?;
    let parents = get_parents(&target)?;

    let result = cherry_pick(&repo, target.detach(), &parents, true)?;

    insta::assert_debug_snapshot!(result, @r"
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

    insta::assert_debug_snapshot!(result, @r"
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

    insta::assert_debug_snapshot!(result, @r"
    Commit(
        Sha1(8349ba92f5c1de9187b7a581a26439d5c57b8181),
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

    insta::assert_debug_snapshot!(result, @r"
    Commit(
        Sha1(88ff3dede1088638d76d2fff43f175f4e9169f38),
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

    insta::assert_debug_snapshot!(result, @r"
    ConflictedCommit(
        Sha1(b92ddaf506b9ba53051a04403c15bd5cd2025024),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    d92cbc8
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
    └── README.txt:100644:2af04b7 "You have checked out a GitButler Conflicted commit. You probably didn\'t mean to do this."
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

    insta::assert_debug_snapshot!(result, @r"
    ConflictedCommit(
        Sha1(219e13bf40e6451d281759652bfd2c4c665ea717),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    let result = cherry_pick(&repo, id, &parents, true)?;

    insta::assert_debug_snapshot!(result, @r"
    Commit(
        Sha1(01d8bb8621c478c98db95e38b8c6ad5e5607d572),
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
