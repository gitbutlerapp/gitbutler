use anyhow::Result;
use git2::ConfigLevel;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GbConfig {
    pub sign_commits: Option<bool>,
    pub signing_key: Option<String>,
    pub signing_format: Option<String>,
    pub gpg_program: Option<String>,
    pub gpg_ssh_program: Option<String>,
}
const SIGN_COMMITS: &str = "gitbutler.signCommits";
const SIGNING_KEY: &str = "user.signingKey";
const SIGNING_FORMAT: &str = "gpg.format";
const GPG_PROGRAM: &str = "gpg.program";
const GPG_SSH_PROGRAM: &str = "gpg.ssh.program";

pub trait GitConfig {
    fn gb_config(&self) -> Result<GbConfig>;
    fn set_gb_config(&self, config: GbConfig) -> Result<()>;
}

impl GitConfig for git2::Repository {
    fn gb_config(&self) -> Result<GbConfig> {
        let sign_commits = get_bool(self, SIGN_COMMITS)?;
        let signing_key = get_string(self, SIGNING_KEY)?;
        let signing_format = get_string(self, SIGNING_FORMAT)?;
        let gpg_program = get_string(self, GPG_PROGRAM)?;
        let gpg_ssh_program = get_string(self, GPG_SSH_PROGRAM)?;
        Ok(GbConfig {
            sign_commits,
            signing_key,
            signing_format,
            gpg_program,
            gpg_ssh_program,
        })
    }
    fn set_gb_config(&self, config: GbConfig) -> Result<()> {
        if let Some(sign_commits) = config.sign_commits {
            set_local_bool(self, SIGN_COMMITS, sign_commits)?;
        };
        if let Some(signing_key) = config.signing_key {
            set_local_string(self, SIGNING_KEY, &signing_key)?;
        };
        if let Some(signing_format) = config.signing_format {
            set_local_string(self, SIGNING_FORMAT, &signing_format)?;
        }
        if let Some(gpg_program) = config.gpg_program {
            set_local_string(self, GPG_PROGRAM, &gpg_program)?;
        }
        if let Some(gpg_ssh_program) = config.gpg_ssh_program {
            set_local_string(self, GPG_SSH_PROGRAM, &gpg_ssh_program)?;
        }
        Ok(())
    }
}

fn get_bool(repo: &git2::Repository, key: &str) -> Result<Option<bool>> {
    let config = repo.config()?;
    match config.get_bool(key) {
        Ok(value) => Ok(Some(value)),
        Err(err) => match err.code() {
            git2::ErrorCode::NotFound => Ok(None),
            _ => Err(err.into()),
        },
    }
}

fn get_string(repo: &git2::Repository, key: &str) -> Result<Option<String>> {
    let config = repo.config()?;
    match config.get_string(key) {
        Ok(value) => Ok(Some(value)),
        Err(err) => match err.code() {
            git2::ErrorCode::NotFound => Ok(None),
            _ => Err(err.into()),
        },
    }
}

fn set_local_bool(repo: &git2::Repository, key: &str, val: bool) -> Result<()> {
    let config = repo.config()?;
    match config.open_level(ConfigLevel::Local) {
        Ok(mut local) => local.set_bool(key, val).map_err(Into::into),
        Err(err) => Err(err.into()),
    }
}

fn set_local_string(repo: &git2::Repository, key: &str, val: &str) -> Result<()> {
    let config = repo.config()?;
    match config.open_level(ConfigLevel::Local) {
        Ok(mut local) => local.set_str(key, val).map_err(Into::into),
        Err(err) => Err(err.into()),
    }
}
