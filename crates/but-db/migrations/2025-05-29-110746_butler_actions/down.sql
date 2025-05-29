-- This file should undo anything in `up.sql`

DROP INDEX IF EXISTS `idx_butler_actions_created_at`;
DROP TABLE IF EXISTS `butler_actions`;
