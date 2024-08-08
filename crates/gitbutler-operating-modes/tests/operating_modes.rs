use gitbutler_command_context::CommandContext;

/// Creates a branch from the head commit
fn create_and_checkout_branch(ctx: &CommandContext, branch_name: &str) {
    let repository = ctx.repository();
    repository
        .branch(
            branch_name,
            &repository.head().unwrap().peel_to_commit().unwrap(),
            true,
        )
        .unwrap();

    repository
        .set_head(format!("refs/heads/{}", branch_name).as_str())
        .unwrap();
}

mod operating_modes {
    mod open_workspace_mode {
        use gitbutler_operating_modes::{assure_open_workspace_mode, in_open_workspace_mode};
        use gitbutler_testsupport::{Case, Suite};

        use crate::create_and_checkout_branch;

        #[test]
        fn in_open_workspace_mode_true_when_in_gitbutler_integration() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/integration");

            let in_open_workspace = in_open_workspace_mode(ctx).unwrap();
            assert!(in_open_workspace);
        }

        #[test]
        fn in_open_workspace_mode_false_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let in_open_workspace = in_open_workspace_mode(ctx).unwrap();
            assert!(!in_open_workspace);
        }

        #[test]
        fn assure_open_workspace_mode_ok_when_on_gitbutler_integration() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/integration");

            assert!(assure_open_workspace_mode(ctx).is_ok());
        }

        #[test]
        fn assure_open_workspace_mode_err_when_on_other_branch() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            assert!(assure_open_workspace_mode(ctx).is_err());
        }
    }

    mod outside_workspace_mode {
        use gitbutler_operating_modes::{assure_outside_workspace_mode, in_outside_workspace_mode};
        use gitbutler_testsupport::{Case, Suite};

        use crate::create_and_checkout_branch;

        #[test]
        fn in_outside_workspace_mode_true_when_in_gitbutler_integration() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let in_outside_workspace = in_outside_workspace_mode(ctx).unwrap();
            assert!(in_outside_workspace);
        }

        #[test]
        fn in_outside_workspace_mode_false_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/integration");

            let in_outside_worskpace = in_outside_workspace_mode(ctx).unwrap();
            assert!(!in_outside_worskpace);
        }

        #[test]
        fn assure_outside_workspace_mode_ok_when_on_gitbutler_integration() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            assert!(assure_outside_workspace_mode(ctx).is_ok());
        }

        #[test]
        fn assure_outside_workspace_mode_err_when_on_other_branch() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/integration");

            assert!(assure_outside_workspace_mode(ctx).is_err());
        }
    }
}
