use crate::virtual_branches::list;
use gitbutler_branch_actions::BranchListingDetails;

#[test]
fn one_vbranch_in_workspace_empty_details() -> anyhow::Result<()> {
    let list = branch_details(
        &list::project_ctx("one-vbranch-in-workspace")?,
        Some("virtual"),
    )?;
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0],
        BranchListingDetails {
            name: "virtual".into(),
            lines_added: 0,
            lines_removed: 0,
            number_of_files: 0,
            number_of_commits: 0,
            authors: vec![],
        }
    );
    Ok(())
}

#[test]
fn one_vbranch_in_workspace_single_commit() -> anyhow::Result<()> {
    let list = branch_details(
        &list::project_ctx("one-vbranch-in-workspace-one-commit")?,
        Some("virtual"),
    )?;
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0],
        BranchListingDetails {
            name: "virtual".into(),
            lines_added: 2,
            lines_removed: 0,
            number_of_files: 2,
            number_of_commits: 1,
            authors: vec![default_author()],
        }
    );
    Ok(())
}

#[test]
fn many_commits_in_all_branch_types() -> anyhow::Result<()> {
    let ctx = project_ctx("complex-repo")?;
    let list = branch_details(&ctx, ["feature", "non-virtual-feature"])?;
    assert_eq!(list.len(), 2);
    assert_eq!(
        list[0],
        BranchListingDetails {
            name: "feature".into(),
            lines_added: 100,
            lines_removed: 0,
            number_of_files: 1,
            number_of_commits: 100,
            authors: vec![default_author()],
        },
        "local branches use the *current* local tracking branch…"
    );
    assert_eq!(
        list[1],
        BranchListingDetails {
            name: "non-virtual-feature".into(),
            lines_added: 50,
            lines_removed: 0,
            number_of_files: 1,
            number_of_commits: 50,
            authors: vec![default_author()],
        },
        "This is a non-virtual brnach, so it sees the local tracking branch as well"
    );
    Ok(())
}

mod util {
    use gitbutler_branch::BranchIdentity;
    use gitbutler_branch_actions::{Author, BranchListingDetails};
    use gitbutler_command_context::CommandContext;

    pub fn branch_details(
        ctx: &CommandContext,
        branch_names: impl IntoIterator<Item = impl TryInto<BranchIdentity>>,
    ) -> anyhow::Result<Vec<BranchListingDetails>> {
        let mut details = gitbutler_branch_actions::get_branch_listing_details(ctx, branch_names)?;
        details.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(details)
    }

    pub fn default_author() -> Author {
        Author {
            name: Some("author".into()),
            email: Some("author@example.com".into()),
            gravatar_url: Some("https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro".into()),
        }
    }

    pub fn project_ctx(name: &str) -> anyhow::Result<CommandContext> {
        gitbutler_testsupport::read_only::fixture("for-details.sh", name)
    }
}
use util::{branch_details, default_author, project_ctx};
