pub mod commands {
    use std::path::Path;

    use anyhow::Context;
    use gitbutler_forge::{
        forge::ForgeName,
        review::{
            available_review_templates, get_review_template_functions, ReviewTemplateFunctions,
        },
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
        forge: ForgeName,
    ) -> Result<Vec<String>, Error> {
        let project = projects.get_validated(project_id)?;
        Ok(available_review_templates(&project.path, &forge))
    }

    #[tauri::command(async)]
    #[instrument(skip(projects))]
    pub fn get_review_template_contents(
        projects: State<'_, Controller>,
        project_id: ProjectId,
        relative_path: &Path,
        forge: ForgeName,
    ) -> anyhow::Result<String, Error> {
        let project = projects.get_validated(project_id)?;

        let ReviewTemplateFunctions {
            is_valid_review_template_path,
            ..
        } = get_review_template_functions(&forge);

        if !is_valid_review_template_path(relative_path, &project.path) {
            return Err(anyhow::format_err!(
                "Invalid review template path: {:?}",
                Path::join(&project.path, relative_path)
            )
            .into());
        }
        Ok(project
            .read_file_from_workspace(None, relative_path)?
            .content
            .context("PR template was not valid UTF-8")?)
    }
}
