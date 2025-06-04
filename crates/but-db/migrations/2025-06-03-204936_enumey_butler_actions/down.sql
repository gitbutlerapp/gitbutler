-- This file should undo anything in `up.sql`

-- Add back the created_at column to butler_mcp_actions
ALTER TABLE `butler_mcp_actions` ADD COLUMN created_at TIMESTAMP;

-- Populate the created_at column from the new butler_actions table
UPDATE `butler_mcp_actions` 
SET created_at = (
    SELECT created_at 
    FROM butler_actions 
    WHERE butler_actions.mcp_action_id = butler_mcp_actions.id
);

-- Recreate the old index on butler_mcp_actions.created_at
CREATE INDEX `idx_butler_actions_created_at` ON `butler_mcp_actions`(`created_at`);

-- Drop the new butler_actions table (this also drops its index)
DROP TABLE IF EXISTS `butler_actions`;

-- Drop the butler_revert_actions table
DROP TABLE IF EXISTS `butler_revert_actions`;

-- Rename butler_mcp_actions back to butler_actions
ALTER TABLE `butler_mcp_actions` RENAME TO `butler_actions`;
