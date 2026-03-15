use anyhow::Result;

pub struct Config<'a> {
    repo: &'a gix::Repository,
}

impl<'a> From<&'a gix::Repository> for Config<'a> {
    fn from(value: &'a gix::Repository) -> Self {
        Self { repo: value }
    }
}

// TODO: Remove this in favor of gitbutler-core::config::git::GitConfig
impl Config<'_> {
    pub fn user_real_comitter(&self) -> Result<bool> {
        let commit_as_gitbutler = self
            .repo
            .config_snapshot()
            .boolean("gitbutler.gitbutlerCommitter")
            .unwrap_or(false);
        Ok(!commit_as_gitbutler)
    }
}
