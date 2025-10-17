use std::str;

use anyhow::{Context, Result, anyhow, bail};
use bstr::BString;
use but_core::{GitConfigSettings, RepositoryExt as RepositoryExtGix};
use but_status::create_wd_tree;
use git2::Tree;
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_error::error::Code;
use gitbutler_oxidize::{
    ObjectIdExt as _, RepoExt, git2_signature_to_gix_signature, git2_to_gix_object_id,
    gix_to_git2_oid, gix_to_git2_signature,
};
use gitbutler_reference::{Refname, RemoteRefname};
use gix::objs::WriteTo;
use tracing::instrument;

use crate::{Config, SignaturePurpose};

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    fn find_branch_by_refname(&self, name: &Refname) -> Result<git2::Branch<'_>>;
    /// Returns the common ancestor of the given commit Oids.
    ///
    /// This is like `git merge-base --octopus`.
    ///
    /// This method is called `merge_base_octopussy` so that it doesn't
    /// conflict with the libgit2 binding I upstreamed when it eventually
    /// gets merged.
    fn merge_base_octopussy(&self, ids: &[git2::Oid]) -> Result<git2::Oid>;
    fn signatures(&self) -> Result<(git2::Signature<'_>, git2::Signature<'_>)>;

    fn remote_branches(&self) -> Result<Vec<RemoteRefname>>;
    fn remotes_as_string(&self) -> Result<Vec<String>>;
    /// `buffer` is the commit object to sign, but in theory could be anything to compute the signature for.
    /// Returns the computed signature.
    fn sign_buffer(&self, buffer: &[u8]) -> Result<BString>;
    fn checkout_tree_builder<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler<'a>;
    fn maybe_find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch<'_>>>;
    /// Add all untracked and modified files in the worktree to
    /// the object database, and create a tree from it.
    ///
    /// Use `untracked_limit_in_bytes` to control the maximum file size for untracked files
    /// before we stop tracking them automatically. Set it to 0 to disable the limit.
    ///
    /// It should also be noted that this will fail if run on an empty branch
    /// or if the HEAD branch has no commits.
    fn create_wd_tree(&self, untracked_limit_in_bytes: u64) -> Result<Tree<'_>>;

    /// Returns the `gitbutler/workspace` branch if the head currently points to it, or fail otherwise.
    /// Use it before any modification to the repository, or extra defensively each time the
    /// workspace is needed.
    ///
    /// This is for safety to assure the repository actually is in 'gitbutler mode'.
    fn workspace_ref_from_head(&self) -> Result<git2::Reference<'_>>;

    #[expect(clippy::too_many_arguments)]
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

    fn maybe_find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch<'_>>> {
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

    fn find_branch_by_refname(&self, name: &Refname) -> Result<git2::Branch<'_>> {
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

    /// Creates a tree containing the uncommited changes in the project.
    /// This includes files in the index that are considered conflicted.
    #[instrument(level = tracing::Level::DEBUG, skip(self, untracked_limit_in_bytes), err(Debug))]
    fn create_wd_tree(&self, untracked_limit_in_bytes: u64) -> Result<Tree<'_>> {
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

        let tree = create_wd_tree(&repo, untracked_limit_in_bytes)?;
        Ok(self.find_tree(tree.to_git2())?)
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
        let repo = self.to_gix()?;
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

        if repo.git_settings()?.gitbutler_sign_commits.unwrap_or(false) {
            let mut buf = Vec::new();
            commit.write_to(&mut buf)?;
            let signature = self.sign_buffer(&buf);
            match signature {
                Ok(signature) => {
                    commit.extra_headers.push(("gpgsig".into(), signature));
                }
                Err(err) => {
                    // If signing fails, set the "gitbutler.signCommits" config to false before erroring out
                    if repo
                        .config_snapshot()
                        .boolean_filter("gitbutler.signCommits", |md| {
                            md.source != gix::config::Source::Local
                        })
                        .is_none()
                    {
                        repo.set_git_settings(&GitConfigSettings {
                            gitbutler_sign_commits: Some(false),
                            ..Default::default()
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

        // TODO: extra-headers should be supported in `gix` directly.
        let oid = gix_to_git2_oid(repo.write_object(&commit)?);

        // update reference
        if let Some(refname) = update_ref {
            self.reference(&refname.to_string(), oid, true, message)?;
        }
        Ok(oid)
    }

    fn sign_buffer(&self, buffer: &[u8]) -> Result<BString> {
        but_rebase::commit::sign_buffer(&gix::open(self.path())?, buffer)
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

    fn signatures(&self) -> Result<(git2::Signature<'_>, git2::Signature<'_>)> {
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
