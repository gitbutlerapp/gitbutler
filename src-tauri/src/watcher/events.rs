use std::{fmt::Display, path, time};

use crate::{bookmarks, deltas, sessions};

#[derive(Debug, PartialEq)]
pub enum Event {
    Tick(time::SystemTime),
    Flush(sessions::Session),
    Fetch,

    FileChange(path::PathBuf),
    GitFileChange(path::PathBuf),
    GitIndexChange,
    GitActivity,
    GitFetch,
    GitHeadChange(String),

    ProjectFileChange(path::PathBuf),

    Session(sessions::Session),
    SessionFile((String, path::PathBuf, String)),
    SessionDelta((String, path::PathBuf, deltas::Delta)),
    Bookmark(bookmarks::Bookmark),

    IndexAll,
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::IndexAll => write!(f, "IndexAll"),
            Event::Tick(ts) => write!(f, "Tick({:?})", ts),
            Event::Flush(session) => write!(f, "Flush({})", session.id),
            Event::Fetch => write!(f, "Fetch"),
            Event::GitFetch => write!(f, "GitFetch"),
            Event::FileChange(_) => write!(f, "FileChange"),
            Event::GitFileChange(_) => write!(f, "GitFileChange"),
            Event::GitIndexChange => write!(f, "GitIndexChange"),
            Event::GitActivity => write!(f, "GitActivity"),
            Event::GitHeadChange(head) => write!(f, "GitHeadChange({})", head),
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
