use crate::{M, SchemaVersion};

pub const APP_MIGRATIONS: &[&[M<'static>]] = &[update::M, M_FULLY_REMOVED];
pub const PROJECT_MIGRATIONS: &[&[M<'static>]] = &[removed_change_ids::M];

pub(crate) mod removed_change_ids;
pub(crate) mod update;

/// Move migrations that relate to tables that don't have their module anymore here.
///
/// Removing a feature does not justify a schema-version bump: keep these at
/// [`SchemaVersion::Zero`] and leave tables that older binaries still read in place.
const M_FULLY_REMOVED: &[M<'static>] = &[M::up(
    2026_06_09__12_00_00,
    SchemaVersion::Zero,
    "CREATE TABLE `agent-skill-notice`(
    `id` INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
    `shown_at` TIMESTAMP NOT NULL
);",
)];
