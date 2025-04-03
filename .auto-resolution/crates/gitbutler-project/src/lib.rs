pub mod access;
mod controller;
mod default_true;
mod project;
mod storage;

pub use controller::Controller;
pub use project::{ApiProject, AuthKey, CodePushState, FetchResult, Project, ProjectId};
pub use storage::UpdateRequest;

/// A utility to be used from applications to optimize `git2` configuration.
/// See comments for details.
pub fn configure_git2() {
    // Do not re-hash each decoded objects for quite a significant performance gain.
    // This delegates object validation to `git fsck`, which seems fair.
    git2::opts::strict_hash_verification(false);
    // Thus far, no broken object was created, and if that would be the case, tests should catch it.
    // These settings are only changed from `main` of applications.
    git2::opts::strict_object_creation(false);
}

/// The maximum size of files to automatically start tracking, i.e. untracked files we pick up for tree-creation.
/// **Inactive for now** while it's hard to tell if it's safe *not* to pick up everything.
pub const AUTO_TRACK_LIMIT_BYTES: u64 = 0;
