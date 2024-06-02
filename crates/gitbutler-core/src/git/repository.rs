use super::{Oid, Refname, Result, Url};
use git2::{BlameOptions, Submodule};
use git2_hooks::HookResult;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Stdio;
use std::{io::Write, path::Path, str};

// wrapper around git2::Repository to get control over how it's used.
pub struct Repository(git2::Repository);

impl<'a> From<&'a Repository> for &'a git2::Repository {
    fn from(repo: &'a Repository) -> Self {
        &repo.0
    }
}

impl From<git2::Repository> for Repository {
    fn from(repo: git2::Repository) -> Self {
        Self(repo)
    }
}

impl Repository {
    pub fn init<P: AsRef<Path>>(path: P) -> Result<Self> {
        let inner = git2::Repository::init(path)?;
        Ok(Repository(inner))
    }

    pub fn init_opts<P: AsRef<Path>>(path: P, opts: &git2::RepositoryInitOptions) -> Result<Self> {
        let inner = git2::Repository::init_opts(path, opts)?;
        Ok(Repository(inner))
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let inner = git2::Repository::open(path)?;
        Ok(Repository(inner))
    }

    pub fn add_submodule<P: AsRef<Path>>(&self, url: &Url, path: P) -> Result<Submodule<'_>> {
        self.0
            .submodule(&url.to_string(), path.as_ref(), false)
            .map_err(Into::into)
    }

    pub fn rebase(
        &self,
        branch_oid: Option<Oid>,
        upstream_oid: Option<Oid>,
        onto_oid: Option<Oid>,
        opts: Option<&mut git2::RebaseOptions<'_>>,
    ) -> Result<git2::Rebase<'_>> {
        let annotated_branch = if let Some(branch) = branch_oid {
            Some(self.0.find_annotated_commit(branch.into())?)
        } else {
            None
        };

        let annotated_upstream = if let Some(upstream) = upstream_oid {
            Some(self.0.find_annotated_commit(upstream.into())?)
        } else {
            None
        };

        let annotated_onto = if let Some(onto) = onto_oid {
            Some(self.0.find_annotated_commit(onto.into())?)
        } else {
            None
        };

        self.0
            .rebase(
                annotated_branch.as_ref(),
                annotated_upstream.as_ref(),
                annotated_onto.as_ref(),
                opts,
            )
            .map_err(Into::into)
    }

    pub fn merge_base(&self, one: Oid, two: Oid) -> Result<Oid> {
        self.0
            .merge_base(one.into(), two.into())
            .map(Oid::from)
            .map_err(Into::into)
    }

    pub fn merge_trees(
        &self,
        ancestor_tree: &git2::Tree<'_>,
        our_tree: &git2::Tree<'_>,
        their_tree: &git2::Tree<'_>,
    ) -> Result<git2::Index> {
        self.0
            .merge_trees(ancestor_tree, our_tree, their_tree, None)
            .map_err(Into::into)
    }

    pub fn diff_tree_to_tree(
        &self,
        old_tree: Option<&git2::Tree<'_>>,
        new_tree: Option<&git2::Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        self.0
            .diff_tree_to_tree(old_tree, new_tree, opts)
            .map_err(Into::into)
    }

    pub fn diff_tree_to_workdir(
        &self,
        old_tree: Option<&git2::Tree<'_>>,
        opts: Option<&mut git2::DiffOptions>,
    ) -> Result<git2::Diff<'_>> {
        if let Ok(mut index) = self.0.index() {
            index.update_all(vec!["*"], None)?;
        }
        self.0
            .diff_tree_to_workdir_with_index(old_tree, opts)
            .map_err(Into::into)
    }

    pub fn reset(
        &self,
        commit: &git2::Commit<'_>,
        kind: git2::ResetType,
        checkout: Option<&mut git2::build::CheckoutBuilder<'_>>,
    ) -> Result<()> {
        let commit: &git2::Commit = commit;
        self.0
            .reset(commit.as_object(), kind, checkout)
            .map_err(Into::into)
    }

    pub fn find_reference(&self, name: &Refname) -> Result<git2::Reference> {
        self.0.find_reference(&name.to_string()).map_err(Into::into)
    }

    pub fn head(&self) -> Result<git2::Reference> {
        self.0.head().map_err(Into::into)
    }

    pub fn find_tree(&self, id: Oid) -> Result<git2::Tree> {
        self.0.find_tree(id.into()).map_err(Into::into)
    }

    pub fn find_commit(&self, id: Oid) -> Result<git2::Commit> {
        self.0.find_commit(id.into()).map_err(Into::into)
    }

    pub fn revwalk(&self) -> Result<git2::Revwalk> {
        self.0.revwalk().map_err(Into::into)
    }

    pub fn is_path_ignored<P: AsRef<Path>>(&self, path: P) -> Result<bool> {
        self.0.is_path_ignored(path).map_err(Into::into)
    }

    pub fn branches(
        &self,
        filter: Option<git2::BranchType>,
    ) -> Result<impl Iterator<Item = Result<git2::Branch>>> {
        self.0
            .branches(filter)
            .map(|branches| {
                branches.map(|branch| branch.map(|(branch, _)| branch).map_err(Into::into))
            })
            .map_err(Into::into)
    }

    pub fn index(&self) -> Result<git2::Index> {
        self.0.index().map_err(Into::into)
    }

    pub fn index_size(&self) -> Result<usize> {
        Ok(self.0.index()?.len())
    }

    pub fn blob_path<P: AsRef<Path>>(&self, path: P) -> Result<Oid> {
        self.0
            .blob_path(path.as_ref())
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn cherry_pick(&self, base: &git2::Commit, target: &git2::Commit) -> Result<git2::Index> {
        self.0
            .cherrypick_commit(target, base, 0, None)
            .map_err(Into::into)
    }

    pub fn blob(&self, data: &[u8]) -> Result<Oid> {
        self.0.blob(data).map(Into::into).map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn commit(
        &self,
        update_ref: Option<&Refname>,
        author: &git2::Signature<'_>,
        committer: &git2::Signature<'_>,
        message: &str,
        tree: &git2::Tree<'_>,
        parents: &[&git2::Commit<'_>],
        change_id: Option<&str>,
    ) -> Result<Oid> {
        let commit_buffer = self
            .0
            .commit_create_buffer(author, committer, message, tree, parents)?;

        let commit_buffer = Self::inject_change_id(&commit_buffer, change_id)?;

        let oid = self.commit_buffer(commit_buffer)?;

        // update reference
        if let Some(refname) = update_ref {
            self.0.reference(&refname.to_string(), oid, true, message)?;
        }
        Ok(oid.into())
    }

    /// takes raw commit data and commits it to the repository
    /// - if the git config commit.gpgSign is set, it will sign the commit
    /// returns an oid of the new commit object
    pub fn commit_buffer(&self, buffer: String) -> Result<git2::Oid> {
        // check git config for gpg.signingkey
        let should_sign = self.0.config()?.get_bool("commit.gpgSign").unwrap_or(false);
        if should_sign {
            // TODO: support gpg.ssh.defaultKeyCommand to get the signing key if this value doesn't exist
            let signing_key = self.0.config()?.get_string("user.signingkey");
            if let Ok(signing_key) = signing_key {
                let sign_format = self.0.config()?.get_string("gpg.format");
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

                    let gpg_program = self.0.config()?.get_string("gpg.ssh.program");
                    let mut cmd =
                        std::process::Command::new(gpg_program.unwrap_or("ssh-keygen".to_string()));
                    cmd.args(["-Y", "sign", "-n", "git", "-f"]);

                    #[cfg(windows)]
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

                    let output;
                    // support literal ssh key
                    if let (true, signing_key) = Self::is_literal_ssh_key(&signing_key) {
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
                        let signature = String::from_utf8_lossy(&sig_data);
                        let oid = self
                            .0
                            .commit_signed(&buffer, &signature, None)
                            .map(Into::into)
                            .map_err(Into::into);
                        return oid;
                    }
                } else {
                    // is gpg
                    let gpg_program = self.0.config()?.get_string("gpg.program");
                    let mut cmd =
                        std::process::Command::new(gpg_program.unwrap_or("gpg".to_string()));
                    cmd.args(["--status-fd=2", "-bsau", &signing_key])
                        //.arg(&signed_storage)
                        .arg("-")
                        .stdout(Stdio::piped())
                        .stdin(Stdio::piped());

                    #[cfg(windows)]
                    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

                    let mut child = cmd.spawn()?;
                    child
                        .stdin
                        .take()
                        .expect("configured")
                        .write_all(buffer.to_string().as_ref())?;

                    let output = child.wait_with_output()?;
                    if output.status.success() {
                        // read stdout
                        let signature = String::from_utf8_lossy(&output.stdout);
                        let oid = self
                            .0
                            .commit_signed(&buffer, &signature, None)
                            .map(Into::into)
                            .map_err(Into::into);
                        return oid;
                    }
                }
            }
        }

        let oid = self
            .0
            .odb()?
            .write(git2::ObjectType::Commit, buffer.as_bytes())?;

        Ok(oid)
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
        let commit_buffer = str::from_utf8(commit_buffer).unwrap();
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

    pub fn config(&self) -> Result<git2::Config> {
        self.0.config().map_err(Into::into)
    }

    pub fn path(&self) -> &Path {
        self.0.path()
    }

    pub fn workdir(&self) -> Option<&Path> {
        self.0.workdir()
    }

    pub fn statuses(
        &self,
        options: Option<&mut git2::StatusOptions>,
    ) -> Result<git2::Statuses<'_>> {
        self.0.statuses(options).map_err(Into::into)
    }

    pub fn remote_anonymous(&self, url: &super::Url) -> Result<git2::Remote> {
        self.0
            .remote_anonymous(&url.to_string())
            .map_err(Into::into)
    }

    pub fn find_remote(&self, name: &str) -> Result<git2::Remote> {
        self.0.find_remote(name).map_err(Into::into)
    }

    pub fn find_branch(&self, name: &Refname) -> Result<git2::Branch> {
        self.0
            .find_branch(
                &name.simple_name(),
                match name {
                    Refname::Virtual(_) | Refname::Local(_) | Refname::Other(_) => {
                        git2::BranchType::Local
                    }
                    Refname::Remote(_) => git2::BranchType::Remote,
                },
            )
            .map_err(Into::into)
    }

    pub fn refname_to_id(&self, name: &str) -> Result<Oid> {
        self.0
            .refname_to_id(name)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn checkout_head(&self, opts: Option<&mut git2::build::CheckoutBuilder>) -> Result<()> {
        self.0.checkout_head(opts).map_err(Into::into)
    }

    pub fn checkout_index<'a>(&'a self, index: &'a mut git2::Index) -> CheckoutIndexBuilder {
        CheckoutIndexBuilder {
            index,
            repo: &self.0,
            checkout_builder: git2::build::CheckoutBuilder::new(),
        }
    }

    pub fn checkout_index_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.path(path.as_ref());
        builder.force();

        let mut index = self.0.index()?;
        self.0
            .checkout_index(Some(&mut index), Some(&mut builder))?;

        Ok(())
    }

    pub fn checkout_tree<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler {
        CheckoutTreeBuidler {
            tree,
            repo: &self.0,
            checkout_builder: git2::build::CheckoutBuilder::new(),
        }
    }

    pub fn set_head(&self, refname: &Refname) -> Result<()> {
        self.0.set_head(&refname.to_string()).map_err(Into::into)
    }

    pub fn set_head_detached(&self, commitish: Oid) -> Result<()> {
        self.0
            .set_head_detached(commitish.into())
            .map_err(Into::into)
    }

    pub fn reference(
        &self,
        name: &Refname,
        id: Oid,
        force: bool,
        log_message: &str,
    ) -> Result<git2::Reference> {
        self.0
            .reference(&name.to_string(), id.into(), force, log_message)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn remote(&self, name: &str, url: &Url) -> Result<git2::Remote> {
        self.0.remote(name, &url.to_string()).map_err(Into::into)
    }

    pub fn references(&self) -> Result<impl Iterator<Item = Result<git2::Reference>>> {
        self.0
            .references()
            .map(|iter| iter.map(|reference| reference.map(Into::into).map_err(Into::into)))
            .map_err(Into::into)
    }

    pub fn references_glob(
        &self,
        glob: &str,
    ) -> Result<impl Iterator<Item = Result<git2::Reference>>> {
        self.0
            .references_glob(glob)
            .map(|iter| iter.map(|reference| reference.map(Into::into).map_err(Into::into)))
            .map_err(Into::into)
    }

    pub fn run_hook_pre_commit(&self) -> Result<HookResult> {
        let res = git2_hooks::hooks_pre_commit(&self.0, Some(&["../.husky"]))?;
        Ok(res)
    }

    pub fn run_hook_commit_msg(&self, msg: &mut String) -> Result<HookResult> {
        let res = git2_hooks::hooks_commit_msg(&self.0, Some(&["../.husky"]), msg)?;
        Ok(res)
    }

    pub fn run_hook_post_commit(&self) -> Result<()> {
        git2_hooks::hooks_post_commit(&self.0, Some(&["../.husky"]))?;
        Ok(())
    }

    pub fn blame(
        &self,
        path: &Path,
        min_line: u32,
        max_line: u32,
        oldest_commit: &Oid,
        newest_commit: &Oid,
    ) -> Result<git2::Blame> {
        let mut opts = BlameOptions::new();
        opts.min_line(min_line as usize)
            .max_line(max_line as usize)
            .newest_commit(git2::Oid::from(*newest_commit))
            .oldest_commit(git2::Oid::from(*oldest_commit))
            .first_parent(true);
        self.0
            .blame_file(path, Some(&mut opts))
            .map_err(super::Error::Blame)
    }

    /// Returns a list of remotes
    ///
    /// Returns `Vec<String>` instead of StringArray because StringArray cannot safly be sent between threads
    pub fn remotes(&self) -> Result<Vec<String>> {
        self.0
            .remotes()
            .map(|string_array| {
                string_array
                    .iter()
                    .filter_map(|s| s.map(String::from))
                    .collect()
            })
            .map_err(super::Error::Remotes)
    }

    pub fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        self.0.remote(name, url)?;
        Ok(())
    }
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
