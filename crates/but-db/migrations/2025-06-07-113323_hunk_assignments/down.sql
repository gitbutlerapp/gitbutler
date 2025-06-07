-- This file should undo anything in `up.sql`

ALTER TABLE `hunk_assignments` ADD COLUMN `hunk_locks` TEXT NOT NULL;

