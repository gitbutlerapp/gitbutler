use std::{fmt::Display, path, time};

use crate::{bookmarks, deltas, events, sessions};

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Tick(String, time::SystemTime),
    Flush(String, sessions::Session),

    FetchGitbutlerData(String, time::SystemTime),

    GitFileChange(String, path::PathBuf),

    ProjectFileChange(String, path::PathBuf),

    Session(String, sessions::Session),
    SessionFile((String, String, path::PathBuf, String)),
    SessionDelta((String, String, path::PathBuf, deltas::Delta)),
    Bookmark(bookmarks::Bookmark),

    IndexAll(String),

    Emit(events::Event),
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
            Event::Flush(project_id, session) => write!(f, "Flush({}, {})", project_id, session.id),
            Event::GitFileChange(project_id, path) => {
                write!(f, "GitFileChange({}, {})", project_id, path.display())
            }
            Event::ProjectFileChange(project_id, path) => {
                write!(f, "ProjectFileChange({}, {})", project_id, path.display())
            }
            Event::Session(pid, session) => write!(f, "Session({}, {})", pid, session.id),
            Event::Bookmark(b) => write!(f, "Bookmark({})", b.project_id),
            Event::SessionFile((pid, sid, path, _)) => {
                write!(f, "File({}, {}, {})", pid, sid, path.display())
            }
            Event::SessionDelta((pid, sid, path, delta)) => {
                write!(
                    f,
                    "Deltas({}, {}, {}, {})",
                    pid,
                    sid,
                    path.display(),
                    delta.timestamp_ms
                )
            }
        }
    }
}
