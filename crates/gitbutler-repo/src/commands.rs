use anyhow::Result;
use bstr::BString;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

use crate::{Config, RepositoryExt};

pub trait RepoCommands {
    fn add_remote(&self, name: &str, url: &str) -> Result<()>;
    fn remotes(&self) -> Result<Vec<String>>;
    fn get_local_config(&self, key: &str) -> Result<Option<String>>;
    fn set_local_config(&self, key: &str, value: &str) -> Result<()>;
    fn check_signing_settings(&self) -> Result<bool>;
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
        let repo = CommandContext::open(self)?;
        let signed = repo
            .repository()
            .sign_buffer(&BString::new("test".into()).into());
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
}
