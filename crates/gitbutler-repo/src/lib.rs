use gix::date::parse::TimeBuf;

pub mod rebase;

mod commands;
mod traversal {
    use anyhow::Result;

    /// Return first-parent ancestors from `from` until `stop_before`, excluding `stop_before`.
    pub fn first_parent_commit_ids_until(
        repo: &gix::Repository,
        from: gix::ObjectId,
        stop_before: gix::ObjectId,
    ) -> Result<Vec<gix::ObjectId>> {
        use gix::prelude::ObjectIdExt as _;

        from.attach(repo)
            .ancestors()
            .first_parent_only()
            .with_hidden(Some(stop_before))
            .all()?
            .map(|info| Ok(info?.id))
            .collect()
    }
}
pub use traversal::first_parent_commit_ids_until;

pub use commands::{FileInfo, RepoCommands};
pub use remote::GitRemote;

mod repository_ext;
pub use repository_ext::{RepositoryExt, commit_with_signature_gix, commit_without_signature_gix};

pub mod credentials;

mod config;
pub use but_hooks::hook_manager;
use config::Config;
pub mod hooks;
pub use but_hooks::managed_hooks;
mod remote;
pub mod staging;

pub mod commit_message;

pub use but_core::{GITBUTLER_COMMIT_AUTHOR_EMAIL, GITBUTLER_COMMIT_AUTHOR_NAME};
use but_oxidize::gix_to_git2_signature;

pub enum SignaturePurpose {
    Author,
    Committer,
}

/// Provide a signature with the GitButler author, and the current time or the time overridden
/// depending on the value for `purpose`.
pub fn signature(purpose: SignaturePurpose) -> anyhow::Result<git2::Signature<'static>> {
    let signature = signature_gix(purpose);
    gix_to_git2_signature(signature.to_ref(&mut TimeBuf::default()))
}

/// Provide a `gix` signature with the GitButler author and the current or overridden time.
pub fn signature_gix(purpose: SignaturePurpose) -> gix::actor::Signature {
    gix::actor::Signature {
        name: GITBUTLER_COMMIT_AUTHOR_NAME.into(),
        email: GITBUTLER_COMMIT_AUTHOR_EMAIL.into(),
        time: but_core::commit_time(match purpose {
            SignaturePurpose::Author => "GIT_AUTHOR_DATE",
            SignaturePurpose::Committer => "GIT_COMMITTER_DATE",
        }),
    }
}
