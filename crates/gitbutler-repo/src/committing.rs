use anyhow::{anyhow, Result};
use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_config::git::{GbConfig, GitConfig as _};
use gitbutler_error::error::Code;
use gitbutler_oxidize::{git2_signature_to_gix_signature, git2_to_gix_object_id, gix_to_git2_oid};
use gitbutler_reference::Refname;
use gix::objs::WriteTo as _;

use crate::sigining::sign_buffer;

pub trait RepositoryExt {
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
            let signature = sign_buffer(self, &buf);
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
}
