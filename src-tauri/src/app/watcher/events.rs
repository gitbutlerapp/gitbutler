use std::{path, time};

use crate::{app, deltas, projects};

pub enum Event {
    Tick(time::SystemTime),
    FlushSession(app::Session),
    SessionFlushed(app::Session),

    FileChange(path::PathBuf),
    GitFileChange(path::PathBuf),
    GitIndexChange(projects::Project),
    GitActivity(projects::Project),
    GitHeadChange((projects::Project, String)),

    ProjectFileChange(path::PathBuf),

    Session((projects::Project, app::Session)),
    Deltas(
        (
            projects::Project,
            app::Session,
            path::PathBuf,
            Vec<deltas::Delta>,
        ),
    ),
}
