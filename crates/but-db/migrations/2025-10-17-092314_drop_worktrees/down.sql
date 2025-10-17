-- Recreate worktrees table (for rollback only, not functional)
CREATE TABLE `worktrees`(
  path BLOB NOT NULL PRIMARY KEY,
  base TEXT NOT NULL,
  created_from_ref BLOB
);
