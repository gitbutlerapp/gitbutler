#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{io::Write, path::Path, process::Stdio, str};

use crate::{Config, LogUntil};
use anyhow::{anyhow, bail, Context, Result};
use bstr::BString;
use git2::{BlameOptions, StatusOptions, Tree};
use gitbutler_branch::SignaturePurpose;
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_config::git::{GbConfig, GitConfig};
use gitbutler_error::error::Code;
use gitbutler_oxidize::{
    git2_signature_to_gix_signature, git2_to_gix_object_id, gix_to_git2_oid, gix_to_git2_signature,
};
use gitbutler_reference::{Refname, RemoteRefname};
use gix::fs::is_executable;
use gix::objs::WriteTo;
use tracing::instrument;

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    fn signatures(&self) -> Result<(git2::Signature, git2::Signature)>;
    fn l(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Oid>>;
    fn list_commits(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Commit>>;
    fn log(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Commit>>;
    /// Return `HEAD^{commit}` - ideal for obtaining the integration branch commit in open-workspace mode
    /// when it's clear that it's representing the current state.
    ///
    /// Ideally, this is used in places of `get_workspace_head()`.
    fn head_commit(&self) -> Result<git2::Commit<'_>>;
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
    /// Fetches the workspace commit from the gitbutler/workspace branch
    fn workspace_commit(&self) -> Result<git2::Commit<'_>>;
    /// `buffer` is the commit object to sign, but in theory could be anything to compute the signature for.
    /// Returns the computed signature.
    fn sign_buffer(&self, buffer: &[u8]) -> Result<BString>;

    fn checkout_index_builder<'a>(&'a self, index: &'a mut git2::Index) -> CheckoutIndexBuilder;
    fn checkout_index_path_builder<P: AsRef<Path>>(&self, path: P) -> Result<()>;
    fn checkout_tree_builder<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler;
    fn find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch>>;
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
    fn head_commit(&self) -> Result<git2::Commit<'_>> {
        self.head()
            .context("Failed to get head")?
            .peel_to_commit()
            .context("Failed to get head commit")
    }

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

    /// Note that this will add all untracked and modified files in the worktree to
    /// the object database, and create a tree from it.
    ///
    /// Note that right now, it doesn't skip big files.
    ///
    /// It should also be noted that this will fail if run on an empty branch
    /// or if the HEAD branch has no commits
    #[instrument(level = tracing::Level::DEBUG, skip(self), err(Debug))]
    fn create_wd_tree(&self) -> Result<Tree> {
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
                } else {
                    let file = std::fs::read(&file_path)?;
                    let blob = self.blob(&file)?;

                    let file_type = if is_executable(&file_path.metadata()?) {
                        git2::FileMode::BlobExecutable
                    } else {
                        git2::FileMode::Blob
                    };

                    tree_update_builder.upsert(path, blob, file_type);
                }
            }
        }

        let head_tree = self.head_commit()?.tree()?;
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

    fn workspace_commit(&self) -> Result<git2::Commit<'_>> {
        let workspace_ref = self.workspace_ref_from_head()?;
        Ok(workspace_ref.peel_to_commit()?)
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

    // returns a list of commit oids from the first oid to the second oid
    fn l(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Oid>> {
        match to {
            LogUntil::Commit(oid) => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .hide(oid)
                    .context(format!("failed to hide {}", oid))?;
                revwalk
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
            LogUntil::Take(n) => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .take(n)
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
            LogUntil::When(cond) => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                let mut oids: Vec<git2::Oid> = vec![];
                for oid in revwalk {
                    let oid = oid.context("failed to get oid")?;
                    oids.push(oid);

                    let commit = self.find_commit(oid).context("failed to find commit")?;

                    if cond(&commit).context("failed to check condition")? {
                        break;
                    }
                }
                Ok(oids)
            }
            LogUntil::End => {
                let mut revwalk = self.revwalk().context("failed to create revwalk")?;
                revwalk
                    .push(from)
                    .context(format!("failed to push {}", from))?;
                revwalk
                    .map(|oid| oid.map(Into::into))
                    .collect::<Result<Vec<_>, _>>()
            }
        }
        .context("failed to collect oids")
    }

    fn list_commits(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<git2::Commit>> {
        Ok(self
            .l(from, LogUntil::Commit(to))?
            .into_iter()
            .map(|oid| self.find_commit(oid))
            .collect::<Result<Vec<_>, _>>()?)
    }

    // returns a list of commits from the first oid to the second oid
    fn log(&self, from: git2::Oid, to: LogUntil) -> Result<Vec<git2::Commit>> {
        self.l(from, to)?
            .into_iter()
            .map(|oid| self.find_commit(oid))
            .collect::<Result<Vec<_>, _>>()
            .context("failed to collect commits")
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
                .unwrap_or_else(|| gitbutler_branch::signature(SignaturePurpose::Committer))
        } else {
            gitbutler_branch::signature(SignaturePurpose::Committer)
        }?;

        Ok((author, committer))
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

pub trait GixRepositoryExt: Sized {
    /// Configure the repository for diff operations between trees.
    /// This means it needs an object cache relative to the amount of files in the repository.
    fn for_tree_diffing(self) -> Result<Self>;
}

impl GixRepositoryExt for gix::Repository {
    fn for_tree_diffing(mut self) -> anyhow::Result<Self> {
        let bytes = self.compute_object_cache_size_for_tree_diffs(&***self.index_or_empty()?);
        self.object_cache_size_if_unset(bytes);
        Ok(self)
    }
}
