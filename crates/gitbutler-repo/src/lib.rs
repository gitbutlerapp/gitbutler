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
pub mod hooks;
pub mod managed_hooks;
mod remote;
pub mod staging;

pub mod commit_message;

pub const GITBUTLER_COMMIT_AUTHOR_NAME: &str = "GitButler";
pub const GITBUTLER_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";

pub enum SignaturePurpose {
    Author,
    Committer,
}

/// Provide a `gix` signature with the GitButler author and the current or overridden time.
pub fn signature_gix(purpose: SignaturePurpose) -> gix::actor::Signature {
    gix::actor::Signature {
        name: GITBUTLER_COMMIT_AUTHOR_NAME.into(),
        email: GITBUTLER_COMMIT_AUTHOR_EMAIL.into(),
        time: commit_time(match purpose {
            SignaturePurpose::Author => "GIT_AUTHOR_DATE",
            SignaturePurpose::Committer => "GIT_COMMITTER_DATE",
        }),
    }
}

/// Return the time of a commit as `now` unless the `overriding_variable_name` contains a parseable date,
/// which is used instead.
fn commit_time(overriding_variable_name: &str) -> gix::date::Time {
    std::env::var(overriding_variable_name)
        .ok()
        .and_then(|time| gix::date::parse(&time, Some(std::time::SystemTime::now())).ok())
        .unwrap_or_else(gix::date::Time::now_local_or_utc)
}
