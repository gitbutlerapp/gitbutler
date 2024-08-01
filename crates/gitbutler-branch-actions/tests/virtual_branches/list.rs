use anyhow::Result;
use gitbutler_branch_actions::{list_branches, BranchListingFilter};

#[test]
fn one_vbranch_on_integration() -> Result<()> {
    init_env();
    let list = list_branches(&project_ctx("one-vbranch-on-integration")?, None)?;
    assert_eq!(list.len(), 1);

    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "virtual",
            virtual_branch_given_name: Some("virtual"),
            virtual_branch_in_workspace: true,
            ..Default::default()
        },
        "It's a bare virtual branch with no commit",
    );
    Ok(())
}

#[test]
fn one_vbranch_on_integration_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx("one-vbranch-on-integration-one-commit")?;
    let list = list_branches(&ctx, None)?;
    assert_eq!(list.len(), 1);

    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "virtual",
            virtual_branch_given_name: Some("virtual"),
            virtual_branch_in_workspace: true,
            number_of_commits: 1,
            ..Default::default()
        },
        "It's a bare virtual branch with a single commit",
    );
    Ok(())
}

#[test]
fn two_vbranches_on_integration_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx("two-vbranches-on-integration-one-applied")?;
    let list = list_branches(
        &ctx,
        Some(BranchListingFilter {
            own_branches: Some(true),
            applied: Some(true),
        }),
    )?;
    assert_eq!(list.len(), 1, "only one of these is applied");
    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "other",
            virtual_branch_given_name: Some("other"),
            virtual_branch_in_workspace: true,
            ..Default::default()
        },
        "It's a bare virtual branch without any branches with the same identity",
    );

    let list = list_branches(
        &ctx,
        Some(BranchListingFilter {
            own_branches: Some(true),
            applied: Some(false),
        }),
    )?;
    assert_eq!(list.len(), 1, "only one of these is *not* applied");
    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "virtual",
            virtual_branch_given_name: Some("virtual"),
            virtual_branch_in_workspace: false,
            number_of_commits: 1,
            ..Default::default()
        },
        "It's a bare virtual branch without any branches with the same identity",
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
        1,
        "it finds our single virtual branch despit it having the same 'identity' as the target branch: 'main'"
    );
    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "main",
            remotes: vec![],
            // remotes: vec!["origin"], // TODO: should be this
            virtual_branch_given_name: Some("main"),
            virtual_branch_in_workspace: true,
            number_of_commits: 1,
            ..Default::default()
        },
        "virtual branches can have the name of the target, even though it's probably not going to work when pushing. \
        The remotes of the local `refs/heads/main` are shown."
    );

    Ok(())
}

#[test]
#[ignore = "TBD"]
fn push_to_two_remotes() -> Result<()> {
    Ok(())
}

#[test]
#[ignore = "TBD"]
fn own_branch_without_virtual_branch() -> Result<()> {
    Ok(())
}

mod util {
    use bstr::BString;
    use gitbutler_branch_actions::{Author, BranchListing};
    use gitbutler_command_context::CommandContext;

    /// A flattened and simplified mirror of `BranchListing` for comparing the actual and expected data.
    #[derive(Default, Debug, PartialEq)]
    pub struct ExpectedBranchListing<'a> {
        pub identity: &'a str,
        pub remotes: Vec<&'a str>,
        pub virtual_branch_given_name: Option<&'a str>,
        pub virtual_branch_in_workspace: bool,
        pub number_of_commits: usize,
        pub authors: Vec<Author>,
        pub own_branch: bool,
    }

    pub fn assert_equal(
        BranchListing {
            name,
            remotes,
            virtual_branch,
            number_of_commits,
            updated_at: _,
            authors,
            own_branch,
            head: _, // NOTE: can't have stable commits while `gitbutler-change-id` is not stable/is a UUID.
        }: &BranchListing,
        mut expected: ExpectedBranchListing,
        msg: &str,
    ) {
        assert_eq!(*name, expected.identity, "{msg}");
        assert_eq!(
            *remotes,
            expected
                .remotes
                .into_iter()
                .map(BString::from)
                .collect::<Vec<_>>(),
            "{msg}"
        );
        assert_eq!(
            virtual_branch.as_ref().map(|b| b.given_name.as_str()),
            expected.virtual_branch_given_name,
            "{msg}"
        );
        assert_eq!(
            virtual_branch.as_ref().map_or(false, |b| b.in_workspace),
            expected.virtual_branch_in_workspace,
            "{msg}"
        );
        assert_eq!(*number_of_commits, expected.number_of_commits, "{msg}");
        if expected.number_of_commits > 0 && expected.authors.is_empty() {
            expected.authors = vec![default_author()];
        }
        assert_eq!(*authors, expected.authors, "{msg}");
        if expected.virtual_branch_given_name.is_some() {
            expected.own_branch = true;
        }
        assert_eq!(*own_branch, expected.own_branch, "{msg}");
    }

    /// This function affects all tests, but those who care should just call it, assuming
    /// they all care for the same default value.
    /// If not, they should be placed in their own integration test or run with `#[serial_test:serial]`.
    /// For `list_branches` it's needed as it compares the current author with commit authors to determine ownership.
    pub fn init_env() {
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

    pub fn default_author() -> Author {
        Author {
            name: Some("author".into()),
            email: Some("author@example.com".into()),
        }
    }

    pub fn project_ctx(name: &str) -> anyhow::Result<CommandContext> {
        gitbutler_testsupport::read_only::fixture("for-listing.sh", name)
    }
}
use util::{assert_equal, init_env, project_ctx, ExpectedBranchListing};
