use std::path::{Path, PathBuf};

use crate::{
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};
use anyhow::Context as _;

/// Initialize a repository without project registration or workspace setup.
///
/// There is no oplog snapshot: a new repository has no prior state to restore,
/// and existing repositories are not modified.
pub fn run(path: &Path) -> anyhow::Result<InitOutcome> {
    // Open only this directory, not an ancestor: initializing a nested repository
    // must not silently select the parent repository.
    let (repo, created) = match gix::open(path) {
        Ok(repo) => (repo, false),
        Err(_) => (
            gix::init(path).with_context(|| {
                format!("Failed to initialize Git repository at {}", path.display())
            })?,
            true,
        ),
    };
    Ok(InitOutcome {
        repository_path: std::path::absolute(repo.git_dir())
            .context("Failed to resolve repository path")?,
        created,
    })
}

#[must_use]
pub struct InitOutcome {
    repository_path: PathBuf,
    created: bool,
}

impl CliOutputHuman for InitOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        if self.created {
            writeln!(
                out,
                "Initialized empty Git repository in {}",
                self.repository_path.display()
            )?;
        } else {
            writeln!(
                out,
                "Git repository already exists in {}",
                self.repository_path.display()
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
        }
        Output {
            repository_path: self.repository_path,
            created: self.created,
        }
    }
}
