use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLabProjectId {
    /// The username or group name that owns the project.
    ///
    /// This should be the the namespace of the project in the git remote.
    username: String,
    /// The name of the project
    ///
    /// This should be the name of the project in the git remote, without the namespace.
    project_name: String,
}

impl GitLabProjectId {
    pub fn new(username: &str, project_name: &str) -> Self {
        GitLabProjectId {
            username: username.to_string(),
            project_name: project_name.to_string(),
        }
    }
}

impl Display for GitLabProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let project_id = format!("{}/{}", self.username, self.project_name);
        let encoded = urlencoding::encode(&project_id);
        write!(f, "{encoded}")
    }
}
