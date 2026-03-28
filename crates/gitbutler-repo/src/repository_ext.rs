use std::str;

use anyhow::{Context as _, Result, bail};
use bstr::BStr;
use but_core::{
    RepositoryExt as RepositoryExtGix,
    commit::{Headers, SignCommit},
};
use but_oxidize::{ObjectIdExt as _, OidExt, git2_signature_to_gix_signature};
use gitbutler_reference::Refname;

/// Extension trait for `git2::Repository`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Repository`
pub trait RepositoryExt {
    /// Returns the common ancestor of the given commit Oids.
    ///
    /// This is like `git merge-base --octopus`.
    ///
    /// This method is called `merge_base_octopussy` so that it doesn't
    /// conflict with the libgit2 binding I upstreamed when it eventually
    /// gets merged.
    fn merge_base_octopussy(&self, ids: &[git2::Oid]) -> Result<git2::Oid>;
    fn checkout_tree_builder<'a>(&'a self, tree: &'a git2::Tree<'a>) -> CheckoutTreeBuidler<'a>;
    fn maybe_find_branch_by_refname(&self, name: &Refname) -> Result<Option<git2::Branch<'_>>>;

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
