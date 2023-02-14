use crate::{deltas, projects, sessions};
use serde;

pub fn session<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    project: &projects::project::Project,
    session: &sessions::Session,
) {
    let event_name = format!("project://{}/sessions", project.id);
    match window.emit(&event_name, &session) {
        Ok(_) => {}
        Err(e) => log::error!("Error: {:?}", e),
    };
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DeltasEvent {
    file_path: String,
    deltas: Vec<deltas::Delta>,
}

pub fn deltas<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    project: &projects::project::Project,
    session: &sessions::Session,
    deltas: &Vec<deltas::Delta>,
    relative_file_path: &std::path::Path,
) {
    let event_name = format!("project://{}/deltas/{}", project.id, session.id);
    match window.emit(
        &event_name,
        &DeltasEvent {
            deltas: deltas.clone(),
            file_path: relative_file_path.to_str().unwrap().to_string(),
        },
    ) {
        Ok(_) => {}
        Err(e) => log::error!("Error: {:?}", e),
    };
}
