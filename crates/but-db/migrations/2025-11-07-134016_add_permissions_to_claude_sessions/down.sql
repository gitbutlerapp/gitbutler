-- Revert permissions columns from claude_sessions table
ALTER TABLE claude_sessions DROP COLUMN approved_permissions;
ALTER TABLE claude_sessions DROP COLUMN denied_permissions;
