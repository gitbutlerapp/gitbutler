use crate::projects::Project;
use anyhow::Result;

use super::git::{GbConfig, GitConfig};

impl Project {
    pub fn gb_config(&self) -> Result<GbConfig> {
        let repo = git2::Repository::open(&self.path)?;
        repo.gb_config()
    }
    pub fn set_gb_config(&self, config: GbConfig) -> Result<()> {
        let repo = git2::Repository::open(&self.path)?;
        repo.set_gb_config(config)
    }
}
