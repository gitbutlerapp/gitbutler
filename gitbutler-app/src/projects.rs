pub mod commands {
    use std::path;

    use gitbutler_core::projects::{
        self,
        controller::{self, Controller},
    };
    use tauri::Manager;
    use tracing::instrument;

    use crate::error::{Code, Error};

    impl From<controller::UpdateError> for Error {
        fn from(value: controller::UpdateError) -> Self {
            match value {
                controller::UpdateError::Validation(
                    controller::UpdateValidationError::KeyNotFound(path),
                ) => Error::UserError {
                    code: Code::Projects,
                    message: format!("'{}' not found", path.display()),
                },
                controller::UpdateError::Validation(
                    controller::UpdateValidationError::KeyNotFile(path),
                ) => Error::UserError {
                    code: Code::Projects,
                    message: format!("'{}' is not a file", path.display()),
                },
                controller::UpdateError::NotFound => Error::UserError {
                    code: Code::Projects,
                    message: "Project not found".into(),
                },
                controller::UpdateError::Other(error) => {
                    tracing::error!(?error, "failed to update project");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn update_project(
        handle: tauri::AppHandle,
        project: projects::UpdateRequest,
    ) -> Result<projects::Project, Error> {
        handle
            .state::<Controller>()
            .update(&project)
            .await
            .map_err(Into::into)
    }

    impl From<controller::AddError> for Error {
        fn from(value: controller::AddError) -> Self {
            match value {
                controller::AddError::NotAGitRepository => Error::UserError {
                    code: Code::Projects,
                    message: "Must be a git directory".to_string(),
                },
                controller::AddError::AlreadyExists => Error::UserError {
                    code: Code::Projects,
                    message: "Project already exists".to_string(),
                },
                controller::AddError::OpenProjectRepository(error) => error.into(),
                controller::AddError::NotADirectory => Error::UserError {
                    code: Code::Projects,
                    message: "Not a directory".to_string(),
                },
                controller::AddError::PathNotFound => Error::UserError {
                    code: Code::Projects,
                    message: "Path not found".to_string(),
                },
                controller::AddError::SubmodulesNotSupported => Error::UserError {
                    code: Code::Projects,
                    message: "Repositories with git submodules are not supported".to_string(),
                },
                controller::AddError::User(error) => error.into(),
                controller::AddError::Other(error) => {
                    tracing::error!(?error, "failed to add project");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn add_project(
        handle: tauri::AppHandle,
        path: &path::Path,
    ) -> Result<projects::Project, Error> {
        handle.state::<Controller>().add(path).map_err(Into::into)
    }

    impl From<controller::GetError> for Error {
        fn from(value: controller::GetError) -> Self {
            match value {
                controller::GetError::NotFound => Error::UserError {
                    code: Code::Projects,
                    message: "Project not found".into(),
                },
                controller::GetError::Other(error) => {
                    tracing::error!(?error, "failed to get project");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn get_project(
        handle: tauri::AppHandle,
        id: &str,
    ) -> Result<projects::Project, Error> {
        let id = id.parse().map_err(|_| Error::UserError {
            code: Code::Validation,
            message: "Malformed project id".into(),
        })?;
        handle.state::<Controller>().get(&id).map_err(Into::into)
    }

    impl From<controller::ListError> for Error {
        fn from(value: controller::ListError) -> Self {
            match value {
                controller::ListError::Other(error) => {
                    tracing::error!(?error, "failed to list projects");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn list_projects(handle: tauri::AppHandle) -> Result<Vec<projects::Project>, Error> {
        handle.state::<Controller>().list().map_err(Into::into)
    }

    impl From<controller::DeleteError> for Error {
        fn from(value: controller::DeleteError) -> Self {
            match value {
                controller::DeleteError::Other(error) => {
                    tracing::error!(?error, "failed to delete project");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn delete_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
        let id = id.parse().map_err(|_| Error::UserError {
            code: Code::Validation,
            message: "Malformed project id".into(),
        })?;
        handle
            .state::<Controller>()
            .delete(&id)
            .await
            .map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn git_get_local_config(
        handle: tauri::AppHandle,
        id: &str,
        key: &str,
    ) -> Result<Option<String>, Error> {
        let id = id.parse().map_err(|_| Error::UserError {
            code: Code::Validation,
            message: "Malformed project id".into(),
        })?;
        handle
            .state::<Controller>()
            .get_local_config(&id, key)
            .map_err(|e| Error::UserError {
                code: Code::Projects,
                message: e.to_string(),
            })
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn git_set_local_config(
        handle: tauri::AppHandle,
        id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        let id = id.parse().map_err(|_| Error::UserError {
            code: Code::Validation,
            message: "Malformed project id".into(),
        })?;
        handle
            .state::<Controller>()
            .set_local_config(&id, key, value)
            .map_err(|e| Error::UserError {
                code: Code::Projects,
                message: e.to_string(),
            })
    }
}
