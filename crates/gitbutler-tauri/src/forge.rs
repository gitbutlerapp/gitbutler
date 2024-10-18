pub mod commands {
    use gitbutler_forge::review::available_review_templates;
    use gitbutler_project::{Controller, ProjectId};
    use tauri::State;
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn get_available_review_templates(
        projects: State<'_, Controller>,
        project_id: ProjectId,
    ) -> Result<Vec<String>, Error> {
        let project = projects.get_validated(project_id)?;
        let root_path = &project.path;
        let forge_type = project.git_host.host_type;

        let review_templates = forge_type
            .map(|forge_type| available_review_templates(root_path, &forge_type))
            .unwrap_or_default();
        Ok(review_templates)
    }
}
