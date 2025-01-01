use std::{io::Write as _, process::Stdio};

use anyhow::{bail, Context as _, Result};
use bstr::BString;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use crate::repository_ext::is_literal_ssh_key;

/// `buffer` is the commit object to sign, but in theory could be anything to compute the signature for.
/// Returns the computed signature.
pub(crate) fn sign_buffer(repository: &git2::Repository, buffer: &[u8]) -> Result<BString> {
    // check git config for gpg.signingkey
    // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
    let signing_key = repository.config()?.get_string("user.signingkey");
    if let Ok(signing_key) = signing_key {
        let sign_format = repository.config()?.get_string("gpg.format");
        let is_ssh = if let Ok(sign_format) = sign_format {
            sign_format == "ssh"
        } else {
            false
        };

        if is_ssh {
            // write commit data to a temp file so we can sign it
            let mut signature_storage = tempfile::NamedTempFile::new()?;
            signature_storage.write_all(buffer)?;
            let buffer_file_to_sign_path = signature_storage.into_temp_path();

            let gpg_program = repository.config()?.get_string("gpg.ssh.program");
            let mut gpg_program = gpg_program.unwrap_or("ssh-keygen".to_string());
            // if cmd is "", use gpg
            if gpg_program.is_empty() {
                gpg_program = "ssh-keygen".to_string();
            }

            let mut cmd_string = format!("{} -Y sign -n git -f ", gpg_program);

            let buffer_file_to_sign_path_str = buffer_file_to_sign_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Failed to convert path to string"))?
                .to_string();

            let output;
            // support literal ssh key
            if let (true, signing_key) = is_literal_ssh_key(&signing_key) {
                // write the key to a temp file
                let mut key_storage = tempfile::NamedTempFile::new()?;
                key_storage.write_all(signing_key.as_bytes())?;

                // if on unix
                #[cfg(unix)]
                {
                    // make sure the tempfile permissions are acceptable for a private ssh key
                    let mut permissions = key_storage.as_file().metadata()?.permissions();
                    permissions.set_mode(0o600);
                    key_storage.as_file().set_permissions(permissions)?;
                }

                let key_file_path = key_storage.into_temp_path();

                let args = format!(
                    "{} -U {}",
                    key_file_path.to_string_lossy(),
                    buffer_file_to_sign_path.to_string_lossy()
                );
                cmd_string += &args;

                let mut signing_cmd: std::process::Command =
                    gix::command::prepare(cmd_string).with_shell().into();

                #[cfg(windows)]
                signing_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

                signing_cmd.stderr(Stdio::piped());
                signing_cmd.stdout(Stdio::piped());
                signing_cmd.stdin(Stdio::null());

                let child = signing_cmd.spawn()?;
                output = child.wait_with_output()?;
            } else {
                let args = format!("{} {}", signing_key, buffer_file_to_sign_path_str);
                cmd_string += &args;

                let mut signing_cmd: std::process::Command =
                    gix::command::prepare(cmd_string).with_shell().into();

                #[cfg(windows)]
                signing_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

                signing_cmd.stderr(Stdio::piped());
                signing_cmd.stdout(Stdio::piped());
                signing_cmd.stdin(Stdio::null());

                let child = signing_cmd.spawn()?;
                output = child.wait_with_output()?;
            }

            if output.status.success() {
                // read signed_storage path plus .sig
                let signature_path = buffer_file_to_sign_path.with_extension("sig");
                let sig_data = std::fs::read(signature_path)?;
                let signature = BString::new(sig_data);
                return Ok(signature);
            } else {
                let stderr = BString::new(output.stderr);
                let stdout = BString::new(output.stdout);
                let std_both = format!("{} {}", stdout, stderr);
                bail!("Failed to sign SSH: {}", std_both);
            }
        } else {
            let gpg_program = repository
                .config()?
                .get_path("gpg.program")
                .ok()
                .filter(|gpg| !gpg.as_os_str().is_empty())
                .unwrap_or_else(|| "gpg".into());

            let mut cmd = std::process::Command::new(&gpg_program);

            cmd.args(["--status-fd=2", "-bsau", &signing_key])
                .arg("-")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::piped());

            #[cfg(windows)]
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    bail!("Could not find '{}'. Please make sure it is in your `PATH` or configure the full path using `gpg.program` in the Git configuration", gpg_program.display())
                }
                Err(err) => {
                    return Err(err)
                        .context(format!("Could not execute GPG program using {:?}", cmd))
                }
            };
            child.stdin.take().expect("configured").write_all(buffer)?;

            let output = child.wait_with_output()?;
            if output.status.success() {
                // read stdout
                let signature = BString::new(output.stdout);
                return Ok(signature);
            } else {
                let stderr = BString::new(output.stderr);
                let stdout = BString::new(output.stdout);
                let std_both = format!("{} {}", stdout, stderr);
                bail!("Failed to sign GPG: {}", std_both);
            }
        }
    }
    Err(anyhow::anyhow!("No signing key found"))
}
