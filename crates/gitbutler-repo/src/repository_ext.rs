use std::str;

use anyhow::{Context as _, Result, anyhow, bail};
use bstr::{BStr, BString};
use but_core::{
    RepositoryExt as RepositoryExtGix,
    commit::{Headers, SignCommit},
};
use but_error::Code;
use but_oxidize::{
    ObjectIdExt as _, OidExt, git2_signature_to_gix_signature, gix_to_git2_signature,
};
use gitbutler_reference::{Refname, RemoteRefname};

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
        commit_headers: Option<Headers>,
    ) -> Result<git2::Oid>;
}

/// Create a commit with GitButler signing and trailer behavior using `gix`-native inputs.
#[expect(clippy::too_many_arguments)]
fn commit_gix(
    repo: &gix::Repository,
    update_ref: Option<&Refname>,
    author: gix::actor::Signature,
    committer: gix::actor::Signature,
    message: &BStr,
    tree: gix::ObjectId,
    parents: &[gix::ObjectId],
    commit_headers: Option<Headers>,
    sign_commit: SignCommit,
) -> Result<gix::ObjectId> {
    let mut commit = gix::objs::Commit {
        message: message.into(),
        tree,
        author,
        committer,
        encoding: None,
        parents: parents.to_vec().into(),
        extra_headers: commit_headers.map(|h| (&h).into()).unwrap_or_default(),
    };

    if repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false) {
        but_gerrit::set_trailers(&mut commit);
    }

    let update_ref = update_ref
        .map(|refname| gix::refs::FullName::try_from(refname.to_string()))
        .transpose()?;
    but_core::commit::create(
        repo,
        commit,
        update_ref.as_ref().map(|name| name.as_ref()),
        sign_commit,
    )
}

/// Create a commit and sign it if GitButler signing is enabled in repository configuration.
#[expect(clippy::too_many_arguments)]
pub fn commit_with_signature_gix(
    repo: &gix::Repository,
    update_ref: Option<&Refname>,
    author: gix::actor::Signature,
    committer: gix::actor::Signature,
    message: &BStr,
    tree: gix::ObjectId,
    parents: &[gix::ObjectId],
    commit_headers: Option<Headers>,
) -> Result<gix::ObjectId> {
    commit_gix(
        repo,
        update_ref,
        author,
        committer,
        message,
        tree,
        parents,
        commit_headers,
        SignCommit::IfSignCommitsEnabled,
    )
}

/// Create a commit without applying GitButler commit-signing configuration.
#[expect(clippy::too_many_arguments)]
pub fn commit_without_signature_gix(
    repo: &gix::Repository,
    update_ref: Option<&Refname>,
    author: gix::actor::Signature,
    committer: gix::actor::Signature,
    message: &BStr,
    tree: gix::ObjectId,
    parents: &[gix::ObjectId],
    commit_headers: Option<Headers>,
) -> Result<gix::ObjectId> {
    commit_gix(
        repo,
        update_ref,
        author,
        committer,
        message,
        tree,
        parents,
        commit_headers,
        SignCommit::No,
    )
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
        commit_headers: Option<Headers>,
    ) -> Result<git2::Oid> {
        let repo = gix::open(self.path())?;
        commit_with_signature_gix(
            &repo,
            update_ref,
            git2_signature_to_gix_signature(author),
            git2_signature_to_gix_signature(committer),
            message.into(),
            tree.id().to_gix(),
            &parents
                .iter()
                .map(|commit| commit.id().to_gix())
                .collect::<Vec<_>>(),
            commit_headers,
        )
        .map(|oid| oid.to_git2())
    }

    fn sign_buffer(&self, buffer: &[u8]) -> Result<BString> {
        but_core::commit::sign_buffer(&gix::open(self.path())?, buffer)
    }

    fn remotes_as_string(&self) -> Result<Vec<String>> {
        Ok(gix::open(self.path())?
            .remote_names()
            .iter()
            .map(|name| name.to_string())
            .collect())
    }

    fn remote_branches(&self) -> Result<Vec<RemoteRefname>> {
        use bstr::ByteSlice;

        let repo = gix::open_opts(self.path(), gix::open::Options::isolated())?;
        repo.references()?
            .remote_branches()?
            .filter_map(Result::ok)
            // TODO: the question is if we really need this? Probably not, but it's part
            // of the `gix` migration and we'd rather play it safe. Goal is for `gitbutler-` crates
            // to not exist anyway.
            .filter(|reference| !reference.name().as_bstr().ends_with_str("/HEAD"))
            .map(|reference| {
                reference
                    .name()
                    .to_string()
                    .parse()
                    .context("failed to convert branch to remote name")
            })
            .collect()
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

        let config: Config = (&repo).into();
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
