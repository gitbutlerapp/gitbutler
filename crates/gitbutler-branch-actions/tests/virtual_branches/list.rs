use anyhow::Result;
use gitbutler_branch_actions::BranchListingFilter;

#[test]
fn one_vbranch_in_workspace() -> Result<()> {
    init_env();
    let list = list_branches(&project_ctx("one-vbranch-in-workspace")?, None)?;
    assert_eq!(list.len(), 1);

    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "virtual".into(),
            virtual_branch_given_name: Some("virtual"),
            virtual_branch_in_workspace: true,
            has_local: true,
            ..Default::default()
        },
        "It's a bare virtual branch with no commit",
    );
    Ok(())
}

#[test]
fn one_vbranch_in_workspace_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx("one-vbranch-in-workspace-one-commit")?;
    let list = list_branches(&ctx, None)?;
    assert_eq!(list.len(), 1);

    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "virtual".into(),
            virtual_branch_given_name: Some("virtual"),
            virtual_branch_in_workspace: true,
            has_local: true,
            ..Default::default()
        },
        "It's a bare virtual branch with a single commit",
    );
    Ok(())
}

#[test]
fn two_vbranches_in_workspace_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx_without_ws3("two-vbranches-in-workspace-one-applied")?;
    let list = list_branches(
        &ctx,
        Some(BranchListingFilter {
            local: Some(true),
            applied: Some(true),
        }),
    )?;
    assert_eq!(list.len(), 1, "only one of these is applied");
    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "other".into(),
            virtual_branch_given_name: Some("other"),
            virtual_branch_in_workspace: true,
            has_local: true,
            ..Default::default()
        },
        "It's a bare virtual branch without any branches with the same identity",
    );

    let list = list_branches(
        &ctx,
        Some(BranchListingFilter {
            local: Some(true),
            applied: Some(false),
        }),
    )?;
    assert_eq!(list.len(), 1, "only one of these is *not* applied");

    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "virtual".into(),
            virtual_branch_given_name: Some("virtual"),
            virtual_branch_in_workspace: false,
            has_local: true,
            ..Default::default()
        },
        "It's a bare virtual branch without any branches with the same identity",
    );
    Ok(())
}

#[test]
fn one_feature_branch_and_one_vbranch_in_workspace_one_commit() -> Result<()> {
    init_env();
    let ctx = project_ctx_without_ws3("a-vbranch-named-like-target-branch-short-name")?;
    let list = list_branches(&ctx, None)?;
    assert_eq!(
        list.len(),
        1,
        "it finds our single virtual branch despite it having the same 'identity' as the target branch: 'main'"
    );
    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "main".into(),
            remotes: vec![],
            virtual_branch_given_name: Some("main"),
            virtual_branch_in_workspace: true,
            has_local: true,
        },
        "virtual branches can have the name of the target, even though it's probably not going to work when pushing. \
        The remotes of the local `refs/heads/main` are not shown"
    );

    Ok(())
}

#[test]
fn one_branch_in_workspace_multiple_remotes() -> Result<()> {
    init_env();
    let ctx = project_ctx_without_ws3("one-vbranch-in-workspace-two-remotes")?;
    let list = list_branches(&ctx, None)?;
    assert_eq!(list.len(), 1, "a single virtual branch");

    assert_equal(
        &list[0],
        ExpectedBranchListing {
            identity: "main".into(),
            remotes: vec!["other-remote"],
            virtual_branch_given_name: Some("main"),
            virtual_branch_in_workspace: true,
            has_local: true,
        },
        "only the seconf remote is detected",
    );
    Ok(())
}

mod util {
    use anyhow::Result;
    use but_settings::app_settings::FeatureFlags;
    use but_settings::AppSettings;
    use gitbutler_branch::BranchIdentity;
    use gitbutler_branch_actions::{BranchListing, BranchListingFilter};
    use gitbutler_command_context::CommandContext;

    /// A flattened and simplified mirror of `BranchListing` for comparing the actual and expected data.
    #[derive(Debug, PartialEq)]
    pub struct ExpectedBranchListing<'a> {
        pub identity: BranchIdentity,
        pub remotes: Vec<&'a str>,
        pub virtual_branch_given_name: Option<&'a str>,
        pub virtual_branch_in_workspace: bool,
        pub has_local: bool,
    }

    impl Default for ExpectedBranchListing<'static> {
        fn default() -> Self {
            ExpectedBranchListing {
                identity: "invalid-identity-should-always-be-specified".into(),
                remotes: vec![],
                virtual_branch_given_name: None,
                virtual_branch_in_workspace: false,
                has_local: false,
            }
        }
    }

    pub fn assert_equal(
        BranchListing {
            name,
            remotes,
            stack: virtual_branch,
            updated_at: _,
            head: _, // NOTE: can't have stable commits while `gitbutler-change-id` is not stable/is a UUID.
            last_commiter: _,
            has_local,
        }: &BranchListing,
        expected: ExpectedBranchListing,
        msg: &str,
    ) {
        assert_eq!(*name, expected.identity, "identity: {msg}");
        assert_eq!(
            *remotes,
            expected
                .remotes
                .into_iter()
                .map(|name| gix::remote::Name::Symbol(name.into()))
                .collect::<Vec<_>>(),
            "remotes: {msg}"
        );
        assert_eq!(
            virtual_branch.as_ref().map(|b| b.given_name.as_str()),
            expected.virtual_branch_given_name,
            "virtual-branch-name: {msg}"
        );
        assert_eq!(
            virtual_branch.as_ref().is_some_and(|b| b.in_workspace),
            expected.virtual_branch_in_workspace,
            "virtual-branch-in-workspace: {msg}"
        );
        assert_eq!(*has_local, expected.has_local, "{msg}");
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

    pub fn project_ctx(name: &str) -> Result<CommandContext> {
        gitbutler_testsupport::read_only::fixture("for-listing.sh", name)
    }

    pub fn project_ctx_without_ws3(name: &str) -> Result<CommandContext> {
        gitbutler_testsupport::read_only::fixture_with_features(
            "for-listing.sh",
            name,
            FeatureFlags {
                ws3: false,
                ..AppSettings::default().feature_flags
            },
        )
    }

    pub fn list_branches(
        ctx: &CommandContext,
        filter: Option<BranchListingFilter>,
    ) -> Result<Vec<BranchListing>> {
        let mut branches = gitbutler_branch_actions::list_branches(ctx, filter, None)?;
        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }
}
use crate::virtual_branches::list::util::project_ctx_without_ws3;
pub use util::{assert_equal, init_env, list_branches, project_ctx, ExpectedBranchListing};
