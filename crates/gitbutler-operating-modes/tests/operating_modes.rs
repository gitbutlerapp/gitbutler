use gitbutler_command_context::CommandContext;
use gitbutler_operating_modes::{EditModeMetadata, write_edit_mode_metadata};

/// Creates a branch from the head commit
fn create_and_checkout_branch(ctx: &CommandContext, branch_name: &str) {
    let repository = ctx.repo();
    repository
        .branch(
            branch_name,
            &repository.head().unwrap().peel_to_commit().unwrap(),
            true,
        )
        .unwrap();

    repository
        .set_head(format!("refs/heads/{branch_name}").as_str())
        .unwrap();
}

fn create_edit_mode_metadata(ctx: &CommandContext) {
    write_edit_mode_metadata(
        ctx,
        &EditModeMetadata {
            commit_oid: git2::Oid::zero(),
            stack_id: uuid::Uuid::new_v4().into(),
        },
    )
    .unwrap();
}

mod operating_modes {
    mod open_workspace_mode {
        use gitbutler_operating_modes::{ensure_open_workspace_mode, in_open_workspace_mode};
        use gitbutler_testsupport::{Case, Suite};

        use crate::create_and_checkout_branch;

        #[test]
        fn in_open_workspace_mode_true_when_in_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            let in_open_workspace = in_open_workspace_mode(ctx);
            assert!(in_open_workspace);
        }

        #[test]
        fn in_open_workspace_mode_false_when_in_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            let in_open_workspace = in_open_workspace_mode(ctx);
            assert!(!in_open_workspace);
        }

        #[test]
        fn in_open_workspace_mode_false_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let in_open_workspace = in_open_workspace_mode(ctx);
            assert!(!in_open_workspace);
        }

        #[test]
        fn assure_open_workspace_mode_ok_when_on_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            assert!(ensure_open_workspace_mode(ctx).is_ok());
        }

        #[test]
        fn assure_open_workspace_mode_err_when_on_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            assert!(ensure_open_workspace_mode(ctx).is_err());
        }

        #[test]
        fn assure_open_workspace_mode_err_when_on_other_branch() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            assert!(ensure_open_workspace_mode(ctx).is_err());
        }
    }

    mod outside_workspace_mode {
        use gitbutler_operating_modes::{ensure_outside_workspace_mode, in_outside_workspace_mode};
        use gitbutler_testsupport::{Case, Suite};

        use crate::{create_and_checkout_branch, create_edit_mode_metadata};

        #[test]
        fn in_outside_workspace_mode_true_when_in_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let in_outside_workspace = in_outside_workspace_mode(ctx);
            assert!(in_outside_workspace);
        }

        #[test]
        fn in_outside_workspace_mode_false_when_on_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            let in_outside_worskpace = in_outside_workspace_mode(ctx);
            assert!(!in_outside_worskpace);
        }

        #[test]
        fn in_outside_workspace_mode_false_when_on_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            let in_outside_worskpace = in_outside_workspace_mode(ctx);
            assert!(!in_outside_worskpace);
        }

        #[test]
        fn assure_outside_workspace_mode_ok_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            assert!(ensure_outside_workspace_mode(ctx).is_ok());
        }

        #[test]
        fn assure_outside_workspace_mode_err_when_on_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            assert!(ensure_outside_workspace_mode(ctx).is_err());
        }

        #[test]
        fn assure_outside_workspace_mode_err_when_on_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            assert!(ensure_outside_workspace_mode(ctx).is_err());
        }
    }

    mod edit_mode {
        use gitbutler_operating_modes::{ensure_edit_mode, in_edit_mode};
        use gitbutler_testsupport::{Case, Suite};

        use crate::{create_and_checkout_branch, create_edit_mode_metadata};

        #[test]
        fn in_edit_mode_true_when_in_edit_mode() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            let in_edit_mode = in_edit_mode(ctx);
            assert!(in_edit_mode);
        }

        #[test]
        fn in_edit_mode_false_when_in_edit_mode_with_no_metadata() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            let in_edit_mode = in_edit_mode(ctx);
            assert!(!in_edit_mode);
        }

        #[test]
        fn in_edit_mode_false_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");
            create_edit_mode_metadata(ctx);

            let in_edit_mode = in_edit_mode(ctx);
            assert!(!in_edit_mode);
        }

        #[test]
        fn assert_edit_mode_ok_when_in_edit_mode() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            assert!(ensure_edit_mode(ctx).is_ok());
        }

        #[test]
        fn assert_edit_mode_err_when_in_edit_mode_with_no_metadata() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            assert!(ensure_edit_mode(ctx).is_err());
        }

        #[test]
        fn assert_edit_mode_err_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");
            create_edit_mode_metadata(ctx);

            assert!(ensure_edit_mode(ctx).is_err());
        }
    }
}
