use std::{path::PathBuf, sync::Arc};

use super::{events, Change};
use anyhow::{Context, Result};
use gitbutler_branch_actions::{RemoteBranchFile, VirtualBranchActions, VirtualBranches};
use gitbutler_command_context::CommandContext;
use gitbutler_diff::DiffByPathMap;
use gitbutler_error::error::Marker;
use gitbutler_operating_modes::{
    in_open_workspace_mode, in_outside_workspace_mode, operating_mode,
};
use gitbutler_oplog::{
    entry::{OperationKind, SnapshotDetails},
    OplogExt,
};
use gitbutler_project::ProjectId;
use gitbutler_project::{self as projects, Project};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_sync::cloud::{push_oplog, push_repo};
use gitbutler_user as users;
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
    projects: projects::Controller,
    users: users::Controller,

    /// A function to send events - decoupled from app-handle for testing purposes.
    #[allow(clippy::type_complexity)]
    send_event: Arc<dyn Fn(Change) -> Result<()> + Send + Sync + 'static>,
}

impl Handler {
    /// A constructor whose primary use is the test-suite.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        projects: projects::Controller,
        users: users::Controller,
        send_event: impl Fn(Change) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Handler {
            projects,
            users,
            send_event: Arc::new(send_event),
        }
    }

    /// Handle the events that come in from the filesystem, or the public API.
    #[instrument(skip(self), fields(event = %event), err(Debug))]
    pub(super) fn handle(&self, event: events::InternalEvent) -> Result<()> {
        match event {
            events::InternalEvent::ProjectFilesChange(project_id, path) => {
                self.recalculate_everything(path, project_id)
            }

            events::InternalEvent::GitFilesChange(project_id, paths) => self
                .git_files_change(paths, project_id)
                .context("failed to handle git file change event"),

            events::InternalEvent::GitButlerOplogChange(project_id) => self
                .gitbutler_oplog_change(project_id)
                .context("failed to handle gitbutler oplog change event"),

            // This is only produced at the end of mutating Tauri commands to trigger a fresh state being served to the UI.
            events::InternalEvent::CalculateVirtualBranches(project_id) => self
                .calculate_virtual_branches(project_id, None)
                .context("failed to handle virtual branch event"),
        }
    }
}

impl Handler {
    fn emit_app_event(&self, event: Change) -> Result<()> {
        (self.send_event)(event).context("failed to send event")
    }

    fn open_command_context(&self, project_id: ProjectId) -> Result<CommandContext> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        CommandContext::open(&project).context("Failed to create a command context")
    }

    #[instrument(skip(self, project_id, worktree_changes))]
    fn calculate_virtual_branches(
        &self,
        project_id: ProjectId,
        worktree_changes: Option<DiffByPathMap>,
    ) -> Result<()> {
        let ctx = self.open_command_context(project_id)?;
        // Skip if we're not on the open workspace mode
        if !in_open_workspace_mode(&ctx) {
            return Ok(());
        }

        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        match VirtualBranchActions.list_virtual_branches_cached(&project, worktree_changes) {
            Ok((branches, skipped_files)) => self.emit_app_event(Change::VirtualBranches {
                project_id: project.id,
                virtual_branches: VirtualBranches {
                    branches,
                    skipped_files,
                },
            }),
            Err(err)
                if matches!(
                    err.downcast_ref::<Marker>(),
                    Some(Marker::VerificationFailure)
                ) =>
            {
                Ok(())
            }
            Err(err) => Err(err.context("failed to list virtual branches")),
        }
    }

    #[instrument(skip(self, paths, project_id), fields(paths = paths.len()))]
    fn recalculate_everything(&self, paths: Vec<PathBuf>, project_id: ProjectId) -> Result<()> {
        let ctx = self.open_command_context(project_id)?;

        let worktree_changes = self.emit_uncommited_files(ctx.project()).ok();

        if in_open_workspace_mode(&ctx) {
            self.maybe_create_snapshot(project_id).ok();
            self.calculate_virtual_branches(project_id, worktree_changes)?;
        }

        Ok(())
    }

    /// Try to emit uncommited files. Swollow errors if they arrise.
    fn emit_uncommited_files(&self, project: &Project) -> Result<DiffByPathMap> {
        let files = VirtualBranchActions.get_uncommited_files_reusable(project)?;

        let _ = self.emit_app_event(Change::UncommitedFiles {
            project_id: project.id,
            files: files
                .clone()
                .into_iter()
                .map(|(path, file)| {
                    let binary = file.hunks.iter().any(|h| h.binary);
                    RemoteBranchFile {
                        path,
                        hunks: file.hunks,
                        binary,
                    }
                })
                .collect(),
        });
        Ok(files)
    }

    fn maybe_create_snapshot(&self, project_id: ProjectId) -> anyhow::Result<()> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        if project
            .should_auto_snapshot(std::time::Duration::from_secs(300))
            .unwrap_or_default()
        {
            let mut guard = project.exclusive_worktree_access();
            project.create_snapshot(
                SnapshotDetails::new(OperationKind::FileChanges),
                guard.write_permission(),
            )?;
        }
        Ok(())
    }

    pub fn git_files_change(&self, paths: Vec<PathBuf>, project_id: ProjectId) -> Result<()> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;

        for path in paths {
            let Some(file_name) = path.to_str() else {
                continue;
            };
            match file_name {
                "FETCH_HEAD" => {
                    self.emit_app_event(Change::GitFetch(project_id))?;
                }
                "logs/HEAD" => {
                    self.emit_app_event(Change::GitActivity(project.id))?;
                }
                "HEAD" => {
                    let ctx = CommandContext::open(&project)
                        .context("Failed to create a command context")?;

                    // If the user has left gitbutler/workspace, we want to delete the reference.
                    // TODO: why do we want to do this?
                    if in_outside_workspace_mode(&ctx) {
                        let mut workspace_reference = ctx.repository().find_reference(
                            &Refname::from(LocalRefname::new("gitbutler/workspace", None))
                                .to_string(),
                        )?;
                        workspace_reference.delete()?;
                    }

                    let head_ref = ctx.repository().head().context("failed to get head")?;
                    if let Some(head) = head_ref.name() {
                        self.emit_app_event(Change::GitHead {
                            project_id,
                            head: head.to_string(),
                            operating_mode: operating_mode(&ctx),
                        })?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Invoked whenever there's a new oplog entry.
    /// If synchronizing with GitButler's servers is enabled it will push Oplog refs
    fn gitbutler_oplog_change(&self, project_id: ProjectId) -> Result<()> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;

        if let Some(user) = self.users.get_user()? {
            let ctx = CommandContext::open(&project)?;
            if project.oplog_sync_enabled() {
                push_oplog(&ctx, &user)?;
            }
            if project.code_sync_enabled() {
                push_repo(&ctx, &user, &self.projects)?;
            }
        }
        Ok(())
    }
}
