diesel::table! {
    hunk_assignments (path, hunk_header) {
        hunk_header -> Nullable<Text>,
        path -> Text,
        path_bytes -> Binary,
        stack_id -> Nullable<Text>,
        hunk_locks -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    butler_actions,
    butler_revert_actions,
    butler_mcp_actions
);

diesel::joinable!(butler_actions -> butler_revert_actions (revert_action_id));
diesel::joinable!(butler_actions -> butler_mcp_actions (mcp_action_id));

diesel::table! {
    butler_actions (id) {
        id -> Text,
        created_at -> Timestamp,
        mcp_action_id -> Nullable<Text>,
        revert_action_id -> Nullable<Text>,
    }
}

diesel::table! {
    butler_revert_actions (id) {
        id -> Text,
        snapshot -> Text,
        description -> Text
    }
}

diesel::table! {
    butler_mcp_actions (id) {
        id -> Text,
        external_prompt -> Nullable<Text>,
        external_summary -> Text,
        handler -> Text,
        handler_prompt -> Nullable<Text>,
        snapshot_before -> Text,
        snapshot_after -> Text,
        response -> Nullable<Text>,
        error -> Nullable<Text>,
    }
}
