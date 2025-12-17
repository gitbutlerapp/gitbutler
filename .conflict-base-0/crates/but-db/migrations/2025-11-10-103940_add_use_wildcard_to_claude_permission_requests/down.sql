-- Revert: Remove use_wildcard column from claude_permission_requests table
ALTER TABLE `claude_permission_requests` DROP COLUMN `use_wildcard`;
