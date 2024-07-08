use anyhow::Result;
use gitbutler_project::Project;

use super::git::{GbConfig, GitConfig};

pub trait ProjectCommands {
    fn gb_config(&self) -> Result<GbConfig>;
    fn set_gb_config(&self, config: GbConfig) -> Result<()>;
}

impl ProjectCommands for Project {
    fn gb_config(&self) -> Result<GbConfig> {
        let repo = git2::Repository::open(&self.path)?;
        repo.gb_config()
    }
    fn set_gb_config(&self, config: GbConfig) -> Result<()> {
        let repo = git2::Repository::open(&self.path)?;
        repo.set_gb_config(config)
    }
}
