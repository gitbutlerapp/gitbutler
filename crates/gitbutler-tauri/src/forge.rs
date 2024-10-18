pub mod commands {
    use std::path::Path;

    use anyhow::Context;
    use gitbutler_forge::review::{
        available_review_templates, get_review_template_functions, ReviewTemplateFunctions,
    };
    use gitbutler_project::{Controller, ProjectId};
    use gitbutler_repo::RepoCommands;
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

    #[tauri::command(async)]
    #[instrument(skip(projects))]
    pub fn get_review_template_contents(
        projects: State<'_, Controller>,
        project_id: ProjectId,
        relative_path: &Path,
    ) -> Result<String, Error> {
        let project = projects.get_validated(project_id)?;
        let forge_type = project
            .git_host
            .host_type
            .clone()
            .context("Project does not have a forge type")?;

        let ReviewTemplateFunctions {
            is_valid_review_template_path,
            ..
        } = get_review_template_functions(&forge_type);

        if !is_valid_review_template_path(relative_path, &project.path) {
            return Err(anyhow::anyhow!("Invalid review template path").into());
        }

        let file_info = project.read_file_from_workspace(None, relative_path)?;

        Ok(file_info
            .content
            .context("PR template was not valid UTF-8")?)
    }
}
