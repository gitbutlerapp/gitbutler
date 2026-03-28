use anyhow::{Context as _, bail};
use bstr::BStr;
use but_core::{RepositoryExt, commit::SignCommit};
use gix::config::Source;

/// What to do with the committer (actor) and the commit time when [creating a new commit](create()).
#[derive(Debug, Copy, Clone)]
pub enum DateMode {
    /// Update both the committer and author time.
    CommitterUpdateAuthorUpdate,
    /// Obtain the current committer and the current local time and update it, keeping only the author time.
    CommitterUpdateAuthorKeep,
    /// Keep the currently set committer-time and author-time.
    CommitterKeepAuthorKeep,
}

/// Set `user.name` to `name` if unset and `user.email` to `email` if unset, or error if both are already set
/// as per `repo` configuration, and write the changes back to the file at `destination`, keeping
/// user comments and custom formatting.
pub fn save_author_if_unset_in_repo<'a, 'b>(
    repo: &gix::Repository,
    destination: Source,
    name: impl Into<&'a BStr>,
    email: impl Into<&'b BStr>,
) -> anyhow::Result<()> {
    let config = repo.config_snapshot();
    let name = config
        .string(gix::config::tree::User::NAME)
        .is_none()
        .then_some(name.into());
    let email = config
        .string(gix::config::tree::User::EMAIL)
        .is_none()
        .then_some(email.into());
    let config_path = destination
        .storage_location(&mut |name| std::env::var_os(name))
        .context("Failed to determine storage location for Git user configuration")?;
    // TODO(gix): there should be a `gix::Repository` version of this that takes care of this detail.
    let config_path = if config_path.is_relative() {
        if destination == gix::config::Source::Local {
            repo.common_dir().join(config_path)
        } else {
            repo.git_dir().join(config_path)
        }
    } else {
        config_path.into_owned()
    };

    if !config_path.exists() {
        std::fs::create_dir_all(config_path.parent().context("Git user config is never /")?)?;
        std::fs::File::create(&config_path)?;
    }

    let mut config = gix::config::File::from_path_no_includes(config_path.clone(), destination)?;
    let mut something_was_set = false;
    if let Some(name) = name {
        config.set_raw_value(gix::config::tree::User::NAME, name)?;
        something_was_set = true;
    }
    if let Some(email) = email {
        config.set_raw_value(gix::config::tree::User::EMAIL, email)?;
        something_was_set = true;
    }

    if !something_was_set {
        bail!("Refusing to overwrite an existing user.name and user.email");
    }

    config.write_to(
        &mut std::fs::OpenOptions::new()
            .write(true)
            .create(false)
            .truncate(true)
            .open(config_path)?,
    )?;

    Ok(())
}

/// Use the given `commit` and possibly sign it, replacing a possibly existing signature,
/// or removing the signature if GitButler is not configured to keep it.
///
/// Signatures will be removed automatically if signing is disabled to prevent an amended commit
/// to use the old signature.
pub fn create(
    repo: &gix::Repository,
    mut commit: gix::objs::Commit,
    committer: DateMode,
    sign_if_configured: bool,
) -> anyhow::Result<gix::ObjectId> {
    match committer {
        DateMode::CommitterUpdateAuthorKeep => {
            update_committer(repo, &mut commit)?;
        }
        DateMode::CommitterKeepAuthorKeep => {}
        DateMode::CommitterUpdateAuthorUpdate => {
            update_committer(repo, &mut commit)?;
            update_author_time(repo, &mut commit)?;
        }
    }
    let settings = repo.git_settings()?;
    if settings.gitbutler_gerrit_mode.unwrap_or(false) {
        but_gerrit::set_trailers(&mut commit);
    }
    but_core::commit::create(
        repo,
        commit,
        None,
        if sign_if_configured {
            SignCommit::IfSignCommitsEnabled
        } else {
            SignCommit::No
        },
    )
}

/// Update the committer of `commit` to be the current one.
pub(crate) fn update_committer(
    repo: &gix::Repository,
    commit: &mut gix::objs::Commit,
) -> anyhow::Result<()> {
    commit.committer = repo
        .committer()
        .transpose()?
        .context("Need committer to be configured when creating a new commit")?
        .into();
    Ok(())
}

/// Update only the author-time of `commit`.
pub(crate) fn update_author_time(
    repo: &gix::Repository,
    commit: &mut gix::objs::Commit,
) -> anyhow::Result<()> {
    let author = repo
        .author()
        .transpose()?
        .context("Need author to be configured when creating a new commit")?;
    commit.author.time = author.time()?;
    Ok(())
}
