use bstr::ByteSlice;
use but_core::branch::{find_unique_refname, find_unique_refname_with_remote_check};
use but_testsupport::writable_scenario;
use gix::refs::{self, Category, transaction::PreviousValue};

#[test]
fn with_existing_numerical_suffix() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");

    let id = repo.object_hash().null();
    let branch_1 = refs::Category::LocalBranch.to_full_name("branch-1".as_bytes().as_bstr())?;
    repo.reference(branch_1.as_ref(), id, PreviousValue::Any, "test")?;
    let branch_2 = refs::Category::LocalBranch.to_full_name("branch-2".as_bytes().as_bstr())?;
    repo.reference(branch_2.as_ref(), id, PreviousValue::Any, "test")?;

    let unique = find_unique_refname(&repo, branch_1.as_ref())?;
    assert_eq!(unique.category(), Some(Category::LocalBranch));
    assert_eq!(unique.shorten(), "branch-3", "it increments 1 till 3");

    let unique = find_unique_refname(&repo, branch_2.as_ref())?;
    assert_eq!(unique.category(), Some(Category::LocalBranch));
    assert_eq!(unique.shorten(), "branch-3", "it increments 2 till 3 as well");

    Ok(())
}

#[test]
fn without_numerical_suffix_it_appends_one() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");

    let id = repo.object_hash().null();

    // Create a branch named "feature"
    let feature = refs::Category::Note.to_full_name("feature".as_bytes().as_bstr())?;
    repo.reference(feature.as_ref(), id, PreviousValue::Any, "test")?;

    let unique = find_unique_refname(&repo, feature.as_ref())?;
    assert_eq!(unique.category(), Some(Category::Note));
    assert_eq!(
        unique.shorten(),
        "notes/feature-1",
        "it starts appending suffixes for uniqueness"
    );

    Ok(())
}

#[test]
fn returns_original_if_unique() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");

    let branch_1 = refs::Category::LocalBranch.to_full_name("branch-1".as_bytes().as_bstr())?;
    let unique = find_unique_refname(&repo, branch_1.as_ref())?;
    assert_eq!(unique, branch_1);

    Ok(())
}

mod with_remote_check {
    use super::*;

    #[test]
    fn avoids_remote_tracking_branches() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario("unborn-empty");

        let id = repo.object_hash().null();

        // Create a remote tracking branch at refs/remotes/origin/feature-1
        let remote_branch = gix::refs::FullName::try_from("refs/remotes/origin/feature-1".to_string())?;
        repo.reference(remote_branch.as_ref(), id, PreviousValue::Any, "test")?;

        // Now try to get a unique name for "feature-1" - it should skip to feature-2
        // because feature-1 exists on the remote
        let template = refs::Category::LocalBranch.to_full_name("feature-1".as_bytes().as_bstr())?;
        let unique = find_unique_refname_with_remote_check(&repo, template.as_ref(), "origin")?;
        assert_eq!(unique.category(), Some(Category::LocalBranch));
        assert_eq!(
            unique.shorten(),
            "feature-2",
            "it skips feature-1 because it exists on the remote"
        );

        Ok(())
    }

    #[test]
    fn returns_original_if_not_on_remote() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario("unborn-empty");

        let template = refs::Category::LocalBranch.to_full_name("feature-1".as_bytes().as_bstr())?;
        let unique = find_unique_refname_with_remote_check(&repo, template.as_ref(), "origin")?;
        assert_eq!(
            unique, template,
            "returns original when neither local nor remote exists"
        );

        Ok(())
    }

    #[test]
    fn avoids_both_local_and_remote() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario("unborn-empty");

        let id = repo.object_hash().null();

        // Create local branch feature-1
        let local_branch = refs::Category::LocalBranch.to_full_name("feature-1".as_bytes().as_bstr())?;
        repo.reference(local_branch.as_ref(), id, PreviousValue::Any, "test")?;

        // Create remote tracking branch feature-2
        let remote_branch = gix::refs::FullName::try_from("refs/remotes/origin/feature-2".to_string())?;
        repo.reference(remote_branch.as_ref(), id, PreviousValue::Any, "test")?;

        // Now try to get a unique name for "feature-1" - it should skip to feature-3
        // because feature-1 exists locally and feature-2 exists on the remote
        let template = refs::Category::LocalBranch.to_full_name("feature-1".as_bytes().as_bstr())?;
        let unique = find_unique_refname_with_remote_check(&repo, template.as_ref(), "origin")?;
        assert_eq!(unique.category(), Some(Category::LocalBranch));
        assert_eq!(
            unique.shorten(),
            "feature-3",
            "it skips feature-1 (local) and feature-2 (remote)"
        );

        Ok(())
    }

    #[test]
    fn different_remote_does_not_conflict() -> anyhow::Result<()> {
        let (repo, _tmp) = writable_scenario("unborn-empty");

        let id = repo.object_hash().null();

        // Create a remote tracking branch on a different remote (upstream, not origin)
        let remote_branch = gix::refs::FullName::try_from("refs/remotes/upstream/feature-1".to_string())?;
        repo.reference(remote_branch.as_ref(), id, PreviousValue::Any, "test")?;

        // Looking at "origin" remote, feature-1 should be available
        let template = refs::Category::LocalBranch.to_full_name("feature-1".as_bytes().as_bstr())?;
        let unique = find_unique_refname_with_remote_check(&repo, template.as_ref(), "origin")?;
        assert_eq!(unique, template, "refs on other remotes don't conflict");

        Ok(())
    }
}
