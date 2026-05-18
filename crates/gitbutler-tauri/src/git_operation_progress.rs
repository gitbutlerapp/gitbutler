use std::time::Instant;

use but_ctx::ProjectHandleOrLegacyProjectId;
use tauri::{Emitter, EventTarget, Manager, Window};

/// Unified progress event emitted while GitButler performs long-running Git operations.
pub const GIT_OPERATION_PROGRESS_EVENT: &str = "git_operation_progress";

/// A user-facing progress update for Git operations that can be slow in large repositories.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitOperationProgress {
    /// Stable operation identifier.
    pub operation: String,
    /// Stable phase identifier within the operation.
    pub phase: String,
    /// Human-readable phase label.
    pub phase_label: String,
    /// Elapsed operation time in milliseconds.
    pub elapsed_ms: u128,
    /// Current path involved in the operation, if known.
    pub path: Option<String>,
    /// Current path index, if known.
    pub current_path: Option<u64>,
    /// Total path count, if known.
    pub total_paths: Option<u64>,
    /// Completed byte count, if known.
    pub bytes_done: Option<u64>,
    /// Total byte count, if known.
    pub bytes_total: Option<u64>,
    /// Transfer speed in bytes per second, if known.
    pub bytes_per_second: Option<f64>,
    /// Git LFS transfer direction, if the phase is driven by LFS.
    pub lfs_direction: Option<String>,
    /// Additional user-facing detail.
    pub detail: Option<String>,
}

/// Emits progress events for a project-scoped operation.
pub struct GitOperationProgressEmitter {
    window: Window,
    event_name: String,
    operation: String,
    started_at: Instant,
}

impl GitOperationProgressEmitter {
    /// Create an emitter targeting the current window and project.
    pub fn new(
        window: &Window,
        project_id: &ProjectHandleOrLegacyProjectId,
        operation: impl Into<String>,
    ) -> GitOperationProgressEmitter {
        GitOperationProgressEmitter {
            window: window.clone(),
            event_name: format!("project://{project_id}/{GIT_OPERATION_PROGRESS_EVENT}"),
            operation: operation.into(),
            started_at: Instant::now(),
        }
    }

    /// Emit a simple phase transition.
    pub fn phase(
        &self,
        phase: impl Into<String>,
        phase_label: impl Into<String>,
        detail: Option<String>,
    ) {
        self.emit(GitOperationProgress {
            operation: self.operation.clone(),
            phase: phase.into(),
            phase_label: phase_label.into(),
            elapsed_ms: self.started_at.elapsed().as_millis(),
            path: None,
            current_path: None,
            total_paths: None,
            bytes_done: None,
            bytes_total: None,
            bytes_per_second: None,
            lfs_direction: None,
            detail: detail.into(),
        });
    }

    /// Return the fully qualified Tauri event name for this emitter.
    pub fn event_name(&self) -> &str {
        &self.event_name
    }

    /// Return the stable operation identifier.
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Emit a fully specified progress payload.
    pub fn emit(&self, mut progress: GitOperationProgress) {
        progress.operation = self.operation.clone();
        progress.elapsed_ms = self.started_at.elapsed().as_millis();
        let _ = self.window.app_handle().emit_to(
            EventTarget::window(self.window.label()),
            &self.event_name,
            progress,
        );
    }
}
