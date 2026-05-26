use but_testsupport::visualize_commit_graph_all;
use gitbutler_branch_actions::BranchListingDetails;

use crate::virtual_branches::list;

#[test]
fn one_vbranch_in_workspace_empty_details() -> anyhow::Result<()> {
    let list = branch_details(
        &list::project_ctx("one-vbranch-in-workspace")?,
        Some("virtual"),
    )?;

    insta::assert_debug_snapshot!(list, @r#"
    [
        BranchListingDetails {
            name: BranchIdentity(
                PartialName(
                    "virtual",
                ),
            ),
            lines_added: 0,
            lines_removed: 0,
            number_of_files: 0,
            number_of_commits: 0,
            authors: [],
            stack: Some(
                StackReference {
                    given_name: "virtual",
                    id: 00000000-0000-0000-0000-000000000001,
                    in_workspace: true,
                    branches: [
                        "virtual",
                    ],
                    pull_requests: {},
                },
            ),
        },
    ]
    "#);

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
            stack: list[0].stack.clone()
        }
    );
    assert!(list[0].stack.is_some());
    Ok(())
}

#[test]
fn many_commits_in_all_branch_types() -> anyhow::Result<()> {
    let ctx = project_ctx("complex-repo")?;
    let list = branch_details(&ctx, ["feature", "non-virtual-feature"])?;
    let repo = ctx.repo.get()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 010a408 (feature) feat-10
    * 0321c2d feat-9
    * 0418d31 feat-8
    * a06934b feat-7
    * 76f0b9e feat-6
    * f733b14 feat-5
    * 814f19d feat-4
    * a6687e9 feat-3
    * 30d2a44 feat-2
    * 494ba0f feat-1
    | * 1f13cc4 (gitbutler/workspace) GitButler Workspace Commit
    | * f179ebf (origin/main, origin/HEAD, a-branch-1) virt-3
    | * 56122fd virt-2
    | * 00ca34e virt-1
    |/  
    | * 21f9396 (HEAD -> non-virtual-feature) non-virtual-feat-10
    | * f721808 non-virtual-feat-9
    | * bcb3226 non-virtual-feat-8
    | * fa23cf8 non-virtual-feat-7
    | * 350bf8a non-virtual-feat-6
    | * 39192f7 non-virtual-feat-5
    | * 40a444b non-virtual-feat-4
    | * 0996ec4 non-virtual-feat-3
    | * 1562511 non-virtual-feat-2
    | * 15264e2 non-virtual-feat-1
    |/  
    * c214aea (main) main-5
    * 0223093 main-4
    * a6590f8 main-3
    * 9ade221 main-2
    * eeb938e main-1
    * 9483946 init
    ");

    insta::assert_debug_snapshot!(list, @r#"
    [
        BranchListingDetails {
            name: BranchIdentity(
                PartialName(
                    "feature",
                ),
            ),
            lines_added: 10,
            lines_removed: 0,
            number_of_files: 1,
            number_of_commits: 10,
            authors: [
                Author {
                    name: Some(
                        BStringForFrontend(
                            "author",
                        ),
                    ),
                    email: Some(
                        BStringForFrontend(
                            "author@example.com",
                        ),
                    ),
                    gravatar_url: Some(
                        BStringForFrontend(
                            "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro",
                        ),
                    ),
                },
            ],
            stack: None,
        },
        BranchListingDetails {
            name: BranchIdentity(
                PartialName(
                    "non-virtual-feature",
                ),
            ),
            lines_added: 10,
            lines_removed: 0,
            number_of_files: 1,
            number_of_commits: 10,
            authors: [
                Author {
                    name: Some(
                        BStringForFrontend(
                            "author",
                        ),
                    ),
                    email: Some(
                        BStringForFrontend(
                            "author@example.com",
                        ),
                    ),
                    gravatar_url: Some(
                        BStringForFrontend(
                            "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro",
                        ),
                    ),
                },
            ],
            stack: Some(
                StackReference {
                    given_name: "non-virtual-feature",
                    id: 00000000-0000-0000-0000-000000000001,
                    in_workspace: true,
                    branches: [
                        "non-virtual-feature",
                    ],
                    pull_requests: {},
                },
            ),
        },
    ]
    "#);

    Ok(())
}

mod util {
    use but_ctx::Context;
    use gitbutler_branch::BranchIdentity;
    use gitbutler_branch_actions::{Author, BranchListingDetails};

    pub fn branch_details(
        ctx: &Context,
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
            gravatar_url: Some(
                "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro".into(),
            ),
        }
    }

    pub fn project_ctx(name: &str) -> anyhow::Result<Context> {
        crate::driverless::read_only_context("for-details.sh", name)
    }
}
use util::{branch_details, default_author, project_ctx};
