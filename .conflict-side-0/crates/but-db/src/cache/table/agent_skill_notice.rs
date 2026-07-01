use chrono::{DateTime, Utc};

use crate::{AppCacheHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    2026_06_09__12_00_00,
    SchemaVersion::Zero,
    "CREATE TABLE `agent-skill-notice`(
    `id` INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
    `shown_at` TIMESTAMP NOT NULL
);",
)];

const SINGLETON_RECORD_ID: u8 = 1;

/// The most recent agent skill notice shown by the CLI.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentSkillNotice {
    /// When the notice was shown.
    pub shown_at: DateTime<Utc>,
}

/// A utility for accessing the agent skill notice debounce cache.
pub struct AgentSkillNoticeHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

/// A utility for mutating the agent skill notice debounce cache.
pub struct AgentSkillNoticeHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl AppCacheHandle {
    /// Return a handle for read-only agent skill notice debounce data.
    pub fn agent_skill_notice(&self) -> AgentSkillNoticeHandle<'_> {
        AgentSkillNoticeHandle { conn: &self.conn }
    }

    /// Return a handle for mutating agent skill notice debounce data.
    pub fn agent_skill_notice_mut(&mut self) -> rusqlite::Result<AgentSkillNoticeHandleMut<'_>> {
        Ok(AgentSkillNoticeHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl Transaction<'_> {
    /// Return a handle for read-only agent skill notice debounce data.
    pub fn agent_skill_notice(&self) -> AgentSkillNoticeHandle<'_> {
        AgentSkillNoticeHandle { conn: self.inner() }
    }

    /// Return a handle for mutating agent skill notice debounce data.
    pub fn agent_skill_notice_mut(&mut self) -> rusqlite::Result<AgentSkillNoticeHandleMut<'_>> {
        Ok(AgentSkillNoticeHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

impl AgentSkillNoticeHandle<'_> {
    /// Retrieves the cached agent skill notice debounce data if available.
    pub fn get(&self) -> Option<AgentSkillNotice> {
        self.try_get().ok().flatten()
    }

    /// Like [`Self::get`], but fallible.
    pub fn try_get(&self) -> rusqlite::Result<Option<AgentSkillNotice>> {
        let mut stmt = self
            .conn
            .prepare("SELECT shown_at FROM `agent-skill-notice` WHERE id = 1")?;
        let mut rows = stmt.query([])?;

        match rows.next()? {
            Some(row) => {
                let shown_at_naive: chrono::NaiveDateTime = row.get(0)?;
                Ok(Some(AgentSkillNotice {
                    shown_at: DateTime::from_naive_utc_and_offset(shown_at_naive, Utc),
                }))
            }
            None => Ok(None),
        }
    }
}

impl AgentSkillNoticeHandleMut<'_> {
    /// Saves the latest shown agent skill notice debounce data.
    pub fn save(self, notice: &AgentSkillNotice) -> rusqlite::Result<()> {
        let sp = self.sp;

        sp.execute(
            "INSERT OR REPLACE INTO `agent-skill-notice`
             (id, shown_at)
             VALUES (?1, ?2)",
            rusqlite::params![SINGLETON_RECORD_ID, notice.shown_at.naive_utc()],
        )?;

        sp.commit()?;
        Ok(())
    }
}
