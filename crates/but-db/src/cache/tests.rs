use crate::M;

mod open_with_migrations_infallible {
    use super::{migrations, table_exists};

    #[test]
    fn destination_writable() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let tmp_path = tmp.path().join("cache.sqlite");
        let (conn, url) = open_with_migrations_infallible(&tmp_path, migrations());
        assert_eq!(
            url, tmp_path,
            "writable location means we get to write there"
        );
        assert!(table_exists(&conn, "foo")?);
        Ok(())
    }
    use crate::cache::open_with_migrations_infallible;

    #[test]
    fn destination_no_writable() {
        let (conn, url) = open_with_migrations_infallible(
            "/proc/cannot-be-created.sqlite".as_ref(),
            migrations(),
        );
        assert_eq!(
            url, ":memory:",
            "Permanent failures to open fall back to memory"
        );
        assert!(table_exists(&conn, "foo").unwrap());
    }

    #[test]
    fn destination_corrupt_auto_cleans_the_database() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let url = tmp.path().join("corrupted-db.sqlite");
        std::fs::write(&url, "definitely not valid sqlite")?;

        let (conn, actual_url) = open_with_migrations_infallible(&url, migrations());
        assert_eq!(
            actual_url, url,
            "it removed the broken file and opened the database anyway"
        );
        assert!(table_exists(&conn, "foo")?);
        Ok(())
    }

    #[test]
    fn from_memory() -> anyhow::Result<()> {
        let mem_url = ":memory:";
        let (conn, actual_url) = open_with_migrations_infallible(mem_url.as_ref(), migrations());
        assert_eq!(actual_url, mem_url, "it's OK to open from memory directly.");
        assert!(table_exists(&conn, "foo")?);
        Ok(())
    }
}

fn migrations() -> impl Iterator<Item = M<'static>> + Clone {
    Some(M::up(
        1,
        "CREATE TABLE `foo`(
            `id` TEXT NOT NULL PRIMARY KEY
        );",
    ))
    .into_iter()
}

fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT 1 FROM sqlite_master
         WHERE type='table' AND name=?
         LIMIT 1",
    )?;
    // `query_row` will return the first row (if any) or an error if none.
    // The `?` in the query is bound to the table name we passed.
    let mut rows = stmt.query([table_name])?;
    Ok(rows.next().expect("valid query").is_some())
}
