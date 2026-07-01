use bstr::ByteSlice;
use but_core::branch::find_unique_refname;
use but_testsupport::{
    read_only_in_memory_scenario, read_only_in_memory_scenario_named_with_post,
    visualize_commit_graph_all,
};
use gix::refs::{self, Category, transaction::PreviousValue};

#[test]
fn with_existing_numerical_suffix() -> anyhow::Result<()> {
    let (repo, (branch_1, branch_2)) =
        read_only_in_memory_scenario_named_with_post("unborn-empty", "", 1, |fixture| {
            let repo = but_testsupport::open_repo(fixture.path())?;

            let id = repo.object_hash().null();
            let branch_1 =
                refs::Category::LocalBranch.to_full_name("branch-1".as_bytes().as_bstr())?;
            repo.reference(branch_1.as_ref(), id, PreviousValue::Any, "test")?;
            let branch_2 =
                refs::Category::LocalBranch.to_full_name("branch-2".as_bytes().as_bstr())?;
            repo.reference(branch_2.as_ref(), id, PreviousValue::Any, "test")?;

            Ok((branch_1, branch_2))
        })?;

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
fn it_considers_remote_tracking_branches_even_if_unregistered() -> anyhow::Result<()> {
    // The problem is us picking up remote tracking branches for local tracking branches automatically,
    // even without Git configuration.
    // That way, a newly created branch can wrongly be associated with an old and stale remote tracking branch,
    // causing all kinds of funkiness.
    let (repo, _) =
        read_only_in_memory_scenario_named_with_post("unborn-empty", "", 1, |fixture| {
            let repo = but_testsupport::open_repo(fixture.path())?;

            // Create a remote tracking branch for 'a' in a non-existing remote.
            // It's a stale branch basically.
            let id = repo.object_hash().null();
            let rtb: &gix::refs::FullNameRef = "refs/remotes/non-existing-remote/a".try_into()?;
            repo.reference(rtb, id, PreviousValue::Any, "test")?;
            let rtb: &gix::refs::FullNameRef = "refs/remotes/non-existing-remote/a-1".try_into()?;
            repo.reference(rtb, id, PreviousValue::Any, "test")?;

            Ok(())
        })?;

    let unique = find_unique_refname(&repo, "refs/heads/a".try_into()?)?;
    assert_eq!(
        unique.as_bstr(),
        "refs/heads/a-2",
        "it looks through all remote tracking branches and avoids RBTs from matching, for now.
        This also works if the RTB is stray, and has no remote configured."
    );
    Ok(())
}

#[test]
fn registered_remotes_help_deal_with_slashed_remote_names() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("multiple-remotes-with-tracking-branches")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, 
        @"
    * 14f7926 (nested/remote-b/main, nested/remote-b/in-nested-remote-b, nested/remote-b/HEAD) init-c
    * 5efb67f (nested/remote/main, nested/remote/in-nested-remote, nested/remote/HEAD) init-b
    * 263500f (origin/normal-remote, origin/main, origin/HEAD) init-a
    ");

    let unique = find_unique_refname(&repo, "refs/heads/in-nested-remote".try_into()?)?;
    assert_eq!(
        unique.as_bstr(),
        "refs/heads/in-nested-remote-1",
        "it deduplicates correctly even if symbolic remote names have slashes in them"
    );

    let unique = find_unique_refname(&repo, "refs/heads/normal-remote".try_into()?)?;
    assert_eq!(
        unique.as_bstr(),
        "refs/heads/normal-remote-1",
        "registered remotes without slashes also work"
    );
    Ok(())
}

#[test]
fn without_numerical_suffix_it_appends_one() -> anyhow::Result<()> {
    let (repo, feature) =
        read_only_in_memory_scenario_named_with_post("unborn-empty", "", 1, |fixture| {
            let repo = but_testsupport::open_repo(fixture.path())?;

            // Create a branch named "feature"
            let id = repo.object_hash().null();
            let feature = refs::Category::Note.to_full_name("feature".as_bytes().as_bstr())?;
            repo.reference(feature.as_ref(), id, PreviousValue::Any, "test")?;

            Ok(feature)
        })?;

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
    let repo = read_only_in_memory_scenario("unborn-empty")?;

    let branch_1 = refs::Category::LocalBranch.to_full_name("branch-1".as_bytes().as_bstr())?;
    let unique = find_unique_refname(&repo, branch_1.as_ref())?;
    assert_eq!(unique, branch_1);

    Ok(())
}
