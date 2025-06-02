use std::{fmt::Display, path::PathBuf};

use gitbutler_project::ProjectId;

/// An event for internal use, as merge between [super::file_monitor::Event] and [Action].
#[derive(Debug)]
pub enum InternalEvent {
    // From public action API
    CalculateVirtualBranches(ProjectId),

    // From file monitor
    GitFilesChange(ProjectId, Vec<PathBuf>),
    ProjectFilesChange(ProjectId, Vec<PathBuf>),
    // Triggered on change in the `.git/gitbutler` directory
    GitButlerOplogChange(ProjectId),
    ReloadSignal(ProjectId),
}

impl Display for InternalEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternalEvent::GitFilesChange(project_id, paths) => {
                write!(
                    f,
                    "GitFileChange({}, {})",
                    project_id,
                    comma_separated_paths(paths)
                )
            }
            InternalEvent::GitButlerOplogChange(project_id) => {
                write!(f, "GitButlerOplogChange({})", project_id)
            }
            InternalEvent::ProjectFilesChange(project_id, paths) => {
                write!(
                    f,
                    "ProjectFileChange({}, {})",
                    project_id,
                    comma_separated_paths(paths)
                )
            }
            InternalEvent::CalculateVirtualBranches(pid) => write!(f, "VirtualBranch({})", pid),
            InternalEvent::ReloadSignal(project_id) => write!(f, "ReloadSignal({})", project_id),
        }
    }
}

fn comma_separated_paths(paths: &[PathBuf]) -> String {
    const MAX_LISTING: usize = 5;
    let listing = paths
        .iter()
        .take(MAX_LISTING)
        .filter_map(|path| path.to_str())
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = paths.len().saturating_sub(MAX_LISTING);
    if remaining > 0 {
        format!("{listing} […{remaining} more]")
    } else {
        listing
    }
}

pub const RELOAD_SIGNAL_FILE: &str = "butler_signal";

/// Creates a signal file intended to trigger a reload of the workspace.
pub fn send_reload_signal(gb_dir: &std::path::Path) {
    let file_path = gb_dir.join(RELOAD_SIGNAL_FILE);
    if let Err(e) = std::fs::File::create(&file_path) {
        tracing::warn!("Failed to create reload signal file: {e}");
    }
}
