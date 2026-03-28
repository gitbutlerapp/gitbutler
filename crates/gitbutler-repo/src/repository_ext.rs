use anyhow::{Context as _, Result, bail};
use bstr::BStr;
use but_core::{
    RepositoryExt as RepositoryExtGix,
    commit::{Headers, SignCommit},
};

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
}

/// Create a commit with GitButler signing and trailer behavior using `gix`-native inputs.
#[expect(clippy::too_many_arguments)]
fn commit_gix(
    repo: &gix::Repository,
    update_ref: Option<&gitbutler_reference::Refname>,
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
    update_ref: Option<&gitbutler_reference::Refname>,
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
    update_ref: Option<&gitbutler_reference::Refname>,
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
