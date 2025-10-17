use std::{path::PathBuf, sync::Arc};

use crate::Change;
use anyhow::{Context, Result};
use but_core::TreeChange;
use but_hunk_assignment::HunkAssignment;
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_settings::{AppSettings, AppSettingsWithDiskSync};
use gitbutler_command_context::CommandContext;
use gitbutler_filemonitor::InternalEvent;
use gitbutler_operating_modes::operating_mode;
use gitbutler_project::ProjectId;
use tracing::instrument;

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
                let ctx =
                    &mut self.open_command_context(project_id, app_settings.get()?.clone())?;
                self.project_files_change(paths, ctx)
            }

            InternalEvent::GitFilesChange(project_id, paths) => {
                let ctx =
                    &mut self.open_command_context(project_id, app_settings.get()?.clone())?;
                self.git_files_change(paths, ctx)
                    .context("failed to handle git file change event")
            }
        }
    }

    fn emit_app_event(&self, event: Change) -> Result<()> {
        (self.send_event)(event).context("failed to send event")
    }

    fn open_command_context(
        &self,
        project_id: ProjectId,
        app_settings: AppSettings,
    ) -> Result<CommandContext> {
        let project = gitbutler_project::get(project_id).context("failed to get project")?;
        CommandContext::open(&project, app_settings).context("Failed to create a command context")
    }

    #[instrument(skip(self, paths, ctx), fields(paths = paths.len()))]
    fn project_files_change(&self, paths: Vec<PathBuf>, ctx: &mut CommandContext) -> Result<()> {
        let _ = self.emit_worktree_changes(ctx);

        Ok(())
    }

    fn emit_worktree_changes(&self, ctx: &mut CommandContext) -> Result<()> {
        let wt_changes = but_core::diff::worktree_changes(&ctx.gix_repo()?)?;

        let dependencies = hunk_dependencies_for_workspace_changes_by_worktree_dir(
            ctx,
            &ctx.project().path,
            &ctx.project().gb_dir(),
            Some(wt_changes.changes.clone()),
        );

        let (assignments, assignments_error) =
            assignments_and_errors(ctx, wt_changes.changes.clone(), &dependencies)?;

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
        if ctx.app_settings().feature_flags.rules
            && let Ok(update_count) = but_rules::handler::process_workspace_rules(
                ctx,
                &assignments,
                &dependencies.as_ref().ok().cloned(),
            )
            && update_count > 0
        {
            // Getting these again since they were updated
            let (assignments, assignments_error) =
                assignments_and_errors(ctx, wt_changes.changes.clone(), &dependencies)?;
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
            project_id: ctx.project().id,
            changes,
        });
        Ok(())
    }

    pub fn git_files_change(&self, paths: Vec<PathBuf>, ctx: &mut CommandContext) -> Result<()> {
        for path in paths {
            let Some(file_name) = path.to_str() else {
                continue;
            };
            match file_name {
                "FETCH_HEAD" => {
                    self.emit_app_event(Change::GitFetch(ctx.project().id))?;
                }
                "logs/HEAD" => {
                    self.emit_app_event(Change::GitActivity(ctx.project().id))?;
                }
                "index" => {
                    let _ = self.emit_worktree_changes(ctx);
                }
                "HEAD" => {
                    let head_ref = ctx.repo().head().context("failed to get head")?;
                    if let Some(head) = head_ref.name() {
                        self.emit_app_event(Change::GitHead {
                            project_id: ctx.project().id,
                            head: head.to_string(),
                            operating_mode: operating_mode(ctx),
                        })?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn assignments_and_errors(
    ctx: &mut CommandContext,
    tree_changes: Vec<TreeChange>,
    dependencies: &Result<but_hunk_dependency::ui::HunkDependencies>,
) -> Result<(Vec<HunkAssignment>, Option<serde_error::Error>)> {
    let (assignments, assignments_error) = match &dependencies {
        Ok(dependencies) => but_hunk_assignment::assignments_with_fallback(
            ctx,
            false,
            Some(tree_changes),
            Some(dependencies),
        )?,
        Err(e) => (
            vec![],
            Some(anyhow::anyhow!("failed to get hunk dependencies: {}", e)),
        ),
    };
    Ok((
        assignments,
        assignments_error.map(|err| serde_error::Error::new(&*err)),
    ))
}
