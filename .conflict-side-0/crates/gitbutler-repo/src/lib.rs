pub mod rebase;

mod commands;
pub use commands::{FileInfo, RepoCommands};
pub use remote::GitRemote;

mod repository_ext;
pub use repository_ext::RepositoryExt;

pub mod credentials;

mod config;
pub mod hooks;
mod remote;
pub mod staging;

pub use config::Config;

pub mod temporary_workdir;

pub mod logging;

pub mod commit_message;

use gitbutler_oxidize::gix_to_git2_signature;
pub const GITBUTLER_COMMIT_AUTHOR_NAME: &str = "GitButler";
pub const GITBUTLER_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";

pub enum SignaturePurpose {
    Author,
    Committer,
}

/// Provide a signature with the GitButler author, and the current time or the time overridden
/// depending on the value for `purpose`.
pub fn signature(purpose: SignaturePurpose) -> anyhow::Result<git2::Signature<'static>> {
    let signature = gix::actor::SignatureRef {
        name: GITBUTLER_COMMIT_AUTHOR_NAME.into(),
        email: GITBUTLER_COMMIT_AUTHOR_EMAIL.into(),
        time: commit_time(match purpose {
            SignaturePurpose::Author => "GIT_AUTHOR_DATE",
            SignaturePurpose::Committer => "GIT_COMMITTER_DATE",
        }),
    };
    gix_to_git2_signature(signature)
}

/// Return the time of a commit as `now` unless the `overriding_variable_name` contains a parseable date,
/// which is used instead.
fn commit_time(overriding_variable_name: &str) -> gix::date::Time {
    std::env::var(overriding_variable_name)
        .ok()
        .and_then(|time| gix::date::parse(&time, Some(std::time::SystemTime::now())).ok())
        .unwrap_or_else(gix::date::Time::now_local_or_utc)
}
