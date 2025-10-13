use diesel::{ExpressionMethods, Identifiable, OptionalExtension as _, QueryDsl, RunQueryDsl};

use crate::DbHandle;
use crate::schema::worktrees::dsl::worktrees;

use diesel::prelude::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorktreeSource {
    Branch(String),
}

#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable, Identifiable,
)]
#[diesel(table_name = crate::schema::worktrees)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Worktree {
    pub id: String,
    pub base: String,
    pub path: String,
    pub source: String,
}

impl DbHandle {
    pub fn worktrees(&mut self) -> WorktreesHandle<'_> {
        WorktreesHandle { db: self }
    }
}

pub struct WorktreesHandle<'a> {
    db: &'a mut DbHandle,
}

impl WorktreesHandle<'_> {
    pub fn insert(&mut self, worktree: Worktree) -> Result<(), diesel::result::Error> {
        diesel::insert_into(worktrees)
            .values(worktree)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn get(&mut self, id: &str) -> Result<Option<Worktree>, diesel::result::Error> {
        let worktree = worktrees
            .filter(crate::schema::worktrees::id.eq(id))
            .first::<Worktree>(&mut self.db.conn)
            .optional()?;
        Ok(worktree)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), diesel::result::Error> {
        diesel::delete(worktrees.filter(crate::schema::worktrees::id.eq(id)))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list(&mut self) -> Result<Vec<Worktree>, diesel::result::Error> {
        let worktree_list = worktrees.load::<Worktree>(&mut self.db.conn)?;
        Ok(worktree_list)
    }
}
