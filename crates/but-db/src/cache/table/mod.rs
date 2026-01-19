use crate::M;

/// The migrations to run for application wide caches.
pub const APP_MIGRATIONS: &[&[M<'static>]] = &[update::M];

pub(crate) mod update;
