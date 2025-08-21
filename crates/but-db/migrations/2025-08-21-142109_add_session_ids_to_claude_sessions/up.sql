-- Add session_ids column to claude_sessions table to track all session IDs used
ALTER TABLE claude_sessions ADD COLUMN session_ids TEXT NOT NULL DEFAULT '[]';

-- Initialize existing sessions with their current_id in the session_ids array
UPDATE claude_sessions SET session_ids = json_array(current_id);