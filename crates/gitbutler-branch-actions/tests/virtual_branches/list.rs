use anyhow::Result;
use gitbutler_branch_actions::{list_branches, Author, BranchListingFilter};
use gitbutler_command_context::CommandContext;

#[test]
fn one_vbranch_on_integration() -> Result<()> {
    init_env();
    let list = list_branches(&project_ctx("one-vbranch-on-integration")?, None)?;
    assert_eq!(list.len(), 1);

    let branch = &list[0];
    assert_eq!(branch.name, "virtual");
    assert!(branch.remotes.is_empty(), "no remote is associated yet");
    assert_eq!(branch.number_of_commits, 0);
    assert_eq!(
        branch
            .virtual_branch
            .as_ref()
            .map(|v| v.given_name.as_str()),
        Some("virtual")
    );
    assert_eq!(branch.authors, []);
    assert!(branch.own_branch, "zero commits means user owns the branch");
    Ok(())
}

#[test]
fn one_vbranch_on_integration_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx("one-vbranch-on-integration-one-commit")?;
    let list = list_branches(&ctx, None)?;
    assert_eq!(list.len(), 1);

    let branch = &list[0];
    assert_eq!(branch.name, "virtual");
    assert!(branch.remotes.is_empty(), "no remote is associated yet");
    assert_eq!(
        branch
            .virtual_branch
            .as_ref()
            .map(|v| v.given_name.as_str()),
        Some("virtual")
    );
    assert_eq!(branch.number_of_commits, 1, "one commit created on vbranch");
    assert_eq!(branch.authors, [default_author()]);
    assert!(branch.own_branch);
    Ok(())
}

#[test]
fn two_vbranches_on_integration_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx("two-vbranches-on-integration-one-applied")?;
    // let list = list_branches(&ctx, None)?;
    // assert_eq!(list.len(), 2, "all branches are listed");

    let list = list_branches(
        &ctx,
        Some(BranchListingFilter {
            own_branches: Some(true),
            applied: Some(true),
        }),
    )?;
    assert_eq!(list.len(), 1, "only one of these is applied");
    let branch = &list[0];
    assert_eq!(branch.name, "other");
    assert!(branch.remotes.is_empty(), "no remote is associated yet");
    assert_eq!(
        branch
            .virtual_branch
            .as_ref()
            .map(|v| v.given_name.as_str()),
        Some("other")
    );
    assert_eq!(
        branch.number_of_commits, 0,
        "this one has only pending changes in the worktree"
    );
    assert_eq!(branch.authors, []);
    assert!(
        branch.own_branch,
        "empty branches are always considered owned (or something the user is involved in)"
    );

    let list = list_branches(
        &ctx,
        Some(BranchListingFilter {
            own_branches: Some(true),
            applied: Some(false),
        }),
    )?;
    assert_eq!(list.len(), 1, "only one of these is *not* applied");
    let branch = &list[0];
    assert_eq!(branch.name, "virtual");
    assert!(branch.remotes.is_empty(), "no remote is associated yet");
    assert_eq!(
        branch
            .virtual_branch
            .as_ref()
            .map(|v| v.given_name.as_str()),
        Some("virtual")
    );
    assert_eq!(branch.number_of_commits, 1, "here we have a commit");
    assert_eq!(branch.authors, [default_author()]);
    assert!(
        branch.own_branch,
        "the current user (as identified by signature) created the commit"
    );
    Ok(())
}

#[test]
fn one_feature_branch_and_one_vbranch_on_integration_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx("a-vbranch-named-like-target-branch-short-name")?;
    let list = list_branches(&ctx, None)?;
    assert_eq!(
        list.len(),
        0,
        "Strange, one is definitely there and it seems valid to name vbranches\
            after the target branch but it's filtered out here"
    );

    Ok(())
}

/// This function affects all tests, but those who care should just call it, assuming
/// they all care for the same default value.
/// If not, they should be placed in their own integration test or run with `#[serial_test:serial]`.
/// For `list_branches` it's needed as it compares the current author with commit authors to determine ownership.
fn init_env() {
    for (name, value) in [
        ("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000"),
        ("GIT_AUTHOR_EMAIL", "author@example.com"),
        ("GIT_AUTHOR_NAME", "author"),
        ("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000"),
        ("GIT_COMMITTER_EMAIL", "committer@example.com"),
        ("GIT_COMMITTER_NAME", "committer"),
    ] {
        std::env::set_var(name, value);
    }
}

fn default_author() -> Author {
    Author {
        name: Some("author".into()),
        email: Some("author@example.com".into()),
    }
}

fn project_ctx(name: &str) -> anyhow::Result<CommandContext> {
    gitbutler_testsupport::read_only::fixture("for-listing.sh", name)
}
