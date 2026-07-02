use std::{path::PathBuf, sync::Arc};

use anyhow::{Context as _, Result};
use but_core::{TreeChange, sync::RepoExclusive};
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
use but_db::HunkAssignmentsHandleMut;
use but_hunk_assignment::HunkAssignment;
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_project_handle::{REFRESH_SENTINEL_PATH, process_sentinel_token};
use but_settings::{AppSettings, AppSettingsWithDiskSync};
use gitbutler_filemonitor::{
    FETCH_HEAD, HEAD, HEAD_ACTIVITY, INDEX, InternalEvent, LOCAL_REFS_DIR, REMOTE_REFS_DIR,
};
use gitbutler_operating_modes::operating_mode;
use gix::bstr::ByteSlice as _;
use tracing::instrument;

use crate::Change;

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

        let wt_changes = but_core::diff::worktree_changes(&repo)?;

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

        let changes = but_hunk_assignment::WorktreeChanges {
            worktree_changes: wt_changes.clone().into(),
            assignments: assignments.clone(),
            assignments_error: assignments_error.clone(),
            dependencies: dependencies.as_ref().ok().cloned(),
            dependencies_error: dependencies
                .as_ref()
                .err()
                .map(|err| serde_error::Error::new(&**err)),
        };
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
        let mut saw_workspace_activity = false;
        for path in paths {
            let Some(file_name) = path.to_str() else {
                continue;
            };
            match file_name {
                FETCH_HEAD => {
                    self.emit_app_event(Change::GitFetch(project_id.clone()))?;
                }
                // Watch all local branches. HEAD ref changes affect the current
                // commit, while other branch refs can change workspace topology.
                _ if file_name.starts_with(LOCAL_REFS_DIR) && !file_name.ends_with(".lock") => {
                    if file_name == head_ref_name {
                        self.emit_app_event(Change::GitActivity {
                            project_id: project_id.clone(),
                            head_sha: head_sha.clone(),
                        })?;
                    } else {
                        saw_workspace_activity = true;
                    }
                }
                // Remote-ref updates and the refresh sentinel both mean "re-read
                // workspace state"; coalesce into one emission after the loop.
                _ if file_name.starts_with(REMOTE_REFS_DIR) => {
                    saw_workspace_activity = true;
                }
                REFRESH_SENTINEL_PATH => {
                    // Skip the echo of our own write (its pid is already handled
                    // in-process); a different pid, or a read error, means refresh.
                    let sentinel = ctx.repo.get()?.path().join(file_name);
                    let wrote_by_us = std::fs::read_to_string(sentinel)
                        .map(|content| content.trim() == process_sentinel_token())
                        .unwrap_or(false);
                    if !wrote_by_us {
                        saw_workspace_activity = true;
                    }
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
                    let head = head_ref
                        .referent_name()
                        .map(|name| name.as_bstr().to_str_lossy().into_owned());
                    self.emit_app_event(Change::GitHead {
                        project_id: project_id.clone(),
                        head,
                        operating_mode: operating_mode(ctx, perm.read_permission())?,
                    })?;
                }
                _ => { /* Ignore other files */ }
            }
        }
        if saw_workspace_activity {
            self.emit_app_event(Change::WorkspaceActivity { project_id })?;
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
