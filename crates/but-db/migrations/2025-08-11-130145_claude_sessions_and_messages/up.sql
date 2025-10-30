-- Your SQL goes here
CREATE TABLE `claude_sessions`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`current_id` TEXT NOT NULL UNIQUE,
	`created_at` TIMESTAMP NOT NULL,
	`updated_at` TIMESTAMP NOT NULL
);

CREATE INDEX index_claude_sessions_on_current_id ON claude_sessions (current_id);

CREATE TABLE `claude_messages`(
	`id` TEXT NOT NULL PRIMARY KEY,
	`session_id` TEXT NOT NULL REFERENCES claude_sessions(id),
	`created_at` TIMESTAMP NOT NULL,
	`content_type` TEXT NOT NULL,
	`content` TEXT NOT NULL
);

CREATE INDEX index_claude_messages_on_session_id ON claude_messages (session_id);
CREATE INDEX index_claude_messages_on_created_at ON claude_messages (created_at);
