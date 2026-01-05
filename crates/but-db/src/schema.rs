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
        approved_permissions -> Text,
        denied_permissions -> Text,
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
        decision -> Nullable<Text>,
        use_wildcard -> Bool,
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

diesel::table! {
    forge_reviews (number) {
        html_url -> Text,
        number -> BigInt,
        title -> Text,
        body -> Nullable<Text>,
        author -> Nullable<Text>,
        labels -> Text,
        draft -> Bool,
        source_branch -> Text,
        target_branch -> Text,
        sha -> Text,
        created_at -> Nullable<Timestamp>,
        modified_at -> Nullable<Timestamp>,
        merged_at -> Nullable<Timestamp>,
        closed_at -> Nullable<Timestamp>,
        repository_ssh_url -> Nullable<Text>,
        repository_https_url -> Nullable<Text>,
        repo_owner -> Nullable<Text>,
        reviewers -> Text,
        unit_symbol -> Text,
        last_sync_at -> Timestamp,
        struct_version -> Integer,
    }
}

diesel::table! {
    ci_checks (id) {
        id -> BigInt,
        name -> Text,
        output_summary -> Text,
        output_text -> Text,
        output_title -> Text,
        started_at -> Nullable<Timestamp>,
        status_type -> Text,
        status_conclusion -> Nullable<Text>,
        status_completed_at -> Nullable<Timestamp>,
        head_sha -> Text,
        url -> Text,
        html_url -> Text,
        details_url -> Text,
        pull_requests -> Text,
        reference -> Text,
        last_sync_at -> Timestamp,
        struct_version -> Integer,
    }
}
