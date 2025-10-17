use diesel::{
    ExpressionMethods, QueryDsl, RunQueryDsl,
    prelude::{Insertable, Queryable, Selectable},
};
use serde::{Deserialize, Serialize};

use crate::{DbHandle, schema::file_write_locks::dsl::file_write_locks};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::file_write_locks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct FileWriteLock {
    pub path: String,
    pub created_at: chrono::NaiveDateTime,
    pub owner: String,
}

impl DbHandle {
    pub fn file_write_locks(&mut self) -> FileWriteLocksHandle<'_> {
        FileWriteLocksHandle { db: self }
    }
}

pub struct FileWriteLocksHandle<'a> {
    db: &'a mut DbHandle,
}

impl FileWriteLocksHandle<'_> {
    pub fn insert(&mut self, lock: FileWriteLock) -> Result<(), diesel::result::Error> {
        diesel::insert_into(file_write_locks)
            .values(lock)
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn delete(&mut self, path: &str) -> Result<(), diesel::result::Error> {
        diesel::delete(file_write_locks.filter(crate::schema::file_write_locks::path.eq(path)))
            .execute(&mut self.db.conn)?;
        Ok(())
    }

    pub fn list(&mut self) -> Result<Vec<FileWriteLock>, diesel::result::Error> {
        let locks = file_write_locks.load::<FileWriteLock>(&mut self.db.conn)?;
        Ok(locks)
    }
}
