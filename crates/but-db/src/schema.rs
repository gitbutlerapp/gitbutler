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
        handler_prompt -> Nullable<Text>,
        snapshot_before -> Text,
        snapshot_after -> Text,
        response -> Nullable<Text>,
        error -> Nullable<Text>,
    }
}

diesel::table! {
    ai_conversations (id) {
        id -> Text,
        name -> Text,
    }
}

diesel::table! {
    ai_messages (id) {
        id -> Text,
        conversation_id -> Text,
        role -> Text,
        content -> Text,
        tool_call_id -> Nullable<Text>,
        order -> Integer,
    }
}

diesel::joinable!(ai_messages -> ai_conversations (conversation_id));

diesel::allow_tables_to_appear_in_same_query!(ai_conversations, ai_messages,);
