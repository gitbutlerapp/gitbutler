use crate::{DbHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20260219130000,
    SchemaVersion::Zero,
    "CREATE TABLE `vb_state`(
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

CREATE TABLE `vb_branch_targets`(
	`stack_id` TEXT NOT NULL PRIMARY KEY,
	`remote_name` TEXT NOT NULL,
	`branch_name` TEXT NOT NULL,
	`remote_url` TEXT NOT NULL,
	`sha` TEXT NOT NULL,
	`push_remote_name` TEXT,
	FOREIGN KEY(`stack_id`) REFERENCES `vb_stacks`(`id`) ON DELETE CASCADE
);

CREATE INDEX `idx_vb_stacks_sort_order` ON `vb_stacks`(`sort_order`);
CREATE INDEX `idx_vb_stacks_in_workspace` ON `vb_stacks`(`in_workspace`);
CREATE INDEX `idx_vb_stack_heads_stack_id` ON `vb_stack_heads`(`stack_id`);
",
)];

/// One-row state table for virtual branches metadata (`vb_state`).
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct VbState {
    /// Remote name of the default/base target (for example `origin`).
    pub default_target_remote_name: Option<String>,
    /// Branch name of the default/base target (for example `main`).
    pub default_target_branch_name: Option<String>,
    /// Remote URL of the default/base target.
    pub default_target_remote_url: Option<String>,
    /// Object id (hex) associated with the default/base target.
    pub default_target_sha: Option<String>,
    /// Optional remote name used for pushes of the default/base target.
    pub default_target_push_remote_name: Option<String>,
    /// Last pushed base object id (hex), mirrored from `VirtualBranches::last_pushed_base`.
    pub last_pushed_base_sha: Option<String>,

    /// `true` once VB storage has been bootstrapped and synchronized at least once.
    pub initialized: bool,
    /// Last observed mtime (ns since unix epoch) for `virtual_branches.toml`.
    pub toml_last_seen_mtime_ns: Option<i64>,
    /// Last observed SHA-256 for `virtual_branches.toml`.
    pub toml_last_seen_sha256: Option<String>,
}

/// Normalized stack row from `vb_stacks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbStack {
    /// Stable stack id (`StackId`) serialized as text.
    pub id: String,
    /// If present, the original branch reference this stack came from.
    pub source_refname: Option<String>,
    /// Upstream remote component of the stack tracking reference.
    pub upstream_remote_name: Option<String>,
    /// Upstream branch component of the stack tracking reference.
    pub upstream_branch_name: Option<String>,
    /// UI/display sort order for this stack.
    pub sort_order: i64,
    /// Whether the stack is currently in the workspace (applied and visible to the user).
    pub in_workspace: bool,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_name: String,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_notes: String,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_ownership: String,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_allow_rebasing: bool,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_post_commits: bool,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_tree_sha: String,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_head_sha: String,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_created_timestamp_ms: String,
    /// Legacy field preserved for backward-compatible TOML round-trips.
    pub legacy_updated_timestamp_ms: String,
}

/// A stack head ("series") row from `vb_stack_heads`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbStackHead {
    /// Parent stack id this head belongs to.
    pub stack_id: String,
    /// Zero-based ordering position within the parent stack.
    pub position: i64,
    /// Name of the virtual head/series.
    pub name: String,
    /// Object id (hex) this head currently points to.
    pub head_sha: String,
    /// Optional pull request number associated with this head.
    pub pr_number: Option<i64>,
    /// Whether this head is archived/integrated.
    pub archived: bool,
    /// Optional review identifier associated with this head.
    pub review_id: Option<String>,
}

/// Per-stack target row from `vb_branch_targets`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbBranchTarget {
    /// Parent stack id this target belongs to.
    pub stack_id: String,
    /// Remote name of the branch target.
    pub remote_name: String,
    /// Branch name of the branch target.
    pub branch_name: String,
    /// Remote URL backing the target.
    pub remote_url: String,
    /// Target object id (hex).
    pub sha: String,
    /// Optional remote name used for push operations.
    pub push_remote_name: Option<String>,
}

/// Canonical normalized VB payload read from / written to the database.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VirtualBranchesSnapshot {
    /// Global singleton VB state row.
    pub state: VbState,
    /// All stacks ordered by `sort_order, id` when read.
    pub stacks: Vec<VbStack>,
    /// All stack heads ordered by `stack_id, position` when read.
    pub heads: Vec<VbStackHead>,
    /// All stack-specific branch targets ordered by `stack_id` when read.
    pub branch_targets: Vec<VbBranchTarget>,
}

impl DbHandle {
    /// Return a read-only handle for virtual-branches tables on this database connection.
    pub fn virtual_branches(&self) -> VirtualBranchesHandle<'_> {
        VirtualBranchesHandle { conn: &self.conn }
    }

    /// Return a mutating handle for virtual-branches tables backed by a savepoint.
    ///
    /// Mutating methods on [`VirtualBranchesHandleMut`] consume the handle and commit automatically.
    pub fn virtual_branches_mut(&mut self) -> rusqlite::Result<VirtualBranchesHandleMut<'_>> {
        Ok(VirtualBranchesHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl<'conn> Transaction<'conn> {
    /// Return a read-only handle for virtual-branches tables scoped to this transaction.
    pub fn virtual_branches(&self) -> VirtualBranchesHandle<'_> {
        VirtualBranchesHandle { conn: self.inner() }
    }

    /// Return a mutating handle for virtual-branches tables scoped to this transaction.
    ///
    /// Mutating methods on [`VirtualBranchesHandleMut`] consume the handle and commit automatically.
    pub fn virtual_branches_mut(&mut self) -> rusqlite::Result<VirtualBranchesHandleMut<'_>> {
        Ok(VirtualBranchesHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

/// Read-only accessor for virtual-branches tables.
///
/// Created from [`DbHandle::virtual_branches`] or [`Transaction::virtual_branches`].
pub struct VirtualBranchesHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

/// Mutating accessor for virtual-branches tables, scoped to a savepoint.
///
/// Created from [`DbHandle::virtual_branches_mut`] or [`Transaction::virtual_branches_mut`].
/// Methods on this type consume the handle and commit on success.
pub struct VirtualBranchesHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl VirtualBranchesHandle<'_> {
    /// Read the complete normalized VB snapshot from the database.
    ///
    /// Returns `Ok(None)` if the singleton `vb_state` row does not exist yet.
    pub fn get_snapshot(&self) -> rusqlite::Result<Option<VirtualBranchesSnapshot>> {
        // In autocommit mode, each SELECT would otherwise get its own snapshot.
        // We open one deferred read transaction so all reads in this method are consistent.
        // When already inside an outer transaction/savepoint (autocommit = false), we must not
        // issue BEGIN/COMMIT here and instead compose with the caller's transaction scope.
        let started_read_tx = if self.conn.is_autocommit() {
            self.conn.execute_batch("BEGIN DEFERRED TRANSACTION;")?;
            true
        } else {
            false
        };

        let out = (|| {
            let state = {
                let mut stmt = self.conn.prepare(
                    "SELECT initialized,
                            default_target_remote_name,
                            default_target_branch_name,
                            default_target_remote_url,
                            default_target_sha,
                            default_target_push_remote_name,
                            last_pushed_base_sha,
                            toml_last_seen_mtime_ns,
                            toml_last_seen_sha256
                     FROM vb_state
                     WHERE id = 1",
                )?;
                let mut rows = stmt.query([])?;
                let Some(row) = rows.next()? else {
                    return Ok(None);
                };
                VbState {
                    initialized: row.get(0)?,
                    default_target_remote_name: row.get(1)?,
                    default_target_branch_name: row.get(2)?,
                    default_target_remote_url: row.get(3)?,
                    default_target_sha: row.get(4)?,
                    default_target_push_remote_name: row.get(5)?,
                    last_pushed_base_sha: row.get(6)?,
                    toml_last_seen_mtime_ns: row.get(7)?,
                    toml_last_seen_sha256: row.get(8)?,
                }
            };

            let stacks = {
                let mut stmt = self.conn.prepare(
                    "SELECT id,
                            source_refname,
                            upstream_remote_name,
                            upstream_branch_name,
                            sort_order,
                            in_workspace,
                            legacy_name,
                            legacy_notes,
                            legacy_ownership,
                            legacy_allow_rebasing,
                            legacy_post_commits,
                            legacy_tree_sha,
                            legacy_head_sha,
                            legacy_created_timestamp_ms,
                            legacy_updated_timestamp_ms
                     FROM vb_stacks
                     ORDER BY sort_order, id",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(VbStack {
                        id: row.get(0)?,
                        source_refname: row.get(1)?,
                        upstream_remote_name: row.get(2)?,
                        upstream_branch_name: row.get(3)?,
                        sort_order: row.get(4)?,
                        in_workspace: row.get(5)?,
                        legacy_name: row.get(6)?,
                        legacy_notes: row.get(7)?,
                        legacy_ownership: row.get(8)?,
                        legacy_allow_rebasing: row.get(9)?,
                        legacy_post_commits: row.get(10)?,
                        legacy_tree_sha: row.get(11)?,
                        legacy_head_sha: row.get(12)?,
                        legacy_created_timestamp_ms: row.get(13)?,
                        legacy_updated_timestamp_ms: row.get(14)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            };

            let heads = {
                let mut stmt = self.conn.prepare(
                    "SELECT stack_id, position, name, head_sha, pr_number, archived, review_id
                     FROM vb_stack_heads
                     ORDER BY stack_id, position",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(VbStackHead {
                        stack_id: row.get(0)?,
                        position: row.get(1)?,
                        name: row.get(2)?,
                        head_sha: row.get(3)?,
                        pr_number: row.get(4)?,
                        archived: row.get(5)?,
                        review_id: row.get(6)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            };

            let branch_targets = {
                let mut stmt = self.conn.prepare(
                    "SELECT stack_id, remote_name, branch_name, remote_url, sha, push_remote_name
                     FROM vb_branch_targets
                     ORDER BY stack_id",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(VbBranchTarget {
                        stack_id: row.get(0)?,
                        remote_name: row.get(1)?,
                        branch_name: row.get(2)?,
                        remote_url: row.get(3)?,
                        sha: row.get(4)?,
                        push_remote_name: row.get(5)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()?
            };

            Ok(Some(VirtualBranchesSnapshot {
                state,
                stacks,
                heads,
                branch_targets,
            }))
        })();

        if started_read_tx {
            if out.is_ok() {
                self.conn.execute_batch("COMMIT;")?;
            } else {
                self.conn.execute_batch("ROLLBACK;").ok();
            }
        }

        out
    }
}

impl VirtualBranchesHandleMut<'_> {
    /// Convert this mutating handle into a read-only view on the same savepoint.
    pub fn to_ref(&self) -> VirtualBranchesHandle<'_> {
        VirtualBranchesHandle { conn: &self.sp }
    }

    /// Caller must call commit on the transaction.
    fn set_state_in_place(&mut self, state: &VbState) -> rusqlite::Result<()> {
        self.sp.execute(
            "INSERT INTO vb_state (
                id,
                initialized,
                default_target_remote_name,
                default_target_branch_name,
                default_target_remote_url,
                default_target_sha,
                default_target_push_remote_name,
                last_pushed_base_sha,
                toml_last_seen_mtime_ns,
                toml_last_seen_sha256
             ) VALUES (
                1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9
             )
             ON CONFLICT(id) DO UPDATE SET
                initialized = excluded.initialized,
                default_target_remote_name = excluded.default_target_remote_name,
                default_target_branch_name = excluded.default_target_branch_name,
                default_target_remote_url = excluded.default_target_remote_url,
                default_target_sha = excluded.default_target_sha,
                default_target_push_remote_name = excluded.default_target_push_remote_name,
                last_pushed_base_sha = excluded.last_pushed_base_sha,
                toml_last_seen_mtime_ns = excluded.toml_last_seen_mtime_ns,
                toml_last_seen_sha256 = excluded.toml_last_seen_sha256",
            rusqlite::params![
                state.initialized,
                state.default_target_remote_name,
                state.default_target_branch_name,
                state.default_target_remote_url,
                state.default_target_sha,
                state.default_target_push_remote_name,
                state.last_pushed_base_sha,
                state.toml_last_seen_mtime_ns,
                state.toml_last_seen_sha256,
            ],
        )?;
        Ok(())
    }

    /// Insert or update the singleton `vb_state` row (`id = 1`) and commit the savepoint.
    pub fn set_state(mut self, state: &VbState) -> rusqlite::Result<()> {
        self.set_state_in_place(state)?;
        self.sp.commit()
    }

    /// Replace all VB tables with the provided normalized snapshot.
    ///
    /// Existing stacks are cleared first (dependent rows are removed via FK cascade), followed by inserts.
    pub fn replace_snapshot(mut self, snapshot: &VirtualBranchesSnapshot) -> rusqlite::Result<()> {
        self.set_state_in_place(&snapshot.state)?;

        self.sp.execute("DELETE FROM vb_stacks", [])?;

        {
            let mut insert_stack = self.sp.prepare(
                "INSERT INTO vb_stacks (
                    id,
                    source_refname,
                    upstream_remote_name,
                    upstream_branch_name,
                    sort_order,
                    in_workspace,
                    legacy_name,
                    legacy_notes,
                    legacy_ownership,
                    legacy_allow_rebasing,
                    legacy_post_commits,
                    legacy_tree_sha,
                    legacy_head_sha,
                    legacy_created_timestamp_ms,
                    legacy_updated_timestamp_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?;
            for stack in &snapshot.stacks {
                insert_stack.execute(rusqlite::params![
                    stack.id,
                    stack.source_refname,
                    stack.upstream_remote_name,
                    stack.upstream_branch_name,
                    stack.sort_order,
                    stack.in_workspace,
                    stack.legacy_name,
                    stack.legacy_notes,
                    stack.legacy_ownership,
                    stack.legacy_allow_rebasing,
                    stack.legacy_post_commits,
                    stack.legacy_tree_sha,
                    stack.legacy_head_sha,
                    stack.legacy_created_timestamp_ms,
                    stack.legacy_updated_timestamp_ms,
                ])?;
            }
        }

        {
            let mut insert_head = self.sp.prepare(
                "INSERT INTO vb_stack_heads (
                    stack_id,
                    position,
                    name,
                    head_sha,
                    pr_number,
                    archived,
                    review_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for head in &snapshot.heads {
                insert_head.execute(rusqlite::params![
                    head.stack_id,
                    head.position,
                    head.name,
                    head.head_sha,
                    head.pr_number,
                    head.archived,
                    head.review_id,
                ])?;
            }
        }

        {
            let mut insert_target = self.sp.prepare(
                "INSERT INTO vb_branch_targets (
                    stack_id, remote_name, branch_name, remote_url, sha, push_remote_name
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for target in &snapshot.branch_targets {
                insert_target.execute(rusqlite::params![
                    target.stack_id,
                    target.remote_name,
                    target.branch_name,
                    target.remote_url,
                    target.sha,
                    target.push_remote_name,
                ])?;
            }
        }

        self.sp.commit()
    }
}
