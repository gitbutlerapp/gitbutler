-- This file should undo anything in `up.sql`
-- Restore the old approved column
ALTER TABLE `claude_permission_requests` ADD COLUMN `approved` BOOL;

-- Migrate data back: allowOnce/allowSession/allowProject/allowAlways -> true, deny* -> false
UPDATE `claude_permission_requests`
SET `approved` = CASE
    WHEN `decision` LIKE '%allow%' THEN 1
    WHEN `decision` LIKE '%deny%' THEN 0
    ELSE NULL
END;

-- Drop the decision column
ALTER TABLE `claude_permission_requests` DROP COLUMN `decision`;
