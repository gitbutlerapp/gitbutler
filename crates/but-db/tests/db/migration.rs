mod run {
    use std::time::{Duration, Instant};

    use but_db::{M, SchemaVersion, migration};

    use crate::migration::util::{dump_data, dump_schema};
    const ZERO: SchemaVersion = SchemaVersion::Zero;

    fn run_with_backoff<'a>(
        conn: &mut rusqlite::Connection,
        migrations: &[M<'a>],
    ) -> Result<usize, migration::Error> {
        let mut count = 0;
        let policy = backoff::ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(10))
            .with_randomization_factor(0.0)
            .with_multiplier(1.0)
            .with_max_interval(Duration::from_millis(10))
            .with_max_elapsed_time(Some(Duration::from_secs(1)))
            .build();
        backoff::retry(policy, || {
            count = migration::run(conn, migrations.iter().copied())?;
            Ok::<_, migration::Error>(())
        })?;
        Ok(count)
    }

    #[test]
    fn all_or_nothing() -> anyhow::Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;

        let (good, bad) = (
            M::up(0, ZERO, "CREATE TABLE T1 ( first TEXT PRIMARY KEY );"),
            M::up(1, ZERO, "bad"),
        );
        let err = migration::run(&mut db, [good, bad]).unwrap_err();
        assert!(matches!(err, backoff::Error::Permanent(_)));

        insta::assert_snapshot!(dump_schema(&db)?, @r"");
        insta::assert_snapshot!(dump_data(&db)?, @r"");
        Ok(())
    }

    #[test]
    fn read_only_until_it_needs_a_lock() -> anyhow::Result<()> {
        use rusqlite::TransactionBehavior;
        let tmp = tempfile::TempDir::new()?;
        let db_path = tmp.path().join("db");
        let mut db1 = rusqlite::Connection::open(&db_path)?;
        let mut db2 = rusqlite::Connection::open(&db_path)?;

        let migrations = [
            M::up(0, ZERO, "CREATE TABLE T1 ( first TEXT PRIMARY KEY );"),
            M::up(1, ZERO, "CREATE TABLE T2 ( first TEXT PRIMARY KEY );"),
        ];

        {
            let _blocking_trans = db1.transaction_with_behavior(TransactionBehavior::Immediate)?;

            let start = Instant::now();
            let busy_timeout = Duration::from_millis(100);
            db2.busy_timeout(busy_timeout)?;
            let err = migration::run(&mut db2, migrations).unwrap_err();
            let elapsed = start.elapsed();
            assert!(
                matches!(
                    err,
                    backoff::Error::Transient {
                        ref err,
                        ..
                    } if err.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy)
                ),
                "it wants to write, but can't, but knows it's a locking issue"
            );
            assert!(
                elapsed < busy_timeout / 2,
                "initial migration attempt should fail before busy_timeout when the deferred read snapshot cannot be upgraded: {elapsed:?}"
            );
        }

        let count = migration::run(&mut db2, migrations)?;
        assert_eq!(count, 2, "DB is unlocked and migrations are performed");

        {
            let _blocking_trans = db1.transaction_with_behavior(TransactionBehavior::Immediate)?;

            let count = migration::run(&mut db2, migrations)?;
            assert_eq!(count, 0, "It reads first which doesn't run into the lock");

            let count = migration::run(&mut db2, [migrations[0]])?;
            assert_eq!(
                count, 0,
                "older compatible migration lists also stay read-only despite extra migration rows in the DB"
            );
        }

        insta::assert_snapshot!(dump_schema(&db1)?, @"
        -- table T1
        CREATE TABLE T1 ( first TEXT PRIMARY KEY );

        -- table T2
        CREATE TABLE T2 ( first TEXT PRIMARY KEY );

        -- table __diesel_schema_migrations
        CREATE TABLE __diesel_schema_migrations (
               version VARCHAR(50) PRIMARY KEY NOT NULL,
               run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        ");
        insta::assert_snapshot!(dump_data(&db1)?, @r#"
        Table: __diesel_schema_migrations
        version
        Text("0")
        Text("1")

        Table: T1
        first

        Table: T2
        first
        "#);
        Ok(())
    }

    #[test]
    fn initialization_retries_lock_failures_with_backoff_in_threads() -> anyhow::Result<()> {
        use rusqlite::TransactionBehavior;

        let tmp = tempfile::TempDir::new()?;
        let db_path = tmp.path().join("db");
        let mut db = rusqlite::Connection::open(&db_path)?;
        let blocking_trans = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let hold_lock = Duration::from_millis(250);
        let busy_timeout = Duration::from_secs(1);

        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(0);
        let thread = std::thread::spawn({
            let db_path = db_path.clone();
            move || -> anyhow::Result<Duration> {
                let mut db = rusqlite::Connection::open(&db_path)?;
                db.busy_timeout(busy_timeout)?;

                started_tx.send(())?;
                let start = Instant::now();
                let migrations = [M::up(
                    0,
                    ZERO,
                    "CREATE TABLE T1 ( first TEXT PRIMARY KEY );",
                )];
                let count = run_with_backoff(&mut db, &migrations)?;
                assert_eq!(count, 1, "migration succeeds once the lock is released");
                Ok(start.elapsed())
            }
        });

        started_rx
            .recv()
            .expect("worker starts before we release the lock");
        std::thread::sleep(hold_lock);
        // Release the DB lock.
        drop(blocking_trans);

        let elapsed = thread.join().expect("worker did not panic")?;
        let safety_margin = Duration::from_millis(75);
        assert!(
            elapsed >= hold_lock - safety_margin,
            "busy timeout should wait for lock release, elapsed={elapsed:?}"
        );
        Ok(())
    }

    #[test]
    fn journey() -> anyhow::Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;

        let (old, new) = (
            M::up(0, ZERO, "CREATE TABLE T1 ( first VARCHAR(50) PRIMARY KEY )"),
            M::up(1, ZERO, "ALTER TABLE `T1` ADD COLUMN `two` TEXT"),
        );
        let incorrectly_ordered = [new, old];
        let count = migration::run(&mut db, incorrectly_ordered)?;
        assert_eq!(count, 2, "both migrations ran the first time");

        let count = migration::run(&mut db, incorrectly_ordered)?;
        assert_eq!(count, 0, "everything is up-to-date already");

        insta::assert_snapshot!(dump_schema(&db)?, @"
        -- table T1
        CREATE TABLE T1 ( first VARCHAR(50) PRIMARY KEY , `two` TEXT);

        -- table __diesel_schema_migrations
        CREATE TABLE __diesel_schema_migrations (
               version VARCHAR(50) PRIMARY KEY NOT NULL,
               run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        ");
        insta::assert_snapshot!(dump_data(&db)?, @r#"
        Table: __diesel_schema_migrations
        version
        Text("0")
        Text("1")

        Table: T1
        first | two
        "#);

        let count = migration::run(&mut db, [old])
            .expect("older migration list is accepted if DB is compatible");
        assert_eq!(
            count, 0,
            "extra migrations in the DB are tolerated when forward-compatibility allows it"
        );

        let newer_new = M::up(2, ZERO, "ALTER TABLE `T1` ADD COLUMN `two` TEXT");
        let err = migration::run(&mut db, [old, /* 'new' missing */ newer_new])
            .expect_err("cannot skip a migration");
        assert!(matches!(err, backoff::Error::Permanent(_)));
        Ok(())
    }

    #[test]
    fn rejects_forward_incompatible_database() -> anyhow::Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;

        db.execute_batch(&format!(
            "PRAGMA user_version = {};",
            SchemaVersion::One as u32
        ))?;
        let err = migration::run(
            &mut db,
            [M::up(
                0,
                ZERO,
                "CREATE TABLE T1 ( first TEXT PRIMARY KEY );",
            )],
        )
        .expect_err("future forward-compatibility version must be rejected");
        assert!(matches!(err, backoff::Error::Permanent(_)));
        Ok(())
    }

    #[test]
    fn schema_version_is_derived_from_passed_migrations() -> anyhow::Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;
        let forward_incompatible = [M::up(
            0,
            SchemaVersion::One,
            "CREATE TABLE T1 ( first TEXT PRIMARY KEY );",
        )];

        migration::run(&mut db, forward_incompatible)?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(version, SchemaVersion::One as u32);

        let err = migration::run(
            &mut db,
            [M::up(
                0,
                ZERO,
                "CREATE TABLE T1 ( first TEXT PRIMARY KEY );",
            )],
        )
        .expect_err("older binaries must reject newer schema versions");
        assert!(matches!(err, backoff::Error::Permanent(_)));
        Ok(())
    }

    #[test]
    fn schema_version_bumps_even_if_all_migrations_are_already_applied() -> anyhow::Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;
        let compatible = [M::up(
            0,
            ZERO,
            "CREATE TABLE T1 ( first TEXT PRIMARY KEY );",
        )];
        migration::run(&mut db, compatible)?;

        let initial_version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(initial_version, ZERO as u32);

        let forward_incompatible = [M::up(
            0,
            SchemaVersion::One,
            "CREATE TABLE T1 ( first TEXT PRIMARY KEY );",
        )];
        let count = migration::run(&mut db, forward_incompatible)?;
        assert_eq!(
            count, 0,
            "no SQL migration reruns when only the schema version changes"
        );

        let bumped_version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(
            bumped_version,
            SchemaVersion::One as u32,
            "schema version is still bumped when the migration list is otherwise up-to-date"
        );
        Ok(())
    }

    #[test]
    fn schema_version_bumps_even_if_database_has_extra_migrations() -> anyhow::Result<()> {
        let mut db = rusqlite::Connection::open_in_memory()?;
        let first_sql = "CREATE TABLE T1 ( first TEXT PRIMARY KEY );";
        let compatible = [
            M::up(0, ZERO, first_sql),
            M::up(1, ZERO, "CREATE TABLE T2 ( first TEXT PRIMARY KEY );"),
        ];
        migration::run(&mut db, compatible)?;

        let initial_version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(initial_version, ZERO as u32);

        let forward_incompatible = [M::up(0, SchemaVersion::One, first_sql)];
        let count = migration::run(&mut db, forward_incompatible)?;
        assert_eq!(
            count, 0,
            "no SQL migration reruns when only the schema version changes and the DB already has extra compatible migrations"
        );

        let bumped_version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(
            bumped_version,
            SchemaVersion::One as u32,
            "schema version is still bumped when the DB contains additional compatible migrations"
        );
        Ok(())
    }

    #[test]
    fn run_ours() -> anyhow::Result<()> {
        // See all of our schema combined in the latest version, along with
        // all migration versions we ran to get there.
        let mut db = rusqlite::Connection::open_in_memory()?;
        migration::run(&mut db, but_db::migration::ours())?;

        insta::assert_snapshot!(dump_schema(&db)?, @"
        -- table __diesel_schema_migrations
        CREATE TABLE __diesel_schema_migrations (
               version VARCHAR(50) PRIMARY KEY NOT NULL,
               run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        -- table butler_actions
        CREATE TABLE `butler_actions`(
        	`id` TEXT NOT NULL PRIMARY KEY,
        	`created_at` TIMESTAMP NOT NULL,
        	`handler` TEXT NOT NULL,
        	`snapshot_before` TEXT NOT NULL,
        	`snapshot_after` TEXT NOT NULL,
        	`response` TEXT,
        	`error` TEXT
        , `external_summary` TEXT NOT NULL, `external_prompt` TEXT, `source` TEXT);

        -- table ci_checks
        CREATE TABLE `ci_checks`(
        	`id` BIGINT NOT NULL PRIMARY KEY,
        	`name` TEXT NOT NULL,
        	`output_summary` TEXT NOT NULL,
        	`output_text` TEXT NOT NULL,
        	`output_title` TEXT NOT NULL,
        	`started_at` TIMESTAMP,
        	`status_type` TEXT NOT NULL,
        	`status_conclusion` TEXT,
        	`status_completed_at` TIMESTAMP,
        	`head_sha` TEXT NOT NULL,
        	`url` TEXT NOT NULL,
        	`html_url` TEXT NOT NULL,
        	`details_url` TEXT NOT NULL,
        	`pull_requests` TEXT NOT NULL,
        	`reference` TEXT NOT NULL,
        	`last_sync_at` TIMESTAMP NOT NULL,
        	`struct_version` INTEGER NOT NULL
        );

        -- table claude_messages
        CREATE TABLE `claude_messages`(
        	`id` TEXT NOT NULL PRIMARY KEY,
        	`session_id` TEXT NOT NULL REFERENCES claude_sessions(id),
        	`created_at` TIMESTAMP NOT NULL,
        	`content_type` TEXT NOT NULL,
        	`content` TEXT NOT NULL
        );

        -- table claude_permission_requests
        CREATE TABLE `claude_permission_requests`(
        	`id` TEXT NOT NULL PRIMARY KEY,
        	`created_at` TIMESTAMP NOT NULL,
        	`updated_at` TIMESTAMP NOT NULL,
        	`tool_name` TEXT NOT NULL,
        	`input` TEXT NOT NULL,
        	`decision` TEXT, `use_wildcard` BOOLEAN NOT NULL DEFAULT 0);

        -- table claude_sessions
        CREATE TABLE `claude_sessions`(
        	`id` TEXT NOT NULL PRIMARY KEY,
        	`current_id` TEXT NOT NULL UNIQUE,
        	`created_at` TIMESTAMP NOT NULL,
        	`updated_at` TIMESTAMP NOT NULL
        , in_gui BOOLEAN NOT NULL DEFAULT FALSE, session_ids TEXT NOT NULL DEFAULT '[]', approved_permissions TEXT NOT NULL DEFAULT '[]', denied_permissions TEXT NOT NULL DEFAULT '[]');

        -- table file_write_locks
        CREATE TABLE `file_write_locks`(
        	`path` TEXT NOT NULL PRIMARY KEY,
        	`created_at` TIMESTAMP NOT NULL,
        	`owner` TEXT NOT NULL
        );

        -- table forge_reviews
        CREATE TABLE `forge_reviews`(
        	`html_url` TEXT NOT NULL,
        	`number` BIGINT NOT NULL PRIMARY KEY,
        	`title` TEXT NOT NULL,
        	`body` TEXT,
        	`author` TEXT,
        	`labels` TEXT NOT NULL,
        	`draft` BOOL NOT NULL,
        	`source_branch` TEXT NOT NULL,
        	`target_branch` TEXT NOT NULL,
        	`sha` TEXT NOT NULL,
        	`created_at` TIMESTAMP,
        	`modified_at` TIMESTAMP,
        	`merged_at` TIMESTAMP,
        	`closed_at` TIMESTAMP,
        	`repository_ssh_url` TEXT,
        	`repository_https_url` TEXT,
        	`repo_owner` TEXT,
        	`reviewers` TEXT NOT NULL,
        	`unit_symbol` TEXT NOT NULL,
        	`last_sync_at` TIMESTAMP NOT NULL,
        	`struct_version` INTEGER NOT NULL
        );

        -- table gerrit_metadata
        CREATE TABLE `gerrit_metadata`(
        	`change_id` TEXT NOT NULL PRIMARY KEY,
        	`commit_id` TEXT NOT NULL,
        	`review_url` TEXT NOT NULL,
        	`created_at` TIMESTAMP NOT NULL,
        	`updated_at` TIMESTAMP NOT NULL
        );

        -- table hunk_assignments
        CREATE TABLE `hunk_assignments`(
        	`hunk_header` TEXT,
        	`path` TEXT NOT NULL,
        	`path_bytes` BINARY NOT NULL,
        	`stack_id` TEXT,
        	`id` TEXT,
        	PRIMARY KEY(`path`, `hunk_header`)
        );

        -- table vb_branch_targets
        CREATE TABLE `vb_branch_targets`(
        	`stack_id` TEXT NOT NULL PRIMARY KEY,
        	`remote_name` TEXT NOT NULL,
        	`branch_name` TEXT NOT NULL,
        	`remote_url` TEXT NOT NULL,
        	`sha` TEXT NOT NULL,
        	`push_remote_name` TEXT,
        	FOREIGN KEY(`stack_id`) REFERENCES `vb_stacks`(`id`) ON DELETE CASCADE
        );

        -- table vb_stack_heads
        CREATE TABLE `vb_stack_heads`(
        	`stack_id` TEXT NOT NULL,
        	`position` INTEGER NOT NULL,
        	`name` TEXT NOT NULL,
        	`head_sha` TEXT NOT NULL,
        	`pr_number` INTEGER,
        	`archived` INTEGER NOT NULL DEFAULT 0,
        	`review_id` TEXT,
        	PRIMARY KEY(`stack_id`, `position`),
        	FOREIGN KEY(`stack_id`) REFERENCES `vb_stacks`(`id`) ON DELETE CASCADE
        );

        -- table vb_stacks
        CREATE TABLE `vb_stacks`(
        	`id` TEXT NOT NULL PRIMARY KEY,
        	`source_refname` TEXT,
        	`upstream_remote_name` TEXT,
        	`upstream_branch_name` TEXT,
        	`sort_order` INTEGER NOT NULL,
        	`in_workspace` INTEGER NOT NULL,
        	`legacy_name` TEXT NOT NULL DEFAULT '',
        	`legacy_notes` TEXT NOT NULL DEFAULT '',
        	`legacy_ownership` TEXT NOT NULL DEFAULT '',
        	`legacy_allow_rebasing` INTEGER NOT NULL DEFAULT 1,
        	`legacy_post_commits` INTEGER NOT NULL DEFAULT 0,
        	`legacy_tree_sha` TEXT NOT NULL DEFAULT '0000000000000000000000000000000000000000',
        	`legacy_head_sha` TEXT NOT NULL DEFAULT '0000000000000000000000000000000000000000',
        	`legacy_created_timestamp_ms` TEXT NOT NULL DEFAULT '0',
        	`legacy_updated_timestamp_ms` TEXT NOT NULL DEFAULT '0'
        );

        -- table vb_state
        CREATE TABLE `vb_state`(
        	`id` INTEGER PRIMARY KEY CHECK (`id` = 1),
        	`initialized` INTEGER NOT NULL DEFAULT 0,
        	`default_target_remote_name` TEXT,
        	`default_target_branch_name` TEXT,
        	`default_target_remote_url` TEXT,
        	`default_target_sha` TEXT,
        	`default_target_push_remote_name` TEXT,
        	`last_pushed_base_sha` TEXT,
        	`toml_last_seen_mtime_ns` INTEGER,
        	`toml_last_seen_sha256` TEXT
        );

        -- table workflows
        CREATE TABLE `workflows`(
        	`id` TEXT NOT NULL PRIMARY KEY,
        	`created_at` TIMESTAMP NOT NULL,
        	`kind` TEXT NOT NULL,
        	`triggered_by` TEXT NOT NULL,
        	`status` TEXT NOT NULL,
        	`input_commits` TEXT NOT NULL,
        	`output_commits` TEXT NOT NULL,
        	`summary` TEXT
        );

        -- table workspace_rules
        CREATE TABLE `workspace_rules`(
        	`id` TEXT NOT NULL PRIMARY KEY,
        	`created_at` TIMESTAMP NOT NULL,
        	`enabled` BOOL NOT NULL,
        	`trigger` TEXT NOT NULL,
        	`filters` TEXT NOT NULL,
        	`action` TEXT NOT NULL
        );

        -- index idx_butler_actions_created_at
        CREATE INDEX `idx_butler_actions_created_at` ON `butler_actions`(`created_at`);

        -- index idx_ci_checks_reference
        CREATE INDEX `idx_ci_checks_reference` ON `ci_checks`(`reference`);

        -- index idx_vb_stack_heads_stack_id
        CREATE INDEX `idx_vb_stack_heads_stack_id` ON `vb_stack_heads`(`stack_id`);

        -- index idx_vb_stacks_in_workspace
        CREATE INDEX `idx_vb_stacks_in_workspace` ON `vb_stacks`(`in_workspace`);

        -- index idx_vb_stacks_sort_order
        CREATE INDEX `idx_vb_stacks_sort_order` ON `vb_stacks`(`sort_order`);

        -- index index_claude_messages_on_created_at
        CREATE INDEX index_claude_messages_on_created_at ON claude_messages (created_at);

        -- index index_claude_messages_on_session_id
        CREATE INDEX index_claude_messages_on_session_id ON claude_messages (session_id);

        -- index index_claude_sessions_on_current_id
        CREATE INDEX index_claude_sessions_on_current_id ON claude_sessions (current_id);
        ");

        insta::assert_snapshot!(dump_data(&db)?, @r#"
        Table: __diesel_schema_migrations
        version
        Text("20250526145725")
        Text("20250529110746")
        Text("20250530112246")
        Text("20250603111503")
        Text("20250607113323")
        Text("20250616090656")
        Text("20250619181700")
        Text("20250619192246")
        Text("20250703203544")
        Text("20250704130757")
        Text("20250717150441")
        Text("20250811130145")
        Text("20250812093543")
        Text("20250817195624")
        Text("20250821095340")
        Text("20250821142109")
        Text("20251013092749")
        Text("20251014144801")
        Text("20251015105125")
        Text("20251015212443")
        Text("20251017092314")
        Text("20251106102333")
        Text("20251107134016")
        Text("20251110103940")
        Text("20260101223932")
        Text("20260105095934")
        Text("20260219130000")

        Table: hunk_assignments
        hunk_header | path | path_bytes | stack_id | id

        Table: butler_actions
        id | created_at | handler | snapshot_before | snapshot_after | response | error | external_summary | external_prompt | source

        Table: workflows
        id | created_at | kind | triggered_by | status | input_commits | output_commits | summary

        Table: file_write_locks
        path | created_at | owner

        Table: workspace_rules
        id | created_at | enabled | trigger | filters | action

        Table: claude_sessions
        id | current_id | created_at | updated_at | in_gui | session_ids | approved_permissions | denied_permissions

        Table: claude_messages
        id | session_id | created_at | content_type | content

        Table: claude_permission_requests
        id | created_at | updated_at | tool_name | input | decision | use_wildcard

        Table: gerrit_metadata
        change_id | commit_id | review_url | created_at | updated_at

        Table: forge_reviews
        html_url | number | title | body | author | labels | draft | source_branch | target_branch | sha | created_at | modified_at | merged_at | closed_at | repository_ssh_url | repository_https_url | repo_owner | reviewers | unit_symbol | last_sync_at | struct_version

        Table: ci_checks
        id | name | output_summary | output_text | output_title | started_at | status_type | status_conclusion | status_completed_at | head_sha | url | html_url | details_url | pull_requests | reference | last_sync_at | struct_version

        Table: vb_state
        id | initialized | default_target_remote_name | default_target_branch_name | default_target_remote_url | default_target_sha | default_target_push_remote_name | last_pushed_base_sha | toml_last_seen_mtime_ns | toml_last_seen_sha256

        Table: vb_stacks
        id | source_refname | upstream_remote_name | upstream_branch_name | sort_order | in_workspace | legacy_name | legacy_notes | legacy_ownership | legacy_allow_rebasing | legacy_post_commits | legacy_tree_sha | legacy_head_sha | legacy_created_timestamp_ms | legacy_updated_timestamp_ms

        Table: vb_stack_heads
        stack_id | position | name | head_sha | pr_number | archived | review_id

        Table: vb_branch_targets
        stack_id | remote_name | branch_name | remote_url | sha | push_remote_name
        "#);

        let count = migration::run(&mut db, but_db::migration::ours())?;
        assert_eq!(count, 0, "nothing has to run as everything is uptodate");

        Ok(())
    }
}

mod util {
    use std::fmt::Write;
    pub fn dump_data(conn: &rusqlite::Connection) -> anyhow::Result<String> {
        // Get all table names
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )?;

        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut out = String::new();
        for table in tables {
            dump_table(conn, &table, &mut out)?;
            writeln!(out)?;
        }
        Ok(out)
    }

    pub fn dump_schema(conn: &rusqlite::Connection) -> anyhow::Result<String> {
        let mut stmt = conn.prepare(
            "SELECT type, name, sql FROM sqlite_master
         WHERE sql NOT NULL
         ORDER BY CASE type
             WHEN 'table' THEN 1
             WHEN 'index' THEN 2
             WHEN 'trigger' THEN 3
             WHEN 'view' THEN 4
         END, name",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut out = String::new();
        for row in rows {
            let (type_, name, sql) = row?;
            writeln!(out, "-- {type_} {name}")?;
            writeln!(out, "{sql};")?;
            writeln!(out)?;
        }
        Ok(out)
    }

    fn dump_table(
        conn: &rusqlite::Connection,
        table_name: &str,
        out: &mut String,
    ) -> anyhow::Result<()> {
        let query = if table_name == "__diesel_schema_migrations" {
            format!("SELECT version FROM {table_name}")
        } else {
            format!("SELECT * FROM {table_name}")
        };
        let mut stmt = conn.prepare(&query)?;

        let column_count = stmt.column_count();
        let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

        writeln!(out, "Table: {table_name}")?;
        writeln!(out, "{}", column_names.join(" | "))?;

        let rows = stmt.query_map([], |row| {
            let mut values = Vec::new();
            for i in 0..column_count {
                let val: String = row
                    .get::<_, rusqlite::types::Value>(i)
                    .map(|v| format!("{v:?}"))
                    .unwrap_or_else(|_| "NULL".to_string());
                values.push(val);
            }
            Ok(values.join(" | "))
        })?;

        for row in rows {
            writeln!(out, "{}", row?)?;
        }
        Ok(())
    }
}
