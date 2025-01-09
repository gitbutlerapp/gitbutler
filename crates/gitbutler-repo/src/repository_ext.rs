use crate::Config;
use crate::SignaturePurpose;
use anyhow::{anyhow, bail, Context, Result};
use bstr::{BStr, BString, ByteSlice};
use git2::Tree;
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_config::git::{GbConfig, GitConfig};
use gitbutler_error::error::Code;
use gitbutler_oxidize::{
    git2_signature_to_gix_signature, git2_to_gix_object_id, gix_to_git2_oid, gix_to_git2_signature,
};
use gitbutler_reference::{Refname, RemoteRefname};
use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
use gix::objs::WriteTo;
use gix::status::index_worktree;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::{io::Write, process::Stdio, str};
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
    /// Add all untracked and modified files in the worktree to
    /// the object database, and create a tree from it.
    ///
    /// Use `untracked_limit_in_bytes` to control the maximum file size for untracked files
    /// before we stop tracking them automatically. Set it to 0 to disable the limit.
    ///
    /// It should also be noted that this will fail if run on an empty branch
    /// or if the HEAD branch has no commits.
    fn create_wd_tree(&self, untracked_limit_in_bytes: u64) -> Result<Tree>;

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

    #[instrument(level = tracing::Level::DEBUG, skip(self, untracked_limit_in_bytes), err(Debug))]
    fn create_wd_tree(&self, untracked_limit_in_bytes: u64) -> Result<Tree> {
        use bstr::ByteSlice;
        use gix::dir::walk::EmissionMode;
        use gix::status;
        use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};
        use gix::status::tree_index::TrackRenames;

        let repo = gix::open_opts(
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
        let (mut pipeline, index) = repo.filter_pipeline(None)?;
        let workdir = repo.work_dir().context("Need non-bare repository")?;
        let mut added_worktree_file = |rela_path: &BStr,
                                       head_tree_editor: &mut gix::object::tree::Editor<'_>|
         -> anyhow::Result<bool> {
            let rela_path_as_path = gix::path::from_bstr(rela_path);
            let path = workdir.join(&rela_path_as_path);
            let Ok(md) = std::fs::symlink_metadata(&path) else {
                return Ok(false);
            };
            if untracked_limit_in_bytes != 0 && md.len() > untracked_limit_in_bytes {
                return Ok(false);
            }
            let (id, kind) = if md.is_symlink() {
                let target = std::fs::read_link(&path).with_context(|| {
                    format!(
                        "Failed to read link at '{}' for adding to the object database",
                        path.display()
                    )
                })?;
                let id = repo.write_blob(gix::path::into_bstr(target).as_bytes())?;
                (id, gix::object::tree::EntryKind::Link)
            } else if md.is_file() {
                let file = std::fs::File::open(&path).with_context(|| {
                    format!(
                        "Could not open file at '{}' for adding it to the object database",
                        path.display()
                    )
                })?;
                let file_for_git =
                    pipeline.convert_to_git(file, rela_path_as_path.as_ref(), &index)?;
                let id = match file_for_git {
                    ToGitOutcome::Unchanged(mut file) => repo.write_blob_stream(&mut file)?,
                    ToGitOutcome::Buffer(buf) => repo.write_blob(buf)?,
                    ToGitOutcome::Process(mut read) => repo.write_blob_stream(&mut read)?,
                };

                let kind = if gix::fs::is_executable(&md) {
                    gix::object::tree::EntryKind::BlobExecutable
                } else {
                    gix::object::tree::EntryKind::Blob
                };
                (id, kind)
            } else {
                // This is probably a type-change to something we can't track. Instead of keeping
                // what's in `HEAD^{tree}` we remove the entry.
                head_tree_editor.remove(rela_path)?;
                return Ok(true);
            };

            head_tree_editor.upsert(rela_path, kind, id)?;
            Ok(true)
        };
        let mut head_tree_editor = repo.edit_tree(repo.head_tree_id()?)?;
        let status_changes = repo
            .status(gix::progress::Discard)?
            .tree_index_track_renames(TrackRenames::Disabled)
            .index_worktree_rewrites(None)
            .index_worktree_submodules(gix::status::Submodule::Given {
                ignore: gix::submodule::config::Ignore::Dirty,
                check_dirty: true,
            })
            .index_worktree_options_mut(|opts| {
                if let Some(opts) = opts.dirwalk_options.as_mut() {
                    opts.set_emit_ignored(None)
                        .set_emit_pruned(false)
                        .set_emit_tracked(false)
                        .set_emit_untracked(EmissionMode::Matching)
                        .set_emit_collapsed(None);
                }
            })
            .into_iter(None)?;

        let mut worktreepaths_changed = HashSet::new();
        // We have to apply untracked items last, but don't have ordering here so impose it ourselves.
        let mut untracked_items = Vec::new();
        for change in status_changes {
            let change = change?;
            match change {
                status::Item::TreeIndex(gix::diff::index::Change::Deletion {
                    location, ..
                }) => {
                    // These changes play second fiddle - they are overwritten by worktree-changes,
                    // or we assure we don't overwrite, as we may arrive out of order.
                    if !worktreepaths_changed.contains(location.as_bstr()) {
                        head_tree_editor.remove(location.as_ref())?;
                    }
                }
                status::Item::TreeIndex(
                    gix::diff::index::Change::Addition {
                        location,
                        entry_mode,
                        id,
                        ..
                    }
                    | gix::diff::index::Change::Modification {
                        location,
                        entry_mode,
                        id,
                        ..
                    },
                ) => {
                    if let Some(entry_mode) = entry_mode
                        .to_tree_entry_mode()
                        // These changes play second fiddle - they are overwritten by worktree-changes,
                        // or we assure we don't overwrite, as we may arrive out of order.
                        .filter(|_| !worktreepaths_changed.contains(location.as_bstr()))
                    {
                        head_tree_editor.upsert(
                            location.as_ref(),
                            entry_mode.kind(),
                            id.as_ref(),
                        )?;
                    }
                }
                status::Item::IndexWorktree(index_worktree::Item::Modification {
                    rela_path,
                    status: EntryStatus::Change(Change::Removed),
                    ..
                }) => {
                    head_tree_editor.remove(rela_path.as_bstr())?;
                    worktreepaths_changed.insert(rela_path);
                }
                // modified or untracked files are unconditionally added as blob.
                // Note that this implementation will re-read the whole blob even on type-change
                status::Item::IndexWorktree(index_worktree::Item::Modification {
                    rela_path,
                    status:
                        EntryStatus::Change(Change::Type | Change::Modification { .. })
                        | EntryStatus::IntentToAdd,
                    ..
                }) => {
                    if added_worktree_file(rela_path.as_ref(), &mut head_tree_editor)? {
                        worktreepaths_changed.insert(rela_path);
                    }
                }
                status::Item::IndexWorktree(index_worktree::Item::DirectoryContents {
                    entry:
                        gix::dir::Entry {
                            rela_path,
                            status: gix::dir::entry::Status::Untracked,
                            ..
                        },
                    ..
                }) => {
                    untracked_items.push(rela_path);
                }
                status::Item::IndexWorktree(index_worktree::Item::Modification {
                    rela_path,
                    status: EntryStatus::Change(Change::SubmoduleModification(change)),
                    ..
                }) => {
                    if let Some(possibly_changed_head_commit) = change.checked_out_head_id {
                        head_tree_editor.upsert(
                            rela_path.as_bstr(),
                            gix::object::tree::EntryKind::Commit,
                            possibly_changed_head_commit,
                        )?;
                        worktreepaths_changed.insert(rela_path);
                    }
                }
                status::Item::IndexWorktree(index_worktree::Item::Rewrite { .. })
                | status::Item::TreeIndex(gix::diff::index::Change::Rewrite { .. }) => {
                    unreachable!("disabled")
                }
                status::Item::IndexWorktree(
                    index_worktree::Item::Modification {
                        status: EntryStatus::Conflict(_) | EntryStatus::NeedsUpdate(_),
                        ..
                    }
                    | index_worktree::Item::DirectoryContents {
                        entry:
                            gix::dir::Entry {
                                status:
                                    gix::dir::entry::Status::Tracked
                                    | gix::dir::entry::Status::Pruned
                                    | gix::dir::entry::Status::Ignored(_),
                                ..
                            },
                        ..
                    },
                ) => {}
            }
        }

        for rela_path in untracked_items {
            added_worktree_file(rela_path.as_ref(), &mut head_tree_editor)?;
        }

        let tree_oid = gix_to_git2_oid(head_tree_editor.write()?);
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
        let repo = gix::open(self.path())?;
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

            let gpg_program = config.trusted_program("gpg.ssh.program");
            let mut gpg_program = gpg_program.unwrap_or(Cow::Borrowed(OsStr::new("ssh-keygen")));
            // if cmd is "", use gpg
            if gpg_program.is_empty() {
                gpg_program = Cow::Borrowed(OsStr::new("ssh-keygen"));
            }

            let mut cmd_string = format!(
                "{} -Y sign -n git -f ",
                gix::path::os_str_into_bstr(gpg_program.as_ref())?
            );

            let buffer_file_to_sign_path_str = buffer_file_to_sign_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Failed to convert path to string"))?
                .to_string();

            // support literal ssh key
            if let (true, signing_key) = is_literal_ssh_key(signing_key) {
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
            let mut signing_cmd: std::process::Command = gix::command::prepare(cmd_string)
                .with_shell_disallow_manual_argument_splitting()
                .into();
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
                .map(|program| Cow::Owned(program.into_owned().into()))
                .unwrap_or_else(|| Path::new("gpg").into());

            let mut cmd = std::process::Command::new(gpg_program.as_ref());

            cmd.args(["--status-fd=2", "-bsau", signing_key])
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
                Ok(signature)
            } else {
                let stderr = BString::new(output.stderr);
                let stdout = BString::new(output.stdout);
                let std_both = format!("{} {}", stdout, stderr);
                bail!("Failed to sign GPG: {}", std_both);
            }
        }
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
