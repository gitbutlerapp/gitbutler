use std::{path, time};

use crate::{bookmarks, deltas, sessions};

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

    Session(sessions::Session),
    Bookmark(bookmarks::Bookmark),
    File((String, path::PathBuf, String)),
    Deltas((String, path::PathBuf, Vec<deltas::Delta>)),
}
