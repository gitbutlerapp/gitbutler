use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context as _, Result};
use but_core::{TreeChange, sync::RepoExclusive};
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use but_db::HunkAssignmentsHandleMut;
use but_hunk_assignment::HunkAssignment;
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_settings::{AppSettings, AppSettingsWithDiskSync};
use gitbutler_filemonitor::{
    FETCH_HEAD, HEAD, HEAD_ACTIVITY, INDEX, InternalEvent, LOCAL_REFS_DIR, REMOTE_REFS_DIR,
};
use gitbutler_operating_modes::operating_mode;
use gix::{bstr::ByteSlice as _, config::AsKey as _};
use tracing::instrument;

use crate::Change;

const LOCAL_IGNORED_PATH_KEY: &str = "gitbutler.localignoredpath";

/// A type that contains enough state to make decisions based on changes in the filesystem, which themselves
/// may trigger [Changes](Change)
// NOTE: This is `Clone` as each incoming event is spawned onto a thread for processing.
#[derive(Clone)]
pub struct Handler {
    // The following fields our currently required state as we are running in the background
    // and access it as filesystem events are processed. It's still to be decided how granular it
    // should be, and I can imagine having a top-level `app` handle that keeps the application state of
    // the tauri app, assuming that such application would not be `Send + Sync` everywhere and thus would
    // need extra protection.
    /// A function to send events - decoupled from app-handle for testing purposes.
    send_event: Arc<dyn Fn(Change) -> Result<()> + Send + Sync + 'static>,
}

impl Handler {
    /// A constructor whose primary use is the test-suite.
    pub fn new(send_event: impl Fn(Change) -> Result<()> + Send + Sync + 'static) -> Self {
        Handler {
            send_event: Arc::new(send_event),
        }
    }

    /// Handle the events that come in from the filesystem, or the public API.
    #[instrument(skip(self, app_settings), fields(event = %event), err(Debug))]
    pub(super) fn handle(
        &self,
        event: InternalEvent,
        app_settings: AppSettingsWithDiskSync,
    ) -> Result<()> {
        match event {
            InternalEvent::ProjectFilesChange(project_id, paths) => {
                let mut ctx =
                    self.open_command_context(project_id.clone(), app_settings.get()?.clone())?;
                let mut guard = ctx.exclusive_worktree_access();
                self.project_files_change(project_id, paths, &mut ctx, guard.write_permission())
            }

            InternalEvent::GitFilesChange(project_id, paths) => {
                let mut ctx =
                    self.open_command_context(project_id.clone(), app_settings.get()?.clone())?;
                let mut guard = ctx.exclusive_worktree_access();
                self.git_files_change(project_id, paths, &mut ctx, guard.write_permission())
                    .context("failed to handle git file change event")
            }
        }
    }

    fn emit_app_event(&self, event: Change) -> Result<()> {
        (self.send_event)(event).context("failed to send event")
    }

    fn open_command_context(
        &self,
        project_id: ProjectHandleOrLegacyProjectId,
        app_settings: AppSettings,
    ) -> Result<Context> {
        let mut ctx: Context = project_id.try_into()?;
        ctx.settings = app_settings;
        Ok(ctx)
    }

    #[instrument(skip_all, fields(paths = paths.len()))]
    fn project_files_change(
        &self,
        project_id: ProjectHandleOrLegacyProjectId,
        paths: Vec<PathBuf>,
        ctx: &mut Context,
        perm: &mut RepoExclusive,
    ) -> Result<()> {
        let repo = ctx.repo.get()?;
        let locally_ignored_paths = list_local_ignored_paths(&repo)?;
        if !locally_ignored_paths.is_empty()
            && paths
                .iter()
                .all(|path| path_is_locally_ignored(path, &locally_ignored_paths))
        {
            return Ok(());
        }
        drop(repo);

        _ = self.emit_worktree_changes(project_id, ctx, perm);
        Ok(())
    }

    fn emit_worktree_changes(
        &self,
        project_id: ProjectHandleOrLegacyProjectId,
        ctx: &mut Context,
        perm: &mut RepoExclusive,
    ) -> Result<()> {
        let context_lines = ctx.settings.context_lines;
        let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;

        let locally_ignored_paths = list_local_ignored_paths(&repo)?;
        let mut wt_changes = but_core::diff::worktree_changes(&repo)?;
        filter_locally_ignored_worktree_changes(&mut wt_changes, &locally_ignored_paths);

        let dependencies = hunk_dependencies_for_workspace_changes_by_worktree_dir(
            &repo,
            &ws,
            Some(wt_changes.changes.clone()),
        );

        let (assignments, assignments_error) = assignments_and_errors(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            wt_changes.changes.clone(),
            context_lines,
        )?;

        let mut changes = but_hunk_assignment::WorktreeChanges {
            worktree_changes: wt_changes.clone().into(),
            assignments: assignments.clone(),
            assignments_error: assignments_error.clone(),
            dependencies: dependencies.as_ref().ok().cloned(),
            dependencies_error: dependencies
                .as_ref()
                .err()
                .map(|err| serde_error::Error::new(&**err)),
        };
        drop((repo, ws, db));
        if let Ok(update_count) =
            but_rules::handler::process_workspace_rules(ctx, &assignments, perm)
            && update_count > 0
        {
            let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm.read_permission())?;
            // Getting these again since they were updated
            let (assignments, assignments_error) = assignments_and_errors(
                db.hunk_assignments_mut()?,
                &repo,
                &ws,
                wt_changes.changes.clone(),
                context_lines,
            )?;
            changes = but_hunk_assignment::WorktreeChanges {
                worktree_changes: wt_changes.into(),
                assignments,
                assignments_error: assignments_error.clone(),
                dependencies: dependencies.as_ref().ok().cloned(),
                dependencies_error: dependencies
                    .as_ref()
                    .err()
                    .map(|err| serde_error::Error::new(&**err)),
            };
        }
        let _ = self.emit_app_event(Change::WorktreeChanges {
            project_id,
            changes,
        });
        Ok(())
    }

    pub fn git_files_change(
        &self,
        project_id: ProjectHandleOrLegacyProjectId,
        paths: Vec<PathBuf>,
        ctx: &mut Context,
        perm: &mut RepoExclusive,
    ) -> Result<()> {
        let (head_ref_name, head_sha) = head_info(ctx)?;
        let mut saw_remote_activity = false;
        for path in paths {
            let Some(file_name) = path.to_str() else {
                continue;
            };
            match file_name {
                FETCH_HEAD => {
                    self.emit_app_event(Change::GitFetch(project_id.clone()))?;
                }
                // Watch all local branches. Only emit activity if the HEAD points to that ref.
                _ if file_name.starts_with(LOCAL_REFS_DIR) && file_name == head_ref_name => {
                    self.emit_app_event(Change::GitActivity {
                        project_id: project_id.clone(),
                        head_sha: head_sha.clone(),
                    })?;
                }
                // Track remote ref changes to emit a single event after the loop.
                _ if file_name.starts_with(REMOTE_REFS_DIR) => {
                    saw_remote_activity = true;
                }
                HEAD_ACTIVITY => {
                    self.emit_app_event(Change::GitActivity {
                        project_id: project_id.clone(),
                        head_sha: head_sha.clone(),
                    })?;
                }
                INDEX => {
                    let _ = self.emit_worktree_changes(project_id.clone(), ctx, perm);
                }
                HEAD => {
                    let repo = ctx.repo.get()?;
                    let head_ref = repo.head().context("failed to get head")?;
                    if let Some(head) = head_ref.referent_name() {
                        self.emit_app_event(Change::GitHead {
                            project_id: project_id.clone(),
                            head: head.as_bstr().to_str_lossy().into_owned(),
                            operating_mode: operating_mode(ctx, perm.read_permission())?,
                        })?;
                    }
                }
                _ => { /* Ignore other files */ }
            }
        }
        if saw_remote_activity {
            self.emit_app_event(Change::GitRemoteActivity { project_id })?;
        }
        Ok(())
    }
}

fn head_info(ctx: &mut Context) -> Result<(String, String)> {
    let repo = &*ctx.repo.get()?;
    let mut head_ref = repo.head().context("failed to get head")?;
    let head_name = head_ref
        .referent_name()
        .map(|name| name.as_bstr().to_str_lossy().into_owned())
        .unwrap_or_default();
    let head_sha = head_ref
        .peel_to_commit()
        .context("failed to get head commit")?
        .id
        .to_string();
    Ok((head_name, head_sha))
}

fn assignments_and_errors(
    db: HunkAssignmentsHandleMut,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    tree_changes: Vec<TreeChange>,
    context_lines: u32,
) -> Result<(Vec<HunkAssignment>, Option<serde_error::Error>)> {
    let (assignments, assignments_error) = {
        but_hunk_assignment::assignments_with_fallback(
            db,
            repo,
            workspace,
            Some(tree_changes),
            context_lines,
        )?
    };
    Ok((
        assignments,
        assignments_error.map(|err| serde_error::Error::new(&*err)),
    ))
}

fn list_local_ignored_paths(repo: &gix::Repository) -> Result<Vec<String>> {
    let path = repo.common_dir().join("config");
    if !path.exists() {
        return Ok(vec![]);
    }

    let config = gix::config::File::from_path_no_includes(path.clone(), gix::config::Source::Local)
        .with_context(|| format!("failed to open Local git config at {}", path.display()))?;
    let key = LOCAL_IGNORED_PATH_KEY
        .try_as_key()
        .with_context(|| format!("invalid git config key: {LOCAL_IGNORED_PATH_KEY}"))?;

    let mut paths = BTreeSet::new();
    if let Ok(values) = config.raw_values_by(key.section_name, key.subsection_name, key.value_name)
    {
        for value in values {
            if let Some(normalized) =
                normalize_local_ignore_path_string(&String::from_utf8_lossy(value.as_ref()))
            {
                paths.insert(normalized);
            }
        }
    }
    Ok(paths.into_iter().collect())
}

fn filter_locally_ignored_worktree_changes(
    changes: &mut but_core::WorktreeChanges,
    locally_ignored_paths: &[String],
) {
    if locally_ignored_paths.is_empty() {
        return;
    }

    changes.changes.retain(|change| {
        !path_bytes_are_locally_ignored(change.path.as_slice(), locally_ignored_paths)
    });
    changes.ignored_changes.retain(|change| {
        !path_bytes_are_locally_ignored(change.path.as_slice(), locally_ignored_paths)
    });
    changes.index_conflicts.retain(|(path, _)| {
        !path_bytes_are_locally_ignored(path.as_slice(), locally_ignored_paths)
    });
}

fn path_bytes_are_locally_ignored(path: &[u8], ignored_paths: &[String]) -> bool {
    let Some(normalized_path) = normalize_local_ignore_path_string(&String::from_utf8_lossy(path))
    else {
        return false;
    };
    normalized_path_is_locally_ignored(&normalized_path, ignored_paths)
}

fn path_is_locally_ignored(path: &Path, ignored_paths: &[String]) -> bool {
    let Some(normalized_path) = normalize_local_ignore_path(path) else {
        return false;
    };
    normalized_path_is_locally_ignored(&normalized_path, ignored_paths)
}

fn normalized_path_is_locally_ignored(normalized_path: &str, ignored_paths: &[String]) -> bool {
    ignored_paths.iter().any(|ignored_path| {
        normalized_path == ignored_path || normalized_path.starts_with(&format!("{ignored_path}/"))
    })
}

fn normalize_local_ignore_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                for nested in part
                    .split(['/', '\\'])
                    .filter(|segment| !segment.is_empty())
                {
                    parts.push(nested.to_owned());
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn normalize_local_ignore_path_string(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            _ => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}
