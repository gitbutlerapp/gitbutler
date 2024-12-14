use anyhow::{Context as _, Result};
use gitbutler_error::error::Code;
use gitbutler_oxidize::gix_to_git2_signature;

use crate::{Config, SignaturePurpose};

pub trait RepositoryExt {
    fn signatures(&self) -> Result<(git2::Signature, git2::Signature)>;
}

impl RepositoryExt for git2::Repository {
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
}
