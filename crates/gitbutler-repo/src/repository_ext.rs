#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{io::Write, path::Path, process::Stdio, str};

use anyhow::{anyhow, bail, Context, Result};
use bstr::BString;
use git2::{BlameOptions, Tree};
use gitbutler_commit::{
    commit_buffer::CommitBuffer, commit_ext::CommitExt, commit_headers::CommitHeadersV2,
};
use gitbutler_config::git::{GbConfig, GitConfig};
use gitbutler_error::error::Code;
use gitbutler_reference::{Refname, RemoteRefname};
use tracing::instrument;

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    fn cherry_pick_gitbutler(
        &self,
        head: &git2::Commit,
        to_rebase: &git2::Commit,
    ) -> Result<git2::Index, anyhow::Error>;
    fn find_real_tree(
        &self,
        commit: &git2::Commit,
        side: Option<String>,
    ) -> Result<git2::Tree, anyhow::Error>;
    fn remote_branches(&self) -> Result<Vec<RemoteRefname>>;
    fn remotes_as_string(&self) -> Result<Vec<String>>;
    /// Open a new in-memory repository and executes the provided closure using it.
    /// This is useful when temporary objects are created for the purpose of comparing or getting a diff.
    /// Note that it's the odb that is in-memory, not the working directory.
    /// Data is never persisted to disk, therefore any Oid that are obtained from this closure are not valid outside of it.
    fn in_memory<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&git2::Repository) -> Result<T>;
    /// Returns a version of `&self` that writes new objects into memory, allowing to prevent touching
    /// disk when doing merges.
    /// Note that these written objects don't persist and will vanish with the returned instance.
    fn in_memory_repo(&self) -> Result<git2::Repository>;
    /// Fetches the integration commit from the gitbutler/integration branch
    fn integration_commit(&self) -> Result<git2::Commit<'_>>;
    /// Takes a CommitBuffer and returns it after being signed by by your git signing configuration
    fn sign_buffer(&self, buffer: &CommitBuffer) -> Result<BString>;

    fn checkout_index_builder<'a>(&'a self, index: &'a mut git2::Index) -> CheckoutIndexBuilder;
    fn checkout_index_path_builder<P: AsRef<Path>>(&self, path: P) -> Result<()>;
    fn checkout_tree_builder<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler;
    fn find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch>>;
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
        commit_headers: Option<CommitHeadersV2>,
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

impl RepositoryExt for git2::Repository {
    fn in_memory<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&git2::Repository) -> Result<T>,
    {
        f(&self.in_memory_repo()?)
    }

    fn in_memory_repo(&self) -> Result<git2::Repository> {
        let repo = git2::Repository::open(self.path())?;
        repo.odb()?.add_new_mempack_backend(999)?;
        Ok(repo)
    }

    fn checkout_index_builder<'a>(&'a self, index: &'a mut git2::Index) -> CheckoutIndexBuilder {
        CheckoutIndexBuilder {
            index,
            repo: self,
            checkout_builder: git2::build::CheckoutBuilder::new(),
        }
    }

    fn checkout_index_path_builder<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.path(path.as_ref());
        builder.force();

        let mut index = self.index()?;
        self.checkout_index(Some(&mut index), Some(&mut builder))?;

        Ok(())
    }
    fn checkout_tree_builder<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler {
        CheckoutTreeBuidler {
            tree,
            repo: self,
            checkout_builder: git2::build::CheckoutBuilder::new(),
        }
    }

    fn find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch>> {
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

    #[instrument(level = tracing::Level::DEBUG, skip(self), err(Debug))]
    fn get_wd_tree(&self) -> Result<Tree> {
        let mut index = self.index()?;
        index.add_all(["*"], git2::IndexAddOption::CHECK_PATHSPEC, None)?;
        let oid = index.write_tree()?;
        self.find_tree(oid).map(Into::into).map_err(Into::into)
    }

    fn integration_ref_from_head(&self) -> Result<git2::Reference<'_>> {
        let head_ref = self.head().context("BUG: head must point to a reference")?;
        if head_ref.name_bytes() == b"refs/heads/gitbutler/integration" {
            Ok(head_ref)
        } else {
            Err(anyhow!(
                "Unexpected state: cannot perform operation on non-integration branch"
            ))
        }
    }

    fn integration_commit(&self) -> Result<git2::Commit<'_>> {
        let integration_ref = self.integration_ref_from_head()?;
        Ok(integration_ref.peel_to_commit()?)
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
        fn commit_buffer(
            repository: &git2::Repository,
            commit_buffer: &CommitBuffer,
        ) -> Result<git2::Oid> {
            let oid = repository
                .odb()?
                .write(git2::ObjectType::Commit, &commit_buffer.as_bstring())?;

            Ok(oid)
        }

        let mut buffer: CommitBuffer = self
            .commit_create_buffer(author, committer, message, tree, parents)?
            .into();

        buffer.set_gitbutler_headers(commit_headers);

        let oid = if self.gb_config()?.sign_commits.unwrap_or(false) {
            let signature = self.sign_buffer(&buffer);
            match signature {
                Ok(signature) => self
                    .commit_signed(
                        buffer.as_bstring().to_string().as_str(),
                        signature.to_string().as_str(),
                        None,
                    )
                    .map_err(Into::into),
                Err(e) => {
                    // If signing fails, set the "gitbutler.signCommits" config to false before erroring out
                    self.set_gb_config(GbConfig {
                        sign_commits: Some(false),
                        ..GbConfig::default()
                    })?;
                    Err(anyhow!("Failed to sign commit: {}", e).context(Code::CommitSigningFailed))
                }
            }
        } else {
            commit_buffer(self, &buffer)
        }?;
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

    fn sign_buffer(&self, buffer: &CommitBuffer) -> Result<BString> {
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
                signature_storage.write_all(&buffer.as_bstring())?;
                let buffer_file_to_sign_path = signature_storage.into_temp_path();

                let gpg_program = self.config()?.get_string("gpg.ssh.program");
                let mut gpg_program = gpg_program.unwrap_or("ssh-keygen".to_string());
                // if cmd is "", use gpg
                if gpg_program.is_empty() {
                    gpg_program = "ssh-keygen".to_string();
                }

                let mut cmd = std::process::Command::new(gpg_program);
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
                    cmd.stderr(Stdio::piped());
                    cmd.stdout(Stdio::piped());
                    cmd.stdin(Stdio::null());

                    let child = cmd.spawn()?;
                    output = child.wait_with_output()?;
                } else {
                    cmd.arg(signing_key);
                    cmd.arg(&buffer_file_to_sign_path);
                    cmd.stderr(Stdio::piped());
                    cmd.stdout(Stdio::piped());
                    cmd.stdin(Stdio::null());

                    let child = cmd.spawn()?;
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
                // is gpg
                let gpg_program = self.config()?.get_string("gpg.program");
                let mut gpg_program = gpg_program.unwrap_or("gpg".to_string());
                // if cmd is "", use gpg
                if gpg_program.is_empty() {
                    gpg_program = "gpg".to_string();
                }

                let mut cmd = std::process::Command::new(gpg_program);

                cmd.args(["--status-fd=2", "-bsau", &signing_key])
                    //.arg(&signed_storage)
                    .arg("-")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
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
                    .write_all(&buffer.as_bstring())?;

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

    // cherry-pick, but understands GitButler conflicted states
    fn cherry_pick_gitbutler(
        &self,
        head: &git2::Commit,
        to_rebase: &git2::Commit,
    ) -> Result<git2::Index, anyhow::Error> {
        // we need to do a manual 3-way patch merge
        // find the base, which is the parent of to_rebase
        let base_commit = to_rebase.parent(0)?;
        let base = self.find_real_tree(&base_commit, None)?;
        let ours = self.find_real_tree(head, None)?;
        let thiers = self.find_real_tree(to_rebase, Some(".conflict-side-1".to_string()))?;

        self.merge_trees(&base, &ours, &thiers, None)
            .context("failed to merge trees for cherry pick")
    }

    // find the real tree of a commit, which is the tree of the commit if it's not in a conflicted state
    // or the parent parent tree if it is in a conflicted state
    fn find_real_tree(
        &self,
        commit: &git2::Commit,
        side: Option<String>,
    ) -> Result<git2::Tree, anyhow::Error> {
        let tree = commit.tree()?;
        let entry_name = match side {
            Some(side) => side,
            None => ".conflict-side-0".to_string(),
        };
        if commit.is_conflicted() {
            let conflicted_side = tree
                .get_name(&entry_name)
                .context("Failed to get conflicted side of commit")?;
            self.find_tree(conflicted_side.id())
                .context("failed to find subtree")
        } else {
            self.find_tree(tree.id()).context("failed to find subtree")
        }
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

pub struct CheckoutIndexBuilder<'a> {
    repo: &'a git2::Repository,
    index: &'a mut git2::Index,
    checkout_builder: git2::build::CheckoutBuilder<'a>,
}

impl CheckoutIndexBuilder<'_> {
    pub fn force(&mut self) -> &mut Self {
        self.checkout_builder.force();
        self
    }

    pub fn allow_conflicts(&mut self) -> &mut Self {
        self.checkout_builder.allow_conflicts(true);
        self
    }

    pub fn conflict_style_merge(&mut self) -> &mut Self {
        self.checkout_builder.conflict_style_merge(true);
        self
    }

    pub fn checkout(&mut self) -> Result<()> {
        self.repo
            .checkout_index(Some(&mut self.index), Some(&mut self.checkout_builder))
            .map_err(Into::into)
    }
}
