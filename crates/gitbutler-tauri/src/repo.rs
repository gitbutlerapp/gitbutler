pub mod commands {
    use crate::error::{Error, UnmarkedError};
    use anyhow::Result;
    use base64::{engine::general_purpose, Engine as _};
    use gitbutler_branch_actions::RemoteBranchFile;
    use gitbutler_command_context::CommandContext;
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_repo::RepoCommands;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use tauri::State;
    use tracing::instrument;

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn git_get_local_config(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
        key: &str,
    ) -> Result<Option<String>, Error> {
        let project = projects.get(id)?;
        Ok(project.get_local_config(key)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn git_set_local_config(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        let project = projects.get(id)?;
        project.set_local_config(key, value).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn check_signing_settings(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
    ) -> Result<bool, Error> {
        let project = projects.get(id)?;
        project.check_signing_settings().map_err(Into::into)
    }

    #[tauri::command(async)]
    pub fn git_clone_repository(
        repository_url: &str,
        target_dir: &Path,
    ) -> Result<(), UnmarkedError> {
        let should_interrupt = AtomicBool::new(false);

        gix::prepare_clone(repository_url, target_dir)?
            .fetch_then_checkout(gix::progress::Discard, &should_interrupt)
            .map(|(checkout, _outcome)| checkout)?
            .main_worktree(gix::progress::Discard, &should_interrupt)?;
        Ok(())
    }

    #[tauri::command(async)]
    pub fn get_uncommited_files(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = projects.get(id)?;

        Ok(gitbutler_branch_actions::get_uncommited_files(&project)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects))]
    pub fn get_pr_template_contents(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        relative_path: &Path,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project)?;
        if &project
            .path
            .join(relative_path)
            .canonicalize()?
            .as_path()
            .starts_with(project.path.clone())
        {
            let tree = ctx.repository().head()?.peel_to_tree()?;
            let entry = tree.get_path(relative_path)?;
            let blob = ctx.repository().find_blob(entry.id())?;
            if !blob.is_binary() {
                let content = std::str::from_utf8(blob.content())?;
                Ok(content.to_string())
            } else {
                let binary_content = blob.content();
                let encoded_content = general_purpose::STANDARD.encode(&binary_content);
                Ok(encoded_content)
            }
        }
        else {
            anyhow::bail!("Invalid workspace file");
        }
    }
}
