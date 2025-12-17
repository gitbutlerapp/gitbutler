-- Add use_wildcard column to claude_permission_requests table
ALTER TABLE `claude_permission_requests` ADD COLUMN `use_wildcard` BOOLEAN NOT NULL DEFAULT 0;
