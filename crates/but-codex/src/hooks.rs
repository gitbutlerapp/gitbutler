use std::{
    collections::BTreeSet,
    io::BufRead,
    path::{Path, PathBuf},
};

use anyhow::Result;
use but_action::Source;
use but_claude::hooks::{
    AgentSessionStopRequest, EmptyStructuredPatchAssignment, StructuredPatch,
    assign_agent_hunks_post_tool_call, handle_agent_session_stop, lock_agent_file_for_tool_call,
};
use but_ctx::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct CodexStopInput {
    #[serde(alias = "sessionId")]
    session_id: Option<String>,
    #[serde(alias = "turnId")]
    turn_id: Option<String>,
    cwd: Option<String>,
    #[serde(alias = "transcriptPath")]
    transcript_path: Option<String>,
    #[serde(alias = "lastAssistantMessage")]
    last_assistant_message: Option<String>,
}

impl CodexStopInput {
    fn session_id(&self) -> Result<Uuid> {
        codex_session_id_from_parts(self.session_id.as_deref(), self.turn_id.as_deref())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CodexToolUseInput {
    #[serde(alias = "sessionId")]
    session_id: Option<String>,
    #[serde(alias = "turnId")]
    turn_id: Option<String>,
    cwd: Option<String>,
    #[serde(default, alias = "toolName")]
    tool_name: String,
    #[serde(default, alias = "toolInput")]
    tool_input: Value,
    #[serde(default, alias = "toolResponse")]
    tool_response: Value,
}

impl CodexToolUseInput {
    fn session_id(&self) -> Result<Uuid> {
        codex_session_id_from_parts(self.session_id.as_deref(), self.turn_id.as_deref())
    }
}

/// Output classification for Codex hooks.
pub enum CodexHookOutput {
    /// Hook handled successfully.
    Success,
    /// Hook failed internally, but Codex should not be blocked by the process exit status.
    Error(anyhow::Error),
}

/// Handle a Codex pre-tool hook payload.
pub fn handle_pre_tool_call_input(input: Value) -> Result<CodexHookOutput> {
    let input: CodexToolUseInput = serde_json::from_value(input)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {e}"))?;
    match handle_pre_tool_call(input) {
        Ok(output) => Ok(output),
        Err(e) => Ok(CodexHookOutput::Error(e)),
    }
}

/// Handle a Codex post-tool hook payload.
pub fn handle_post_tool_call_input(input: Value) -> Result<CodexHookOutput> {
    let input: CodexToolUseInput = serde_json::from_value(input)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {e}"))?;
    match handle_post_tool_call(input) {
        Ok(output) => Ok(output),
        Err(e) => Ok(CodexHookOutput::Error(e)),
    }
}

/// Handle a Codex stop hook payload.
pub fn handle_stop(input: Value) -> Result<CodexHookOutput> {
    let input: CodexStopInput = serde_json::from_value(input)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {e}"))?;
    match handle_stop_input(input) {
        Ok(output) => Ok(output),
        Err(e) => Ok(CodexHookOutput::Error(e)),
    }
}

fn handle_pre_tool_call(input: CodexToolUseInput) -> Result<CodexHookOutput> {
    let paths = changed_paths_from_tool_input(&input.tool_name, &input.tool_input);
    if paths.is_empty() {
        return Ok(CodexHookOutput::Success);
    }

    let session_id = input.session_id()?;
    let cwd = codex_cwd(input.cwd.as_deref())?;
    let ctx = Context::discover(&cwd)?;
    let sync_ctx = ctx.to_sync();
    for path in paths {
        let absolute_path = absolute_tool_path(&cwd, &path);
        lock_agent_file_for_tool_call(
            sync_ctx.clone(),
            session_id,
            &absolute_path.to_string_lossy(),
        )?;
    }

    Ok(CodexHookOutput::Success)
}

fn handle_post_tool_call(input: CodexToolUseInput) -> Result<CodexHookOutput> {
    let paths = changed_paths_from_tool_input(&input.tool_name, &input.tool_input);
    if paths.is_empty() {
        return Ok(CodexHookOutput::Success);
    }

    let session_id = input.session_id()?;
    let cwd = codex_cwd(input.cwd.as_deref())?;
    let structured_patch = structured_patches_from_tool_payload(&input);
    for path in paths {
        let absolute_path = absolute_tool_path(&cwd, &path);
        let ctx = Context::discover(&cwd)?;
        assign_agent_hunks_post_tool_call(
            ctx,
            session_id,
            &absolute_path.to_string_lossy(),
            &structured_patch,
            true,
            EmptyStructuredPatchAssignment::Skip,
        )?;
    }

    Ok(CodexHookOutput::Success)
}

fn handle_stop_input(input: CodexStopInput) -> Result<CodexHookOutput> {
    let session_id = input.session_id()?;
    let cwd = codex_cwd(input.cwd.as_deref())?;
    let ctx = Context::discover(&cwd)?;
    let prompt = input
        .transcript_path
        .as_deref()
        .and_then(|path| {
            last_user_prompt_from_codex_transcript(Path::new(path))
                .ok()
                .flatten()
        })
        .unwrap_or_default();
    let summary = input
        .last_assistant_message
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| prompt.clone());
    let summary = if summary.trim().is_empty() {
        "Codex session changes".to_string()
    } else {
        summary
    };

    handle_agent_session_stop(
        ctx,
        AgentSessionStopRequest {
            session_id,
            summary,
            prompt,
            source: Source::Codex(session_id.to_string()),
            require_reword: true,
        },
    )?;

    Ok(CodexHookOutput::Success)
}

fn codex_session_id(session_id: &str) -> Uuid {
    if let Ok(uuid) = Uuid::parse_str(session_id) {
        return uuid;
    }

    let mut first = 0xcbf29ce484222325_u64;
    let mut second = 0x84222325cbf29ce4_u64;
    for byte in session_id.bytes() {
        first ^= u64::from(byte);
        first = first.wrapping_mul(0x100000001b3);
        second ^= u64::from(byte.rotate_left(1));
        second = second.wrapping_mul(0x100000001b3);
    }
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&first.to_be_bytes());
    bytes[8..].copy_from_slice(&second.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn codex_session_id_from_parts(session_id: Option<&str>, turn_id: Option<&str>) -> Result<Uuid> {
    let id = session_id
        .or(turn_id)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Codex hook input is missing session_id or turn_id"))?;
    Ok(codex_session_id(id))
}

fn changed_paths_from_tool_input(tool_name: &str, tool_input: &Value) -> Vec<String> {
    let tool_name = tool_name.to_ascii_lowercase();
    let mut paths = BTreeSet::new();
    for key in ["file_path", "filePath", "path"] {
        if let Some(path) = tool_input.get(key).and_then(Value::as_str)
            && !path.is_empty()
        {
            paths.insert(path.to_string());
        }
    }
    if !paths.is_empty() {
        return paths.into_iter().collect();
    }

    if !tool_name.contains("apply_patch") && tool_name != "bash" {
        return vec![];
    }
    let Some(command) = tool_command(tool_input) else {
        return vec![];
    };
    if !command.contains("*** Begin Patch") {
        return vec![];
    }

    for line in command.lines().map(str::trim) {
        for prefix in [
            "*** Add File: ",
            "*** Update File: ",
            "*** Delete File: ",
            "*** Move to: ",
        ] {
            if let Some(path) = line.strip_prefix(prefix).map(str::trim)
                && !path.is_empty()
            {
                paths.insert(path.to_string());
            }
        }
    }

    paths.into_iter().collect()
}

fn tool_command(tool_input: &Value) -> Option<&str> {
    tool_input
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| tool_input.get("cmd").and_then(Value::as_str))
        .or_else(|| tool_input.as_str())
}

fn structured_patches_from_tool_payload(input: &CodexToolUseInput) -> Vec<StructuredPatch> {
    structured_patches_from_value(&input.tool_response)
        .or_else(|| structured_patches_from_value(&input.tool_input))
        .unwrap_or_default()
}

fn structured_patches_from_value(value: &Value) -> Option<Vec<StructuredPatch>> {
    for key in ["structuredPatch", "structured_patch"] {
        if let Some(patches) = value.get(key)
            && let Ok(patches) = serde_json::from_value::<Vec<StructuredPatch>>(patches.clone())
        {
            return Some(patches);
        }
    }

    for key in ["toolResponse", "tool_response"] {
        if let Some(response) = value.get(key)
            && let Some(patches) = structured_patches_from_value(response)
        {
            return Some(patches);
        }
    }

    None
}

fn absolute_tool_path(cwd: &str, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        PathBuf::from(cwd).join(path)
    }
}

fn codex_cwd(cwd: Option<&str>) -> Result<String> {
    if let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) {
        return Ok(normalize_hook_cwd(cwd).to_string_lossy().to_string());
    }

    Ok(std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Failed to determine current directory: {e}"))?
        .to_string_lossy()
        .to_string())
}

/// Normalize hook-provided working directories for the process environment that handles the hook.
pub fn normalize_hook_cwd(cwd: &str) -> PathBuf {
    #[cfg(not(windows))]
    {
        if let Some(path) = wsl_unc_to_linux_path(cwd) {
            return path;
        }
        if let Some(path) = windows_drive_to_wsl_path(cwd) {
            return path;
        }
    }

    PathBuf::from(cwd)
}

#[cfg(not(windows))]
fn wsl_unc_to_linux_path(cwd: &str) -> Option<PathBuf> {
    if !running_in_wsl() {
        return None;
    }

    let normalized = cwd.trim().replace('\\', "/");
    let rest = normalized
        .strip_prefix("//wsl.localhost/")
        .or_else(|| normalized.strip_prefix("//wsl$/"))?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let distro = parts.next()?;
    if let Ok(current_distro) = std::env::var("WSL_DISTRO_NAME")
        && !current_distro.eq_ignore_ascii_case(distro)
    {
        return None;
    }

    let mut path = PathBuf::from("/");
    for part in parts {
        path.push(part);
    }
    Some(path)
}

#[cfg(not(windows))]
fn windows_drive_to_wsl_path(cwd: &str) -> Option<PathBuf> {
    if !running_in_wsl() {
        return None;
    }

    let normalized = cwd.trim().replace('\\', "/");
    let bytes = normalized.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || bytes[2] != b'/' {
        return None;
    }

    let mut path = PathBuf::from("/mnt");
    path.push((bytes[0] as char).to_ascii_lowercase().to_string());
    for part in normalized[3..].split('/').filter(|part| !part.is_empty()) {
        path.push(part);
    }
    Some(path)
}

#[cfg(not(windows))]
fn running_in_wsl() -> bool {
    std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some()
}

fn last_user_prompt_from_codex_transcript(path: &Path) -> Result<Option<String>> {
    let file =
        std::fs::File::open(path).map_err(|e| anyhow::anyhow!("Failed to open file: {e}"))?;
    let reader = std::io::BufReader::new(file);
    let mut last_prompt = None;

    for line in reader.lines() {
        let line = line.map_err(|e| anyhow::anyhow!("Failed to read line: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line)
            && let Some(prompt) = extract_codex_user_text(&value)
        {
            last_prompt = Some(prompt);
        }
    }

    Ok(last_prompt)
}

fn extract_codex_user_text(value: &Value) -> Option<String> {
    let Value::Object(map) = value else {
        return None;
    };

    let is_user = map
        .get("role")
        .and_then(Value::as_str)
        .is_some_and(|role| role == "user")
        || map
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| matches!(kind, "user" | "user_message"));

    if is_user {
        for key in ["content", "message", "text", "input", "prompt"] {
            if let Some(text) = map.get(key).and_then(codex_value_to_text)
                && !text.trim().is_empty()
            {
                return Some(text);
            }
        }
    }

    for key in ["item", "payload", "msg", "message"] {
        if let Some(text) = map.get(key).and_then(extract_codex_user_text) {
            return Some(text);
        }
    }

    None
}

fn codex_value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(codex_value_to_text)
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        Value::Object(map) => ["text", "content", "message", "input", "prompt"]
            .into_iter()
            .find_map(|key| map.get(key).and_then(codex_value_to_text)),
        _ => None,
    }
}

/// Output Codex hook errors in the format expected by the CLI wrapper.
pub trait OutputCodexJson {
    /// Emit hook output and preserve the original result.
    fn output_codex_json(self) -> Self;
}

impl OutputCodexJson for Result<CodexHookOutput> {
    fn output_codex_json(self) -> Self {
        match self {
            Ok(CodexHookOutput::Success) => Ok(CodexHookOutput::Success),
            Ok(CodexHookOutput::Error(e)) => {
                eprintln!("{e}");
                Ok(CodexHookOutput::Error(e))
            }
            Err(e) => {
                eprintln!("{e}");
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{
        CodexToolUseInput, structured_patches_from_tool_payload, structured_patches_from_value,
    };

    #[test]
    fn reads_structured_patch_from_tool_response() {
        let input = CodexToolUseInput {
            session_id: Some("turn-1".to_string()),
            turn_id: None,
            cwd: None,
            tool_name: "apply_patch".to_string(),
            tool_input: Value::Null,
            tool_response: serde_json::json!({
                "structuredPatch": [{
                    "oldStart": 1,
                    "oldLines": 1,
                    "newStart": 1,
                    "newLines": 2,
                    "lines": ["-old", "+new", "+line"]
                }]
            }),
        };

        let patches = structured_patches_from_tool_payload(&input);

        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].old_start, 1);
        assert_eq!(patches[0].old_lines, 1);
        assert_eq!(patches[0].new_start, 1);
        assert_eq!(patches[0].new_lines, 2);
    }

    #[test]
    fn ignores_payload_without_structured_patch() {
        let patches = structured_patches_from_value(&serde_json::json!({
            "filePath": "src/lib.rs"
        }));

        assert!(patches.is_none());
    }
}
