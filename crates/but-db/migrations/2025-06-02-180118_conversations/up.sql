-- Your SQL goes here

CREATE TABLE ai_conversations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE ai_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tool_call_id TEXT,
    "order" INTEGER NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES ai_conversations(id)
);

