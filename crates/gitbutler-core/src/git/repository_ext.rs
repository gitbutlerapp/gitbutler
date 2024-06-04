use anyhow::{anyhow, bail, Context, Result};
use git2::{BlameOptions, Repository, Tree};
use std::{path::Path, process::Stdio, str};
use tracing::instrument;

use crate::{config::CFG_SIGN_COMMITS, error::Code};

use super::Refname;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    /// Based on the index, add all data similar to `git add .` and create a tree from it, which is returned.
    fn get_wd_tree(&self) -> Result<Tree>;

    /// Returns the `gitbutler/integration` branch if the head currently points to it, or fail otherwise.
    /// Use it before any modification to the repository, or extra defensively each time the
    /// integration is needed.
    ///
    /// This is for safety to assure the repository actually is in 'gitbutler mode'.
    fn integration_ref_from_head(&self) -> Result<git2::Reference<'_>>;

    #[allow(clippy::too_many_arguments)]
    fn commit_with_signature(
        &self,
        update_ref: Option<&Refname>,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &git2::Tree<'_>,
        parents: &[&git2::Commit<'_>],
        change_id: Option<&str>,
    ) -> Result<git2::Oid>;

    fn blame(
        &self,
        path: &Path,
        min_line: u32,
        max_line: u32,
        oldest_commit: git2::Oid,
        newest_commit: git2::Oid,
    ) -> Result<git2::Blame, git2::Error>;
}

impl RepositoryExt for Repository {
    #[instrument(level = tracing::Level::DEBUG, skip(self), err(Debug))]
    fn get_wd_tree(&self) -> Result<Tree> {
        let mut index = self.index()?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
        let oid = index.write_tree()?;
        self.find_tree(oid).map(Into::into).map_err(Into::into)
    }

    fn integration_ref_from_head(&self) -> Result<git2::Reference<'_>> {
        let head_ref = self.head().context("BUG: head must point to a reference")?;
        if head_ref.name_bytes() == b"refs/heads/gitbutler/integration" {
            Ok(head_ref)
        } else {
            bail!("Unexpected state: cannot perform operation on non-integration branch")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_with_signature(
        &self,
        update_ref: Option<&Refname>,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &git2::Tree<'_>,
        parents: &[&git2::Commit<'_>],
        change_id: Option<&str>,
    ) -> Result<git2::Oid> {
        let commit_buffer = self.commit_create_buffer(author, committer, message, tree, parents)?;

        let commit_buffer = inject_change_id(&commit_buffer, change_id)?;

        let should_sign = self.config()?.get_bool(CFG_SIGN_COMMITS).unwrap_or(false);

        let oid = if should_sign {
            let signature = sign_buffer(self, &commit_buffer)
                .map_err(|e| anyhow!(e).context(Code::CommitSigningFailed))?;
            self.commit_signed(&commit_buffer, &signature, None)?
        } else {
            self.odb()?
                .write(git2::ObjectType::Commit, commit_buffer.as_bytes())?
        };
        // update reference
        if let Some(refname) = update_ref {
            self.reference(&refname.to_string(), oid, true, message)?;
        }
        Ok(oid)
    }

    fn blame(
        &self,
        path: &Path,
        min_line: u32,
        max_line: u32,
        oldest_commit: git2::Oid,
        newest_commit: git2::Oid,
    ) -> Result<git2::Blame, git2::Error> {
        let mut opts = BlameOptions::new();
        opts.min_line(min_line as usize)
            .max_line(max_line as usize)
            .newest_commit(newest_commit)
            .oldest_commit(oldest_commit)
            .first_parent(true);
        self.blame_file(path, Some(&mut opts))
    }
}

/// Signs the buffer with the configured gpg key, returning the signature.
fn sign_buffer(repo: &git2::Repository, buffer: &String) -> Result<String> {
    // check git config for gpg.signingkey
    // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
    let signing_key = repo.config()?.get_string("user.signingkey");
    if let Ok(signing_key) = signing_key {
        let sign_format = repo.config()?.get_string("gpg.format");
        let is_ssh = if let Ok(sign_format) = sign_format {
            sign_format == "ssh"
        } else {
            false
        };

        if is_ssh {
            // write commit data to a temp file so we can sign it
            let mut signature_storage = tempfile::NamedTempFile::new()?;
            signature_storage.write_all(buffer.as_ref())?;
            let buffer_file_to_sign_path = signature_storage.into_temp_path();

            let gpg_program = repo.config()?.get_string("gpg.ssh.program");
            let mut cmd =
                std::process::Command::new(gpg_program.unwrap_or("ssh-keygen".to_string()));
            cmd.args(["-Y", "sign", "-n", "git", "-f"]);

            #[cfg(windows)]
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

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

                cmd.arg(&key_file_path);
                cmd.arg("-U");
                cmd.arg(&buffer_file_to_sign_path);
                cmd.stdout(Stdio::piped());
                cmd.stdin(Stdio::null());

                let child = cmd.spawn()?;
                output = child.wait_with_output()?;
            } else {
                cmd.arg(signing_key);
                cmd.arg(&buffer_file_to_sign_path);
                cmd.stdout(Stdio::piped());
                cmd.stdin(Stdio::null());

                let child = cmd.spawn()?;
                output = child.wait_with_output()?;
            }

            if output.status.success() {
                // read signed_storage path plus .sig
                let signature_path = buffer_file_to_sign_path.with_extension("sig");
                let sig_data = std::fs::read(signature_path)?;
                let signature = String::from_utf8_lossy(&sig_data).into_owned();
                return Ok(signature);
            }
        } else {
            // is gpg
            let gpg_program = repo.config()?.get_string("gpg.program");
            let mut cmd = std::process::Command::new(gpg_program.unwrap_or("gpg".to_string()));
            cmd.args(["--status-fd=2", "-bsau", &signing_key])
                //.arg(&signed_storage)
                .arg("-")
                .stdout(Stdio::piped())
                .stdin(Stdio::piped());

            #[cfg(windows)]
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

            let mut child = cmd
                .spawn()
                .context(anyhow::format_err!("failed to spawn {:?}", cmd))?;
            child
                .stdin
                .take()
                .expect("configured")
                .write_all(buffer.to_string().as_ref())?;

            let output = child.wait_with_output()?;
            if output.status.success() {
                // read stdout
                let signature = String::from_utf8_lossy(&output.stdout).into_owned();
                return Ok(signature);
            }
        }
    }
    Err(anyhow::anyhow!("Unsupported commit signing method"))
}

fn is_literal_ssh_key(string: &str) -> (bool, &str) {
    if let Some(key) = string.strip_prefix("key::") {
        return (true, key);
    }
    if string.starts_with("ssh-") {
        return (true, string);
    }
    (false, string)
}

// in commit_buffer, inject a line right before the first `\n\n` that we see:
// `change-id: <id>`
fn inject_change_id(commit_buffer: &[u8], change_id: Option<&str>) -> Result<String> {
    // if no change id, generate one
    let change_id = change_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| format!("{}", uuid::Uuid::new_v4()));

    let commit_ends_in_newline = commit_buffer.ends_with(b"\n");
    let commit_buffer = str::from_utf8(commit_buffer)?;
    let lines = commit_buffer.lines();
    let mut new_buffer = String::new();
    let mut found = false;
    for line in lines {
        if line.is_empty() && !found {
            new_buffer.push_str(&format!("change-id {}\n", change_id));
            found = true;
        }
        new_buffer.push_str(line);
        new_buffer.push('\n');
    }
    if !commit_ends_in_newline {
        // strip last \n
        new_buffer.pop();
    }
    Ok(new_buffer)
}
