use anyhow::{Context, anyhow, bail};
use bstr::{BStr, BString, ByteSlice};
use but_core::cmd::prepare_with_shell_on_windows;
use but_core::{GitConfigSettings, RepositoryExt};
use gitbutler_error::error::Code;
use gix::config::Source;
use gix::objs::WriteTo;
use std::borrow::Cow;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

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
        .string(&gix::config::tree::User::NAME)
        .is_none()
        .then_some(name.into());
    let email = config
        .string(&gix::config::tree::User::EMAIL)
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
        config.set_raw_value(&gix::config::tree::User::NAME, name)?;
        something_was_set = true;
    }
    if let Some(email) = email {
        config.set_raw_value(&gix::config::tree::User::EMAIL, email)?;
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
    if let Some(pos) = commit
        .extra_headers()
        .find_pos(gix::objs::commit::SIGNATURE_FIELD_NAME)
    {
        commit.extra_headers.remove(pos);
    }
    if repo.git_settings()?.gitbutler_sign_commits.unwrap_or(false) {
        let mut buf = Vec::new();
        commit.write_to(&mut buf)?;
        match sign_buffer(repo, &buf) {
            Ok(signature) => {
                commit
                    .extra_headers
                    .push((gix::objs::commit::SIGNATURE_FIELD_NAME.into(), signature));
            }
            Err(err) => {
                // If signing fails, turn off signing automatically and let everyone know,
                // but only if it's not already configured globally (which implies user intervention).
                if repo
                    .config_snapshot()
                    .boolean_filter("gitbutler.signCommits", |md| {
                        md.source != gix::config::Source::Local
                    })
                    .is_none()
                {
                    repo.set_git_settings(&GitConfigSettings {
                        gitbutler_sign_commits: Some(false),
                        ..GitConfigSettings::default()
                    })?;
                    return Err(anyhow!("Failed to sign commit: {}", err)
                        .context(Code::CommitSigningFailed));
                } else {
                    tracing::warn!(
                        "Commit signing failed but remains enabled as gitbutler.signCommits is explicitly enabled globally"
                    );
                    return Err(err);
                }
            }
        }
    }

    Ok(repo.write_object(&commit)?.detach())
}

/// Update the commiter of `commit` to be the current one.
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

/// Sign the given `buffer` using configuration from `repo`, just like Git would.
pub fn sign_buffer(repo: &gix::Repository, buffer: &[u8]) -> anyhow::Result<BString> {
    // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
    let config = repo.config_snapshot();
    let signing_key = signing_key(repo)?;
    let sign_format = config.string("gpg.format");
    let is_ssh = if let Some(sign_format) = sign_format {
        sign_format.as_ref() == "ssh"
    } else {
        false
    };

    if is_ssh {
        // write commit data to a temp file so we can sign it
        let mut signature_storage = tempfile::NamedTempFile::new()?;
        signature_storage.write_all(buffer)?;
        let buffer_file_to_sign_path = signature_storage.into_temp_path();

        let gpg_program = config
            .trusted_program("gpg.ssh.program")
            .filter(|program| !program.is_empty())
            .map_or_else(
                || Path::new("ssh-keygen").into(),
                |program| Cow::Owned(program.into_owned().into()),
            );

        let mut signing_cmd = prepare_with_shell_on_windows(gpg_program.into_owned())
            .args(["-Y", "sign", "-n", "git", "-f"]);

        // Write the key to a temp file. This is needs to be created in the
        // same scope where its used; IE: in the command, otherwise the
        // tmpfile will get removed too early.
        let _key_storage;
        signing_cmd = if let Some(signing_key) = as_literal_key(signing_key.as_bstr()) {
            let mut keyfile = tempfile::NamedTempFile::new()?;
            keyfile.write_all(signing_key.as_bytes())?;

            // if on unix
            #[cfg(unix)]
            {
                use std::os::unix::prelude::PermissionsExt;
                // make sure the tempfile permissions are acceptable for a private ssh key
                let mut permissions = keyfile.as_file().metadata()?.permissions();
                permissions.set_mode(0o600);
                keyfile.as_file().set_permissions(permissions)?;
            }

            let keyfile_path = keyfile.path().to_owned();
            _key_storage = keyfile.into_temp_path();
            signing_cmd
                .arg(keyfile_path)
                .arg("-U")
                .arg(buffer_file_to_sign_path.to_path_buf())
        } else {
            let signing_key = config
                .trusted_path("user.signingkey")
                .transpose()?
                .with_context(|| format!("Didn't trust 'ssh.signingKey': {signing_key}"))?;
            signing_cmd
                .arg(signing_key.into_owned())
                .arg(buffer_file_to_sign_path.to_path_buf())
        };
        let output = into_command(signing_cmd)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .output()?;

        if output.status.success() {
            // read signed_storage path plus .sig
            let signature_path = buffer_file_to_sign_path.with_extension("sig");
            let sig_data = std::fs::read(signature_path)?;
            let signature = BString::new(sig_data);
            Ok(signature)
        } else {
            let stderr = BString::new(output.stderr);
            let stdout = BString::new(output.stdout);
            let std_both = format!("{} {}", stdout, stderr);
            bail!("Failed to sign SSH: {}", std_both);
        }
    } else {
        let gpg_program = config
            .trusted_program("gpg.program")
            .filter(|program| !program.is_empty())
            .map_or_else(
                || Path::new("gpg").into(),
                |program| Cow::Owned(program.into_owned().into()),
            );

        let mut cmd = into_command(
            prepare_with_shell_on_windows(gpg_program.as_ref())
                .args(["--status-fd=2", "-bsau"])
                .arg(gix::path::from_bstring(signing_key))
                .arg("-"),
        );
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                bail!(
                    "Could not find '{}'. Please make sure it is in your `PATH` or configure the full path using `gpg.program` in the Git configuration",
                    gpg_program.display()
                )
            }
            Err(err) => {
                return Err(err).context(format!("Could not execute GPG program using {:?}", cmd));
            }
        };
        child.stdin.take().expect("configured").write_all(buffer)?;

        let output = child.wait_with_output()?;
        if output.status.success() {
            // read stdout
            let signature = BString::new(output.stdout);
            Ok(signature)
        } else {
            let stderr = BString::new(output.stderr);
            let stdout = BString::new(output.stdout);
            let std_both = format!("{} {}", stdout, stderr);
            bail!("Failed to sign GPG: {}", std_both);
        }
    }
}

fn into_command(prepare: gix::command::Prepare) -> std::process::Command {
    let cmd: std::process::Command = prepare.into();
    tracing::debug!(?cmd, "command to produce commit signature");
    cmd
}

fn as_literal_key(maybe_key: &BStr) -> Option<&BStr> {
    if let Some(key) = maybe_key.strip_prefix(b"key::") {
        return Some(key.into());
    }
    if maybe_key.starts_with(b"ssh-") {
        return Some(maybe_key);
    }
    None
}

/// Fail if there is no usable signing key.
fn signing_key(repo: &gix::Repository) -> anyhow::Result<BString> {
    if let Some(key) = repo.config_snapshot().string("user.signingkey") {
        return Ok(key.into_owned());
    }
    tracing::info!("Falling back to commiter identity as user.signingKey isn't configured.");
    let mut buf = Vec::<u8>::new();
    repo.committer()
        .transpose()?
        .context("user.signingKey isn't configured and no committer is available either")?
        .actor()
        .trim()
        .write_to(&mut buf)?;
    Ok(buf.into())
}
