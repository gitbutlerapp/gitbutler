use anyhow::Result;
use gitbutler_branch_actions::{list_branches, Author};
use gitbutler_command_context::CommandContext;

#[test]
fn on_main_single_branch_no_vbranch() -> Result<()> {
    init_env();
    let list = list_branches(&project_ctx("single-branch-no-vbranch")?, None)?;
    assert_eq!(list.len(), 1);

    let branch = &list[0];
    assert_eq!(branch.name, "main", "short names are used");
    assert_eq!(branch.remotes, ["origin"]);
    assert_eq!(branch.virtual_branch, None);
    assert_eq!(branch.number_of_commits, 0);
    assert_eq!(
        branch.authors,
        [],
        "there is no local commit, so no authors are known"
    );
    assert!(branch.own_branch);
    Ok(())
}

#[test]
fn on_main_single_branch_no_vbranch_multiple_remotes() -> Result<()> {
    init_env();
    let list = list_branches(&project_ctx("single-branch-no-vbranch-multi-remote")?, None)?;
    assert_eq!(list.len(), 1);

    let branch = &list[0];
    assert_eq!(branch.name, "main");
    assert_eq!(branch.remotes, ["other-origin", "origin"]);
    assert_eq!(branch.virtual_branch, None);
    assert_eq!(branch.number_of_commits, 0);
    assert_eq!(branch.authors, []);
    assert!(branch.own_branch);
    Ok(())
}

#[test]
fn on_main_single_branch_no_vbranch_one_commit() -> Result<()> {
    init_env();
    let list = list_branches(&project_ctx("single-branch-no-vbranch-one-commit")?, None)?;
    assert_eq!(list.len(), 1);

    let branch = &list[0];
    assert_eq!(branch.name, "main");
    assert_eq!(branch.remotes, ["origin"]);
    assert_eq!(branch.virtual_branch, None);
    assert_eq!(
        branch.number_of_commits, 0,
        "local-only commits aren't detected"
    );
    assert_eq!(
        branch.authors,
        [],
        "and thus there is no ownership information"
    );
    assert!(branch.own_branch);
    Ok(())
}

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
    let list = list_branches(&project_ctx("one-vbranch-on-integration-one-commit")?, None)?;
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

/// This function affects all tests, but those who care should just call it, assuming
/// they all care for the same default value.
/// If not, they should be placed in their own integration test or run with `#[serial_test:serial]`.
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
