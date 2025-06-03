-- Your SQL goes here

-- Rename `butler_actions` to `butler_mcp_actions`
ALTER TABLE `butler_actions` RENAME TO `butler_mcp_actions`;

-- Drop old index that is not going to be needed
DROP INDEX IF EXISTS idx_butler_actions_created_at;

-- Create table `butler_revert_actions` which has a `snapshot` property which is a string
CREATE TABLE `butler_revert_actions` (
    id TEXT PRIMARY KEY,
    snapshot TEXT NOT NULL,
    description TEXT NOT NULL
);

-- Create new `butler_actions` table which has two optional FKs to `butler_mcp_actions` and `butler_revert_actions`, an auto-incrementing ID, and a `created_at` timestamp which has an index
CREATE TABLE `butler_actions` (
    id TEXT PRIMARY KEY,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    mcp_action_id TEXT,
    revert_action_id TEXT,
    FOREIGN KEY (mcp_action_id) REFERENCES butler_mcp_actions(id),
    FOREIGN KEY (revert_action_id) REFERENCES butler_revert_actions(id)
);

CREATE INDEX idx_butler_actions_new_created_at ON butler_actions(created_at);

-- For each existing butler_mcp_actions row, create a butler_actions row with the same id and created_at, mcp_action_id set to id, revert_action_id set to NULL
INSERT INTO butler_actions (id, created_at, mcp_action_id, revert_action_id)
SELECT id, created_at, id, NULL FROM butler_mcp_actions;

-- Remove the created_at column from butler_mcp_actions
ALTER TABLE `butler_mcp_actions` DROP COLUMN created_at;