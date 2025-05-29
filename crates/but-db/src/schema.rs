diesel::table! {
    hunk_assignments (path, hunk_header) {
        hunk_header -> Nullable<Text>,
        path -> Text,
        path_bytes -> Binary,
        stack_id -> Nullable<Text>,
        hunk_locks -> Text,
    }
}

diesel::table! {
    butler_actions (id) {
        id -> Text,
        created_at -> Timestamp,
        external_prompt -> Text,
        handler -> Text,
        handler_prompt -> Nullable<Text>,
        snapshot_before -> Text,
        snapshot_after -> Text,
        response -> Nullable<Text>,
        error -> Nullable<Text>,
    }
}
