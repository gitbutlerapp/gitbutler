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
            EmptyStructuredPatchAssignment::AssignFile,
        )?;
    }

    Ok(CodexHookOutput::Success)
}

fn handle_stop_input(input: CodexStopInput) -> Result<CodexHookOutput> {
    let session_id = input.session_id()?;
    let cwd = codex_cwd(input.cwd.as_deref())?;
    let transcript_path = input.transcript_path.as_deref().map(Path::new);
    let ctx = Context::discover(&cwd)?;

    if let Some(transcript_path) = transcript_path {
        assign_changed_paths_from_codex_transcript(&cwd, session_id, transcript_path)?;
    }

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
            require_reword: false,
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
    if is_apply_patch_tool(&tool_name) {
        return changed_paths_from_apply_patch_input(tool_input);
    }

    if is_shell_tool(&tool_name) {
        return changed_paths_from_shell_tool_input(tool_input);
    }

    if !is_file_mutation_tool(&tool_name) {
        return vec![];
    }

    changed_paths_from_file_mutation_input(tool_input)
}

fn is_apply_patch_tool(tool_name: &str) -> bool {
    tool_name == "apply_patch"
}

fn is_file_mutation_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "edit" | "multiedit" | "write" | "str_replace_editor"
    )
}

fn is_shell_tool(tool_name: &str) -> bool {
    matches!(tool_name, "bash" | "exec_command" | "shell_command")
}

fn changed_paths_from_file_mutation_input(tool_input: &Value) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for key in ["file_path", "filePath", "path"] {
        if let Some(path) = tool_input.get(key).and_then(Value::as_str)
            && !path.is_empty()
        {
            paths.insert(path.to_string());
        }
    }

    paths.into_iter().collect()
}

fn changed_paths_from_shell_tool_input(tool_input: &Value) -> Vec<String> {
    tool_command(tool_input)
        .map(changed_paths_from_shell_command)
        .unwrap_or_default()
}

fn changed_paths_from_apply_patch_input(tool_input: &Value) -> Vec<String> {
    let mut paths = BTreeSet::new();
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

fn changed_paths_from_shell_command(command: &str) -> Vec<String> {
    let apply_patch_paths =
        changed_paths_from_apply_patch_input(&Value::String(command.to_string()));
    if !apply_patch_paths.is_empty() {
        return apply_patch_paths;
    }

    let Ok(tokens) = shell_words::split(command) else {
        return vec![];
    };
    let mut paths = BTreeSet::new();
    collect_shell_command_paths(&tokens, &mut paths);
    paths.into_iter().collect()
}

fn collect_shell_command_paths(tokens: &[String], paths: &mut BTreeSet<String>) {
    let mut segment_start = 0;
    for (index, token) in tokens.iter().enumerate() {
        if is_shell_segment_separator(token) {
            collect_shell_segment_paths(&tokens[segment_start..index], paths);
            segment_start = index + 1;
        }
    }
    collect_shell_segment_paths(&tokens[segment_start..], paths);
}

fn collect_shell_segment_paths(segment: &[String], paths: &mut BTreeSet<String>) {
    collect_output_redirection_targets(segment, paths);

    let mut command_index = 0;
    while segment
        .get(command_index)
        .is_some_and(|token| is_shell_env_assignment(token))
    {
        command_index += 1;
    }
    let Some(command) = segment
        .get(command_index)
        .map(|token| shell_command_name(token))
    else {
        return;
    };
    let args = &segment[command_index + 1..];

    match command.as_str() {
        "touch" | "rm" | "unlink" => collect_non_option_paths(args, paths, &[]),
        "truncate" => collect_non_option_paths(args, paths, &["-s", "--size"]),
        "mv" => collect_non_option_paths(args, paths, &[]),
        "cp" | "install" => collect_last_non_option_path(args, paths),
        "tee" => collect_non_option_paths(args, paths, &[]),
        "sed" => collect_sed_in_place_paths(args, paths),
        "perl" => collect_perl_in_place_paths(args, paths),
        _ => {}
    }
}

fn collect_output_redirection_targets(tokens: &[String], paths: &mut BTreeSet<String>) {
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if is_output_redirection_operator(token) {
            if let Some(path) = tokens.get(index + 1) {
                insert_shell_path(paths, path);
            }
            index += 2;
            continue;
        }
        if let Some(path) = output_redirection_target(token) {
            insert_shell_path(paths, path);
        }
        index += 1;
    }
}

fn collect_non_option_paths(
    args: &[String],
    paths: &mut BTreeSet<String>,
    option_value_flags: &[&str],
) {
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if is_output_redirection_operator(arg) {
            index += 2;
            continue;
        }
        if output_redirection_target(arg).is_some() {
            index += 1;
            continue;
        }
        if option_value_flags.iter().any(|flag| arg == flag) {
            index += 2;
            continue;
        }
        if option_value_flags
            .iter()
            .any(|flag| arg.starts_with(&format!("{flag}=")) || short_option_with_value(arg, flag))
        {
            index += 1;
            continue;
        }
        if !arg.starts_with('-') {
            insert_shell_path(paths, arg);
        }
        index += 1;
    }
}

fn collect_last_non_option_path(args: &[String], paths: &mut BTreeSet<String>) {
    if let Some(path) = args
        .iter()
        .filter(|arg| !arg.starts_with('-'))
        .filter(|arg| !is_output_redirection_operator(arg))
        .filter(|arg| output_redirection_target(arg).is_none())
        .next_back()
    {
        insert_shell_path(paths, path);
    }
}

fn collect_sed_in_place_paths(args: &[String], paths: &mut BTreeSet<String>) {
    if !args
        .iter()
        .any(|arg| arg == "--in-place" || arg.starts_with("-i"))
    {
        return;
    }

    let mut files = Vec::new();
    let mut expression_provided_by_flag = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-e" | "--expression" | "-f" | "--file" => {
                expression_provided_by_flag = true;
                index += 2;
            }
            "--in-place" => index += 1,
            _ if arg.starts_with("--in-place=") || arg.starts_with("-i") => index += 1,
            _ if arg.starts_with('-') => index += 1,
            _ => {
                files.push(arg.clone());
                index += 1;
            }
        }
    }

    let files = if expression_provided_by_flag {
        files.as_slice()
    } else {
        files.get(1..).unwrap_or_default()
    };
    for file in files {
        insert_shell_path(paths, file);
    }
}

fn collect_perl_in_place_paths(args: &[String], paths: &mut BTreeSet<String>) {
    if !args.iter().any(|arg| arg == "-i" || arg.starts_with("-i")) {
        return;
    }

    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-e" | "-E" | "-M" | "-I" => index += 2,
            _ if arg.starts_with('-') => index += 1,
            _ => {
                insert_shell_path(paths, arg);
                index += 1;
            }
        }
    }
}

fn insert_shell_path(paths: &mut BTreeSet<String>, path: &str) {
    let path = path.trim();
    if path.is_empty()
        || path.starts_with('&')
        || path == "-"
        || path.contains('\0')
        || path.starts_with('$')
    {
        return;
    }
    paths.insert(path.to_string());
}

fn shell_command_name(command: &str) -> String {
    Path::new(command)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| command.to_string())
}

fn is_shell_segment_separator(token: &str) -> bool {
    matches!(token, ";" | "&&" | "||" | "|")
}

fn is_shell_env_assignment(token: &str) -> bool {
    let Some((name, _value)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
        && !name.as_bytes()[0].is_ascii_digit()
}

fn is_output_redirection_operator(token: &str) -> bool {
    matches!(token, ">" | ">>" | ">|" | "1>" | "1>>" | "2>" | "2>>")
}

fn output_redirection_target(token: &str) -> Option<&str> {
    let redirection_start = token.find('>')?;
    if redirection_start > 0
        && !token[..redirection_start]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let rest = &token[redirection_start..];
    let target = rest
        .strip_prefix(">>")
        .or_else(|| rest.strip_prefix(">|"))
        .or_else(|| rest.strip_prefix('>'))?;
    (!target.is_empty()).then_some(target)
}

fn short_option_with_value(arg: &str, flag: &str) -> bool {
    flag.len() == 2 && arg.starts_with(flag) && arg.len() > flag.len()
}

fn tool_command(tool_input: &Value) -> Option<&str> {
    tool_input
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| tool_input.get("cmd").and_then(Value::as_str))
        .or_else(|| tool_input.as_str())
}

fn assign_changed_paths_from_codex_transcript(
    cwd: &str,
    session_id: Uuid,
    transcript_path: &Path,
) -> Result<()> {
    let paths = changed_paths_from_codex_transcript(transcript_path).unwrap_or_default();
    for path in paths {
        let absolute_path = absolute_tool_path(cwd, &path);
        let ctx = Context::discover(cwd)?;
        assign_agent_hunks_post_tool_call(
            ctx,
            session_id,
            &absolute_path.to_string_lossy(),
            &[],
            true,
            EmptyStructuredPatchAssignment::AssignFile,
        )?;
    }

    Ok(())
}

fn changed_paths_from_codex_transcript(path: &Path) -> Result<Vec<String>> {
    let file =
        std::fs::File::open(path).map_err(|e| anyhow::anyhow!("Failed to open file: {e}"))?;
    let reader = std::io::BufReader::new(file);
    let mut paths = BTreeSet::new();

    for line in reader.lines() {
        let line = line.map_err(|e| anyhow::anyhow!("Failed to read line: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            paths.extend(changed_paths_from_codex_transcript_event(&value));
        }
    }

    Ok(paths.into_iter().collect())
}

fn changed_paths_from_codex_transcript_event(value: &Value) -> Vec<String> {
    let payload = value.get("payload").unwrap_or(value);
    let Some(tool_name) = payload.get("name").and_then(Value::as_str) else {
        return vec![];
    };

    let tool_input = payload
        .get("input")
        .cloned()
        .or_else(|| {
            payload
                .get("arguments")
                .and_then(|arguments| parse_codex_tool_arguments(arguments, payload))
        })
        .unwrap_or(Value::Null);
    let tool_cwd = tool_input
        .get("workdir")
        .and_then(Value::as_str)
        .or_else(|| tool_input.get("cwd").and_then(Value::as_str));

    changed_paths_from_tool_input(tool_name, &tool_input)
        .into_iter()
        .map(|path| absolutize_transcript_tool_path(tool_cwd, path))
        .collect()
}

fn parse_codex_tool_arguments(arguments: &Value, payload: &Value) -> Option<Value> {
    match arguments {
        Value::String(arguments) => serde_json::from_str(arguments)
            .ok()
            .or_else(|| Some(Value::String(arguments.clone()))),
        Value::Object(_) => Some(arguments.clone()),
        _ => payload.get("input").cloned(),
    }
}

fn absolutize_transcript_tool_path(tool_cwd: Option<&str>, path: String) -> String {
    if Path::new(&path).is_absolute() {
        return path;
    }

    tool_cwd
        .map(|cwd| PathBuf::from(cwd).join(&path).to_string_lossy().to_string())
        .unwrap_or(path)
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
        CodexToolUseInput, changed_paths_from_shell_command, changed_paths_from_tool_input,
        structured_patches_from_tool_payload, structured_patches_from_value,
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

    #[test]
    fn reads_paths_for_mutating_file_tools() {
        let paths =
            changed_paths_from_tool_input("Edit", &serde_json::json!({"file_path": "src/lib.rs"}));

        assert_eq!(paths, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn reads_paths_for_apply_patch_tool() {
        let paths = changed_paths_from_tool_input(
            "apply_patch",
            &serde_json::json!({
                "cmd": "*** Begin Patch\n*** Add File: src/lib.rs\n+pub fn added() {}\n*** End Patch\n"
            }),
        );

        assert_eq!(paths, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn ignores_paths_for_read_only_tools() {
        let paths = changed_paths_from_tool_input(
            "mcp__filesystem__read_file",
            &serde_json::json!({"path": "src/lib.rs"}),
        );

        assert!(paths.is_empty());
    }

    #[test]
    fn reads_paths_for_shell_touch_tool() {
        let paths = changed_paths_from_tool_input(
            "Bash",
            &serde_json::json!({
                "command": "touch src/lib.rs"
            }),
        );

        assert_eq!(paths, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn reads_paths_for_shell_output_redirection() {
        let paths = changed_paths_from_shell_command("printf '%s\\n' value > src/lib.rs");

        assert_eq!(paths, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn reads_apply_patch_paths_from_shell_command() {
        let paths = changed_paths_from_tool_input(
            "Bash",
            &serde_json::json!({
                "command": "apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: src/lib.rs\n+pub fn added() {}\n*** End Patch\nPATCH\n"
            }),
        );

        assert_eq!(paths, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn ignores_read_only_shell_commands() {
        let paths = changed_paths_from_tool_input(
            "Bash",
            &serde_json::json!({
                "command": "ls -l src/lib.rs && cat src/lib.rs"
            }),
        );

        assert!(paths.is_empty());
    }
}
