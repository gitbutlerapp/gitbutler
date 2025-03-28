use anyhow::Result;
use git2::ConfigLevel;
use gix::bstr::{BStr, ByteVec};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ffi::OsStr;

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
        let repo = gix::open(self.path())?;
        let config = repo.config_snapshot();
        let sign_commits = config.boolean(SIGN_COMMITS);
        let signing_key = config.string(SIGNING_KEY).and_then(bstring_into_string);
        let signing_format = config.string(SIGNING_FORMAT).and_then(bstring_into_string);
        let gpg_program = config
            .trusted_program(GPG_PROGRAM)
            .and_then(osstr_into_string);
        let gpg_ssh_program = config
            .trusted_program(GPG_SSH_PROGRAM)
            .and_then(osstr_into_string);
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

fn bstring_into_string(s: Cow<'_, BStr>) -> Option<String> {
    match Vec::from(s.into_owned()).into_string() {
        Ok(s) => Some(s),
        Err(err) => {
            tracing::warn!("Could not convert to string due to illegal UTF8: {err}");
            None
        }
    }
}

fn osstr_into_string(s: Cow<'_, OsStr>) -> Option<String> {
    match Vec::from(gix::path::try_os_str_into_bstr(s).ok()?.into_owned()).into_string() {
        Ok(s) => Some(s),
        Err(err) => {
            tracing::warn!("Could not convert to string due to illegal UTF8: {err}");
            None
        }
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
