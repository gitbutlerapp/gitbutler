use crate::Config;
use crate::SignaturePurpose;
use anyhow::{anyhow, bail, Context, Result};
use bstr::BString;
use git2::{StatusOptions, Tree};
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_config::git::{GbConfig, GitConfig};
use gitbutler_error::error::Code;
use gitbutler_oxidize::{
    git2_signature_to_gix_signature, git2_to_gix_object_id, gix_to_git2_oid, gix_to_git2_signature,
};
use gitbutler_reference::{Refname, RemoteRefname};
use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
use gix::fs::is_executable;
use gix::objs::WriteTo;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{io::Write, path::Path, process::Stdio, str};
use tracing::instrument;

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    fn find_branch_by_refname(&self, name: &Refname) -> Result<git2::Branch>;
    /// Returns the common ancestor of the given commit Oids.
    ///
    /// This is like `git merge-base --octopus`.
    ///
    /// This method is called `merge_base_octopussy` so that it doesn't
    /// conflict with the libgit2 binding I upstreamed when it eventually
    /// gets merged.
    fn merge_base_octopussy(&self, ids: &[git2::Oid]) -> Result<git2::Oid>;
    fn signatures(&self) -> Result<(git2::Signature, git2::Signature)>;

    fn remote_branches(&self) -> Result<Vec<RemoteRefname>>;
    fn remotes_as_string(&self) -> Result<Vec<String>>;
    /// `buffer` is the commit object to sign, but in theory could be anything to compute the signature for.
    /// Returns the computed signature.
    fn sign_buffer(&self, buffer: &[u8]) -> Result<BString>;
    fn checkout_tree_builder<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler<'a>;
    fn maybe_find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch>>;
    /// Based on the index, add all data similar to `git add .` and create a tree from it, which is returned.
    fn create_wd_tree(&self) -> Result<Tree>;

    /// Returns the `gitbutler/workspace` branch if the head currently points to it, or fail otherwise.
    /// Use it before any modification to the repository, or extra defensively each time the
    /// workspace is needed.
    ///
    /// This is for safety to assure the repository actually is in 'gitbutler mode'.
    fn workspace_ref_from_head(&self) -> Result<git2::Reference<'_>>;

    #[allow(clippy::too_many_arguments)]
    fn commit_with_signature(
        &self,
        update_ref: Option<&Refname>,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &git2::Tree<'_>,
        parents: &[&git2::Commit<'_>],
        commit_headers: Option<CommitHeadersV2>,
    ) -> Result<git2::Oid>;
}

impl RepositoryExt for git2::Repository {
    fn checkout_tree_builder<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler<'a> {
        CheckoutTreeBuidler {
            tree,
            repo: self,
            checkout_builder: git2::build::CheckoutBuilder::new(),
        }
    }

    fn maybe_find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch>> {
        let branch = self.find_branch(
            &name.simple_name(),
            match name {
                Refname::Virtual(_) | Refname::Local(_) | Refname::Other(_) => {
                    git2::BranchType::Local
                }
                Refname::Remote(_) => git2::BranchType::Remote,
            },
        );
        match branch {
            Ok(branch) => Ok(Some(branch)),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn find_branch_by_refname(&self, name: &Refname) -> Result<git2::Branch> {
        let branch = self.find_branch(
            &name.simple_name(),
            match name {
                Refname::Virtual(_) | Refname::Local(_) | Refname::Other(_) => {
                    git2::BranchType::Local
                }
                Refname::Remote(_) => git2::BranchType::Remote,
            },
        )?;

        Ok(branch)
    }

    /// Note that this will add all untracked and modified files in the worktree to
    /// the object database, and create a tree from it.
    ///
    /// Note that right now, it doesn't skip big files.
    ///
    /// It should also be noted that this will fail if run on an empty branch
    /// or if the HEAD branch has no commits
    #[instrument(level = tracing::Level::DEBUG, skip(self), err(Debug))]
    fn create_wd_tree(&self) -> Result<Tree> {
        let gix_repo = gix::open_opts(
            self.path(),
            gix::open::Options::default().permissions(gix::open::Permissions {
                config: gix::open::permissions::Config {
                    // Whenever we deal with worktree filters, we'd want to have the installation configuration as well.
                    git_binary: cfg!(windows),
                    ..Default::default()
                },
                ..Default::default()
            }),
        )?;
        let (mut pipeline, index) = gix_repo.filter_pipeline(None)?;
        let mut tree_update_builder = git2::build::TreeUpdateBuilder::new();

        let worktree_path = self.workdir().context("Could not find worktree path")?;

        let statuses = self.statuses(Some(
            StatusOptions::new()
                .renames_from_rewrites(false)
                .renames_head_to_index(false)
                .renames_index_to_workdir(false)
                .include_untracked(true)
                .recurse_untracked_dirs(true),
        ))?;

        // Truth table for upsert/remove:
        // | HEAD Tree -> Index | Index -> Worktree | Action    |
        // | add                | delete            | no-action |
        // | modify             | delete            | remove    |
        // |                    | delete            | remove    |
        // | delete             |                   | remove    |
        // | delete             | add               | upsert    |
        // | add                |                   | upsert    |
        // |                    | add               | upsert    |
        // | add                | modify            | upsert    |
        // | modify             | modify            | upsert    |

        let mut buf = Vec::with_capacity(1024);
        for status_entry in &statuses {
            let status = status_entry.status();
            let path = status_entry.path().context("Failed to get path")?;
            let path = Path::new(path);

            if status.is_index_new() && status.is_wt_deleted() {
                // This is a no-op
            } else if (status.is_index_deleted() && !status.is_wt_new()) || status.is_wt_deleted() {
                tree_update_builder.remove(path);
            } else {
                let file_path = worktree_path.join(path).to_owned();

                if file_path.is_symlink() {
                    let resolved_path = file_path.read_link()?;
                    let path_str = resolved_path
                        .to_str()
                        .context("Failed to convert path to str")?;

                    let blob = self.blob(path_str.as_bytes())?;
                    tree_update_builder.upsert(path, blob, git2::FileMode::Link);
                } else if let io::Result::Ok(file) = std::fs::File::open(&file_path) {
                    // We might have an entry for a file that does not exist on disk,
                    // like in the case of a file conflict.
                    let file_for_git = pipeline.convert_to_git(file, path, &index)?;
                    let data = match file_for_git {
                        ToGitOutcome::Unchanged(mut file) => {
                            buf.clear();
                            std::io::copy(&mut file, &mut buf)?;
                            &buf
                        }
                        ToGitOutcome::Buffer(buf) => buf,
                        ToGitOutcome::Process(mut read) => {
                            buf.clear();
                            std::io::copy(&mut read, &mut buf)?;
                            &buf
                        }
                    };
                    let blob_id = self.blob(data)?;

                    let file_type = if is_executable(&file_path.metadata()?) {
                        git2::FileMode::BlobExecutable
                    } else {
                        git2::FileMode::Blob
                    };

                    tree_update_builder.upsert(path, blob_id, file_type);
                }
            }
        }

        let head_tree = self.head()?.peel_to_tree()?;
        let tree_oid = tree_update_builder.create_updated(self, &head_tree)?;

        Ok(self.find_tree(tree_oid)?)
    }

    fn workspace_ref_from_head(&self) -> Result<git2::Reference<'_>> {
        let head_ref = self.head().context("BUG: head must point to a reference")?;
        if head_ref.name_bytes() == b"refs/heads/gitbutler/workspace" {
            Ok(head_ref)
        } else {
            Err(anyhow!(
                "Unexpected state: cannot perform operation on non-workspace branch"
            ))
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
        commit_headers: Option<CommitHeadersV2>,
    ) -> Result<git2::Oid> {
        let repo = gix::open(self.path())?;
        let mut commit = gix::objs::Commit {
            message: message.into(),
            tree: git2_to_gix_object_id(tree.id()),
            author: git2_signature_to_gix_signature(author),
            committer: git2_signature_to_gix_signature(committer),
            encoding: None,
            parents: parents
                .iter()
                .map(|commit| git2_to_gix_object_id(commit.id()))
                .collect(),
            extra_headers: commit_headers.unwrap_or_default().into(),
        };

        if self.gb_config()?.sign_commits.unwrap_or(false) {
            let mut buf = Vec::new();
            commit.write_to(&mut buf)?;
            let signature = self.sign_buffer(&buf);
            match signature {
                Ok(signature) => {
                    commit.extra_headers.push(("gpgsig".into(), signature));
                }
                Err(e) => {
                    // If signing fails, set the "gitbutler.signCommits" config to false before erroring out
                    self.set_gb_config(GbConfig {
                        sign_commits: Some(false),
                        ..GbConfig::default()
                    })?;
                    return Err(
                        anyhow!("Failed to sign commit: {}", e).context(Code::CommitSigningFailed)
                    );
                }
            }
        }
        // TODO: extra-headers should be supported in `gix` directly.
        let oid = gix_to_git2_oid(repo.write_object(&commit)?);

        // update reference
        if let Some(refname) = update_ref {
            self.reference(&refname.to_string(), oid, true, message)?;
        }
        Ok(oid)
    }

    fn sign_buffer(&self, buffer: &[u8]) -> Result<BString> {
        // check git config for gpg.signingkey
        // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
        let signing_key = self.config()?.get_string("user.signingkey");
        if let Ok(signing_key) = signing_key {
            let sign_format = self.config()?.get_string("gpg.format");
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

                let gpg_program = self.config()?.get_string("gpg.ssh.program");
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
                } else {
                    let args = format!("{} {}", signing_key, buffer_file_to_sign_path_str);
                    cmd_string += &args;
                };
                let mut signing_cmd: std::process::Command =
                    gix::command::prepare(cmd_string).with_shell().into();
                let output = signing_cmd
                    .stderr(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stdin(Stdio::null())
                    .output()?;

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
                let gpg_program = self
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

    fn remotes_as_string(&self) -> Result<Vec<String>> {
        Ok(self.remotes().map(|string_array| {
            string_array
                .iter()
                .filter_map(|s| s.map(String::from))
                .collect()
        })?)
    }

    fn remote_branches(&self) -> Result<Vec<RemoteRefname>> {
        self.branches(Some(git2::BranchType::Remote))?
            .flatten()
            .map(|(branch, _)| {
                RemoteRefname::try_from(&branch).context("failed to convert branch to remote name")
            })
            .collect::<Result<Vec<_>>>()
    }

    fn signatures(&self) -> Result<(git2::Signature, git2::Signature)> {
        let repo = gix::open(self.path())?;

        let author = repo
            .author()
            .transpose()?
            .map(gix_to_git2_signature)
            .transpose()?
            .context("No author is configured in Git")
            .context(Code::AuthorMissing)?;

        let config: Config = self.into();
        let committer = if config.user_real_comitter()? {
            repo.committer()
                .transpose()?
                .map(gix_to_git2_signature)
                .unwrap_or_else(|| crate::signature(SignaturePurpose::Committer))
        } else {
            crate::signature(SignaturePurpose::Committer)
        }?;

        Ok((author, committer))
    }

    fn merge_base_octopussy(&self, ids: &[git2::Oid]) -> Result<git2::Oid> {
        if ids.len() < 2 {
            bail!("Merge base octopussy requires at least two commit ids to operate on");
        };

        let first_oid = ids[0];

        let output = ids[1..].iter().try_fold(first_oid, |base, oid| {
            self.merge_base(base, *oid)
                .context("Failed to find merge base")
        })?;

        Ok(output)
    }
}

/// Signs the buffer with the configured gpg key, returning the signature.
pub fn is_literal_ssh_key(string: &str) -> (bool, &str) {
    if let Some(key) = string.strip_prefix("key::") {
        return (true, key);
    }
    if string.starts_with("ssh-") {
        return (true, string);
    }
    (false, string)
}

pub struct CheckoutTreeBuidler<'a> {
    repo: &'a git2::Repository,
    tree: &'a git2::Tree<'a>,
    checkout_builder: git2::build::CheckoutBuilder<'a>,
}

impl CheckoutTreeBuidler<'_> {
    pub fn force(&mut self) -> &mut Self {
        self.checkout_builder.force();
        self
    }

    pub fn remove_untracked(&mut self) -> &mut Self {
        self.checkout_builder.remove_untracked(true);
        self
    }

    pub fn checkout(&mut self) -> Result<()> {
        self.repo
            .checkout_tree(self.tree.as_object(), Some(&mut self.checkout_builder))
            .map_err(Into::into)
    }
}
