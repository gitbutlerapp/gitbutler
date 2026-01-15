use diesel::SqliteConnection;

const FILE_NAME: &str = "but.sqlite";

mod handle;
pub mod poll;
mod schema;

mod table;
pub use table::butler_actions::ButlerAction;
pub use table::ci_checks::CiCheck;
pub use table::claude::{ClaudeMessage, ClaudePermissionRequest, ClaudeSession};
pub use table::file_write_locks::FileWriteLock;
pub use table::forge_reviews::ForgeReview;
pub use table::gerrit_metadata::GerritMeta;
pub use table::hunk_assignments::HunkAssignment;
pub use table::workflows::Workflow;
pub use table::workspace_rules::WorkspaceRule;

use diesel_migrations::{EmbeddedMigrations, embed_migrations};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub struct DbHandle {
    conn: SqliteConnection,
    /// The URL at which the connection was opened, mainly for debugging.
    url: String,
}
