use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use but_api_macros::but_api;
use but_core::git_config::{edit_repo_config, set_config_value};
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use but_error::Code;
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn update_project(
    project: gitbutler_project::UpdateRequest,
) -> Result<gitbutler_project::api::Project> {
    Ok(gitbutler_project::update(project)?.into())
}

/// Adds an existing git repository as a GitButler project.
/// `path` is the Git repository to remember as project.
#[but_api]
#[instrument(err(Debug))]
pub fn add_project(path: PathBuf) -> Result<gitbutler_project::AddProjectOutcome> {
    gitbutler_project::add(&path)
}

/// Add a project by a given path.
/// It will look for other existing projects and try to match the path
/// to them, allowing to open projects from paths within the repository.
#[but_api]
#[instrument(err(Debug))]
pub fn add_project_best_effort(path: PathBuf) -> Result<gitbutler_project::AddProjectOutcome> {
    gitbutler_project::add_with_best_effort(&path)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_project(
    project_id: ProjectHandleOrLegacyProjectId,
    no_validation: Option<bool>,
) -> Result<gitbutler_project::api::Project> {
    let no_validation = no_validation.unwrap_or(false);
    match project_id {
        ProjectHandleOrLegacyProjectId::ProjectHandle(handle) => {
            if no_validation {
                let project = gitbutler_project::get_raw(
                    ProjectHandleOrLegacyProjectId::ProjectHandle(handle),
                )?
                .migrated()?
                .into();
                Ok(project)
            } else {
                Ok(gitbutler_project::get_validated(
                    ProjectHandleOrLegacyProjectId::ProjectHandle(handle),
                )?
                .into())
            }
        }
        ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => Ok(if no_validation {
            gitbutler_project::get_raw(ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id))?
                .migrated()?
                .into()
        } else {
            gitbutler_project::get_validated(ProjectHandleOrLegacyProjectId::LegacyProjectId(
                project_id,
            ))?
            .into()
        }),
    }
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_projects_stateless() -> Result<Vec<ProjectForFrontend>> {
    list_projects(vec![])
}

/// List all stored projects for the frontend.
///
/// `opened_projects` identifies projects the frontend currently considers open so the returned
/// entries can be annotated with `is_open`. Stale opened-project handles are ignored because the
/// frontend may still hold them briefly after project deletion.
///
/// This front-end specific behaviour needs review when this comes out of legacy.
#[but_api]
#[instrument(err(Debug))]
pub fn list_projects(
    opened_projects: Vec<ProjectHandleOrLegacyProjectId>,
) -> Result<Vec<ProjectForFrontend>> {
    // Skip handles that can no longer be resolved — e.g. the project was just deleted
    // from storage but the frontend's `opened_projects` set hasn't caught up yet.
    // Failing the whole listing on a stale entry would break the post-deletion refresh
    // flow. Mirrors the warn-and-skip pattern used below for migration failures.
    let opened_projects: std::collections::HashSet<_> = opened_projects
        .into_iter()
        .filter_map(
            |project_id| match gitbutler_project::get_raw(project_id.clone()) {
                Ok(project) => Some(project.id),
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        ?project_id,
                        "Skipping over opened project as its handle could not be resolved"
                    );
                    None
                }
            },
        )
        .collect();

    gitbutler_project::assure_app_can_startup_or_fix_it(
        gitbutler_project::dangerously_list_projects_without_migration(),
    )
    .map(|projects| {
        projects
            .into_iter()
            .map(|project| {
                anyhow::Ok(ProjectForFrontend {
                    is_open: opened_projects.contains(&project.id),
                    inner: project.migrated().map(Into::into)?,
                })
            })
            .filter_map(|res| match res {
                Ok(p) => Some(p),
                Err(err) => {
                    tracing::warn!(?err, "Skipping over project as it failed migration");
                    None
                }
            })
            .collect()
    })
}

#[but_api]
#[instrument(err(Debug))]
pub fn delete_project(project_id: ProjectHandleOrLegacyProjectId) -> Result<()> {
    delete_project_at_app_data_dir(but_path::app_data_dir()?, project_id)
}

fn delete_project_at_app_data_dir(
    app_data_dir: impl AsRef<Path>,
    project_id: ProjectHandleOrLegacyProjectId,
) -> Result<()> {
    gitbutler_project::delete_with_path(app_data_dir, project_id)
}

/// Prepare an already-known project for activation in the UI or server.
///
/// This repairs missing target metadata in freshly selected storage locations and then reconciles
/// the legacy metadata view with the workspace currently present in Git. It is safe for activation
/// paths because it avoids rewriting `gitbutler/workspace`.
pub fn prepare_project_for_activation(ctx: &mut Context) -> Result<()> {
    assure_repo_ownership(&*ctx.repo.get()?)?;
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::base::bootstrap_default_target_if_missing(ctx)?;
    super::meta::reconcile_in_workspace_state_of_vb_toml(ctx, guard.write_permission()).ok();
    Ok(())
}

/// Return activation-time warnings for repositories that need additional operator awareness.
///
/// Today this focuses on Unity repositories because they have well-known data-loss traps around
/// serialization mode, merge-driver setup, and Git LFS-managed scene data.
pub fn project_activation_headsup(repo: &gix::Repository) -> Result<Option<String>> {
    let unityyamlmerge_path = std::env::var_os("GITBUTLER_UNITYYAMLMERGE_PATH").map(PathBuf::from);
    project_activation_headsup_with_unityyamlmerge_path(repo, unityyamlmerge_path.as_deref())
}

fn project_activation_headsup_with_unityyamlmerge_path(
    repo: &gix::Repository,
    unityyamlmerge_path: Option<&Path>,
) -> Result<Option<String>> {
    let Some(workdir) = repo.workdir() else {
        return Ok(None);
    };
    if !looks_like_unity_project(workdir) {
        return Ok(None);
    }

    let mut notices = Vec::new();
    let mut warnings = Vec::new();

    let auto_configured_unityyamlmerge_path =
        ensure_unityyamlmerge_is_configured(repo, workdir, unityyamlmerge_path)?;
    if let Some(path) = &auto_configured_unityyamlmerge_path {
        notices.push(format!(
            "Configured UnityYAMLMerge in local Git config using `{}`.",
            path.display()
        ));
    }

    let editor_settings_path = workdir.join("ProjectSettings").join("EditorSettings.asset");
    if let Some(editor_settings) = read_optional_text_file(&editor_settings_path)? {
        if !editor_settings.contains("m_SerializationMode: 2") {
            warnings.push(
                "Asset Serialization is not set to Force Text (`m_SerializationMode: 2`), so \
                 scene and prefab merges are unsafe."
                    .to_owned(),
            );
        }
    }

    let gitattributes_path = workdir.join(".gitattributes");
    if let Some(gitattributes) = read_optional_text_file(&gitattributes_path)? {
        if auto_configured_unityyamlmerge_path.is_none()
            && gitattributes.contains("merge=unityyamlmerge")
            && !unityyamlmerge_is_configured(repo)
        {
            warnings.push(
                "`.gitattributes` expects `unityyamlmerge`, but this machine does not have a \
                 Git config entry for `merge.unityyamlmerge.driver` or \
                 `mergetool.unityyamlmerge.cmd`."
                    .to_owned(),
            );
        }

        let lfs_managed_unity_paths = lfs_managed_unity_paths(&gitattributes);
        if !lfs_managed_unity_paths.is_empty() {
            let protected_scenes = protected_scene_paths(workdir, &gitattributes)?;
            let examples = if protected_scenes.is_empty() {
                String::new()
            } else {
                format!(" Examples: {}.", protected_scenes.join(", "))
            };
            warnings.push(format!(
                "Unity scene or asset paths are managed by Git LFS ({patterns}). GitButler \
                 workspace operations do not apply worktree filters, so these paths should be \
                 treated as externally managed.{examples}",
                patterns = lfs_managed_unity_paths.join(", "),
                examples = examples
            ));
        }

        let unmanaged_lighting_data = unmanaged_lighting_data_assets(workdir, &gitattributes)?;
        if !unmanaged_lighting_data.is_empty() {
            warnings.push(format!(
                "`LightingData.asset` should stay binary or externally managed. Found: {}.",
                unmanaged_lighting_data.join(", ")
            ));
        }
    } else {
        let unmanaged_lighting_data = unmanaged_lighting_data_assets(workdir, "")?;
        if !unmanaged_lighting_data.is_empty() {
            warnings.push(format!(
                "`LightingData.asset` should stay binary or externally managed. Found: {}.",
                unmanaged_lighting_data.join(", ")
            ));
        }
    }

    if unity_editor_appears_open(workdir) {
        warnings.push(
            "Unity editor appears to be open (lock files are present). Close Unity before \
             checkout, rebase, undo, or other destructive Git operations."
                .to_owned(),
        );
    }

    let orphaned_meta_files = orphaned_meta_files(workdir)?;
    if !orphaned_meta_files.is_empty() {
        warnings.push(format!(
            "Unity meta file without a paired asset or folder detected: {}.",
            orphaned_meta_files.join(", ")
        ));
    }

    if warnings.is_empty() && notices.is_empty() {
        return Ok(None);
    }

    let mut headsup = String::from("Unity repository checks:\n");
    for notice in notices {
        headsup.push_str("- ");
        headsup.push_str(&notice);
        headsup.push('\n');
    }
    for warning in warnings {
        headsup.push_str("- ");
        headsup.push_str(&warning);
        headsup.push('\n');
    }
    headsup.pop();
    Ok(Some(headsup))
}

// TODO(gix): remove this once there is no `git2` as `gix` provides safety by not trusting Git configuration instead.
fn assure_repo_ownership(repo: &gix::Repository) -> Result<()> {
    if repo.git_dir_trust() == gix::sec::Trust::Full {
        return Ok(());
    }

    let path = repo.workdir().unwrap_or(repo.git_dir());
    Err(anyhow!(
        "The git directory is considered unsafe as it's not owned by the current user. Use `git config --global --add safe.directory '{}'` to allow it",
        path.display()
    )
    .context(Code::RepoOwnership))
}

fn looks_like_unity_project(workdir: &Path) -> bool {
    workdir.join("Assets").is_dir()
        && workdir.join("Packages").join("manifest.json").is_file()
        && workdir
            .join("ProjectSettings")
            .join("ProjectVersion.txt")
            .is_file()
}

fn read_optional_text_file(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn ensure_unityyamlmerge_is_configured(
    repo: &gix::Repository,
    workdir: &Path,
    explicit_tool_path: Option<&Path>,
) -> Result<Option<PathBuf>> {
    if unityyamlmerge_is_fully_configured(repo) {
        return Ok(None);
    }
    let Some(unityyamlmerge_path) = find_unityyamlmerge_path(workdir, explicit_tool_path) else {
        return Ok(None);
    };

    configure_unityyamlmerge(repo, &unityyamlmerge_path)?;
    Ok(Some(unityyamlmerge_path))
}

fn unityyamlmerge_is_configured(repo: &gix::Repository) -> bool {
    let config = repo.config_snapshot();
    config.string("merge.unityyamlmerge.driver").is_some()
        || config.string("mergetool.unityyamlmerge.cmd").is_some()
}

fn unityyamlmerge_is_fully_configured(repo: &gix::Repository) -> bool {
    let config = repo.config_snapshot();
    matches!(
        config.string("merge.tool").map(|value| value.to_string()),
        Some(value) if value == "unityyamlmerge"
    ) && config.string("merge.unityyamlmerge.driver").is_some()
        && config.string("mergetool.unityyamlmerge.cmd").is_some()
}

fn find_unityyamlmerge_path(workdir: &Path, explicit_tool_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit_tool_path.filter(|path| path.is_file()) {
        return Some(path.to_path_buf());
    }

    for candidate in unityyamlmerge_candidates(workdir) {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    which::which("UnityYAMLMerge").ok()
}

fn unityyamlmerge_candidates(workdir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let version = unity_editor_version(workdir);

    if cfg!(target_os = "windows") {
        for root in unity_install_roots() {
            if let Some(version) = &version {
                candidates.push(
                    root.join("Unity")
                        .join("Hub")
                        .join("Editor")
                        .join(version)
                        .join("Editor")
                        .join("Data")
                        .join("Tools")
                        .join("UnityYAMLMerge.exe"),
                );
            }
            candidates.push(
                root.join("Unity")
                    .join("Editor")
                    .join("Data")
                    .join("Tools")
                    .join("UnityYAMLMerge.exe"),
            );
        }
    } else if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from(
            "/Applications/Unity/Hub/Editor/Unity.app/Contents/Tools/UnityYAMLMerge",
        ));
        if let Some(version) = &version {
            candidates.push(
                PathBuf::from("/Applications/Unity/Hub/Editor")
                    .join(version)
                    .join("Unity.app")
                    .join("Contents")
                    .join("Tools")
                    .join("UnityYAMLMerge"),
            );
        }
        candidates.push(PathBuf::from(
            "/Applications/Unity/Unity.app/Contents/Helpers/UnityYAMLMerge",
        ));
    }

    candidates
}

fn unity_install_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        let Some(value) = std::env::var_os(var) else {
            continue;
        };
        let path = PathBuf::from(value);
        if !roots.contains(&path) {
            roots.push(path);
        }
    }
    roots
}

fn unity_editor_version(workdir: &Path) -> Option<String> {
    let path = workdir.join("ProjectSettings").join("ProjectVersion.txt");
    let contents = std::fs::read_to_string(path).ok()?;
    contents.lines().find_map(|line| {
        line.strip_prefix("m_EditorVersion:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn configure_unityyamlmerge(repo: &gix::Repository, unityyamlmerge_path: &Path) -> Result<()> {
    let path = shell_quoted_path(unityyamlmerge_path);
    let merge_driver = format!(r#"{path} merge -p "%O" "%A" "%B" "%A""#);
    let mergetool_cmd = format!(r#"{path} merge -p "$BASE" "$REMOTE" "$LOCAL" "$MERGED""#);

    _ = edit_repo_config(repo, gix::config::Source::Local, |config| {
        set_config_value(config, "merge.tool", "unityyamlmerge")?;
        set_config_value(config, "merge.unityyamlmerge.name", "Unity SmartMerge")?;
        set_config_value(config, "merge.unityyamlmerge.driver", &merge_driver)?;
        set_config_value(config, "merge.unityyamlmerge.recursive", "binary")?;
        set_config_value(config, "mergetool.unityyamlmerge.trustExitCode", "false")?;
        set_config_value(config, "mergetool.unityyamlmerge.cmd", &mergetool_cmd)?;
        Ok(())
    })?;
    Ok(())
}

fn shell_quoted_path(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn lfs_managed_unity_paths(gitattributes: &str) -> Vec<String> {
    let mut matches = Vec::new();
    for line in gitattributes.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || !line.contains("filter=lfs") {
            continue;
        }
        let Some(pattern) = line.split_whitespace().next() else {
            continue;
        };
        if [".unity", ".asset", ".prefab", ".meta"]
            .iter()
            .any(|ext| pattern.contains(ext))
        {
            matches.push(pattern.to_owned());
        }
    }
    matches.sort();
    matches.dedup();
    matches
}

fn protected_scene_paths(workdir: &Path, gitattributes: &str) -> Result<Vec<String>> {
    if !gitattributes_contains_lfs_for_extension(gitattributes, ".unity") {
        return Ok(Vec::new());
    }

    let scenes_root = workdir.join("Assets").join("Scenes");
    let mut scenes = Vec::new();
    collect_files_with_extension(&scenes_root, ".unity", workdir, &mut scenes, 5)?;
    Ok(scenes)
}

fn unmanaged_lighting_data_assets(workdir: &Path, gitattributes: &str) -> Result<Vec<String>> {
    if lighting_data_is_externally_managed(gitattributes) {
        return Ok(Vec::new());
    }

    let mut assets = Vec::new();
    collect_files_named(
        &workdir.join("Assets"),
        "LightingData.asset",
        workdir,
        &mut assets,
        5,
    )?;
    Ok(assets)
}

fn gitattributes_contains_lfs_for_extension(gitattributes: &str, extension: &str) -> bool {
    gitattributes.lines().map(str::trim).any(|line| {
        !line.is_empty()
            && !line.starts_with('#')
            && line.contains("filter=lfs")
            && line
                .split_whitespace()
                .next()
                .is_some_and(|pattern| pattern.contains(extension))
    })
}

fn lighting_data_is_externally_managed(gitattributes: &str) -> bool {
    gitattributes.lines().map(str::trim).any(|line| {
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        let Some(pattern) = line.split_whitespace().next() else {
            return false;
        };
        (pattern.contains("LightingData.asset") || pattern.contains("*.asset"))
            && (line.contains("filter=lfs")
                || line.ends_with(" binary")
                || line.contains(" binary "))
    })
}

fn collect_files_with_extension(
    dir: &Path,
    extension: &str,
    repo_root: &Path,
    out: &mut Vec<String>,
    max_entries: usize,
) -> Result<()> {
    if out.len() >= max_entries || !dir.is_dir() {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if out.len() >= max_entries {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, repo_root, out, max_entries)?;
            continue;
        }
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case(extension.trim_start_matches('.')))
        {
            let relative = path
                .strip_prefix(repo_root)
                .unwrap_or(path.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            out.push(relative);
        }
    }

    Ok(())
}

fn collect_files_named(
    dir: &Path,
    file_name: &str,
    repo_root: &Path,
    out: &mut Vec<String>,
    max_entries: usize,
) -> Result<()> {
    if out.len() >= max_entries || !dir.is_dir() {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if out.len() >= max_entries {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_files_named(&path, file_name, repo_root, out, max_entries)?;
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
        {
            let relative = path
                .strip_prefix(repo_root)
                .unwrap_or(path.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            out.push(relative);
        }
    }

    Ok(())
}

fn unity_editor_appears_open(workdir: &Path) -> bool {
    [
        workdir.join("Temp").join("UnityLockfile"),
        workdir.join("Library").join("SourceAssetDB-lock"),
        workdir.join("Library").join("ArtifactDB-lock"),
    ]
    .into_iter()
    .any(|path| path.exists())
}

fn orphaned_meta_files(workdir: &Path) -> Result<Vec<String>> {
    let mut meta_files = Vec::new();
    collect_orphaned_meta_files(&workdir.join("Assets"), workdir, &mut meta_files, 5)?;
    Ok(meta_files)
}

fn collect_orphaned_meta_files(
    dir: &Path,
    repo_root: &Path,
    out: &mut Vec<String>,
    max_entries: usize,
) -> Result<()> {
    if out.len() >= max_entries || !dir.is_dir() {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if out.len() >= max_entries {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_orphaned_meta_files(&path, repo_root, out, max_entries)?;
            continue;
        }
        let Some(path_str) = path.to_str() else {
            continue;
        };
        if !path_str.ends_with(".meta") {
            continue;
        }

        let paired_path = PathBuf::from(path_str.trim_end_matches(".meta"));
        if paired_path.exists() {
            continue;
        }

        let relative = path
            .strip_prefix(repo_root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        out.push(relative);
    }

    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn is_gerrit(ctx: &but_ctx::Context) -> Result<bool> {
    gitbutler_project::gerrit::is_used_by_default_remote(&*ctx.repo.get()?)
}

#[derive(serde::Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: gitbutler_project::api::Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ProjectForFrontend);

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn delete_project_is_idempotent() -> Result<()> {
        let app_data_dir = tempfile::tempdir()?;
        let repo_dir = tempfile::tempdir()?;
        gix::init(repo_dir.path())?;
        let project = gitbutler_project::add_at_app_data_dir(app_data_dir.path(), repo_dir.path())?
            .unwrap_project();
        let project_id = project.id.clone();

        delete_project_at_app_data_dir(app_data_dir.path(), project_id.clone())?;
        delete_project_at_app_data_dir(app_data_dir.path(), project_id.clone())?;

        assert!(gitbutler_project::get_with_path(app_data_dir.path(), project_id).is_err());
        Ok(())
    }

    #[test]
    fn project_activation_headsup_ignores_non_unity_repositories() -> Result<()> {
        let repo_dir = tempfile::tempdir()?;
        let repo = gix::init(repo_dir.path())?;

        assert_eq!(project_activation_headsup(&repo)?, None);
        Ok(())
    }

    #[test]
    fn project_activation_headsup_warns_when_force_text_is_disabled() -> Result<()> {
        let (repo, _tmp) = unity_repo(
            r#"%YAML 1.1
EditorSettings:
  m_SerializationMode: 1
"#,
            Some(
                r#"Library/
Temp/
Logs/
UserSettings/
"#,
            ),
            None,
            &[],
        )?;

        let headsup = project_activation_headsup(&repo)?.expect("a warning");
        assert!(
            headsup.contains("Force Text"),
            "must explain that Unity serialization should use Force Text: {headsup}"
        );
        Ok(())
    }

    #[test]
    fn project_activation_headsup_warns_when_unityyamlmerge_is_not_configured_locally() -> Result<()>
    {
        let (repo, _tmp) = unity_repo(
            r#"%YAML 1.1
EditorSettings:
  m_SerializationMode: 2
"#,
            Some(
                r#"Library/
Temp/
Logs/
UserSettings/
"#,
            ),
            Some("*.prefab -text merge=unityyamlmerge diff\n"),
            &[("Assets/Example.prefab", "Prefab:\n")],
        )?;

        let headsup = project_activation_headsup(&repo)?.expect("a warning");
        assert!(
            headsup.contains("unityyamlmerge"),
            "must mention the missing UnityYAMLMerge setup: {headsup}"
        );
        assert!(
            headsup.contains("merge.unityyamlmerge.driver")
                || headsup.contains("mergetool.unityyamlmerge.cmd"),
            "must point at the Git config keys that need local setup: {headsup}"
        );
        Ok(())
    }

    #[test]
    fn project_activation_headsup_auto_configures_unityyamlmerge_when_tool_is_available()
    -> Result<()> {
        let (mut repo, _tmp) = unity_repo(
            r#"%YAML 1.1
EditorSettings:
  m_SerializationMode: 2
"#,
            Some(
                r#"Library/
Temp/
Logs/
UserSettings/
"#,
            ),
            Some("*.prefab -text merge=unityyamlmerge diff\n"),
            &[("Assets/Example.prefab", "Prefab:\n")],
        )?;
        let tool_dir = tempfile::tempdir()?;
        let tool_path = tool_dir.path().join("UnityYAMLMerge.exe");
        std::fs::write(&tool_path, "stub")?;

        let headsup = project_activation_headsup_with_unityyamlmerge_path(&repo, Some(&tool_path))?
            .expect("a setup notice");
        assert!(
            headsup.contains("Configured UnityYAMLMerge"),
            "must confirm that setup happened automatically: {headsup}"
        );

        repo.reload()?;
        let config = repo.config_snapshot();
        assert_eq!(
            config.string("merge.tool").map(|value| value.to_string()),
            Some("unityyamlmerge".to_owned()),
            "must set the local merge tool so git mergetool can use UnityYAMLMerge"
        );
        assert!(
            config
                .string("merge.unityyamlmerge.driver")
                .map(|value| value.to_string())
                .is_some_and(|value| value.contains("UnityYAMLMerge.exe")),
            "must configure the merge driver in local git config"
        );
        assert!(
            config
                .string("mergetool.unityyamlmerge.cmd")
                .map(|value| value.to_string())
                .is_some_and(|value| value.contains("UnityYAMLMerge.exe")),
            "must configure the mergetool command in local git config"
        );
        Ok(())
    }

    #[test]
    fn project_activation_headsup_warns_when_unity_scenes_are_lfs_managed() -> Result<()> {
        let (repo, _tmp) = unity_repo(
            r#"%YAML 1.1
EditorSettings:
  m_SerializationMode: 2
"#,
            Some(
                r#"Library/
Temp/
Logs/
UserSettings/
"#,
            ),
            Some(
                r#"*.unity filter=lfs diff=lfs merge=lfs -text
*.asset filter=lfs diff=lfs merge=lfs -text
"#,
            ),
            &[("Assets/Scenes/dealers.unity", "%YAML 1.1\nScene:\n")],
        )?;

        let headsup = project_activation_headsup(&repo)?.expect("a warning");
        assert!(
            headsup.contains("Git LFS"),
            "must mention Git LFS-managed Unity paths: {headsup}"
        );
        assert!(
            headsup.contains("Assets/Scenes/dealers.unity"),
            "must name the concrete Unity scene paths affected by the LFS warning: {headsup}"
        );
        Ok(())
    }

    #[test]
    fn project_activation_headsup_warns_when_lighting_data_is_not_externally_managed() -> Result<()>
    {
        let (repo, _tmp) = unity_repo(
            r#"%YAML 1.1
EditorSettings:
  m_SerializationMode: 2
"#,
            Some(
                r#"Library/
Temp/
Logs/
UserSettings/
"#,
            ),
            Some("*.prefab -text merge=unityyamlmerge diff\n"),
            &[(
                "Assets/Scenes/dealers/LightingData.asset",
                "%YAML 1.1\nLighting:\n",
            )],
        )?;

        let headsup = project_activation_headsup(&repo)?.expect("a warning");
        assert!(
            headsup.contains("LightingData.asset"),
            "must warn about unmanaged LightingData assets: {headsup}"
        );
        assert!(
            headsup.contains("Assets/Scenes/dealers/LightingData.asset"),
            "must name the specific LightingData asset path: {headsup}"
        );
        Ok(())
    }

    #[test]
    fn project_activation_headsup_warns_when_unity_editor_appears_open() -> Result<()> {
        let (repo, _tmp) = unity_repo(
            r#"%YAML 1.1
EditorSettings:
  m_SerializationMode: 2
"#,
            Some(
                r#"Library/
Temp/
Logs/
UserSettings/
"#,
            ),
            Some("*.prefab -text merge=unityyamlmerge diff\n"),
            &[("Temp/UnityLockfile", "locked\n")],
        )?;

        let headsup = project_activation_headsup(&repo)?.expect("a warning");
        assert!(
            headsup.contains("Unity editor appears to be open"),
            "must warn when Unity lock files are present: {headsup}"
        );
        Ok(())
    }

    #[test]
    fn project_activation_headsup_warns_when_meta_file_is_orphaned() -> Result<()> {
        let (repo, _tmp) = unity_repo(
            r#"%YAML 1.1
EditorSettings:
  m_SerializationMode: 2
"#,
            Some(
                r#"Library/
Temp/
Logs/
UserSettings/
"#,
            ),
            Some("*.prefab -text merge=unityyamlmerge diff\n"),
            &[("Assets/Prefabs/Dealers.prefab.meta", "guid: test-guid\n")],
        )?;

        let headsup = project_activation_headsup(&repo)?.expect("a warning");
        assert!(
            headsup.contains("meta file"),
            "must warn about orphaned meta files: {headsup}"
        );
        assert!(
            headsup.contains("Assets/Prefabs/Dealers.prefab.meta"),
            "must name the orphaned meta path: {headsup}"
        );
        Ok(())
    }

    fn unity_repo(
        editor_settings: &str,
        gitignore: Option<&str>,
        gitattributes: Option<&str>,
        extra_files: &[(&str, &str)],
    ) -> Result<(gix::Repository, tempfile::TempDir)> {
        let repo_dir = tempfile::tempdir()?;
        write_file(
            repo_dir.path(),
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 6000.0.0f1\n",
        )?;
        write_file(
            repo_dir.path(),
            "Packages/manifest.json",
            "{\n  \"dependencies\": {}\n}\n",
        )?;
        std::fs::create_dir_all(repo_dir.path().join("Assets"))?;
        write_file(
            repo_dir.path(),
            "ProjectSettings/EditorSettings.asset",
            editor_settings,
        )?;
        if let Some(gitignore) = gitignore {
            write_file(repo_dir.path(), ".gitignore", gitignore)?;
        }
        if let Some(gitattributes) = gitattributes {
            write_file(repo_dir.path(), ".gitattributes", gitattributes)?;
        }
        for (path, contents) in extra_files {
            write_file(repo_dir.path(), path, contents)?;
        }

        let repo = gix::init(repo_dir.path())?;
        Ok((repo, repo_dir))
    }

    fn write_file(root: &Path, relative: &str, contents: &str) -> Result<()> {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
        Ok(())
    }
}
