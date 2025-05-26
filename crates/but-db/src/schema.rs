diesel::table! {
    hunk_assignments (path, hunk_header) {
        hunk_header -> Nullable<Text>,
        path -> Text,
        path_bytes -> Binary,
        stack_id -> Nullable<Text>,
        hunk_locks -> Text,
    }
}
