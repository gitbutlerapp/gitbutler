diesel::table! {
    hunk_assignments (path, hunk_header) {
        id -> Nullable<Text>,
        hunk_header -> Nullable<Text>,
        path -> Text,
        path_bytes -> Binary,
        stack_id -> Nullable<Text>,
    }
}

diesel::table! {
    butler_actions (id) {
        id -> Text,
        created_at -> Timestamp,
        external_prompt -> Nullable<Text>,
        external_summary -> Text,
        handler -> Text,
        snapshot_before -> Text,
        snapshot_after -> Text,
        response -> Nullable<Text>,
        error -> Nullable<Text>,
        source -> Nullable<Text>,
    }
}

diesel::table! {
    workflows (id) {
        id -> Text,
        created_at -> Timestamp,
        kind -> Text,
        triggered_by -> Text,
        status -> Text,
        input_commits -> Text,
        output_commits -> Text,
        summary -> Nullable<Text>,
    }
}
