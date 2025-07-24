-- Your SQL goes here
ALTER TABLE `butler_actions` DROP COLUMN `external_prompt`;
ALTER TABLE `butler_actions` ADD COLUMN `external_summary` TEXT NOT NULL;
ALTER TABLE `butler_actions` ADD COLUMN `external_prompt` TEXT;
