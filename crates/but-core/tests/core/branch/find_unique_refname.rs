use bstr::ByteSlice;
use but_core::branch::find_unique_refname;
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
    assert_eq!(
        unique.shorten(),
        "branch-3",
        "it increments 2 till 3 as well"
    );

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
