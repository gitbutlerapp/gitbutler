use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20260219130000,
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
	`toml_last_seen_sha256` TEXT,
	`toml_mirror_dirty` INTEGER NOT NULL DEFAULT 0
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

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct VbState {
    pub initialized: bool,
    pub default_target_remote_name: Option<String>,
    pub default_target_branch_name: Option<String>,
    pub default_target_remote_url: Option<String>,
    pub default_target_sha: Option<String>,
    pub default_target_push_remote_name: Option<String>,
    pub last_pushed_base_sha: Option<String>,
    pub toml_last_seen_mtime_ns: Option<i64>,
    pub toml_last_seen_sha256: Option<String>,
    pub toml_mirror_dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbStack {
    pub id: String,
    pub source_refname: Option<String>,
    pub upstream_remote_name: Option<String>,
    pub upstream_branch_name: Option<String>,
    pub sort_order: i64,
    pub in_workspace: bool,
    pub legacy_name: String,
    pub legacy_notes: String,
    pub legacy_ownership: String,
    pub legacy_allow_rebasing: bool,
    pub legacy_post_commits: bool,
    pub legacy_tree_sha: String,
    pub legacy_head_sha: String,
    pub legacy_created_timestamp_ms: String,
    pub legacy_updated_timestamp_ms: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbStackHead {
    pub stack_id: String,
    pub position: i64,
    pub name: String,
    pub head_sha: String,
    pub pr_number: Option<i64>,
    pub archived: bool,
    pub review_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbBranchTarget {
    pub stack_id: String,
    pub remote_name: String,
    pub branch_name: String,
    pub remote_url: String,
    pub sha: String,
    pub push_remote_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VirtualBranchesSnapshot {
    pub state: VbState,
    pub stacks: Vec<VbStack>,
    pub heads: Vec<VbStackHead>,
    pub branch_targets: Vec<VbBranchTarget>,
}

impl DbHandle {
    pub fn virtual_branches(&self) -> VirtualBranchesHandle<'_> {
        VirtualBranchesHandle { conn: &self.conn }
    }

    pub fn virtual_branches_mut(&mut self) -> rusqlite::Result<VirtualBranchesHandleMut<'_>> {
        Ok(VirtualBranchesHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl<'conn> Transaction<'conn> {
    pub fn virtual_branches(&self) -> VirtualBranchesHandle<'_> {
        VirtualBranchesHandle { conn: self.inner() }
    }

    pub fn virtual_branches_mut(&mut self) -> rusqlite::Result<VirtualBranchesHandleMut<'_>> {
        Ok(VirtualBranchesHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

pub struct VirtualBranchesHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct VirtualBranchesHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl VirtualBranchesHandle<'_> {
    pub fn get_snapshot(&self) -> rusqlite::Result<Option<VirtualBranchesSnapshot>> {
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
                        toml_last_seen_sha256,
                        toml_mirror_dirty
                 FROM vb_state
                 WHERE id = 1",
            )?;
            let mut rows = stmt.query([])?;
            match rows.next()? {
                Some(row) => Some(VbState {
                    initialized: row.get(0)?,
                    default_target_remote_name: row.get(1)?,
                    default_target_branch_name: row.get(2)?,
                    default_target_remote_url: row.get(3)?,
                    default_target_sha: row.get(4)?,
                    default_target_push_remote_name: row.get(5)?,
                    last_pushed_base_sha: row.get(6)?,
                    toml_last_seen_mtime_ns: row.get(7)?,
                    toml_last_seen_sha256: row.get(8)?,
                    toml_mirror_dirty: row.get(9)?,
                }),
                None => None,
            }
        };

        let Some(state) = state else {
            return Ok(None);
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
    }
}

impl VirtualBranchesHandleMut<'_> {
    pub fn to_ref(&self) -> VirtualBranchesHandle<'_> {
        VirtualBranchesHandle { conn: &self.sp }
    }

    pub fn set_state(&mut self, state: &VbState) -> rusqlite::Result<()> {
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
                toml_last_seen_sha256,
                toml_mirror_dirty
             ) VALUES (
                1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
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
                toml_last_seen_sha256 = excluded.toml_last_seen_sha256,
                toml_mirror_dirty = excluded.toml_mirror_dirty",
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
                state.toml_mirror_dirty,
            ],
        )?;
        Ok(())
    }

    pub fn replace_snapshot(&mut self, snapshot: &VirtualBranchesSnapshot) -> rusqlite::Result<()> {
        self.set_state(&snapshot.state)?;

        self.sp.execute("DELETE FROM vb_stack_heads", [])?;
        self.sp.execute("DELETE FROM vb_branch_targets", [])?;
        self.sp.execute("DELETE FROM vb_stacks", [])?;

        for stack in &snapshot.stacks {
            self.sp.execute(
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
                rusqlite::params![
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
                ],
            )?;
        }

        for head in &snapshot.heads {
            self.sp.execute(
                "INSERT INTO vb_stack_heads (
                    stack_id,
                    position,
                    name,
                    head_sha,
                    pr_number,
                    archived,
                    review_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    head.stack_id,
                    head.position,
                    head.name,
                    head.head_sha,
                    head.pr_number,
                    head.archived,
                    head.review_id,
                ],
            )?;
        }

        for target in &snapshot.branch_targets {
            self.sp.execute(
                "INSERT INTO vb_branch_targets (
                    stack_id, remote_name, branch_name, remote_url, sha, push_remote_name
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    target.stack_id,
                    target.remote_name,
                    target.branch_name,
                    target.remote_url,
                    target.sha,
                    target.push_remote_name,
                ],
            )?;
        }

        Ok(())
    }

    pub fn commit(self) -> rusqlite::Result<()> {
        self.sp.commit()
    }
}
