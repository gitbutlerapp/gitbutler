-- Add inGui column to claude_sessions table
ALTER TABLE claude_sessions ADD COLUMN in_gui BOOLEAN NOT NULL DEFAULT FALSE;
