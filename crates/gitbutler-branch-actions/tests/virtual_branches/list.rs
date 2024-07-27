use anyhow::Result;
use gitbutler_branch_actions::list_branches;
use gitbutler_command_context::ProjectRepository;

#[test]
fn on_main_single_branch_no_vbranch() -> Result<()> {
    let list = list_branches(&project_ctx("single-branch-no-vbranch")?, None)?;
    assert_eq!(list.len(), 1);

    let branch = &list[0];
    assert_eq!(branch.name, "main", "short names are used");
    assert_eq!(branch.remotes, ["origin"]);
    assert_eq!(branch.virtual_branch, None);
    assert_eq!(
        branch.authors,
        [],
        "there is no local commit, so no authors are known"
    );
    Ok(())
}

#[test]
fn on_main_single_branch_no_vbranch_multiple_remotes() -> Result<()> {
    let list = list_branches(&project_ctx("single-branch-no-vbranch-multi-remote")?, None)?;
    assert_eq!(list.len(), 1);

    let branch = &list[0];
    assert_eq!(branch.name, "main");
    assert_eq!(branch.remotes, ["other-origin", "origin"]);
    assert_eq!(branch.virtual_branch, None);
    assert_eq!(branch.authors, []);
    Ok(())
}

#[test]
fn one_vbranch_on_integration() -> Result<()> {
    let list = list_branches(&project_ctx("one-vbranch-on-integration")?, None)?;
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
    assert_eq!(branch.authors, []);
    Ok(())
}

fn project_ctx(name: &str) -> anyhow::Result<ProjectRepository> {
    gitbutler_testsupport::read_only::fixture("for-listing.sh", name)
}
