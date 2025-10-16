#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };

    use git2::{Repository, StatusOptions};
    use gitbutler_branch_actions::hooks;
    use gitbutler_diff::Hunk;
    use gitbutler_repo::hooks::{ErrorData, HookResult, MessageData, MessageHookResult};
    use gitbutler_stack::{BranchOwnershipClaims, OwnershipClaim};
    use gitbutler_testsupport::{Case, Suite};

    #[test]
    fn pre_commit_hook_success() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let selected_hunks = BranchOwnershipClaims { claims: vec![] };
        let hook = b"
#!/bin/sh
# do nothing
";
        git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_PRE_COMMIT, hook);
        assert_eq!(
            hooks::pre_commit(ctx, &selected_hunks)?,
            HookResult::Success
        );
        Ok(())
    }

    #[test]
    fn pre_commit_hook_not_found() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let selected_hunks = BranchOwnershipClaims { claims: vec![] };
        assert_eq!(
            hooks::pre_commit(ctx, &selected_hunks)?,
            HookResult::NotConfigured
        );
        Ok(())
    }

    #[test]
    fn pre_commit_hook_rejection() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, project, .. } =
            &suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "allowed\n")]));

        let hook = r#"
#!/bin/sh
# Rejects if "forbidden" is found in staged changes.
STAGED_DIFF=$(git diff --cached --unified=0)
if echo "$STAGED_DIFF" | grep -qE "^\+.*forbidden"; then
    echo "rejected"
    exit 1
fi
"#;
        git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_PRE_COMMIT, hook.as_bytes());
        std::fs::write(Path::new(&project.path).join("test.txt"), "forbidden\n")?;

        // While we have changed a file to include the forbidden word, the hook should not
        // fail if we pass no ownership claims. These claims are used to select what hunks
        // get committed.
        let ownership1 = BranchOwnershipClaims { claims: vec![] };
        assert_eq!(hooks::pre_commit(ctx, &ownership1)?, HookResult::Success);

        // But when including the change in the ownerships the change will be staged, and
        // the hook therefore fails.
        let ownership2 = BranchOwnershipClaims {
            claims: vec![OwnershipClaim {
                file_path: "test.txt".into(),
                hunks: vec![Hunk {
                    start: 1,
                    end: 2,
                    hash: None,
                    hunk_header: None,
                }],
            }],
        };

        assert!(!is_file_staged(ctx.repo(), "test.txt")?);
        assert_eq!(
            hooks::pre_commit(ctx, &ownership2)?,
            HookResult::Failure(ErrorData {
                error: "rejected\n".to_owned()
            })
        );
        assert!(!is_file_staged(ctx.repo(), "test.txt")?);
        Ok(())
    }

    #[test]
    fn post_commit_hook_rejection() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'rejected'
exit 1
";
        git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_POST_COMMIT, hook);

        assert_eq!(
            gitbutler_repo::hooks::post_commit(ctx)?,
            HookResult::Failure(ErrorData {
                error: "rejected\n".to_owned()
            })
        );
        Ok(())
    }

    #[test]
    fn message_hook_rejection() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'rejected'
exit 1
";
        git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_COMMIT_MSG, hook);

        let message = "commit message".to_owned();
        assert_eq!(
            gitbutler_repo::hooks::commit_msg(ctx, message)?,
            MessageHookResult::Failure(ErrorData {
                error: "rejected\n".to_owned()
            })
        );
        Ok(())
    }

    #[test]
    fn rewrite_message() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'rewritten message' > $1
";
        git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_COMMIT_MSG, hook);

        let message = "commit message".to_owned();
        assert_eq!(
            gitbutler_repo::hooks::commit_msg(ctx, message)?,
            MessageHookResult::Message(MessageData {
                message: "rewritten message\n".to_owned()
            })
        );
        Ok(())
    }

    #[test]
    fn keep_message() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'commit message' > $1
";
        git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_COMMIT_MSG, hook);

        let message = "commit message\n".to_owned();
        assert_eq!(
            gitbutler_repo::hooks::commit_msg(ctx, message)?,
            MessageHookResult::Success
        );
        Ok(())
    }

    fn is_file_staged(repo: &Repository, file_path: &str) -> Result<bool, git2::Error> {
        let mut opts = StatusOptions::new();
        opts.show(git2::StatusShow::Index);
        let statuses = repo.statuses(Some(&mut opts))?;

        for entry in statuses.iter() {
            if let Some(path) = entry.path()
                && path == file_path
            {
                // Check if the file is staged (index updated but not committed)
                let status = entry.status();
                if status.contains(git2::Status::INDEX_NEW)
                    || status.contains(git2::Status::INDEX_MODIFIED)
                    || status.contains(git2::Status::INDEX_DELETED)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
