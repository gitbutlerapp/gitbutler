use std::{fmt::Display, path};

use crate::{
    analytics, bookmarks, deltas, events,
    projects::ProjectId,
    reader,
    sessions::{self, SessionId},
};

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Tick(ProjectId),
    Flush(ProjectId, sessions::Session),

    FetchGitbutlerData(ProjectId),
    PushGitbutlerData(ProjectId),
    PushProjectToGitbutler(ProjectId),
    FetchProjectData(ProjectId),

    GitFileChange(ProjectId, path::PathBuf),

    ProjectFileChange(ProjectId, path::PathBuf),

    Session(ProjectId, sessions::Session),
    SessionFile((ProjectId, SessionId, path::PathBuf, Option<reader::Content>)),
    SessionDelta((ProjectId, SessionId, path::PathBuf, deltas::Delta)),
    Bookmark(bookmarks::Bookmark),

    IndexAll(ProjectId),

    Emit(events::Event),
    Analytics(analytics::Event),

    VirtualBranch(ProjectId),
    SessionProcessing(ProjectId, path::PathBuf),
}

impl Event {
    pub fn project_id(&self) -> &ProjectId {
        match self {
            Event::Analytics(event) => event.project_id(),
            Event::Emit(event) => event.project_id(),
            Event::Bookmark(bookmark) => &bookmark.project_id,
            Event::Tick(project_id)
            | Event::IndexAll(project_id)
            | Event::FetchGitbutlerData(project_id)
            | Event::FetchProjectData(project_id)
            | Event::Flush(project_id, _)
            | Event::GitFileChange(project_id, _)
            | Event::ProjectFileChange(project_id, _)
            | Event::Session(project_id, _)
            | Event::SessionFile((project_id, _, _, _))
            | Event::SessionDelta((project_id, _, _, _))
            | Event::VirtualBranch(project_id)
            | Event::SessionProcessing(project_id, _)
            | Event::PushGitbutlerData(project_id)
            | Event::PushProjectToGitbutler(project_id) => project_id,
        }
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Analytics(event) => write!(f, "Analytics({})", event),
            Event::Emit(event) => write!(f, "Emit({})", event.name()),
            Event::Tick(project_id) => write!(f, "Tick({})", project_id,),
            Event::FetchGitbutlerData(pid) => {
                write!(f, "FetchGitbutlerData({})", pid,)
            }
            Event::FetchProjectData(pid) => {
                write!(f, "FetchProjectData({})", pid,)
            }
            Event::Flush(project_id, session) => write!(f, "Flush({}, {})", project_id, session.id),
            Event::GitFileChange(project_id, path) => {
                write!(f, "GitFileChange({}, {})", project_id, path.display())
            }
            Event::ProjectFileChange(project_id, path) => {
                write!(f, "ProjectFileChange({}, {})", project_id, path.display())
            }
            Event::Session(pid, session) => write!(f, "Session({}, {})", pid, session.id),
            Event::Bookmark(b) => write!(f, "Bookmark({})", b.project_id),
            Event::SessionFile((pid, session_id, path, _)) => {
                write!(f, "File({}, {}, {})", pid, session_id, path.display())
            }
            Event::SessionDelta((pid, session_id, path, delta)) => {
                write!(
                    f,
                    "Deltas({}, {}, {}, {})",
                    pid,
                    session_id,
                    path.display(),
                    delta.timestamp_ms
                )
            }
            Event::VirtualBranch(pid) => write!(f, "VirtualBranch({})", pid),
            Event::SessionProcessing(project_id, path) => {
                write!(f, "SessionProcessing({}, {})", project_id, path.display())
            }
            Event::PushGitbutlerData(pid) => write!(f, "PushGitbutlerData({})", pid),
            Event::PushProjectToGitbutler(pid) => write!(f, "PushProjectToGitbutler({})", pid),
            Event::IndexAll(pid) => write!(f, "IndexAll({})", pid),
        }
    }
}
