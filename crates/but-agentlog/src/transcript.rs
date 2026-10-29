use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::agent::Agent;

#[derive(Debug)]
pub(crate) struct TranscriptBatch {
    pub(crate) session_id: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) tool_version: Option<String>,
    pub(crate) thread_source: Option<String>,
    pub(crate) records: Vec<ParsedRecord>,
}

impl TranscriptBatch {
    pub(crate) fn parse(agent: Agent, snapshot: &[u8]) -> Result<Self> {
        let mut raw_records = snapshot
            .split(|byte| *byte == b'\n')
            .map(|line| line.trim_ascii_end())
            .filter(|line| !line.iter().all(|byte| byte.is_ascii_whitespace()))
            .enumerate()
            .peekable();
        let mut transcript = TranscriptBatch {
            session_id: None,
            provider: match agent {
                Agent::Codex => None,
                Agent::Claude => Some("anthropic".to_string()),
            },
            model: None,
            tool_version: None,
            thread_source: None,
            records: Vec::new(),
        };
        let mut codex_tool_names = HashMap::new();
        let mut codex_spawn_prompts = HashSet::new();
        let mut claude_tool_names = HashMap::new();

        while let Some((index, trimmed)) = raw_records.next() {
            let parsed = match serde_json::from_slice::<Value>(trimmed) {
                Ok(parsed) => parsed,
                Err(_) if raw_records.peek().is_none() => continue,
                Err(_) => bail!("transcript contains malformed JSON before the final record"),
            };

            let record = match agent {
                Agent::Codex => {
                    transcript.apply_codex_metadata(&parsed);
                    ParsedRecord::from_codex_source(
                        index,
                        trimmed,
                        parsed,
                        &mut codex_tool_names,
                        &mut codex_spawn_prompts,
                        transcript.thread_source.as_deref(),
                    )
                }
                Agent::Claude => {
                    transcript.apply_claude_metadata(&parsed);
                    ParsedRecord::from_claude_source(index, trimmed, parsed, &mut claude_tool_names)
                }
            };
            if let Some(record) = record {
                transcript.records.push(record);
            }
        }

        Ok(transcript)
    }

    fn apply_codex_metadata(&mut self, source_record: &Value) {
        if str_at(source_record, &["type"]) == Some("session_meta") {
            if self.session_id.is_none() {
                self.session_id = str_at(source_record, &["payload", "id"])
                    .or_else(|| str_at(source_record, &["payload", "session_id"]))
                    .map(ToOwned::to_owned);
            }
            if self.provider.is_none() {
                self.provider =
                    str_at(source_record, &["payload", "model_provider"]).map(ToOwned::to_owned);
            }
            if self.tool_version.is_none() {
                self.tool_version =
                    str_at(source_record, &["payload", "cli_version"]).map(ToOwned::to_owned);
            }
            if self.thread_source.is_none() {
                self.thread_source = str_at(source_record, &["payload", "thread_source"])
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        value_at(source_record, &["payload", "source", "subagent"])
                            .is_some()
                            .then(|| "subagent".to_owned())
                    });
            }
        }

        if self.model.is_none() {
            self.model = str_at(source_record, &["payload", "model"])
                .or_else(|| str_at(source_record, &["payload", "model_slug"]))
                .map(ToOwned::to_owned);
        }
    }

    fn apply_claude_metadata(&mut self, source_record: &Value) {
        if self.session_id.is_none() {
            self.session_id = str_at(source_record, &["sessionId"]).map(ToOwned::to_owned);
        }
        if self.tool_version.is_none() {
            self.tool_version = str_at(source_record, &["version"]).map(ToOwned::to_owned);
        }
        if self.model.is_none() {
            self.model = str_at(source_record, &["message", "model"])
                .or_else(|| str_at(source_record, &["model"]))
                .map(ToOwned::to_owned);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordKind {
    Message,
    ToolCall,
    ToolResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PromptSource {
    Human,
    SpawnedAgent,
    SystemInjected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolKind {
    Exec,
    FileEdit,
    SubAgent,
    Housekeeping,
    WebSearch,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolOutcome {
    Succeeded,
    Failed,
    CouldNotExecute,
}

#[derive(Debug)]
pub(crate) struct ParsedRecord {
    pub(crate) index: usize,
    pub(crate) source_record_hash: String,
    pub(crate) source_timestamp: Option<String>,
    pub(crate) kind: RecordKind,
    pub(crate) source_event_kind: String,
    pub(crate) role: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) prompt_source: Option<PromptSource>,
    pub(crate) tool_name: Option<String>,
    pub(crate) tool_kind: Option<ToolKind>,
    pub(crate) tool_input: Option<Value>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) tool_outcome: Option<ToolOutcome>,
    pub(crate) source_record: Value,
}

impl ParsedRecord {
    fn from_codex_source(
        index: usize,
        trimmed: &[u8],
        mut source_record: Value,
        tool_names: &mut HashMap<String, String>,
        spawn_prompts: &mut HashSet<String>,
        thread_source: Option<&str>,
    ) -> Option<Self> {
        let raw_event_kind = codex_event_kind(&source_record);
        let kind = codex_kind(&raw_event_kind)?;
        let role = str_at(&source_record, &["payload", "role"])
            .or_else(|| str_at(&source_record, &["payload", "item", "role"]))
            .map(ToOwned::to_owned);
        if kind == RecordKind::Message && matches!(role.as_deref(), Some("developer" | "system")) {
            return None;
        }
        let text = codex_text(&source_record, kind);
        let mut tool_name = [
            &["payload", "tool_name"][..],
            &["payload", "tool"],
            &["payload", "name"],
            &["payload", "item", "name"],
        ]
        .iter()
        .find_map(|path| str_at(&source_record, path).map(ToOwned::to_owned));
        let tool_input = codex_tool_input(&source_record, kind);
        if kind == RecordKind::ToolResult && tool_name.is_none() {
            tool_name = codex_call_id(&source_record)
                .and_then(|call_id| tool_names.get(call_id))
                .cloned();
        }
        if kind == RecordKind::ToolCall
            && let (Some(call_id), Some(name)) = (codex_call_id(&source_record), tool_name.as_ref())
        {
            tool_names.insert(call_id.to_owned(), name.clone());
        }
        if matches!(tool_name.as_deref(), Some("spawn_agent"))
            && let Some(message) = tool_input.as_ref().and_then(spawn_prompt)
        {
            spawn_prompts.insert(normalized_prompt(message).to_owned());
        }
        let prompt_source = prompt_source(
            role.as_deref(),
            text.as_deref(),
            thread_source == Some("subagent"),
            spawn_prompts,
        );
        let tool_kind = tool_name.as_deref().map(classify_tool);
        let exit_code = (kind == RecordKind::ToolResult)
            .then(|| text.as_deref().and_then(parse_exit_code))
            .flatten();
        let tool_outcome = exit_code.map(|code| classify_outcome(code, text.as_deref()));
        prune_codex(&mut source_record, kind, tool_input.is_some());

        Some(ParsedRecord {
            index,
            source_record_hash: sha256_prefixed(trimmed),
            source_timestamp: str_at(&source_record, &["timestamp"]).map(ToOwned::to_owned),
            source_event_kind: format!("codex:{raw_event_kind}"),
            kind,
            role,
            text,
            prompt_source,
            tool_name,
            tool_kind,
            tool_input,
            exit_code,
            tool_outcome,
            source_record,
        })
    }

    fn from_claude_source(
        index: usize,
        trimmed: &[u8],
        mut source_record: Value,
        tool_names: &mut HashMap<String, String>,
    ) -> Option<Self> {
        let content_block = claude_content_block(&source_record);
        let content_type = content_block.and_then(|block| str_at(block, &["type"]));
        let source_event_kind = claude_event_kind(&source_record, content_type);
        let mut role = str_at(&source_record, &["message", "role"])
            .or_else(|| {
                let record_type = str_at(&source_record, &["type"])?;
                matches!(record_type, "user" | "assistant").then_some(record_type)
            })
            .map(ToOwned::to_owned);
        let mut text = claude_content_text(&source_record);
        let kind = claude_kind(content_type, text.is_some())?;
        let mut tool_name = None;
        let mut tool_input = None;

        match (kind, content_block) {
            (RecordKind::ToolCall, Some(block)) => {
                tool_name = str_at(block, &["name"]).map(ToOwned::to_owned);
                tool_input = block.get("input").cloned();
                if let (Some(id), Some(name)) = (str_at(block, &["id"]), tool_name.as_deref()) {
                    tool_names.insert(id.to_owned(), name.to_owned());
                }
                text = None;
            }
            (RecordKind::ToolResult, Some(block)) => {
                role = None;
                text = claude_block_text(block);
                tool_name = str_at(block, &["tool_use_id"])
                    .and_then(|id| tool_names.get(id))
                    .cloned();
            }
            _ => {}
        }
        prune_claude(&mut source_record, kind, tool_input.is_some());
        let mut spawn_prompts = HashSet::new();
        let prompt_source =
            prompt_source(role.as_deref(), text.as_deref(), false, &mut spawn_prompts);
        let tool_kind = tool_name.as_deref().map(classify_tool);
        let exit_code = (kind == RecordKind::ToolResult)
            .then(|| text.as_deref().and_then(parse_exit_code))
            .flatten();
        let tool_outcome = exit_code.map(|code| classify_outcome(code, text.as_deref()));

        Some(ParsedRecord {
            index,
            source_record_hash: sha256_prefixed(trimmed),
            source_timestamp: str_at(&source_record, &["timestamp"]).map(ToOwned::to_owned),
            source_event_kind,
            kind,
            role,
            text,
            prompt_source,
            tool_name,
            tool_kind,
            tool_input,
            exit_code,
            tool_outcome,
            source_record,
        })
    }
}

fn codex_kind(source_event_kind: &str) -> Option<RecordKind> {
    match source_event_kind {
        "response_item:message" => Some(RecordKind::Message),
        "response_item:function_call"
        | "response_item:custom_tool_call"
        | "response_item:web_search_call" => Some(RecordKind::ToolCall),
        "response_item:function_call_output" | "response_item:custom_tool_call_output" => {
            Some(RecordKind::ToolResult)
        }
        _ => None,
    }
}

fn codex_text(source_record: &Value, kind: RecordKind) -> Option<String> {
    match kind {
        RecordKind::Message => first_text_at(
            source_record,
            &[
                &["payload", "content"],
                &["payload", "text"],
                &["payload", "message"],
                &["payload", "item", "content"],
                &["payload", "item", "text"],
            ],
        ),
        RecordKind::ToolResult => first_text_at(
            source_record,
            &[
                &["payload", "output"],
                &["payload", "content"],
                &["payload", "item", "output"],
                &["payload", "item", "content"],
            ],
        ),
        RecordKind::ToolCall => None,
    }
}

fn codex_tool_input(source_record: &Value, kind: RecordKind) -> Option<Value> {
    if kind != RecordKind::ToolCall {
        return None;
    }

    [
        &["payload", "arguments"][..],
        &["payload", "input"],
        &["payload", "item", "arguments"],
        &["payload", "item", "input"],
    ]
    .iter()
    .find_map(|path| value_at(source_record, path))
    .map(json_value)
}

fn codex_call_id(source_record: &Value) -> Option<&str> {
    [
        &["payload", "call_id"][..],
        &["payload", "item", "call_id"],
        &["payload", "id"],
        &["payload", "item", "id"],
    ]
    .iter()
    .find_map(|path| str_at(source_record, path))
}

fn spawn_prompt(input: &Value) -> Option<&str> {
    ["message", "prompt", "task"]
        .into_iter()
        .find_map(|key| input.get(key).and_then(Value::as_str))
}

fn prompt_source(
    role: Option<&str>,
    text: Option<&str>,
    subagent_session: bool,
    spawn_prompts: &mut HashSet<String>,
) -> Option<PromptSource> {
    if role != Some("user") {
        return None;
    }
    let Some(text) = text else {
        return Some(PromptSource::Human);
    };
    if is_system_injected_prompt(text) {
        return Some(PromptSource::SystemInjected);
    }
    if subagent_session || spawn_prompts.remove(normalized_prompt(text)) {
        return Some(PromptSource::SpawnedAgent);
    }
    Some(PromptSource::Human)
}

fn normalized_prompt(text: &str) -> &str {
    text.trim()
}

fn is_system_injected_prompt(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("# AGENTS.md instructions")
        || trimmed.starts_with("<INSTRUCTIONS>")
        || trimmed.starts_with("<environment_context>")
        || trimmed.starts_with("<subagent_notification>")
        || (trimmed.contains("<INSTRUCTIONS>")
            && trimmed.contains("</INSTRUCTIONS>")
            && trimmed.contains("<environment_context>"))
}

fn classify_tool(tool_name: &str) -> ToolKind {
    match tool_name {
        "exec_command" | "Bash" | "bash" | "shell" | "local_shell" | "run_command" => {
            ToolKind::Exec
        }
        "apply_patch" | "edit_file" | "write_file" | "str_replace" | "Edit" | "MultiEdit"
        | "Write" => ToolKind::FileEdit,
        "spawn_agent" | "Task" => ToolKind::SubAgent,
        "write_stdin" | "wait_agent" | "close_agent" | "kill_agent" | "update_plan" | "Read"
        | "Glob" | "Grep" | "LS" | "TodoWrite" => ToolKind::Housekeeping,
        "web_search" | "web_search_call" => ToolKind::WebSearch,
        _ => ToolKind::Other,
    }
}

fn parse_exit_code(text: &str) -> Option<i32> {
    let lower = text.to_ascii_lowercase();
    for marker in ["exited with code ", "exit code ", "exit status "] {
        if let Some(index) = lower.find(marker) {
            let rest = &text[index + marker.len()..];
            let digits = rest
                .trim_start()
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
                .collect::<String>();
            if let Ok(code) = digits.parse() {
                return Some(code);
            }
        }
    }
    None
}

fn classify_outcome(exit_code: i32, text: Option<&str>) -> ToolOutcome {
    match exit_code {
        0 => ToolOutcome::Succeeded,
        126 | 127 => ToolOutcome::CouldNotExecute,
        101 if text.is_some_and(looks_like_cargo_invocation_error) => ToolOutcome::CouldNotExecute,
        _ => ToolOutcome::Failed,
    }
}

fn looks_like_cargo_invocation_error(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "is ambiguous",
        "package id specification",
        "did not match any packages",
        "could not find `cargo.toml`",
        "could not find cargo.toml",
        "no such command",
        "no bin target",
        "could not determine which binary to run",
    ]
    .into_iter()
    .any(|needle| lower.contains(needle))
}

fn prune_codex(source_record: &mut Value, kind: RecordKind, has_tool_input: bool) {
    match kind {
        RecordKind::Message => {
            for path in [
                &["payload", "content"][..],
                &["payload", "text"],
                &["payload", "message"],
                &["payload", "item", "content"],
                &["payload", "item", "text"],
            ] {
                remove_field_at(source_record, path);
            }
        }
        RecordKind::ToolCall if has_tool_input => {
            for path in [
                &["payload", "arguments"][..],
                &["payload", "input"],
                &["payload", "item", "arguments"],
                &["payload", "item", "input"],
            ] {
                remove_field_at(source_record, path);
            }
        }
        RecordKind::ToolResult => {
            for path in [
                &["payload", "output"][..],
                &["payload", "content"],
                &["payload", "item", "output"],
                &["payload", "item", "content"],
            ] {
                remove_field_at(source_record, path);
            }
        }
        RecordKind::ToolCall => {}
    }
}

fn first_text_at(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| text_at(value, path))
}

fn text_at(value: &Value, path: &[&str]) -> Option<String> {
    match value_at(value, path)? {
        Value::String(text) => Some(text.clone()),
        Value::Array(blocks) => joined_block_text(blocks),
        _ => None,
    }
}

fn joined_block_text(blocks: &[Value]) -> Option<String> {
    let text = blocks
        .iter()
        .filter_map(|block| str_at(block, &["text"]))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn json_value(value: &Value) -> Value {
    match value.as_str() {
        Some(text) => serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned())),
        None => value.clone(),
    }
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for component in path {
        current = current.get(*component)?;
    }
    Some(current)
}

fn remove_field_at(value: &mut Value, path: &[&str]) {
    let Some((field, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for component in parents {
        let Some(next) = current.get_mut(*component) else {
            return;
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.remove(*field);
    }
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    value_at(value, path)?.as_str()
}

fn codex_event_kind(source_record: &Value) -> String {
    let top_level_type = str_at(source_record, &["type"]).unwrap_or("unknown");
    match str_at(source_record, &["payload", "type"]) {
        Some(nested_type) => format!("{top_level_type}:{nested_type}"),
        None => top_level_type.to_owned(),
    }
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn claude_event_kind(source_record: &Value, content_type: Option<&str>) -> String {
    let top_level_type = str_at(source_record, &["type"]).unwrap_or("unknown");
    match content_type.or_else(|| str_at(source_record, &["message", "type"])) {
        Some(nested_type) => format!("claude:{top_level_type}:{nested_type}"),
        None if source_record.get("message").is_some() => {
            format!("claude:{top_level_type}:message")
        }
        None => format!("claude:{top_level_type}"),
    }
}

fn claude_kind(content_type: Option<&str>, has_text: bool) -> Option<RecordKind> {
    match content_type {
        Some("text") => Some(RecordKind::Message),
        Some("tool_use") => Some(RecordKind::ToolCall),
        Some("tool_result") => Some(RecordKind::ToolResult),
        _ if has_text => Some(RecordKind::Message),
        _ => None,
    }
}

fn claude_content_block(source_record: &Value) -> Option<&Value> {
    source_record
        .get("message")?
        .get("content")?
        .as_array()?
        .iter()
        .find(|block| str_at(block, &["type"]).is_some())
}

fn claude_content_text(source_record: &Value) -> Option<String> {
    if let Some(content) = str_at(source_record, &["message", "content"]) {
        return Some(content.to_owned());
    }

    joined_block_text(source_record.get("message")?.get("content")?.as_array()?)
}

fn claude_block_text(block: &Value) -> Option<String> {
    match block.get("content") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(blocks)) => joined_block_text(blocks),
        _ => str_at(block, &["text"]).map(ToOwned::to_owned),
    }
}

fn prune_claude(source_record: &mut Value, kind: RecordKind, has_tool_input: bool) {
    if kind == RecordKind::Message {
        remove_field_at(source_record, &["message", "content"]);
        return;
    }

    let Some(blocks) = source_record
        .get_mut("message")
        .and_then(|message| message.get_mut("content"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let Some(block) = blocks
        .iter_mut()
        .find(|block| str_at(block, &["type"]).is_some())
    else {
        return;
    };
    match kind {
        RecordKind::ToolCall if has_tool_input => {
            if let Some(object) = block.as_object_mut() {
                object.remove("input");
            }
        }
        RecordKind::ToolResult => {
            if let Some(object) = block.as_object_mut() {
                object.remove("content");
            }
        }
        RecordKind::Message | RecordKind::ToolCall => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_records_and_extracts_metadata() {
        let data = br#"
{"timestamp":"2026-05-07T09:00:00Z","type":"session_meta","payload":{"id":"session-1","model_provider":"openai","cli_version":"0.1.0"}}
{"timestamp":"2026-05-07T09:00:01Z","type":"turn_context","payload":{"model":"gpt-5.5"}}
{"timestamp":"2026-05-07T09:00:02Z","type":"response_item","payload":{"type":"message","turn_id":"turn-1","role":"user","content":[{"type":"input_text","text":"Implemented change"}]}}
{"timestamp":"2026-05-07T09:00:03Z","type":"event_msg","payload":{"type":"agent_message","message":"Implemented change"}}
{"timestamp":"2026-05-07T09:00:04Z","type":"event_msg","payload":{"type":"user_message","message":"Please do it"}}
{"timestamp":"2026-05-07T09:00:05Z","type":"event_msg","payload":{"type":"info"}}
{"timestamp":"2026-05-07T09:00:06Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"cmd\":\"echo ok\"}"}}
{"timestamp":"2026-05-07T09:00:07Z","type":"response_item","payload":{"type":"message","role":"developer","content":"hidden context"}}
"#;

        let transcript = TranscriptBatch::parse(Agent::Codex, data).expect("parse transcript");

        assert_eq!(transcript.session_id.as_deref(), Some("session-1"));
        assert_eq!(transcript.provider.as_deref(), Some("openai"));
        assert_eq!(transcript.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(transcript.tool_version.as_deref(), Some("0.1.0"));
        assert_eq!(transcript.records.len(), 2);
        assert_eq!(
            transcript.records[0].source_timestamp.as_deref(),
            Some("2026-05-07T09:00:02Z")
        );
        assert_eq!(transcript.records[0].index, 2);
        assert_eq!(transcript.records[0].role.as_deref(), Some("user"));
        assert_eq!(transcript.records[0].kind, RecordKind::Message);
        assert_eq!(
            transcript.records[0].prompt_source,
            Some(PromptSource::Human)
        );
        assert_eq!(
            transcript.records[0].text.as_deref(),
            Some("Implemented change")
        );
        assert_eq!(transcript.records[1].kind, RecordKind::ToolCall);
        assert_eq!(transcript.records[1].tool_name.as_deref(), Some("shell"));
        assert_eq!(transcript.records[1].tool_kind, Some(ToolKind::Exec));
        assert_eq!(
            transcript.records[1]
                .tool_input
                .as_ref()
                .expect("tool input")["cmd"],
            "echo ok"
        );
    }

    #[test]
    fn parses_claude_records_and_extracts_metadata() {
        let data = br#"
{"type":"user","sessionId":"claude-session-1","uuid":"message-1","parentUuid":null,"timestamp":"2026-05-07T09:00:00Z","version":"2.1.111","message":{"role":"user","content":"hello"}}
{"type":"assistant","sessionId":"claude-session-1","uuid":"message-2","parentUuid":"message-1","timestamp":"2026-05-07T09:00:01Z","version":"2.1.111","message":{"id":"msg_api_1","type":"message","role":"assistant","model":"claude-opus-4-5","content":[{"type":"text","text":"Done"}]}}
{"type":"assistant","sessionId":"claude-session-1","uuid":"message-3","parentUuid":"message-2","timestamp":"2026-05-07T09:00:02Z","version":"2.1.111","message":{"type":"message","role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"echo ok"}}]}}
{"type":"user","sessionId":"claude-session-1","uuid":"message-4","parentUuid":"message-3","timestamp":"2026-05-07T09:00:03Z","version":"2.1.111","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"ok"}]}}
"#;

        let transcript = TranscriptBatch::parse(Agent::Claude, data).expect("parse transcript");

        assert_eq!(transcript.session_id.as_deref(), Some("claude-session-1"));
        assert_eq!(transcript.provider.as_deref(), Some("anthropic"));
        assert_eq!(transcript.model.as_deref(), Some("claude-opus-4-5"));
        assert_eq!(transcript.tool_version.as_deref(), Some("2.1.111"));
        assert_eq!(transcript.records.len(), 4);
        assert_eq!(transcript.records[0].role.as_deref(), Some("user"));
        assert_eq!(transcript.records[0].kind, RecordKind::Message);
        assert_eq!(
            transcript.records[0].prompt_source,
            Some(PromptSource::Human)
        );
        assert_eq!(transcript.records[0].text.as_deref(), Some("hello"));
        assert_eq!(transcript.records[1].role.as_deref(), Some("assistant"));
        assert_eq!(transcript.records[1].text.as_deref(), Some("Done"));
        assert_eq!(
            transcript.records[1].source_event_kind,
            "claude:assistant:text"
        );
        assert_eq!(transcript.records[2].kind, RecordKind::ToolCall);
        assert_eq!(transcript.records[2].tool_name.as_deref(), Some("Bash"));
        assert_eq!(transcript.records[2].tool_kind, Some(ToolKind::Exec));
        assert_eq!(
            transcript.records[2]
                .tool_input
                .as_ref()
                .expect("tool input")["command"],
            "echo ok"
        );
        assert_eq!(transcript.records[3].kind, RecordKind::ToolResult);
        assert_eq!(transcript.records[3].role.as_deref(), None);
        assert_eq!(transcript.records[3].tool_name.as_deref(), Some("Bash"));
        assert_eq!(transcript.records[3].tool_kind, Some(ToolKind::Exec));
        assert_eq!(transcript.records[3].text.as_deref(), Some("ok"));
    }

    #[test]
    fn tags_codex_prompt_sources() {
        let data = br##"
{"timestamp":"2026-05-07T09:00:00Z","type":"session_meta","payload":{"id":"session-1","thread_source":"subagent"}}
{"timestamp":"2026-05-07T09:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions\n\n<INSTRUCTIONS>rules</INSTRUCTIONS>\n<environment_context>ctx</environment_context>"}]}}
{"timestamp":"2026-05-07T09:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":"<INSTRUCTIONS>rules</INSTRUCTIONS>"}}
{"timestamp":"2026-05-07T09:00:03Z","type":"response_item","payload":{"type":"message","role":"user","content":"<environment_context>ctx</environment_context>"}}
{"timestamp":"2026-05-07T09:00:04Z","type":"response_item","payload":{"type":"message","role":"user","content":"Repo: /tmp/project. Review the code."}}
"##;

        let transcript = TranscriptBatch::parse(Agent::Codex, data).expect("parse transcript");

        assert_eq!(transcript.thread_source.as_deref(), Some("subagent"));
        assert_eq!(transcript.records.len(), 4);
        assert_eq!(
            transcript.records[0].prompt_source,
            Some(PromptSource::SystemInjected)
        );
        assert_eq!(
            transcript.records[1].prompt_source,
            Some(PromptSource::SystemInjected)
        );
        assert_eq!(
            transcript.records[2].prompt_source,
            Some(PromptSource::SystemInjected)
        );
        assert_eq!(
            transcript.records[3].prompt_source,
            Some(PromptSource::SpawnedAgent)
        );
    }

    #[test]
    fn tags_user_prompt_matching_spawn_agent_message() {
        let data = br#"
{"timestamp":"2026-05-07T09:00:00Z","type":"session_meta","payload":{"id":"session-1"}}
{"timestamp":"2026-05-07T09:00:01Z","type":"response_item","payload":{"type":"function_call","name":"spawn_agent","call_id":"call-1","arguments":"{\"message\":\"Review tests only\"}"}}
{"timestamp":"2026-05-07T09:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":"Review tests only"}}
{"timestamp":"2026-05-07T09:00:03Z","type":"response_item","payload":{"type":"message","role":"user","content":"A real follow-up"}}
"#;

        let transcript = TranscriptBatch::parse(Agent::Codex, data).expect("parse transcript");

        assert_eq!(transcript.records.len(), 3);
        assert_eq!(transcript.records[0].tool_kind, Some(ToolKind::SubAgent));
        assert_eq!(
            transcript.records[1].prompt_source,
            Some(PromptSource::SpawnedAgent)
        );
        assert_eq!(
            transcript.records[2].prompt_source,
            Some(PromptSource::Human)
        );
    }

    #[test]
    fn parses_codex_tool_kind_exit_code_and_outcome() {
        let data = br#"
{"timestamp":"2026-05-07T09:00:00Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"call-1","arguments":"{\"cmd\":\"cargo test\"}"}}
{"timestamp":"2026-05-07T09:00:01Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"error[E0599]\nProcess exited with code 101"}}
{"timestamp":"2026-05-07T09:00:02Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"call-2","arguments":"{\"cmd\":\"cargo fmt --check\"}"}}
{"timestamp":"2026-05-07T09:00:03Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-2","output":"error: `git-meta-lib` is ambiguous\nProcess exited with code 101"}}
{"timestamp":"2026-05-07T09:00:04Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"call-3","arguments":"{\"cmd\":\"missing-tool\"}"}}
{"timestamp":"2026-05-07T09:00:05Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-3","output":"Process exited with code 127"}}
"#;

        let transcript = TranscriptBatch::parse(Agent::Codex, data).expect("parse transcript");

        assert_eq!(
            transcript.records[1].tool_name.as_deref(),
            Some("exec_command")
        );
        assert_eq!(transcript.records[1].tool_kind, Some(ToolKind::Exec));
        assert_eq!(transcript.records[1].exit_code, Some(101));
        assert_eq!(
            transcript.records[1].tool_outcome,
            Some(ToolOutcome::Failed)
        );
        assert_eq!(transcript.records[3].exit_code, Some(101));
        assert_eq!(
            transcript.records[3].tool_outcome,
            Some(ToolOutcome::CouldNotExecute)
        );
        assert_eq!(transcript.records[5].exit_code, Some(127));
        assert_eq!(
            transcript.records[5].tool_outcome,
            Some(ToolOutcome::CouldNotExecute)
        );
    }

    #[test]
    fn skips_malformed_final_record() {
        let data = br#"{"type":"response_item","payload":{"type":"message","content":"ok"}}
{"type":"response_item","payload":}
"#;

        let transcript =
            TranscriptBatch::parse(Agent::Codex, data).expect("parse partial transcript");

        assert_eq!(transcript.records.len(), 1);
    }

    #[test]
    fn fails_malformed_middle_record() {
        let data = br#"{"type":"session_meta","payload":{"id":"session-1"}}
{"type":"response_item","payload":}
{"type":"event_msg","payload":{}}
"#;

        TranscriptBatch::parse(Agent::Codex, data).expect_err("malformed middle line fails");
    }

    #[test]
    fn empty_lines_do_not_count_as_record_indexes() {
        let data = br#"

{"type":"response_item","payload":{"type":"message","content":"first"}}

{"type":"response_item","payload":{"type":"message","content":"second"}}
"#;

        let transcript = TranscriptBatch::parse(Agent::Codex, data).expect("parse transcript");

        assert_eq!(transcript.records[0].index, 0);
        assert_eq!(transcript.records[1].index, 1);
    }
}
