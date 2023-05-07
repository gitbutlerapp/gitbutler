use std::{path, time};

use crate::{
    app::{deltas, sessions},
    projects,
};

pub enum Event {
    Tick(time::SystemTime),
    FlushSession(sessions::Session),
    SessionFlushed(sessions::Session),

    FileChange(path::PathBuf),
    GitFileChange(path::PathBuf),
    GitIndexChange(projects::Project),
    GitActivity(projects::Project),
    GitHeadChange((projects::Project, String)),

    ProjectFileChange(path::PathBuf),

    Session((projects::Project, sessions::Session)),
    Deltas(
        (
            projects::Project,
            sessions::Session,
            path::PathBuf,
            Vec<deltas::Delta>,
        ),
    ),
}
