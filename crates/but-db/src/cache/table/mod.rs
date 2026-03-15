use crate::M;

/// The migrations to run for application wide caches.
pub const APP_MIGRATIONS: &[&[M<'static>]] = &[update::M];
/// The migrations to run for project-local caches.
pub const PROJECT_MIGRATIONS: &[&[M<'static>]] = &[commit_metadata::M];

pub(crate) mod commit_metadata;
pub(crate) mod update;
