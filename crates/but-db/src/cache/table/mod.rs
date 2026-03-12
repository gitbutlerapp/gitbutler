use crate::M;

/// The migrations to run for application wide caches.
pub const APP_MIGRATIONS: &[&[M<'static>]] = &[analytics::M, update::M];

pub(crate) mod analytics;
pub(crate) mod update;
