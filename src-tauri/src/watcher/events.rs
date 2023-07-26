use std::{fmt::Display, path, time};

use crate::{bookmarks, deltas, events, sessions};

#[derive(Debug, PartialEq)]
pub enum Event {
    Tick(time::SystemTime),
    Flush(sessions::Session),

    FetchGitbutlerData(time::SystemTime),

    FileChange(path::PathBuf),
    GitFileChange(path::PathBuf),

    ProjectFileChange(path::PathBuf),

    Session(sessions::Session),
    SessionFile((String, path::PathBuf, String)),
    SessionDelta((String, path::PathBuf, deltas::Delta)),
    Bookmark(bookmarks::Bookmark),

    IndexAll,

    Emit(events::Event),
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Emit(event) => write!(f, "Emit({})", event.name()),
            Event::IndexAll => write!(f, "IndexAll"),
            Event::Tick(ts) => write!(f, "Tick({:?})", ts),
            Event::FetchGitbutlerData(ts) => write!(f, "FetchGitbutlerData({:?})", ts),
            Event::Flush(session) => write!(f, "Flush({})", session.id),
            Event::FileChange(path) => write!(f, "FileChange({})", path.display()),
            Event::GitFileChange(_) => write!(f, "GitFileChange"),
            Event::ProjectFileChange(path) => write!(f, "ProjectFileChange({})", path.display()),
            Event::Session(session) => write!(f, "Session({})", session.id),
            Event::Bookmark(_) => write!(f, "Bookmark"),
            Event::SessionFile((sid, path, _)) => write!(f, "File({},{})", sid, path.display()),
            Event::SessionDelta((sid, path, delta)) => {
                write!(
                    f,
                    "Deltas({},{},{})",
                    sid,
                    path.display(),
                    delta.timestamp_ms
                )
            }
        }
    }
}
