use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::hunk_assignments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct HunkAssignment {
    pub hunk_header: Option<String>,
    pub path: String,
    pub path_bytes: Vec<u8>,
    pub stack_id: Option<String>,
    pub hunk_locks: String,
}
