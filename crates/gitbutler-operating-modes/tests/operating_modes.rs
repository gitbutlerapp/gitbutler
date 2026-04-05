use but_ctx::Context;
use gitbutler_operating_modes::{EditModeMetadata, write_edit_mode_metadata};
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};

/// Creates a branch from the head commit
fn create_and_checkout_branch(ctx: &Context, branch_name: &str) {
    let repo = &*ctx.repo.get().unwrap();
    let head_commit = repo.head_commit().unwrap().id;
    let branch_ref: gix::refs::FullName = format!("refs/heads/{branch_name}").try_into().unwrap();
    repo.reference(
        branch_ref.clone(),
        head_commit,
        PreviousValue::Any,
        "test branch creation",
    )
    .unwrap();
    repo.edit_reference(RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: gix::reference::log::message("test", "switch HEAD".into(), 0),
            },
            expected: PreviousValue::Any,
            new: Target::Symbolic(branch_ref),
        },
        name: "HEAD".try_into().unwrap(),
        deref: false,
    })
    .unwrap();
}

fn create_edit_mode_metadata(ctx: &Context) {
    write_edit_mode_metadata(
        ctx,
        &EditModeMetadata {
            commit_oid: gix::ObjectId::null(gix::hash::Kind::Sha1),
            stack_id: uuid::Uuid::new_v4().into(),
        },
    )
    .unwrap();
}

mod operating_modes {
    mod workspace_ref_names {
        use gitbutler_operating_modes::{
            INTEGRATION_BRANCH_REF, WORKSPACE_BRANCH_REF, is_well_known_workspace_ref,
        };

        #[test]
        fn recognizes_well_known_workspace_refs() {
            let workspace_ref: &gix::refs::FullNameRef = WORKSPACE_BRANCH_REF.try_into().unwrap();
            let integration_ref: &gix::refs::FullNameRef =
                INTEGRATION_BRANCH_REF.try_into().unwrap();
            let other_ref: &gix::refs::FullNameRef = "refs/heads/feature".try_into().unwrap();

            assert!(is_well_known_workspace_ref(workspace_ref));
            assert!(is_well_known_workspace_ref(integration_ref));
            assert!(!is_well_known_workspace_ref(other_ref));
        }
    }

    mod open_workspace_mode {
        use but_testsupport::legacy::{Case, Suite};
        use gitbutler_operating_modes::{ensure_open_workspace_mode, in_open_workspace_mode};

        use crate::create_and_checkout_branch;

        #[test]
        fn in_open_workspace_mode_true_when_in_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            let guard = ctx.shared_worktree_access();
            let in_open_workspace = in_open_workspace_mode(ctx, guard.read_permission()).unwrap();
            assert!(in_open_workspace);
        }

        #[test]
        fn in_open_workspace_mode_false_when_in_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            let guard = ctx.shared_worktree_access();
            let in_open_workspace = in_open_workspace_mode(ctx, guard.read_permission()).unwrap();
            assert!(!in_open_workspace);
        }

        #[test]
        fn in_open_workspace_mode_false_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let guard = ctx.shared_worktree_access();
            let in_open_workspace = in_open_workspace_mode(ctx, guard.read_permission()).unwrap();
            assert!(!in_open_workspace);
        }

        #[test]
        fn assure_open_workspace_mode_ok_when_on_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            let guard = ctx.shared_worktree_access();
            assert!(ensure_open_workspace_mode(ctx, guard.read_permission()).is_ok());
        }

        #[test]
        fn assure_open_workspace_mode_err_when_on_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            let guard = ctx.shared_worktree_access();
            assert!(ensure_open_workspace_mode(ctx, guard.read_permission()).is_err());
        }

        #[test]
        fn assure_open_workspace_mode_err_when_on_other_branch() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let guard = ctx.shared_worktree_access();
            assert!(ensure_open_workspace_mode(ctx, guard.read_permission()).is_err());
        }
    }

    mod outside_workspace_mode {
        use but_testsupport::legacy::{Case, Suite};
        use gitbutler_operating_modes::{ensure_outside_workspace_mode, in_outside_workspace_mode};

        use crate::{create_and_checkout_branch, create_edit_mode_metadata};

        #[test]
        fn in_outside_workspace_mode_true_when_in_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let guard = ctx.shared_worktree_access();
            let in_outside_workspace =
                in_outside_workspace_mode(ctx, guard.read_permission()).unwrap();
            assert!(in_outside_workspace);
        }

        #[test]
        fn in_outside_workspace_mode_false_when_on_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            let guard = ctx.shared_worktree_access();
            let in_outside_worskpace =
                in_outside_workspace_mode(ctx, guard.read_permission()).unwrap();
            assert!(!in_outside_worskpace);
        }

        #[test]
        fn in_outside_workspace_mode_false_when_on_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            let guard = ctx.shared_worktree_access();
            let in_outside_worskpace =
                in_outside_workspace_mode(ctx, guard.read_permission()).unwrap();
            assert!(!in_outside_worskpace);
        }

        #[test]
        fn assure_outside_workspace_mode_ok_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");

            let guard = ctx.shared_worktree_access();
            assert!(ensure_outside_workspace_mode(ctx, guard.read_permission()).is_ok());
        }

        #[test]
        fn assure_outside_workspace_mode_err_when_on_gitbutler_edit() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            let guard = ctx.shared_worktree_access();
            assert!(ensure_outside_workspace_mode(ctx, guard.read_permission()).is_err());
        }

        #[test]
        fn assure_outside_workspace_mode_err_when_on_gitbutler_workspace() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/workspace");

            let guard = ctx.shared_worktree_access();
            assert!(ensure_outside_workspace_mode(ctx, guard.read_permission()).is_err());
        }
    }

    mod edit_mode {
        use but_testsupport::legacy::{Case, Suite};
        use gitbutler_operating_modes::{ensure_edit_mode, in_edit_mode};

        use crate::{create_and_checkout_branch, create_edit_mode_metadata};

        #[test]
        fn in_edit_mode_true_when_in_edit_mode() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            let guard = ctx.shared_worktree_access();
            let in_edit_mode = in_edit_mode(ctx, guard.read_permission()).unwrap();
            assert!(in_edit_mode);
        }

        #[test]
        fn in_edit_mode_false_when_in_edit_mode_with_no_metadata() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            let guard = ctx.shared_worktree_access();
            let in_edit_mode = in_edit_mode(ctx, guard.read_permission()).unwrap();
            assert!(!in_edit_mode);
        }

        #[test]
        fn in_edit_mode_false_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");
            create_edit_mode_metadata(ctx);

            let guard = ctx.shared_worktree_access();
            let in_edit_mode = in_edit_mode(ctx, guard.read_permission()).unwrap();
            assert!(!in_edit_mode);
        }

        #[test]
        fn assert_edit_mode_ok_when_in_edit_mode() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");
            create_edit_mode_metadata(ctx);

            let guard = ctx.shared_worktree_access();
            assert!(ensure_edit_mode(ctx, guard.read_permission()).is_ok());
        }

        #[test]
        fn assert_edit_mode_err_when_in_edit_mode_with_no_metadata() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "gitbutler/edit");

            let guard = ctx.shared_worktree_access();
            assert!(ensure_edit_mode(ctx, guard.read_permission()).is_err());
        }

        #[test]
        fn assert_edit_mode_err_when_on_other_branches() {
            let suite = Suite::default();
            let Case { ctx, .. } = &suite.new_case();

            create_and_checkout_branch(ctx, "testeroni");
            create_edit_mode_metadata(ctx);

            let guard = ctx.shared_worktree_access();
            assert!(ensure_edit_mode(ctx, guard.read_permission()).is_err());
        }
    }
}
