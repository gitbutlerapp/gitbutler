use std::{fmt::Display, path};

use crate::{
    analytics, deltas, events,
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

    IndexAll(ProjectId),

    Emit(events::Event),
    Analytics(analytics::Event),

    CalculateVirtualBranches(ProjectId),
    CalculateDeltas(ProjectId, path::PathBuf),
}

impl Event {
    /// Unique key will be used to make sure that only one event of a given type is running at a time.
    pub fn unique_key(&self) -> Option<String> {
        match self {
            Event::Tick(project_id) => Some(format!("tick::{}", project_id)),
            Event::Flush(project_id, _) => Some(format!("flush::{}", project_id)),
            Event::FetchGitbutlerData(project_id) => Some(format!("fetch-gb::{}", project_id)),
            Event::PushGitbutlerData(project_id) => Some(format!("push-gb::{}", project_id)),
            Event::PushProjectToGitbutler(project_id) => {
                Some(format!("push-project::{}", project_id))
            }
            Event::FetchProjectData(project_id) => Some(format!("fetch-project::{}", project_id)),
            Event::GitFileChange(project_id, path) => {
                Some(format!("git-file::{}-{}", project_id, path.display()))
            }
            Event::ProjectFileChange(project_id, path) => {
                Some(format!("project-file::{}-{}", project_id, path.display()))
            }
            Event::Session(project_id, session) => {
                Some(format!("session::{}-{}", project_id, session.id))
            }
            Event::SessionFile((project_id, session_id, path, _)) => Some(format!(
                "session-file::{}-{}-{}",
                project_id,
                session_id,
                path.display()
            )),
            Event::SessionDelta((project_id, session_id, path, delta)) => Some(format!(
                "session-delta::{}-{}-{}-{}",
                project_id,
                session_id,
                path.display(),
                delta.timestamp_ms
            )),
            Event::IndexAll(project_id) => Some(format!("index-all::{}", project_id)),
            Event::Emit(_event) => None,
            Event::Analytics(_event) => None,
            Event::CalculateVirtualBranches(project_id) => {
                Some(format!("calculate-virtual-branches::{}", project_id))
            }
            Event::CalculateDeltas(project_id, path) => Some(format!(
                "calculate-deltas::{}-{}",
                project_id,
                path.display()
            )),
        }
    }

    pub fn project_id(&self) -> &ProjectId {
        match self {
            Event::Analytics(event) => event.project_id(),
            Event::Emit(event) => event.project_id(),
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
            | Event::CalculateVirtualBranches(project_id)
            | Event::CalculateDeltas(project_id, _)
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
            Event::CalculateVirtualBranches(pid) => write!(f, "VirtualBranch({})", pid),
            Event::CalculateDeltas(project_id, path) => {
                write!(f, "SessionProcessing({}, {})", project_id, path.display())
            }
            Event::PushGitbutlerData(pid) => write!(f, "PushGitbutlerData({})", pid),
            Event::PushProjectToGitbutler(pid) => write!(f, "PushProjectToGitbutler({})", pid),
            Event::IndexAll(pid) => write!(f, "IndexAll({})", pid),
        }
    }
}
