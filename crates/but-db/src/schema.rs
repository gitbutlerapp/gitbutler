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

diesel::table! {
    file_write_locks (path) {
        path -> Text,
        created_at -> Timestamp,
        owner -> Text,
    }
}

diesel::table! {
    workspace_rules (id) {
        id -> Text,
        created_at -> Timestamp,
        enabled -> Bool,
        trigger -> Text,
        filters -> Text,
        action -> Text,
    }
}

diesel::table! {
    claude_sessions (id) {
        id -> Text,
        current_id -> Text,
        session_ids -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        in_gui -> Bool,
    }
}

diesel::table! {
    claude_messages (id) {
        id -> Text,
        session_id -> Text,
        created_at -> Timestamp,
        content_type -> Text,
        content -> Text,
    }
}

diesel::table! {
    claude_permission_requests (id) {
        id -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        tool_name -> Text,
        input -> Text,
        approved -> Nullable<Bool>,
    }
}

diesel::table! {
    worktrees (path) {
        path -> Binary,
        reference -> Binary,
        base -> Text,
        source -> Text,
    }
}

diesel::table! {
    gerrit_metadata (change_id) {
        change_id -> Text,
        commit_id -> Text,
        review_url -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
