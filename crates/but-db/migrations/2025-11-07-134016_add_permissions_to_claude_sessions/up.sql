-- Add permissions columns to claude_sessions table to track approved and denied permissions
ALTER TABLE claude_sessions ADD COLUMN approved_permissions TEXT NOT NULL DEFAULT '[]';
ALTER TABLE claude_sessions ADD COLUMN denied_permissions TEXT NOT NULL DEFAULT '[]';
