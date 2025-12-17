-- Your SQL goes here
-- Replace approved (bool) and scope (text) with a single decision (text) column
ALTER TABLE `claude_permission_requests` ADD COLUMN `decision` TEXT;

-- Migrate existing data: approved=true -> allowSession, approved=false -> denySession
UPDATE `claude_permission_requests`
SET `decision` = CASE
    WHEN `approved` = 1 THEN '"allowSession"'
    WHEN `approved` = 0 THEN '"denySession"'
    ELSE NULL
END;

-- Drop the old columns
ALTER TABLE `claude_permission_requests` DROP COLUMN `approved`;
