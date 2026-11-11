//! What Briefs last said and what's written back and not sent, in a SQLite file of its own, so a
//! page can draw without asking, and removing Briefs leaves nothing behind in any other database.
//!
//! It keeps the server's JSON as it came and reads it as it's used, leniently, so what the server
//! changes takes no migration: a new field or kind of block reads as it does from the server, and
//! a row that can't be read any more is dropped and asked for again. Only [`SCHEMA`] says what
//! the tables here look like, and another number drops what the file holds, but for what's written
//! and not sent, which is someone's own words and nowhere else, and whose it is. Changing `note`
//! takes a migration.
//! The rest is a cache, so opening it doesn't fail on what's in the file: one SQLite can't read is
//! replaced, and with none that opens, it lives in memory until the app quits.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;

use crate::{Asked, Brief, CommentTo, FetchedVersion, Found, RawActivity};

const FILE: &str = "briefs.sqlite";

/// What the tables look like. There are no migrations: another number drops them, but for `note`.
const SCHEMA: i32 = 5;

const TABLES: &str = "
    DROP TABLE IF EXISTS version;
    DROP TABLE IF EXISTS anchor;
    DROP TABLE IF EXISTS activity;
    DROP TABLE IF EXISTS seen;
    CREATE TABLE version (
        brief_id     TEXT NOT NULL,
        version      INTEGER NOT NULL,
        json         TEXT NOT NULL,
        PRIMARY KEY (brief_id, version)
    );
    CREATE TABLE anchor (
        remote   TEXT NOT NULL,
        branch   TEXT NOT NULL,
        asked_at INTEGER NOT NULL,
        found    TEXT NOT NULL,
        PRIMARY KEY (remote, branch)
    );
    CREATE TABLE IF NOT EXISTS account (id TEXT NOT NULL);
    CREATE TABLE activity (
        brief_id TEXT NOT NULL,
        version  INTEGER NOT NULL,
        json     TEXT NOT NULL,
        PRIMARY KEY (brief_id, version)
    );
    CREATE TABLE seen (
        brief_id TEXT PRIMARY KEY,
        version  INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS note (
        id       INTEGER PRIMARY KEY,
        brief_id TEXT NOT NULL,
        version  INTEGER NOT NULL,
        block    INTEGER,
        quote    TEXT,
        parent   TEXT,
        body     TEXT NOT NULL
    );
";

/// How long what was said about a branch stays once nothing asks about it, in seconds.
const KEEP: i64 = 30 * 24 * 60 * 60;

/// What was last said about a branch. No brief is an answer too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchored {
    pub branch: String,
    /// Seconds since the epoch.
    pub asked_at: i64,
    /// The brief that fits it best.
    pub found: Option<Found>,
}

/// What someone wrote about a version of a brief and hasn't sent yet, which only they have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    pub body: String,
    pub to: CommentTo,
}

/// The cache of what Briefs said to one account.
pub struct Cache {
    conn: Connection,
    account: String,
}

impl Cache {
    /// The cache in `dir`, or in memory without one. What another account left is dropped, as a
    /// brief can be private to it. Only failing to open even in memory is an error.
    pub fn open(dir: Option<&Path>, account: &str) -> rusqlite::Result<Self> {
        let file = dir.and_then(|dir| match open_file(dir, account) {
            Ok(conn) => Some(conn),
            Err(err) => {
                tracing::warn!("keeping briefs in memory, as {dir:?} won't open: {err}");
                None
            }
        });
        let conn = match file {
            Some(conn) => conn,
            None => ready(Connection::open_in_memory()?, account)?,
        };
        Ok(Self {
            conn,
            account: account.to_owned(),
        })
    }

    pub fn account(&self) -> &str {
        &self.account
    }

    /// What's known about the branches of the repository at `remote`.
    pub fn anchors(&self, remote: &str) -> rusqlite::Result<Vec<Anchored>> {
        let mut rows = self
            .conn
            .prepare("SELECT branch, asked_at, found FROM anchor WHERE remote = ?1")?;
        rows.query_map([remote], |row| {
            let found: String = row.get(2)?;
            Ok(Anchored {
                branch: row.get(0)?,
                asked_at: row.get(1)?,
                // A shape that can't be read any more counts as no brief, until it's asked again.
                found: (serde_json::from_str::<Vec<Value>>(&found).unwrap_or_default())
                    .into_iter()
                    .find_map(|found| serde_json::from_value(found).ok()),
            })
        })?
        .collect()
    }

    /// `version` of brief `id`, if it's here and can still be read. One that can't is dropped, so
    /// it's asked for again.
    pub fn version(&self, id: &str, version: u32) -> rusqlite::Result<Option<Brief>> {
        let json: Option<String> = (self.conn)
            .query_row(
                "SELECT json FROM version WHERE brief_id = ?1 AND version = ?2",
                params![id, version],
                |row| row.get(0),
            )
            .optional()?;
        let Some(json) = json else {
            return Ok(None);
        };
        match serde_json::from_str(&json) {
            Ok(brief) => Ok(Some(brief)),
            Err(err) => {
                tracing::debug!("dropping a brief that can't be read any more: {err}");
                self.conn.execute(
                    "DELETE FROM version WHERE brief_id = ?1 AND version = ?2",
                    params![id, version],
                )?;
                Ok(None)
            }
        }
    }

    /// The branch `version` of brief `id` was written on, if it's here and says.
    pub fn version_branch(&self, id: &str, version: u32) -> rusqlite::Result<Option<String>> {
        let branch = (self.conn)
            .query_row(
                "SELECT json_extract(json, '$.review.repo.branch') FROM version
                 WHERE brief_id = ?1 AND version = ?2",
                params![id, version],
                |row| row.get(0),
            )
            .optional()?;
        Ok(branch.flatten())
    }

    /// What's been said about `version` of brief `id`, as it was last asked, if it was.
    pub fn activity(&self, id: &str, version: u32) -> rusqlite::Result<Option<RawActivity>> {
        let json: Option<String> = (self.conn)
            .query_row(
                "SELECT json FROM activity WHERE brief_id = ?1 AND version = ?2",
                params![id, version],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json.and_then(|json| serde_json::from_str(&json).ok()))
    }

    pub fn put_activity(
        &self,
        id: &str,
        version: u32,
        activity: &RawActivity,
    ) -> rusqlite::Result<()> {
        let json = serde_json::to_string(activity).unwrap_or_default();
        self.conn.execute(
            "INSERT OR REPLACE INTO activity (brief_id, version, json) VALUES (?1, ?2, ?3)",
            params![id, version, json],
        )?;
        Ok(())
    }

    /// The newest version of brief `id` this account has read here, if any.
    pub fn seen(&self, id: &str) -> rusqlite::Result<Option<u32>> {
        (self.conn)
            .query_row(
                "SELECT version FROM seen WHERE brief_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
    }

    /// Remember `version` of brief `id` was read, unless a newer one was.
    pub fn saw(&self, id: &str, version: u32) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO seen (brief_id, version) VALUES (?1, ?2)
             ON CONFLICT (brief_id) DO UPDATE SET version = max(version, excluded.version)",
            params![id, version],
        )?;
        Ok(())
    }

    /// What's written about `version` of brief `id` and not sent, the oldest first.
    pub fn notes(&self, id: &str, version: u32) -> rusqlite::Result<Vec<Note>> {
        let mut rows = self.conn.prepare(
            "SELECT id, block, quote, parent, body FROM note
             WHERE brief_id = ?1 AND version = ?2 ORDER BY id",
        )?;
        rows.query_map(params![id, version], |row| {
            let block: Option<i64> = row.get(1)?;
            let block = block.map(|block| block as usize);
            let quote: Option<String> = row.get(2)?;
            let parent: Option<String> = row.get(3)?;
            Ok(Note {
                id: row.get(0)?,
                body: row.get(4)?,
                to: match (parent, block, quote) {
                    (Some(parent), _, _) => CommentTo::Thread(parent),
                    (None, Some(block), Some(quote)) => CommentTo::Passage { block, quote },
                    (None, Some(block), None) => CommentTo::Block(block),
                    (None, None, _) => CommentTo::Brief,
                },
            })
        })?
        .collect()
    }

    /// Keep `body`, written about `to` in `version` of brief `id`, until it's sent.
    pub fn add_note(
        &self,
        id: &str,
        version: u32,
        to: &CommentTo,
        body: &str,
    ) -> rusqlite::Result<()> {
        let (block, quote, parent) = match to {
            CommentTo::Brief => (None, None, None),
            CommentTo::Block(block) => (Some(*block as i64), None, None),
            CommentTo::Passage { block, quote } => {
                (Some(*block as i64), Some(quote.as_str()), None)
            }
            CommentTo::Thread(parent) => (None, None, Some(parent.as_str())),
        };
        self.conn.execute(
            "INSERT INTO note (brief_id, version, block, quote, parent, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, version, block, quote, parent, body],
        )?;
        Ok(())
    }

    /// Forget a note, once it's sent or thrown away.
    pub fn remove_note(&self, note: i64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM note WHERE id = ?1", [note])?;
        Ok(())
    }

    /// Keep a version read on its own.
    pub fn put_version(&self, version: &FetchedVersion) -> rusqlite::Result<()> {
        put_version(&self.conn, version)
    }

    /// Keep what `asked` found about the repository at `remote`, at `now`, in seconds since the
    /// epoch. Then what nothing's asked about in a long while goes, and with it the briefs no
    /// branch has any more, as one that's deleted or unshared shouldn't stay on disk.
    pub fn took(&mut self, remote: &str, asked: &Asked, now: i64) -> rusqlite::Result<()> {
        // Immediate, so another instance writing makes it wait: a deferred transaction that has
        // to take the lock later fails at once.
        let write = (self.conn).transaction_with_behavior(TransactionBehavior::Immediate)?;
        for version in &asked.versions {
            put_version(&write, version)?;
        }
        for (branch, found) in &asked.anchors {
            write.execute(
                "INSERT OR REPLACE INTO anchor (remote, branch, asked_at, found)
                 VALUES (?1, ?2, ?3, ?4)",
                params![remote, branch, now, Value::from(found.clone()).to_string()],
            )?;
        }
        write.execute("DELETE FROM anchor WHERE asked_at < ?1", [now - KEEP])?;
        // Notes stay, as what someone wrote is worth more than a tidy file.
        for table in ["version", "activity"] {
            write.execute(
                &format!(
                    "DELETE FROM {table} WHERE brief_id NOT IN
                     (SELECT json_extract(value, '$.id') FROM anchor, json_each(anchor.found))"
                ),
                [],
            )?;
        }
        write.commit()
    }
}

fn put_version(conn: &Connection, version: &FetchedVersion) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO version (brief_id, version, json) VALUES (?1, ?2, ?3)",
        params![version.id, version.version, version.body.to_string()],
    )?;
    Ok(())
}

/// The file in `dir`, replacing one SQLite can't read. Only that: one that's merely locked by
/// another instance holds what's worth keeping.
fn open_file(dir: &Path, account: &str) -> rusqlite::Result<Connection> {
    // Not being able to is the open's error to report.
    std::fs::create_dir_all(dir).ok();
    let path = dir.join(FILE);
    let open = || ready(Connection::open(&path)?, account);
    open().or_else(|err| {
        let invalid = (err.sqlite_error_code()).is_some_and(|code| {
            matches!(code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
        });
        if !invalid {
            return Err(err);
        }
        tracing::warn!("replacing {path:?}, which SQLite can't read: {err}");
        // With its side files, which SQLite would otherwise read into the new one.
        for side in ["", "-wal", "-shm"] {
            let mut file = path.clone().into_os_string();
            file.push(side);
            std::fs::remove_file(file).ok();
        }
        open()
    })
}

/// `conn` with the tables of [`SCHEMA`], holding nothing but what's `account`'s.
fn ready(mut conn: Connection, account: &str) -> rusqlite::Result<Connection> {
    conn.busy_timeout(Duration::from_secs(5))?;
    // WAL so two instances can share it, and NORMAL as losing the last writes to a power cut is
    // fine here.
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
    let write = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let schema: i32 = write.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if schema != SCHEMA {
        write.execute_batch(TABLES)?;
        write.pragma_update(None, "user_version", SCHEMA)?;
    }
    let owner: Option<String> =
        (write.query_row("SELECT id FROM account", [], |row| row.get(0))).optional()?;
    if owner.as_deref() != Some(account) {
        write.execute_batch(
            "DELETE FROM version; DELETE FROM anchor; DELETE FROM account;
             DELETE FROM activity; DELETE FROM note; DELETE FROM seen;",
        )?;
        write.execute("INSERT INTO account (id) VALUES (?1)", [account])?;
    }
    write.commit()?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const REMOTE: &str = "git@example.com:o/r.git";
    const NOW: i64 = 1_000_000_000;

    fn found(id: &str) -> Value {
        json!({
            "id": id,
            "title": "t",
            "author": { "name": "Ada" },
            "latest": 2,
            "match": { "how": "head", "version": 1, "head": null },
        })
    }

    fn body(title: &str, branch: &str) -> Value {
        json!({
            "createdAt": "2026-11-04T12:01:35.551Z",
            "review": { "title": title, "blocks": [], "repo": { "branch": branch } },
        })
    }

    /// Branch `a` has brief `b1`, with its version.
    fn asked() -> Asked {
        Asked {
            anchors: vec![("a".into(), vec![found("b1")])],
            versions: vec![FetchedVersion {
                id: "b1".into(),
                version: 1,
                body: body("First", "a"),
            }],
        }
    }

    fn holding_a_brief(dir: Option<&Path>) -> Cache {
        let mut cache = Cache::open(dir, "me").unwrap();
        cache.took(REMOTE, &asked(), NOW).unwrap();
        cache
    }

    #[test]
    fn what_was_taken_reads_back() {
        let cache = holding_a_brief(None);
        let anchors = cache.anchors(REMOTE).unwrap();
        assert_eq!(anchors.len(), 1, "the one branch asked about");
        assert_eq!(anchors[0].branch, "a");
        assert_eq!(anchors[0].asked_at, NOW);
        assert_eq!(anchors[0].found.as_ref().unwrap().id, "b1");
        let brief = cache.version("b1", 1).unwrap().unwrap();
        assert_eq!(brief.review.title, "First");
        assert_eq!(
            cache.version_branch("b1", 1).unwrap().as_deref(),
            Some("a"),
            "what a batch's answers are told apart by"
        );
        assert!(
            cache.anchors("elsewhere").unwrap().is_empty(),
            "another repository's are its own"
        );
    }

    #[test]
    fn no_brief_is_an_answer() {
        let mut cache = Cache::open(None, "me").unwrap();
        let asked = Asked {
            anchors: vec![("a".into(), Vec::new())],
            versions: Vec::new(),
        };
        cache.took(REMOTE, &asked, NOW).unwrap();
        let anchors = cache.anchors(REMOTE).unwrap();
        assert_eq!(anchors[0].found, None, "asked, with none to show");
    }

    #[test]
    fn a_field_the_server_adds_or_a_shape_it_changes_needs_no_migration() {
        let mut cache = Cache::open(None, "me").unwrap();
        let mut body = body("First", "a");
        body["review"]["somethingNew"] = json!({ "any": "shape" });
        let asked = Asked {
            anchors: vec![
                ("a".into(), vec![found("b1")]),
                ("b".into(), vec![json!({ "reshaped": true })]),
            ],
            versions: vec![FetchedVersion {
                id: "b1".into(),
                version: 1,
                body,
            }],
        };
        cache.took(REMOTE, &asked, NOW).unwrap();
        assert!(
            cache.version("b1", 1).unwrap().is_some(),
            "what's not known is left alone"
        );
        let anchors = cache.anchors(REMOTE).unwrap();
        let b = anchors.iter().find(|anchor| anchor.branch == "b").unwrap();
        assert_eq!(b.found, None, "what can't be read counts as no brief yet");
    }

    #[test]
    fn a_version_that_cannot_be_read_is_dropped_to_be_asked_again() {
        let cache = holding_a_brief(None);
        cache
            .conn
            .execute("UPDATE version SET json = '{\"review\": 1}'", [])
            .unwrap();
        assert!(
            cache.version("b1", 1).unwrap().is_none(),
            "it reads as not here"
        );
        let left: i64 = (cache.conn)
            .query_row("SELECT count(*) FROM version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(left, 0, "so the row is gone rather than failing each time");
    }

    #[test]
    fn a_brief_no_branch_has_any_more_leaves_the_disk() {
        let mut cache = holding_a_brief(None);
        let gone = Asked {
            anchors: vec![("a".into(), Vec::new())],
            versions: Vec::new(),
        };
        cache.took(REMOTE, &gone, NOW + 60).unwrap();
        assert!(
            cache.version("b1", 1).unwrap().is_none(),
            "deleted or unshared, its words shouldn't stay"
        );
    }

    #[test]
    fn a_branch_nothing_asks_about_in_a_long_while_goes() {
        let mut cache = holding_a_brief(None);
        let other = Asked {
            anchors: vec![("other".into(), Vec::new())],
            versions: Vec::new(),
        };
        cache.took(REMOTE, &other, NOW + KEEP + 1).unwrap();
        let anchors = cache.anchors(REMOTE).unwrap();
        assert_eq!(anchors.len(), 1, "only what was asked about lately");
        assert_eq!(anchors[0].branch, "other");
        assert!(
            cache.version("b1", 1).unwrap().is_none(),
            "and its brief with it"
        );
    }

    #[test]
    fn it_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        drop(holding_a_brief(Some(dir.path())));
        let cache = Cache::open(Some(dir.path()), "me").unwrap();
        assert_eq!(cache.anchors(REMOTE).unwrap().len(), 1, "that's the point");
    }

    #[test]
    fn what_someone_wrote_stays_until_they_send_it() {
        let dir = tempfile::tempdir().unwrap();
        {
            let cache = Cache::open(Some(dir.path()), "me").unwrap();
            cache
                .add_note("b1", 2, &CommentTo::Block(3), "Cut this")
                .unwrap();
            let passage = CommentTo::Passage {
                block: 4,
                quote: "the words".into(),
            };
            cache.add_note("b1", 2, &passage, "Why?").unwrap();
            cache.add_note("b1", 1, &CommentTo::Brief, "older").unwrap();
        }
        let cache = Cache::open(Some(dir.path()), "me").unwrap();
        let notes = cache.notes("b1", 2).unwrap();
        assert_eq!(
            notes
                .iter()
                .map(|note| note.body.as_str())
                .collect::<Vec<_>>(),
            ["Cut this", "Why?"],
            "a restart keeps them, in the order written, for that version alone"
        );
        cache.remove_note(notes[0].id).unwrap();
        assert_eq!(cache.notes("b1", 2).unwrap().len(), 1);
    }

    #[test]
    fn the_newest_version_read_is_remembered() {
        let cache = Cache::open(None, "me").unwrap();
        assert_eq!(cache.seen("b1").unwrap(), None);
        cache.saw("b1", 3).unwrap();
        cache.saw("b1", 2).unwrap();
        assert_eq!(
            cache.seen("b1").unwrap(),
            Some(3),
            "looking at an older one doesn't take back having read the newer"
        );
    }

    #[test]
    fn another_account_never_sees_what_someone_wrote() {
        let dir = tempfile::tempdir().unwrap();
        (Cache::open(Some(dir.path()), "me").unwrap())
            .add_note("b1", 2, &CommentTo::Brief, "private")
            .unwrap();
        let cache = Cache::open(Some(dir.path()), "someone else").unwrap();
        assert!(cache.notes("b1", 2).unwrap().is_empty());
    }

    #[test]
    fn another_account_finds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        drop(holding_a_brief(Some(dir.path())));
        let cache = Cache::open(Some(dir.path()), "someone else").unwrap();
        assert!(cache.anchors(REMOTE).unwrap().is_empty());
        assert!(
            cache.version("b1", 1).unwrap().is_none(),
            "a brief can be private to who read it"
        );
    }

    #[test]
    fn another_schema_starts_over_instead_of_migrating() {
        let dir = tempfile::tempdir().unwrap();
        let cache = holding_a_brief(Some(dir.path()));
        (cache.conn)
            .pragma_update(None, "user_version", SCHEMA + 1)
            .unwrap();
        drop(cache);
        let cache = Cache::open(Some(dir.path()), "me").unwrap();
        assert!(
            cache.anchors(REMOTE).unwrap().is_empty(),
            "it's a cache, so it fills again"
        );
    }

    #[test]
    fn another_schema_keeps_what_was_written_and_not_sent() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(Some(dir.path()), "me").unwrap();
        cache.add_note("b1", 1, &CommentTo::Brief, "Why?").unwrap();
        (cache.conn)
            .pragma_update(None, "user_version", SCHEMA + 1)
            .unwrap();
        drop(cache);
        let cache = Cache::open(Some(dir.path()), "me").unwrap();
        assert_eq!(
            cache.notes("b1", 1).unwrap().len(),
            1,
            "someone's own words are nowhere else"
        );
    }

    #[test]
    fn a_file_sqlite_cannot_read_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            "not a database, long enough to say so",
        )
        .unwrap();
        let mut cache = Cache::open(Some(dir.path()), "me").unwrap();
        cache.took(REMOTE, &asked(), NOW).unwrap();
        drop(cache);
        let cache = Cache::open(Some(dir.path()), "me").unwrap();
        assert_eq!(
            cache.anchors(REMOTE).unwrap().len(),
            1,
            "it's a working file again, not one in memory"
        );
    }

    #[test]
    fn a_folder_that_will_not_open_still_works_in_memory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not a folder");
        std::fs::write(&file, "").unwrap();
        let mut cache = Cache::open(Some(&file), "me").unwrap();
        cache.took(REMOTE, &asked(), NOW).unwrap();
        assert_eq!(
            cache.anchors(REMOTE).unwrap().len(),
            1,
            "a bad disk costs what's kept, not the feature"
        );
    }
}
