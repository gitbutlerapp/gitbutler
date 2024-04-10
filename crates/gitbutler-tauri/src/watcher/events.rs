use std::{fmt::Display, path};

use gitbutler_core::{
    deltas,
    projects::ProjectId,
    reader,
    sessions::{self, SessionId},
};

use crate::{analytics, events};

/// An event for internal use, while the event queue exists.
/// NOTE: Must be public for integration tests.
/// TODO(ST): make this obsolete
#[derive(Debug, PartialEq, Clone)]
pub enum PrivateEvent {
    Flush(ProjectId, sessions::Session),

    FetchGitbutlerData(ProjectId),
    PushGitbutlerData(ProjectId),
    PushProjectToGitbutler(ProjectId),

    GitFileChange(ProjectId, path::PathBuf),

    ProjectFileChange(ProjectId, path::PathBuf),

    Session(ProjectId, sessions::Session),
    SessionFile((ProjectId, SessionId, path::PathBuf, Option<reader::Content>)),
    SessionDelta((ProjectId, SessionId, path::PathBuf, deltas::Delta)),

    IndexAll(ProjectId),

    Emit(events::Event),
    Analytics(analytics::Event),

    CalculateVirtualBranches(ProjectId),
    CalculateDeltas(ProjectId, path::PathBuf),

    FilterIgnoredFiles(ProjectId, path::PathBuf),
}

/// This type captures all operations that can be fed into a watcher that runs in the background.
#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Flush(ProjectId, sessions::Session),
    CalculateVirtualBranches(ProjectId),
    FetchGitbutlerData(ProjectId),
    PushGitbutlerData(ProjectId),
}

impl Event {
    pub fn project_id(&self) -> &ProjectId {
        match self {
            Event::FetchGitbutlerData(project_id)
            | Event::Flush(project_id, _)
            | Event::CalculateVirtualBranches(project_id)
            | Event::PushGitbutlerData(project_id) => project_id,
        }
    }
}

impl From<Event> for PrivateEvent {
    fn from(value: Event) -> Self {
        match value {
            Event::Flush(a, b) => PrivateEvent::Flush(a, b),
            Event::CalculateVirtualBranches(v) => PrivateEvent::CalculateVirtualBranches(v),
            Event::FetchGitbutlerData(v) => PrivateEvent::FetchGitbutlerData(v),
            Event::PushGitbutlerData(v) => PrivateEvent::PushGitbutlerData(v),
        }
    }
}

impl PrivateEvent {
    pub fn project_id(&self) -> ProjectId {
        match self {
            PrivateEvent::Analytics(event) => event.project_id(),
            PrivateEvent::Emit(event) => event.project_id(),
            PrivateEvent::IndexAll(project_id)
            | PrivateEvent::FetchGitbutlerData(project_id)
            | PrivateEvent::Flush(project_id, _)
            | PrivateEvent::GitFileChange(project_id, _)
            | PrivateEvent::ProjectFileChange(project_id, _)
            | PrivateEvent::Session(project_id, _)
            | PrivateEvent::SessionFile((project_id, _, _, _))
            | PrivateEvent::SessionDelta((project_id, _, _, _))
            | PrivateEvent::CalculateVirtualBranches(project_id)
            | PrivateEvent::CalculateDeltas(project_id, _)
            | PrivateEvent::FilterIgnoredFiles(project_id, _)
            | PrivateEvent::PushGitbutlerData(project_id)
            | PrivateEvent::PushProjectToGitbutler(project_id) => *project_id,
        }
    }
}

impl Display for PrivateEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrivateEvent::Analytics(event) => write!(f, "Analytics({})", event),
            PrivateEvent::Emit(event) => write!(f, "Emit({})", event.name()),
            PrivateEvent::FetchGitbutlerData(pid) => {
                write!(f, "FetchGitbutlerData({})", pid,)
            }
            PrivateEvent::Flush(project_id, session) => {
                write!(f, "Flush({}, {})", project_id, session.id)
            }
            PrivateEvent::GitFileChange(project_id, path) => {
                write!(f, "GitFileChange({}, {})", project_id, path.display())
            }
            PrivateEvent::ProjectFileChange(project_id, path) => {
                write!(f, "ProjectFileChange({}, {})", project_id, path.display())
            }
            PrivateEvent::Session(pid, session) => write!(f, "Session({}, {})", pid, session.id),
            PrivateEvent::SessionFile((pid, session_id, path, _)) => {
                write!(f, "File({}, {}, {})", pid, session_id, path.display())
            }
            PrivateEvent::SessionDelta((pid, session_id, path, delta)) => {
                write!(
                    f,
                    "Deltas({}, {}, {}, {})",
                    pid,
                    session_id,
                    path.display(),
                    delta.timestamp_ms
                )
            }
            PrivateEvent::CalculateVirtualBranches(pid) => write!(f, "VirtualBranch({})", pid),
            PrivateEvent::CalculateDeltas(project_id, path) => {
                write!(f, "SessionProcessing({}, {})", project_id, path.display())
            }
            PrivateEvent::FilterIgnoredFiles(project_id, path) => {
                write!(f, "FilterIgnoredFiles({}, {})", project_id, path.display())
            }
            PrivateEvent::PushGitbutlerData(pid) => write!(f, "PushGitbutlerData({})", pid),
            PrivateEvent::PushProjectToGitbutler(pid) => {
                write!(f, "PushProjectToGitbutler({})", pid)
            }
            PrivateEvent::IndexAll(pid) => write!(f, "IndexAll({})", pid),
        }
    }
}
