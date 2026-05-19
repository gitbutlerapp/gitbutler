use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, bail};
use but_api::json::Error;
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use tracing::instrument;

#[derive(Default)]
pub struct CodeRabbit {
    inner: Mutex<CodeRabbitState>,
}

#[derive(Default)]
struct CodeRabbitState {
    active: HashMap<String, ActiveReview>,
    findings: HashMap<String, Vec<CodeRabbitFinding>>,
}

struct ActiveReview {
    review_id: String,
    cancel: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRabbitStatus {
    cli_available: bool,
    version: Option<String>,
    authenticated: bool,
    username: Option<String>,
    current_org: Option<String>,
    config_exists: bool,
    active_review_id: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRabbitReviewRequest {
    review_id: Option<String>,
    #[serde(default = "default_review_type")]
    review_type: String,
    base: Option<String>,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    workflows: Vec<CodeRabbitWorkflowId>,
}

fn default_review_type() -> String {
    "uncommitted".to_string()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodeRabbitWorkflowId {
    Default,
    Performance,
    Security,
    Correctness,
}

impl Default for CodeRabbitWorkflowId {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRabbitReviewResult {
    review_id: String,
    findings: Vec<CodeRabbitFinding>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRabbitFindingUpdate {
    finding_id: String,
    status: CodeRabbitFindingStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodeRabbitFindingStatus {
    Open,
    Dismissed,
    Applied,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeRabbitFinding {
    id: String,
    review_id: String,
    project_id: String,
    path: String,
    old_line: Option<u32>,
    new_line: Option<u32>,
    severity: CodeRabbitSeverity,
    category: Option<String>,
    title: String,
    body: String,
    suggested_patch: Option<String>,
    workflow_id: Option<String>,
    status: CodeRabbitFindingStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CodeRabbitSeverity {
    Critical,
    Major,
    Minor,
    Info,
}

#[tauri::command(async)]
#[instrument(skip(coderabbit), err(Debug))]
pub async fn coderabbit_status(
    coderabbit: State<'_, CodeRabbit>,
    project_id: ProjectHandleOrLegacyProjectId,
) -> Result<CodeRabbitStatus, Error> {
    let workdir = project_workdir(project_id.clone())?;
    let project_key = project_id.to_string();
    let active_review_id = coderabbit
        .inner
        .lock()
        .active
        .get(&project_key)
        .map(|active| active.review_id.clone());

    tokio::task::spawn_blocking(move || status_for_workdir(&workdir, active_review_id))
        .await
        .context("failed to join CodeRabbit status task")?
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(coderabbit), err(Debug))]
pub async fn coderabbit_login(
    coderabbit: State<'_, CodeRabbit>,
    project_id: ProjectHandleOrLegacyProjectId,
) -> Result<CodeRabbitStatus, Error> {
    let workdir = project_workdir(project_id.clone())?;
    let project_key = project_id.to_string();
    let active_review_id = coderabbit
        .inner
        .lock()
        .active
        .get(&project_key)
        .map(|active| active.review_id.clone());
    tokio::task::spawn_blocking(move || {
        let _ = Command::new("coderabbit")
            .args(["auth", "login", "--agent"])
            .current_dir(&workdir)
            .status();
        status_for_workdir(&workdir, active_review_id)
    })
    .await
    .context("failed to join CodeRabbit login task")?
    .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(coderabbit), err(Debug))]
pub async fn coderabbit_review(
    coderabbit: State<'_, CodeRabbit>,
    project_id: ProjectHandleOrLegacyProjectId,
    request: CodeRabbitReviewRequest,
) -> Result<CodeRabbitReviewResult, Error> {
    let workdir = project_workdir(project_id.clone())?;
    let project_key = project_id.to_string();
    let review_id = request.review_id.clone().unwrap_or_else(new_review_id);
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut inner = coderabbit.inner.lock();
        if let Some(active) = inner.active.get(&project_key) {
            return Err(
                anyhow::anyhow!("CodeRabbit review already running: {}", active.review_id).into(),
            );
        }
        inner.active.insert(
            project_key.clone(),
            ActiveReview {
                review_id: review_id.clone(),
                cancel: cancel.clone(),
            },
        );
    }

    let project_id_for_findings = project_key.clone();
    let review_id_for_findings = review_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_review(
            &workdir,
            &project_id_for_findings,
            &review_id_for_findings,
            request,
            cancel,
        )
    })
    .await
    .context("failed to join CodeRabbit review task")?;

    let mut inner = coderabbit.inner.lock();
    inner.active.remove(&project_key);
    match result {
        Ok(findings) => {
            inner.findings.insert(project_key, findings.clone());
            Ok(CodeRabbitReviewResult {
                review_id,
                findings,
            })
        }
        Err(err) => Err(err.into()),
    }
}

#[tauri::command(async)]
#[instrument(skip(coderabbit), err(Debug))]
pub fn coderabbit_cancel(
    coderabbit: State<'_, CodeRabbit>,
    project_id: ProjectHandleOrLegacyProjectId,
    review_id: String,
) -> Result<bool, Error> {
    let project_key = project_id.to_string();
    let inner = coderabbit.inner.lock();
    let Some(active) = inner.active.get(&project_key) else {
        return Ok(false);
    };
    if active.review_id != review_id {
        return Ok(false);
    }
    active.cancel.store(true, Ordering::SeqCst);
    Ok(true)
}

#[tauri::command(async)]
#[instrument(skip(coderabbit), err(Debug))]
pub fn coderabbit_findings(
    coderabbit: State<'_, CodeRabbit>,
    project_id: ProjectHandleOrLegacyProjectId,
    review_id: Option<String>,
) -> Result<Vec<CodeRabbitFinding>, Error> {
    let project_key = project_id.to_string();
    let mut findings = coderabbit
        .inner
        .lock()
        .findings
        .get(&project_key)
        .cloned()
        .unwrap_or_default();
    if let Some(review_id) = review_id {
        findings.retain(|finding| finding.review_id == review_id);
    }
    Ok(findings)
}

#[tauri::command(async)]
#[instrument(skip(coderabbit), err(Debug))]
pub fn coderabbit_update_finding(
    coderabbit: State<'_, CodeRabbit>,
    project_id: ProjectHandleOrLegacyProjectId,
    update: CodeRabbitFindingUpdate,
) -> Result<Option<CodeRabbitFinding>, Error> {
    let project_key = project_id.to_string();
    let mut inner = coderabbit.inner.lock();
    let Some(findings) = inner.findings.get_mut(&project_key) else {
        return Ok(None);
    };
    let Some(finding) = findings
        .iter_mut()
        .find(|finding| finding.id == update.finding_id)
    else {
        return Ok(None);
    };
    finding.status = update.status;
    Ok(Some(finding.clone()))
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn coderabbit_write_default_config(
    project_id: ProjectHandleOrLegacyProjectId,
) -> Result<bool, Error> {
    let workdir = project_workdir(project_id)?;
    let path = workdir.join(".coderabbit.yaml");
    if path.exists() {
        return Ok(false);
    }
    std::fs::write(path, DEFAULT_CODERABBIT_CONFIG).map_err(anyhow::Error::from)?;
    Ok(true)
}

const DEFAULT_CODERABBIT_CONFIG: &str = r#"# yaml-language-server: $schema=https://coderabbit.ai/integrations/schema.v2.json
reviews:
  path_filters:
    - "!**/node_modules/**"
    - "!**/target/**"
    - "!**/dist/**"
    - "!**/build/**"
    - "!**/*.unity"
    - "!**/*.prefab"
    - "!**/*.asset"
    - "!**/*.meta"
    - "!**/pnpm-lock.yaml"
    - "!**/package-lock.json"
  path_instructions:
    - path: "crates/**"
      instructions: |
        Focus on Rust correctness, locking, error handling, performance, and Git repository semantics.
    - path: "apps/desktop/**"
      instructions: |
        Focus on Svelte state, async UI behavior, Tauri command use, and user-facing regressions.
"#;

fn project_workdir(project_id: ProjectHandleOrLegacyProjectId) -> anyhow::Result<PathBuf> {
    let ctx: Context = project_id.try_into()?;
    ctx.workdir_or_fail()
}

fn status_for_workdir(
    workdir: &Path,
    active_review_id: Option<String>,
) -> anyhow::Result<CodeRabbitStatus> {
    let version = Command::new("coderabbit")
        .arg("--version")
        .current_dir(workdir)
        .output();

    let Ok(version) = version else {
        return Ok(CodeRabbitStatus {
            cli_available: false,
            version: None,
            authenticated: false,
            username: None,
            current_org: None,
            config_exists: workdir.join(".coderabbit.yaml").exists(),
            active_review_id,
            error: Some("CodeRabbit CLI was not found on PATH".to_string()),
        });
    };

    let version_text = String::from_utf8_lossy(&version.stdout).trim().to_string();
    let auth = Command::new("coderabbit")
        .args(["auth", "status", "--agent"])
        .current_dir(workdir)
        .output();

    let (authenticated, username, current_org, error) = match auth {
        Ok(auth) if auth.status.success() => {
            let value: Value = serde_json::from_slice(&auth.stdout).unwrap_or(Value::Null);
            (
                value
                    .get("authenticated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                value
                    .pointer("/user/username")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                value
                    .pointer("/currentOrg/name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                None,
            )
        }
        Ok(auth) => (
            false,
            None,
            None,
            Some(String::from_utf8_lossy(&auth.stderr).trim().to_string()),
        ),
        Err(err) => (false, None, None, Some(err.to_string())),
    };

    Ok(CodeRabbitStatus {
        cli_available: true,
        version: (!version_text.is_empty()).then_some(version_text),
        authenticated,
        username,
        current_org,
        config_exists: workdir.join(".coderabbit.yaml").exists(),
        active_review_id,
        error,
    })
}

fn run_review(
    workdir: &Path,
    project_id: &str,
    review_id: &str,
    request: CodeRabbitReviewRequest,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<Vec<CodeRabbitFinding>> {
    let mut args = vec!["review".to_string(), "--agent".to_string()];
    args.push("--type".to_string());
    args.push(request.review_type);
    if let Some(base) = request.base {
        args.push("--base".to_string());
        args.push(base);
    }
    let files = request
        .files
        .into_iter()
        .filter(|path| !should_skip_path(path))
        .collect::<Vec<_>>();
    if !files.is_empty() {
        args.push("--files".to_string());
        args.extend(files);
    }

    let instruction_files = write_workflow_instruction_files(workdir, &request.workflows)?;
    for path in &instruction_files {
        args.push("-c".to_string());
        args.push(path.to_string_lossy().to_string());
    }

    let mut child = Command::new("coderabbit")
        .args(&args)
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start CodeRabbit review")?;

    let stdout = child.stdout.take().context("missing CodeRabbit stdout")?;
    let stderr = child.stderr.take().context("missing CodeRabbit stderr")?;
    let stdout_thread = thread::spawn(move || read_to_string(stdout));
    let stderr_thread = thread::spawn(move || read_to_string(stderr));

    let status = loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            bail!("CodeRabbit review was cancelled");
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(100));
    };

    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();

    for file in instruction_files {
        let _ = std::fs::remove_file(file);
    }

    if !status.success() {
        let message = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!("CodeRabbit review failed: {message}");
    }

    Ok(parse_findings(project_id, review_id, &stdout))
}

fn read_to_string(mut reader: impl Read) -> String {
    let mut output = String::new();
    let _ = reader.read_to_string(&mut output);
    output
}

fn write_workflow_instruction_files(
    workdir: &Path,
    workflows: &[CodeRabbitWorkflowId],
) -> anyhow::Result<Vec<PathBuf>> {
    let workflows = if workflows.is_empty() {
        vec![CodeRabbitWorkflowId::Default]
    } else {
        workflows.to_vec()
    };
    let mut paths = Vec::new();
    for workflow in workflows {
        let Some(instructions) = workflow_instructions(&workflow) else {
            continue;
        };
        let path = std::env::temp_dir().join(format!(
            "gitbutler-coderabbit-{}-{}.md",
            workflow_name(&workflow),
            new_review_id()
        ));
        std::fs::write(
            &path,
            format!(
                "{instructions}\n\nRepository root: {}\nSkip Unity raw scene, prefab, asset, meta, generated, dependency, and build-output files unless they are explicitly selected.",
                workdir.display()
            ),
        )?;
        paths.push(path);
    }
    Ok(paths)
}

fn workflow_name(workflow: &CodeRabbitWorkflowId) -> &'static str {
    match workflow {
        CodeRabbitWorkflowId::Default => "default",
        CodeRabbitWorkflowId::Performance => "performance",
        CodeRabbitWorkflowId::Security => "security",
        CodeRabbitWorkflowId::Correctness => "correctness",
    }
}

fn workflow_instructions(workflow: &CodeRabbitWorkflowId) -> Option<&'static str> {
    match workflow {
        CodeRabbitWorkflowId::Default => None,
        CodeRabbitWorkflowId::Performance => Some(
            "Focus this CodeRabbit review on performance risks: avoidable repeated work, expensive rendering/recomputation, N+1 IO, inefficient Git traversal, excessive allocations, and scalability issues. Report only issues that are actionable.",
        ),
        CodeRabbitWorkflowId::Security => Some(
            "Focus this CodeRabbit review on security vulnerabilities: command execution, filesystem access, credential handling, injection, unsafe deserialization, auth bypasses, and secret exposure. Report only issues that are actionable.",
        ),
        CodeRabbitWorkflowId::Correctness => Some(
            "Focus this CodeRabbit review on logic and correctness: state races, stale data, edge cases, error handling, data loss, incorrect line/path mapping, and user-visible regressions. Report only issues that are actionable.",
        ),
    }
}

fn should_skip_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_lowercase();
    lower.contains("/node_modules/")
        || lower.contains("/target/")
        || lower.contains("/dist/")
        || lower.contains("/build/")
        || lower.ends_with(".unity")
        || lower.ends_with(".prefab")
        || lower.ends_with(".asset")
        || lower.ends_with(".meta")
}

fn parse_findings(project_id: &str, review_id: &str, stdout: &str) -> Vec<CodeRabbitFinding> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .map(|kind| kind.eq_ignore_ascii_case("finding"))
                .unwrap_or(false)
        })
        .filter_map(|value| normalize_finding(project_id, review_id, &value))
        .collect()
}

fn normalize_finding(
    project_id: &str,
    review_id: &str,
    value: &Value,
) -> Option<CodeRabbitFinding> {
    let source = value.get("finding").unwrap_or(value);
    let path = string_at(
        source,
        &[
            "/path",
            "/file",
            "/filePath",
            "/filename",
            "/location/path",
            "/location/file",
        ],
    )?;
    if should_skip_path(&path) {
        return None;
    }

    let title = string_at(source, &["/title", "/message", "/summary"])
        .unwrap_or_else(|| "CodeRabbit finding".to_string());
    let body =
        string_at(source, &["/body", "/description", "/details", "/message"]).unwrap_or_default();

    Some(CodeRabbitFinding {
        id: format!("{}-{}", review_id, uuid::Uuid::new_v4()),
        review_id: review_id.to_string(),
        project_id: project_id.to_string(),
        path,
        old_line: number_at(source, &["/oldLine", "/old_line", "/location/oldLine"]),
        new_line: number_at(
            source,
            &[
                "/newLine",
                "/line",
                "/startLine",
                "/location/line",
                "/location/newLine",
            ],
        ),
        severity: severity_at(source),
        category: string_at(source, &["/category", "/rule", "/type"]),
        title,
        body,
        suggested_patch: string_at(
            source,
            &[
                "/suggestedPatch",
                "/suggestion",
                "/fix/patch",
                "/fix/suggestion",
            ],
        ),
        workflow_id: string_at(source, &["/workflowId", "/workflow"]),
        status: CodeRabbitFindingStatus::Open,
    })
}

fn string_at(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .filter_map(|pointer| value.pointer(pointer))
        .find_map(|value| value.as_str().map(ToOwned::to_owned))
        .filter(|value| !value.trim().is_empty())
}

fn number_at(value: &Value, pointers: &[&str]) -> Option<u32> {
    pointers
        .iter()
        .filter_map(|pointer| value.pointer(pointer))
        .find_map(|value| value.as_u64().and_then(|value| u32::try_from(value).ok()))
}

fn severity_at(value: &Value) -> CodeRabbitSeverity {
    match string_at(value, &["/severity", "/level"])
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "critical" | "error" | "high" => CodeRabbitSeverity::Critical,
        "major" | "warning" | "medium" => CodeRabbitSeverity::Major,
        "info" | "informational" | "notice" => CodeRabbitSeverity::Info,
        _ => CodeRabbitSeverity::Minor,
    }
}

fn new_review_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{millis}-{}", uuid::Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_finding() {
        let stdout = r#"{"type":"finding","finding":{"path":"src/main.rs","location":{"line":42},"severity":"major","title":"Slow loop","body":"Avoid repeated scans","suggestedPatch":"patch"}}"#;
        let findings = parse_findings("project", "review", stdout);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "src/main.rs");
        assert_eq!(findings[0].new_line, Some(42));
        assert!(matches!(findings[0].severity, CodeRabbitSeverity::Major));
    }

    #[test]
    fn skips_unity_raw_files() {
        let stdout = r#"{"type":"finding","path":"Assets/Main.unity","line":1,"title":"Noise"}"#;
        assert!(parse_findings("project", "review", stdout).is_empty());
    }
}
