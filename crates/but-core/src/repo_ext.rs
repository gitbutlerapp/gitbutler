use crate::GitConfigSettings;
use anyhow::Context;
use gitbutler_error::error::Code;

/// Easy access of settings relevant to GitButler for retrieval and storage in Git settings.
pub trait RepositoryExt {
    /// Returns a bundle of settings by querying the git configuration itself, assuring fresh data is loaded.
    fn git_settings(&self) -> anyhow::Result<GitConfigSettings>;
    /// Set all fields in `config` that are not `None` to disk into local repository configuration, or none of them.
    fn set_git_settings(&self, config: &GitConfigSettings) -> anyhow::Result<()>;
    /// Return all signatures that would be needed to perform a commit as configured in Git: `(author, committer)`.
    fn commit_signatures(&self) -> anyhow::Result<(gix::actor::Signature, gix::actor::Signature)>;
}

impl RepositoryExt for gix::Repository {
    fn commit_signatures(&self) -> anyhow::Result<(gix::actor::Signature, gix::actor::Signature)> {
        let repo = gix::open(self.path())?;

        let author = repo
            .author()
            .transpose()?
            .context("No author is configured in Git")
            .context(Code::AuthorMissing)?;

        let commit_as_gitbutler = !self
            .config_snapshot()
            .boolean("gitbutler.gitbutlerCommitter")
            .unwrap_or_default();
        let committer = if commit_as_gitbutler {
            committer_signature()
        } else {
            repo.committer()
                .transpose()?
                .map(|s| s.to_owned())
                .unwrap_or_else(|| committer_signature())
        };

        Ok((author.into(), committer))
    }

    fn git_settings(&self) -> anyhow::Result<GitConfigSettings> {
        // TODO: Make it easy to load the latest configuration in `gix`.
        // Re-open just the local configuration to be sure it's fresh before writing it.
        let repo = gix::open_opts(self.path(), self.open_options().clone())?;
        let config = repo.config_snapshot();
        GitConfigSettings::try_from_snapshot(&config)
    }
    fn set_git_settings(&self, settings: &GitConfigSettings) -> anyhow::Result<()> {
        settings.persist_to_local_config(self)
    }
}

const GITBUTLER_COMMIT_AUTHOR_NAME: &str = "GitButler";
const GITBUTLER_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";

/// Provide a signature with the GitButler author, and the current time or the time overridden
/// depending on the value for `purpose`.
fn committer_signature() -> gix::actor::Signature {
    let signature = gix::actor::SignatureRef {
        name: GITBUTLER_COMMIT_AUTHOR_NAME.into(),
        email: GITBUTLER_COMMIT_AUTHOR_EMAIL.into(),
        time: commit_time("GIT_COMMITTER_DATE"),
    };
    signature.into()
}

/// Return the time of a commit as `now` unless the `overriding_variable_name` contains a parseable date,
/// which is used instead.
fn commit_time(overriding_variable_name: &str) -> gix::date::Time {
    std::env::var(overriding_variable_name)
        .ok()
        .and_then(|time| gix::date::parse(&time, Some(std::time::SystemTime::now())).ok())
        .unwrap_or_else(gix::date::Time::now_local_or_utc)
}
