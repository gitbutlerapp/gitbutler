use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use but_api_macros::but_api;
use but_core::RepositoryExt;
use serde::{Deserialize, Serialize};
use tracing::instrument;

const SETTINGS_FILE: &str = "backup_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    pub backup_directory: String,
    pub backup_before_upstream_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupBranch {
    pub name: String,
    pub ref_name: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub id: String,
    pub created_at: u128,
    pub source_project_path: String,
    pub bundle_path: String,
    pub size: u64,
    pub message: Option<String>,
    pub reason: Option<String>,
    pub branches: Vec<BackupBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRef {
    pub name: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupVerification {
    pub valid: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFilePreview {
    pub path: String,
    pub diff: String,
    pub current_exists: bool,
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_backup_settings(ctx: &but_ctx::Context) -> Result<BackupSettings> {
    read_settings(ctx)
}

#[but_api]
#[instrument(err(Debug))]
pub fn update_backup_settings(
    ctx: &but_ctx::Context,
    backup_directory: String,
    backup_before_upstream_default: bool,
) -> Result<BackupSettings> {
    let settings = BackupSettings {
        backup_directory,
        backup_before_upstream_default,
    };
    fs::create_dir_all(settings_dir(ctx)?)?;
    fs::write(settings_path(ctx)?, serde_json::to_vec_pretty(&settings)?)?;
    Ok(settings)
}

#[but_api]
#[instrument(err(Debug))]
pub fn create_backup(
    ctx: &but_ctx::Context,
    branch_names: Vec<String>,
    message: Option<String>,
    reason: Option<String>,
) -> Result<BackupManifest> {
    if branch_names.is_empty() {
        bail!("Select at least one branch to back up");
    }

    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("backups require a worktree repository")?;
    let settings = read_settings(ctx)?;
    let backup_dir = PathBuf::from(&settings.backup_directory);
    fs::create_dir_all(&backup_dir).with_context(|| {
        format!(
            "Failed to create backup directory '{}'",
            backup_dir.display()
        )
    })?;

    let mut branches = Vec::new();
    for branch_name in branch_names {
        let ref_name: gix::refs::FullName = format!("refs/heads/{branch_name}")
            .try_into()
            .with_context(|| format!("Invalid branch name '{branch_name}'"))?;
        let reference = repo
            .try_find_reference(ref_name.as_ref())?
            .with_context(|| format!("Branch '{branch_name}' does not exist"))?;
        branches.push(BackupBranch {
            name: branch_name,
            ref_name: ref_name.to_string(),
            sha: reference.id().to_string(),
        });
    }

    let id = format!("backup-{}", timestamp_ms()?);
    let bundle_path = backup_dir.join(format!("{id}.bundle"));
    let manifest_path = backup_dir.join(format!("{id}.json"));

    let mut command = Command::new("git");
    command.arg("-C").arg(workdir).arg("bundle").arg("create");
    command.arg(&bundle_path);
    for branch in &branches {
        command.arg(&branch.ref_name);
    }
    run(command, "Failed to create git bundle backup")?;

    let size = fs::metadata(&bundle_path)?.len();
    let manifest = BackupManifest {
        id,
        created_at: timestamp_ms()?,
        source_project_path: workdir.display().to_string(),
        bundle_path: bundle_path.display().to_string(),
        size,
        message: message.and_then(non_empty),
        reason: reason.and_then(non_empty),
        branches,
    };
    fs::write(manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest)
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_backups(ctx: &but_ctx::Context) -> Result<Vec<BackupManifest>> {
    let settings = read_settings(ctx)?;
    let backup_dir = PathBuf::from(&settings.backup_directory);
    if !backup_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let manifest: BackupManifest = serde_json::from_slice(&fs::read(&path)?)
            .with_context(|| format!("Failed to read backup manifest '{}'", path.display()))?;
        backups.push(manifest);
    }
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(backups)
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_backup(ctx: &but_ctx::Context, backup_id: String) -> Result<()> {
    let manifest = find_backup(ctx, &backup_id)?;
    let bundle_path = PathBuf::from(&manifest.bundle_path);
    let manifest_path = bundle_path.with_extension("json");
    remove_if_exists(&bundle_path)?;
    remove_if_exists(&manifest_path)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn verify_backup(ctx: &but_ctx::Context, backup_id: String) -> Result<BackupVerification> {
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("backups require a worktree repository")?;
    let manifest = find_backup(ctx, &backup_id)?;
    let output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .arg("bundle")
        .arg("verify")
        .arg(&manifest.bundle_path)
        .output()
        .context("Failed to run git bundle verify")?;
    let text = output_text(&output);
    Ok(BackupVerification {
        valid: output.status.success(),
        message: text,
    })
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_backup_refs(ctx: &but_ctx::Context, backup_id: String) -> Result<Vec<BackupRef>> {
    if backup_id.is_empty() {
        return Ok(Vec::new());
    }
    let manifest = find_backup(ctx, &backup_id)?;
    let output = Command::new("git")
        .arg("bundle")
        .arg("list-heads")
        .arg(&manifest.bundle_path)
        .output()
        .context("Failed to list git bundle heads")?;
    if !output.status.success() {
        bail!(output_text(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (sha, name) = line.split_once(' ')?;
            Some(BackupRef {
                name: name.to_owned(),
                sha: sha.to_owned(),
            })
        })
        .collect())
}

#[but_api]
#[instrument(err(Debug))]
pub fn list_backup_files(
    ctx: &but_ctx::Context,
    backup_id: String,
    ref_name: String,
) -> Result<Vec<String>> {
    if backup_id.is_empty() || ref_name.is_empty() {
        return Ok(Vec::new());
    }
    with_bare_clone(ctx, &backup_id, |git_dir| {
        let output = Command::new("git")
            .arg("--git-dir")
            .arg(git_dir)
            .arg("ls-tree")
            .arg("-r")
            .arg("--name-only")
            .arg(&ref_name)
            .output()
            .context("Failed to list backup files")?;
        if !output.status.success() {
            bail!(output_text(&output));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(ToOwned::to_owned)
            .collect())
    })
}

#[but_api]
#[instrument(err(Debug))]
pub fn preview_backup_file(
    ctx: &but_ctx::Context,
    backup_id: String,
    ref_name: String,
    path: String,
) -> Result<BackupFilePreview> {
    let relative = safe_relative_path(&path)?.to_owned();
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("backups require a worktree repository")?;
    with_bare_clone(ctx, &backup_id, |git_dir| {
        let backup = Command::new("git")
            .arg("--git-dir")
            .arg(git_dir)
            .arg("show")
            .arg(format!("{ref_name}:{path}"))
            .output()
            .with_context(|| format!("Failed to read '{path}' from backup"))?;
        if !backup.status.success() {
            bail!(output_text(&backup));
        }

        let current_path = workdir.join(relative);
        let current_exists = current_path.is_file();
        let current = match fs::read(&current_path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Failed to read '{}'", current_path.display()));
            }
        };

        let temp_dir =
            std::env::temp_dir().join(format!("gitbutler-backup-preview-{}", timestamp_ms()?));
        fs::create_dir_all(&temp_dir)?;
        let backup_path = temp_dir.join("backup");
        let worktree_path = temp_dir.join("worktree");
        fs::write(&backup_path, backup.stdout)?;
        fs::write(&worktree_path, current)?;

        let output = Command::new("git")
            .arg("diff")
            .arg("--no-index")
            .arg("--")
            .arg(&worktree_path)
            .arg(&backup_path)
            .output()
            .context("Failed to preview backup file changes")?;
        let cleanup = fs::remove_dir_all(&temp_dir);
        if !output.status.success() && output.status.code() != Some(1) {
            bail!(output_text(&output));
        }
        cleanup.context("Failed to clean up temporary backup preview")?;

        Ok(BackupFilePreview {
            path,
            diff: output_text(&output),
            current_exists,
        })
    })
}

#[but_api]
#[instrument(err(Debug))]
pub fn restore_backup_branch(
    ctx: &but_ctx::Context,
    backup_id: String,
    ref_name: String,
    target_branch_name: String,
    overwrite: bool,
) -> Result<()> {
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("backups require a worktree repository")?;
    let target_ref: gix::refs::FullName = format!("refs/heads/{target_branch_name}")
        .try_into()
        .with_context(|| format!("Invalid target branch name '{target_branch_name}'"))?;
    if !overwrite && repo.try_find_reference(target_ref.as_ref())?.is_some() {
        bail!("Branch '{target_branch_name}' already exists");
    }
    let manifest = find_backup(ctx, &backup_id)?;
    let refspec = if overwrite {
        format!("+{ref_name}:{target_ref}")
    } else {
        format!("{ref_name}:{target_ref}")
    };
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(workdir)
        .arg("fetch")
        .arg(&manifest.bundle_path)
        .arg(refspec);
    run(command, "Failed to restore branch from backup")
}

#[but_api]
#[instrument(err(Debug))]
pub fn restore_backup_files(
    ctx: &but_ctx::Context,
    backup_id: String,
    ref_name: String,
    paths: Vec<String>,
) -> Result<()> {
    if paths.is_empty() {
        bail!("Select at least one file to restore");
    }
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("backups require a worktree repository")?;
    with_bare_clone(ctx, &backup_id, |git_dir| {
        for path in &paths {
            let relative = safe_relative_path(path)?;
            let output = Command::new("git")
                .arg("--git-dir")
                .arg(git_dir)
                .arg("show")
                .arg(format!("{ref_name}:{path}"))
                .output()
                .with_context(|| format!("Failed to read '{path}' from backup"))?;
            if !output.status.success() {
                bail!(output_text(&output));
            }
            let destination = workdir.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(destination, output.stdout)?;
        }
        Ok(())
    })
}

fn read_settings(ctx: &but_ctx::Context) -> Result<BackupSettings> {
    let path = settings_path(ctx)?;
    if path.is_file() {
        return Ok(serde_json::from_slice(&fs::read(path)?)?);
    }
    Ok(default_settings(ctx)?)
}

fn default_settings(ctx: &but_ctx::Context) -> Result<BackupSettings> {
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("backups require a worktree repository")?;
    let repo_name = workdir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository");
    let parent = workdir.parent().unwrap_or(workdir);
    Ok(BackupSettings {
        backup_directory: parent
            .join("gitbutler-backups")
            .join(repo_name)
            .display()
            .to_string(),
        backup_before_upstream_default: true,
    })
}

fn settings_dir(ctx: &but_ctx::Context) -> Result<PathBuf> {
    ctx.legacy_project
        .open_isolated_repo()?
        .gitbutler_storage_path()
}

fn settings_path(ctx: &but_ctx::Context) -> Result<PathBuf> {
    Ok(settings_dir(ctx)?.join(SETTINGS_FILE))
}

fn find_backup(ctx: &but_ctx::Context, backup_id: &str) -> Result<BackupManifest> {
    list_backups(ctx)?
        .into_iter()
        .find(|backup| backup.id == backup_id)
        .with_context(|| format!("Backup '{backup_id}' was not found"))
}

fn with_bare_clone<T>(
    ctx: &but_ctx::Context,
    backup_id: &str,
    f: impl FnOnce(&Path) -> Result<T>,
) -> Result<T> {
    let manifest = find_backup(ctx, backup_id)?;
    let temp_dir = std::env::temp_dir().join(format!("gitbutler-{backup_id}-{}", timestamp_ms()?));
    let mut command = Command::new("git");
    command
        .arg("clone")
        .arg("--bare")
        .arg(&manifest.bundle_path)
        .arg(&temp_dir);
    run(command, "Failed to open backup bundle")?;
    let result = f(&temp_dir);
    let cleanup = fs::remove_dir_all(&temp_dir);
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(err), _) => Err(err),
        (Ok(_), Err(err)) => Err(err).context("Failed to clean up temporary backup checkout"),
    }
}

fn safe_relative_path(path: &str) -> Result<&Path> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("Invalid backup path '{}'", path.display());
    }
    Ok(path)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("Failed to delete '{}'", path.display())),
    }
}

fn timestamp_ms() -> Result<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn run(mut command: Command, context: &str) -> Result<()> {
    let output = command.output().with_context(|| context.to_owned())?;
    if !output.status.success() {
        bail!("{context}: {}", output_text(&output));
    }
    Ok(())
}

fn output_text(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = format!("{stderr}{stdout}");
    text.trim().to_owned()
}
