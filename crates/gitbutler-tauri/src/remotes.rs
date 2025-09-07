use but_api::commands::remotes;
use gitbutler_project::ProjectId;
use gitbutler_repo::GitRemote;

use but_api::error::Error;

#[tauri::command(async)]
pub fn list_remotes(project_id: ProjectId) -> Result<Vec<GitRemote>, Error> {
    remotes::list_remotes(project_id)
}

#[tauri::command(async)]
pub fn add_remote(project_id: ProjectId, name: String, url: String) -> Result<(), Error> {
    remotes::add_remote(project_id, name, url)
}
