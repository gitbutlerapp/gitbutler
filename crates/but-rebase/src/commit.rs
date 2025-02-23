use anyhow::{anyhow, bail, Context};
use bstr::{BString, ByteSlice};
use but_core::cmd::prepare_with_shell_on_windows;
use but_core::{GitConfigSettings, RepositoryExt};
use gitbutler_error::error::Code;
use gix::objs::WriteTo;
use std::borrow::Cow;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

/// What to do with the committer (actor) and the commit time when [creating a new commit](create()).
#[derive(Debug, Copy, Clone)]
pub enum CommitterMode {
    /// Obtain the current committer and the current local time and set it before creating the commit.
    Update,
    /// Keep the currently set committer and time.
    Keep,
}

/// Use the given `commit` and possibly sign it, replacing a possibly existing signature,
/// or removing the signature if GitButler is not configured to keep it.
///
/// Signatures will be removed automatically if signing is disabled to prevent an amended commit
/// to use the old signature.
#[allow(clippy::too_many_arguments)]
pub fn create(
    repo: &gix::Repository,
    mut commit: gix::objs::Commit,
    committer: CommitterMode,
) -> anyhow::Result<gix::ObjectId> {
    match committer {
        CommitterMode::Update => {
            update_committer(repo, &mut commit)?;
        }
        CommitterMode::Keep => {}
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
                repo.set_git_settings(&GitConfigSettings {
                    gitbutler_sign_commits: Some(false),
                    ..GitConfigSettings::default()
                })?;
                return Err(
                    anyhow!("Failed to sign commit: {}", err).context(Code::CommitSigningFailed)
                );
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

/// Sign the given `buffer` using configuration from `repo`, just like Git would.
pub fn sign_buffer(repo: &gix::Repository, buffer: &[u8]) -> anyhow::Result<BString> {
    // check git config for gpg.signingkey
    // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
    let config = repo.config_snapshot();
    let Some(signing_key) = config.string("user.signingkey") else {
        bail!("No signing key found");
    };
    let signing_key = signing_key.to_str().context("non-utf8 signing key")?;
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

        let cmd = prepare_with_shell_on_windows(gpg_program.into_owned())
            .args(["-Y", "sign", "-n", "git", "-f"]);

        // Write the key to a temp file. This is needs to be created in the
        // same scope where its used; IE: in the command, otherwise the
        // tmpfile will get garbage collected
        let mut key_storage = tempfile::NamedTempFile::new()?;
        // support literal ssh key
        let signing_cmd = if let Some(signing_key) = as_literal_key(signing_key) {
            key_storage.write_all(signing_key.as_bytes())?;

            // if on unix
            #[cfg(unix)]
            {
                use std::os::unix::prelude::PermissionsExt;
                // make sure the tempfile permissions are acceptable for a private ssh key
                let mut permissions = key_storage.as_file().metadata()?.permissions();
                permissions.set_mode(0o600);
                key_storage.as_file().set_permissions(permissions)?;
            }

            cmd.arg(key_storage.path())
                .arg("-U")
                .arg(buffer_file_to_sign_path.to_path_buf())
        } else {
            let signing_key = config
                .trusted_path("user.signingkey")
                .transpose()?
                .with_context(|| format!("Didn't trust 'ssh.signingKey': {signing_key}"))?;
            cmd.arg(signing_key.into_owned())
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

        let mut cmd = into_command(prepare_with_shell_on_windows(gpg_program.as_ref()).args([
            "--status-fd=2",
            "-bsau",
            signing_key,
            "-",
        ]));
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                bail!("Could not find '{}'. Please make sure it is in your `PATH` or configure the full path using `gpg.program` in the Git configuration", gpg_program.display())
            }
            Err(err) => {
                return Err(err).context(format!("Could not execute GPG program using {:?}", cmd))
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

fn as_literal_key(maybe_key: &str) -> Option<&str> {
    if let Some(key) = maybe_key.strip_prefix("key::") {
        return Some(key);
    }
    if maybe_key.starts_with("ssh-") {
        return Some(maybe_key);
    }
    None
}
