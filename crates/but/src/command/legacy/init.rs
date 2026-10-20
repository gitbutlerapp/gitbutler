use std::path::{Path, PathBuf};

use crate::{
    CliResult,
    args::init::Platform,
    id::CommitId,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, Confirm, OutputChannel, WriteWithUtils},
};
use anyhow::Context as _;
use but_api::json::{ChangeIdString, HexHash};

/// Initialize a repository without project registration or workspace setup.
///
/// There is no oplog snapshot: a new repository has no prior state to restore,
/// and existing repositories are not modified.
pub fn run(args: Platform, path: &Path, out: &mut OutputChannel) -> CliResult<InitOutcome> {
    let Platform {
        commit: with_initial_commit,
    } = args;

    // Open only this directory, not an ancestor: initializing a nested repository
    // must not silently select the parent repository.
    let (repo, created, initial_commit) = match gix::open(path) {
        Ok(repo) => (repo, false, None),
        Err(_) => {
            let with_initial_commit = with_initial_commit
                || if let Some(mut input) = out.prepare_for_terminal_input()
                    && input.confirm(
                        "Also create an initial empty commit?",
                        crate::utils::ConfirmDefault::Yes,
                    )? == Confirm::Yes
                {
                    true
                } else {
                    false
                };

            let repo = gix::init(path).with_context(|| {
                format!("Failed to initialize Git repository at {}", path.display())
            })?;

            let new_commit = if with_initial_commit {
                let mut ctx = but_ctx::Context::from_repo_with_settings(
                    repo.clone(),
                    crate::app_settings()?.clone(),
                )?;

                let mut guard = ctx.exclusive_worktree_access();
                let mut meta = ctx.meta()?;
                let (outcome, _ws) = crate::command::legacy::commit::run_in_unborn_repo(
                    &mut ctx,
                    &mut meta,
                    guard.write_permission(),
                    crate::command::legacy::commit::CommitOperation::CommitToNewBranch(
                        crate::command::legacy::commit::CommitToNewBranchOperation {
                            branch_name: None,
                            switch: false,
                        },
                    ),
                    Vec::new(),
                    crate::command::legacy::reword2::CommitMessageSource::Provided(
                        "Initial commit".to_owned(),
                    ),
                )?;

                Some(outcome.new_commit)
            } else {
                None
            };

            (repo, true, new_commit)
        }
    };
    Ok(InitOutcome {
        repository_path: std::path::absolute(repo.git_dir())
            .context("Failed to resolve repository path")?,
        created,
        initial_commit,
    })
}

#[must_use]
pub struct InitOutcome {
    repository_path: PathBuf,
    created: bool,
    initial_commit: Option<CommitId>,
}

impl CliOutputHuman for InitOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self {
            repository_path,
            created,
            initial_commit,
        } = self;

        if created {
            write!(
                out,
                "Initialized empty Git repository in {}",
                repository_path.display()
            )?;

            if let Some(initial_commit) = initial_commit {
                write!(
                    out,
                    " with initial commit {}",
                    crate::theme::Commit(initial_commit)
                )?;
            }

            writeln!(out)?;
        } else {
            writeln!(
                out,
                "Git repository already exists in {}",
                repository_path.display()
            )?;
        }

        Ok(())
    }
}

impl CliOutput for InitOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            repository_path: PathBuf,
            created: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            initial_commit_id: Option<HexHash>,
            #[serde(skip_serializing_if = "Option::is_none")]
            initial_change_id: Option<ChangeIdString>,
        }

        Output {
            repository_path: self.repository_path,
            created: self.created,
            initial_commit_id: self.initial_commit.as_ref().map(|c| c.commit_id.into()),
            initial_change_id: self
                .initial_commit
                .and_then(|c| c.change_id.map(Into::into)),
        }
    }
}
