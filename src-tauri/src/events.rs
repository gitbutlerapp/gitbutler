use crate::{
    app::{deltas, sessions},
    projects,
};

#[derive(Debug)]
pub struct Event {
    pub name: String,
    pub payload: serde_json::Value,
}

impl Event {
    pub fn session(project: &projects::Project, session: &sessions::Session) -> Self {
        let event_name = format!("project://{}/sessions", project.id);
        Event {
            name: event_name,
            payload: serde_json::to_value(session).unwrap(),
        }
    }

    pub fn git_index(project: &projects::Project) -> Self {
        let event_name = format!("project://{}/git/index", project.id);
        Event {
            name: event_name,
            payload: serde_json::json!({}),
        }
    }

    pub fn git_head(project: &projects::Project, head: &str) -> Self {
        let event_name = format!("project://{}/git/head", project.id);
        Event {
            name: event_name,
            payload: serde_json::json!({ "head": head }),
        }
    }

    pub fn git_activity(project: &projects::Project) -> Self {
        let event_name = format!("project://{}/git/activity", project.id);
        Event {
            name: event_name,
            payload: serde_json::json!({}),
        }
    }

    pub fn detlas(
        project: &projects::Project,
        session: &sessions::Session,
        deltas: &Vec<deltas::Delta>,
        relative_file_path: &std::path::Path,
    ) -> Self {
        let event_name = format!("project://{}/sessions/{}/deltas", project.id, session.id);
        let payload = serde_json::json!({
            "deltas": deltas,
            "filePath": relative_file_path,
        });
        Event {
            name: event_name,
            payload,
        }
    }
}
