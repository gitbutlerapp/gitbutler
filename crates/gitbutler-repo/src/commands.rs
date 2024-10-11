use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use std::path::Path;

use crate::{Config, RepositoryExt};

pub trait RepoCommands {
    fn add_remote(&self, name: &str, url: &str) -> Result<()>;
    fn remotes(&self) -> Result<Vec<String>>;
    fn get_local_config(&self, key: &str) -> Result<Option<String>>;
    fn set_local_config(&self, key: &str, value: &str) -> Result<()>;
    fn check_signing_settings(&self) -> Result<bool>;
    fn read_file_from_workspace(&self, relative_path: &Path) -> Result<String>;
}

impl RepoCommands for Project {
    fn get_local_config(&self, key: &str) -> Result<Option<String>> {
        let ctx = CommandContext::open(self)?;
        let config: Config = ctx.repository().into();
        config.get_local(key)
    }

    fn set_local_config(&self, key: &str, value: &str) -> Result<()> {
        let ctx = CommandContext::open(self)?;
        let config: Config = ctx.repository().into();
        config.set_local(key, value)
    }

    fn check_signing_settings(&self) -> Result<bool> {
        let ctx = CommandContext::open(self)?;
        let signed = ctx.repository().sign_buffer(b"test");
        match signed {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn remotes(&self) -> Result<Vec<String>> {
        let ctx = CommandContext::open(self)?;
        ctx.repository().remotes_as_string()
    }

    fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        let ctx = CommandContext::open(self)?;
        ctx.repository().remote(name, url)?;
        Ok(())
    }

    fn read_file_from_workspace(&self, relative_path: &Path) -> Result<String> {
        let ctx = CommandContext::open(self)?;
        let path_in_worktree = gix::path::realpath(self.path.join(relative_path))?;
        if !path_in_worktree.starts_with(self.path.clone()) {
            anyhow::bail!(
                "Path to read from at '{}' isn't in the worktree directory '{}'",
                relative_path.display(),
                self.path.display()
            );
        }

        let tree = ctx.repository().head()?.peel_to_tree()?;
        let entry = tree.get_path(relative_path)?;
        let blob = ctx.repository().find_blob(entry.id())?;

        if !blob.is_binary() {
            let content = std::str::from_utf8(blob.content())?;
            Ok(content.to_string())
        } else {
            anyhow::bail!("File is binary");
        }
    }
}
