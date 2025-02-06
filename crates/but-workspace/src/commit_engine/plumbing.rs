use anyhow::{anyhow, bail, Context};
use bstr::{BString, ByteSlice};
use but_core::cmd::prepare_with_shell_on_windows;
use but_core::{GitConfigSettings, RepositoryExt};
use gitbutler_error::error::Code;
use gix::objs::WriteTo;
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefLog};
use gix::refs::Target;
use std::borrow::Cow;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

/// Create a commit exactly as specified, and sign it depending on Git and GitButler specific Git configuration.
#[allow(clippy::too_many_arguments)]
pub fn create_commit(
    repo: &gix::Repository,
    update_ref: Option<gix::refs::FullName>,
    author: gix::actor::Signature,
    committer: gix::actor::Signature,
    message: &str,
    tree: gix::ObjectId,
    parents: impl IntoIterator<Item = impl Into<gix::ObjectId>>,
    commit_headers: Option<but_core::commit::HeadersV2>,
) -> anyhow::Result<(gix::ObjectId, Option<gix::refs::transaction::RefEdit>)> {
    let mut commit = gix::objs::Commit {
        message: message.into(),
        tree,
        author,
        committer,
        encoding: None,
        parents: parents.into_iter().map(Into::into).collect(),
        extra_headers: commit_headers.unwrap_or_default().into(),
    };

    if repo.git_settings()?.gitbutler_sign_commits.unwrap_or(false) {
        let mut buf = Vec::new();
        commit.write_to(&mut buf)?;
        let signature = sign_buffer(repo, &buf);
        match signature {
            Ok(signature) => {
                commit.extra_headers.push(("gpgsig".into(), signature));
            }
            Err(e) => {
                // If signing fails, turn off signing automatically and let everyone know,
                repo.set_git_settings(&GitConfigSettings {
                    gitbutler_sign_commits: Some(false),
                    ..GitConfigSettings::default()
                })?;
                return Err(
                    anyhow!("Failed to sign commit: {}", e).context(Code::CommitSigningFailed)
                );
            }
        }
    }

    let new_commit_id = repo.write_object(&commit)?.detach();
    let refedit = if let Some(refname) = update_ref {
        // TODO:(ST) should this be something more like what Git does (also in terms of reflog message)?
        //       Probably should support making a commit in full with `gix`.
        let log_message = message;
        let edit = update_reference(repo, refname, new_commit_id, log_message.into())?;
        Some(edit)
    } else {
        None
    };
    Ok((new_commit_id, refedit))
}

fn update_reference(
    repo: &gix::Repository,
    name: gix::refs::FullName,
    target: gix::ObjectId,
    log_message: BString,
) -> anyhow::Result<gix::refs::transaction::RefEdit> {
    let mut edits = repo.edit_reference(gix::refs::transaction::RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: log_message,
            },
            expected: PreviousValue::Any,
            new: Target::Object(target),
        },
        name,
        deref: false,
    })?;
    debug_assert_eq!(
        edits.len(),
        1,
        "only one reference can be created, splits aren't possible"
    );
    Ok(edits.pop().expect("exactly one edit"))
}

fn sign_buffer(repo: &gix::Repository, buffer: &[u8]) -> anyhow::Result<BString> {
    // check git config for gpg.signingkey
    // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
    let config = repo.config_snapshot();
    let signing_key = config.string("user.signingkey");
    let Some(signing_key) = signing_key else {
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
            cmd.arg(signing_key)
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
