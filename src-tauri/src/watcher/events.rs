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
            Event::Tick(_) => write!(f, "Tick"),
            Event::Flush(_) => write!(f, "Flush"),
            Event::SessionFlushed(_) => write!(f, "SessionFlushed"),
            Event::Fetch => write!(f, "Fetch"),
            Event::FileChange(_) => write!(f, "FileChange"),
            Event::GitFileChange(_) => write!(f, "GitFileChange"),
            Event::GitIndexChange => write!(f, "GitIndexChange"),
            Event::GitActivity => write!(f, "GitActivity"),
            Event::GitHeadChange(_) => write!(f, "GitHeadChange"),
            Event::ProjectFileChange(_) => write!(f, "ProjectFileChange"),
            Event::Session(_) => write!(f, "Session"),
            Event::Bookmark(_) => write!(f, "Bookmark"),
            Event::File(_) => write!(f, "File"),
            Event::Deltas(_) => write!(f, "Deltas"),
        }
    }
}
