-- This file should undo anything in `up.sql`
DROP INDEX IF EXISTS `idx_ci_checks_reference`;
DROP TABLE IF EXISTS `ci_checks`;
