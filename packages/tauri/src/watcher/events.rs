use std::{fmt::Display, path, time};

use crate::{analytics, bookmarks, deltas, events, reader, sessions};

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Tick(String, time::SystemTime),
    Flush(String, sessions::Session),

    FetchGitbutlerData(String, time::SystemTime),
    PushGitbutlerData(String),
    FetchProjectData(String, time::SystemTime),

    GitFileChange(String, path::PathBuf),

    ProjectFileChange(String, path::PathBuf),

    Session(String, sessions::Session),
    SessionFile((String, String, path::PathBuf, Option<reader::Content>)),
    SessionDelta((String, String, path::PathBuf, deltas::Delta)),
    Bookmark(bookmarks::Bookmark),

    IndexAll(String),

    Emit(events::Event),
    Analytics(analytics::Event),
}

impl Event {
    pub fn project_id(&self) -> &str {
        match self {
            Event::Analytics(event) => event.project_id(),
            Event::Emit(event) => event.project_id(),
            Event::IndexAll(project_id) => project_id,
            Event::Tick(project_id, _) => project_id,
            Event::FetchGitbutlerData(project_id, _) => project_id,
            Event::FetchProjectData(project_id, _) => project_id,
            Event::Flush(project_id, _) => project_id,
            Event::GitFileChange(project_id, _) => project_id,
            Event::ProjectFileChange(project_id, _) => project_id,
            Event::Session(project_id, _) => project_id,
            Event::Bookmark(bookmark) => &bookmark.project_id,
            Event::SessionFile((project_id, _, _, _)) => project_id,
            Event::SessionDelta((project_id, _, _, _)) => project_id,
            Event::PushGitbutlerData(project_id) => project_id,
        }
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Analytics(event) => write!(f, "Analytics({:?})", event),
            Event::Emit(event) => write!(f, "Emit({})", event.name()),
            Event::IndexAll(project_id) => write!(f, "IndexAll({})", project_id),
            Event::Tick(project_id, ts) => write!(
                f,
                "Tick({}, {})",
                project_id,
                ts.duration_since(time::UNIX_EPOCH).unwrap().as_millis()
            ),
            Event::FetchGitbutlerData(pid, ts) => {
                write!(
                    f,
                    "FetchGitbutlerData({}, {})",
                    pid,
                    ts.duration_since(time::UNIX_EPOCH).unwrap().as_millis()
                )
            }
            Event::FetchProjectData(pid, ts) => {
                write!(
                    f,
                    "FetchProjectData({}, {})",
                    pid,
                    ts.duration_since(time::UNIX_EPOCH).unwrap().as_millis()
                )
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
            Event::PushGitbutlerData(pid) => write!(f, "PushGitbutlerData({})", pid),
        }
    }
}
