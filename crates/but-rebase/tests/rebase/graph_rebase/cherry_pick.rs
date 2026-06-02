use anyhow::{Result, bail};
use bstr::ByteSlice;
use but_core::commit::SignCommit;
use but_rebase::graph_rebase::cherry_pick::{
    CherryPickOutcome, PickMode, TreeMergeMode, cherry_pick,
};
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(5a8f27d64a93b97e42b375c14540acebba2b1d09),
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(c36dcfcf8bab780dcacc0bc43e8a31af2a1e7703),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    1090f8a
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
    let result = cherry_pick(
        &repo,
        target.detach(),
        &parents,
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

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

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(26eb34db737ae1cee6887cd1ab9a48c72c3d9c04),
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(333dd65cbcc689b08744e30717faa343f88b0554),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    4c6dc70
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

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

#[test]
fn synthetic_empty_merge_template_with_conflicting_new_parents_materializes_conflicted_merge_commit()
-> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let template_source = repo.rev_parse_single("base")?.object()?.peel_to_commit()?;
    let mut template_commit: gix::objs::Commit = template_source.decode()?.try_into()?;
    template_commit.tree = gix::ObjectId::empty_tree(repo.object_hash());
    template_commit.parents.clear();
    template_commit.message = b"synthetic merge template".into();
    let target = repo.write_object(template_commit)?.detach();

    let onto = repo.rev_parse_single("single-target")?.detach();
    let onto2 = repo.rev_parse_single("second-conflicting-target")?.detach();

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(009287dffe7bc486ed6e08ce70ac55f291b15354),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("synthetic merge template should materialize a conflicted merge commit");
    };

    assert_eq!(
        &get_parents(&id.attach(&repo))?,
        &[onto, onto2],
        "synthetic merge template should keep the conflicting new parents",
    );

    let commit = id.attach(&repo).object()?.peel_to_commit()?;
    insta::assert_snapshot!(commit.message_raw()?.as_bstr(), @r#"
    [conflict] synthetic merge template

    GitButler-Conflict: This is a GitButler-managed conflicted commit. Files are auto-resolved
       using the "ours" side. The commit tree contains additional directories:
         .conflict-side-0  — our tree
         .conflict-side-1  — their tree
         .conflict-base-0  — the merge base tree
         .auto-resolution  — the auto-resolved tree
         .conflict-files   — metadata about conflicted files
       To manually resolve, check out this commit, remove the directories
       listed above, resolve the conflicts, and amend the commit.
    "#);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    7e70098
    ├── .auto-resolution:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-base-0:964aa4d 
    │   └── base-f:100644:7898192 "a\n"
    ├── .conflict-files:100644:68fb397 "ancestorEntries = []\nourEntries = [\"target-f\"]\ntheirEntries = [\"target-f\"]\n"
    ├── .conflict-side-0:aa3d213 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:eb5a316 "target\n"
    ├── .conflict-side-1:e6ffce2 
    │   ├── base-f:100644:7898192 "a\n"
    │   └── target-f:100644:caac8f9 "target 2\n"
    ├── base-f:100644:7898192 "a\n"
    └── target-f:100644:eb5a316 "target\n"
    "#);

    Ok(())
}

#[test]
fn synthetic_empty_merge_template_with_unrelated_new_parents_uses_empty_base_tree() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("disjoint-orphan-branches")?;

    let template_source = repo.rev_parse_single("main")?.object()?.peel_to_commit()?;
    let mut template_commit: gix::objs::Commit = template_source.decode()?.try_into()?;
    template_commit.tree = gix::ObjectId::empty_tree(repo.object_hash());
    template_commit.parents.clear();
    template_commit.message = b"synthetic unrelated merge template".into();
    let target = repo.write_object(template_commit)?.detach();

    let onto = repo.rev_parse_single("main")?.detach();
    let onto2 = repo.rev_parse_single("orphan")?.detach();

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(8deb9d849412560636bc9dce3a82392324467c0b),
    )
    ");

    let CherryPickOutcome::Commit(id) = result else {
        bail!("unrelated synthetic merge template should merge from the empty tree base");
    };

    assert_eq!(
        &get_parents(&id.attach(&repo))?,
        &[onto, onto2],
        "unrelated synthetic merge template should keep both new parents",
    );

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    9418a9b
    ├── base:100644:df967b9 "base\n"
    ├── main-1:100644:114be18 "main-1\n"
    ├── orphan-base:100644:0b12d02 "orphan-base\n"
    └── orphan-tip:100644:1647ebd "orphan-tip\n"
    "#);

    Ok(())
}

// multiple parent to single parent - clean
#[test]
fn multiple_parents_to_single_parent_clean() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick")?;

    let target = repo.rev_parse_single("merge-clean-commit")?.detach();
    let onto = repo.rev_parse_single("single-target")?.detach();

    let result = cherry_pick(
        &repo,
        target,
        &[onto],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(52a5e75c1e032898649d840c92a4e0a0ef03a60b),
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(3621c40c48ee697ce42ace960055702479ca5267),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    8c4acd1
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

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

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(e1211f8c8ed78875cd0c231a3d9e51cff51186f9),
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(718d834a2a57256c4a54706573a5d9863d54a653),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    1620d95
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

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

    let result = cherry_pick(
        &repo,
        target,
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

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

    let result = cherry_pick(
        &repo,
        target.detach(),
        &parents,
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

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

    let result = cherry_pick(
        &repo,
        target.detach(),
        &[],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

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

    let result = cherry_pick(
        &repo,
        target,
        &[],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(768bf9f3dd8f1e6c5d24af762e87bba00a87cd5c),
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(288077e946039664793caf577166f21a82f70830),
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

    let result = cherry_pick(
        &repo,
        target,
        &[onto],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(46139f9346b201bf8cbff2112adecd3c99a2a956),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto]);

    insta::assert_snapshot!(visualize_tree(id.attach(&repo)), @r#"
    38edd44
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

    let result = cherry_pick(
        &repo,
        target.detach(),
        &[onto, onto2],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    ConflictedCommit(
        Sha1(718d834a2a57256c4a54706573a5d9863d54a653),
    )
    ");

    let CherryPickOutcome::ConflictedCommit(id) = result else {
        bail!("impossible");
    };

    assert_eq!(&get_parents(&id.attach(&repo))?, &[onto, onto2]);

    let result = cherry_pick(
        &repo,
        id,
        &parents,
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(56ee45903fbd7570c857b02d4c5487e718564e72),
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

    let result = cherry_pick(
        &repo,
        target.detach(),
        &[onto, onto2, onto3],
        PickMode::IfChanged,
        TreeMergeMode::WithRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    insta::assert_debug_snapshot!(result, @"
    Commit(
        Sha1(b127ce2b2cf3e3c5bf2527a6082b565df4f8ab65),
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

/// Workspace merges surface delete-vs-modify conflicts instead of hiding
/// them behind false-positive renames.
///
/// Scenario: two stacks share a common base with file-a.txt and file-b.txt.
/// - Stack 1 modifies file-b.txt
/// - Stack 2 deletes both files and adds file-combined.txt (similar content to file-b.txt)
///
/// With rename detection enabled, gix would match file-b.txt to
/// file-combined.txt as a rename, hiding the real delete-vs-modify conflict
/// and silently dropping file-a.txt's deletion. With `TreeMergeMode::WithoutRenames`
/// (which disables rename detection), the conflict is correctly surfaced.
#[test]
fn workspace_merge_surfaces_delete_vs_modify_conflict() -> Result<()> {
    let (repo, _tmpdir, _meta) = fixture_writable("cherry-pick-rename-detection")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8ef051f (stack-2-after) stack-2-after: combine files
    | *   978e614 (HEAD -> workspace-before) GitButler Workspace Commit
    | |\  
    | | * b8f64ac (stack-2-before) stack-2-before: unrelated change
    | |/  
    |/|   
    | * f02613a (stack-1) stack-1: modify file-b
    |/  
    * 57993f6 (main, base) base
    ");

    let workspace = repo.rev_parse_single("workspace-before")?.detach();
    let stack1 = repo.rev_parse_single("stack-1")?.detach();
    let stack2_after = repo.rev_parse_single("stack-2-after")?.detach();

    // The workspace commit currently has parents [stack-1, stack-2-before].
    // We want to rebase it onto [stack-1, stack-2-after] where stack-2-after
    // deletes file-a.txt and file-b.txt, replacing them with file-combined.txt.
    //
    // Stack-1 modifies file-b.txt while stack-2-after deletes it — this is a
    // genuine cross-stack conflict. Previously, rename detection hid this conflict
    // by treating file-b → file-combined as a rename, producing a wrong tree
    // where file-a.txt's deletion was silently dropped.
    //
    // With rename detection disabled, this correctly reports a conflict.
    let result = cherry_pick(
        &repo,
        workspace,
        &[stack1, stack2_after],
        PickMode::Force,
        TreeMergeMode::WithoutRenames,
        SignCommit::IfSignCommitsEnabled,
    )?;

    assert!(
        matches!(result, CherryPickOutcome::FailedToMergeBases { .. }),
        "Expected a conflict due to delete-vs-modify on file-b.txt, got: {result:?}"
    );

    Ok(())
}
