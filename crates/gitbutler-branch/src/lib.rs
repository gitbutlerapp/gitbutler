pub mod branch;
pub mod diff;
pub mod file_ownership;
pub mod hunk;
pub mod ownership;
pub mod serde;
pub mod target;

use lazy_static::lazy_static;
lazy_static! {
    pub static ref GITBUTLER_INTEGRATION_REFERENCE: gitbutler_reference::LocalRefname =
        gitbutler_reference::LocalRefname::new("gitbutler/integration", None);
}

pub const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME: &str = "GitButler";
pub const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";
