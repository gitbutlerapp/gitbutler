use std::{path, time};

use crate::{deltas, sessions};

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
    File((String, path::PathBuf, String)),
    Deltas((String, path::PathBuf, Vec<deltas::Delta>)),
}
