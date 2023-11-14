use std::{ops::Range, time};

use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::error::{Code, Error};

use super::{
    controller::{ListError, UpsertError},
    Bookmark, Controller,
};

impl From<UpsertError> for Error {
    fn from(value: UpsertError) -> Self {
        match value {
            UpsertError::OpenProjectRepository(error) => Error::from(error),
            UpsertError::GetUser(error) => Error::from(error),
            UpsertError::GetProject(error) => Error::from(error),
            UpsertError::Other(error) => {
                tracing::error!(?error);
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn upsert_bookmark(
    handle: AppHandle,
    project_id: String,
    timestamp_ms: u64,
    note: String,
    deleted: bool,
) -> Result<(), Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    let now = time::UNIX_EPOCH
        .elapsed()
        .map_err(|error| {
            tracing::error!(?error);
            Error::Unknown
        })?
        .as_millis();

    let bookmark = Bookmark {
        project_id,
        timestamp_ms: timestamp_ms.into(),
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        note,
        deleted,
    };
    handle
        .state::<Controller>()
        .upsert(&bookmark)
        .map(|_| ())
        .map_err(Into::into)
}

impl From<ListError> for Error {
    fn from(value: ListError) -> Self {
        match value {
            ListError::Other(error) => {
                tracing::error!(?error);
                Error::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_bookmarks(
    handle: AppHandle,
    project_id: &str,
    range: Option<Range<u128>>,
) -> Result<Vec<Bookmark>, Error> {
    let project_id = project_id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".to_string(),
    })?;
    handle
        .state::<Controller>()
        .list(&project_id, range)
        .map_err(Into::into)
}
