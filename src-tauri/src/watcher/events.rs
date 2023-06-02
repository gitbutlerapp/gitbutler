use std::{fmt::Display, path, time};

use crate::{bookmarks, deltas, sessions};

#[derive(Debug, PartialEq)]
pub enum Event {
    Tick(time::SystemTime),
    Flush(sessions::Session),
    SessionFlushed(sessions::Session),
    Fetch,

    FileChange(path::PathBuf),
    GitFileChange(path::PathBuf),
    GitIndexChange,
    GitActivity,
    GitHeadChange(String),

    ProjectFileChange(path::PathBuf),

    Reindex,
    Session(sessions::Session),
    Bookmark(bookmarks::Bookmark),
    File((String, path::PathBuf, String)),
    Deltas((String, path::PathBuf, Vec<deltas::Delta>)),
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Reindex => write!(f, "Reindex"),
            Event::Tick(ts) => write!(f, "Tick({:?})", ts),
            Event::Flush(session) => write!(f, "Flush({})", session.id),
            Event::SessionFlushed(session) => write!(f, "SessionFlushed({})", session.id),
            Event::Fetch => write!(f, "Fetch"),
            Event::FileChange(_) => write!(f, "FileChange"),
            Event::GitFileChange(_) => write!(f, "GitFileChange"),
            Event::GitIndexChange => write!(f, "GitIndexChange"),
            Event::GitActivity => write!(f, "GitActivity"),
            Event::GitHeadChange(head) => write!(f, "GitHeadChange({})", head),
            Event::ProjectFileChange(path) => write!(f, "ProjectFileChange({})", path.display()),
            Event::Session(session) => write!(f, "Session({})", session.id),
            Event::Bookmark(_) => write!(f, "Bookmark"),
            Event::File((sid, path, _)) => write!(f, "File({},{})", sid, path.display()),
            Event::Deltas((sid, path, deltas)) => write!(f, "Deltas({},{},{})", sid, path.display(), deltas.len()),
        }
    }
}
