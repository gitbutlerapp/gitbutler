-- This file should undo anything in `up.sql`
ALTER TABLE `butler_actions` DROP COLUMN `external_summary`;
ALTER TABLE `butler_actions` DROP COLUMN `external_prompt`;
ALTER TABLE `butler_actions` ADD COLUMN `external_prompt` TEXT NOT NULL;


