use std::{collections::HashMap, ops, path, time};

use anyhow::Context;
use futures::future::join_all;
use tauri::Manager;
use tracing::instrument;

use crate::{
    app, assets, bookmarks, deltas, error::Error, git, reader, search, sessions, virtual_branches,
};

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn search(
    handle: tauri::AppHandle,
    project_id: &str,
    query: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<search::Results, Error> {
    let app = handle.state::<app::App>();

    let query = search::Query {
        project_id: project_id.to_string(),
        q: query.to_string(),
        limit: limit.unwrap_or(100),
        offset,
    };

    let results = app.search(&query).with_context(|| {
        format!(
            "failed to search for query {} in project {}",
            query.q, query.project_id
        )
    })?;

    Ok(results)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_sessions(
    handle: tauri::AppHandle,
    project_id: &str,
    earliest_timestamp_ms: Option<u128>,
) -> Result<Vec<sessions::Session>, Error> {
    let app = handle.state::<app::App>();
    let sessions = app
        .list_sessions(project_id, earliest_timestamp_ms)
        .with_context(|| format!("failed to list sessions for project {}", project_id))?;
    Ok(sessions)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_session_files(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<path::PathBuf>>,
) -> Result<HashMap<path::PathBuf, reader::Content>, Error> {
    let app = handle.state::<app::App>();
    let files = app
        .list_session_files(project_id, session_id, paths)
        .with_context(|| {
            format!(
                "failed to list files for session {} in project {}",
                session_id, project_id
            )
        })?;
    Ok(files)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_deltas(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<&str>>,
) -> Result<HashMap<String, Vec<deltas::Delta>>, Error> {
    let app = handle.state::<app::App>();
    let deltas = app
        .list_session_deltas(project_id, session_id, paths)
        .with_context(|| {
            format!(
                "failed to list deltas for session {} in project {}",
                session_id, project_id
            )
        })?;
    Ok(deltas)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_wd_diff(
    handle: tauri::AppHandle,
    project_id: &str,
    context_lines: u32,
) -> Result<HashMap<path::PathBuf, String>, Error> {
    let app = handle.state::<app::App>();
    let diff = app
        .git_wd_diff(project_id, context_lines)
        .with_context(|| format!("failed to get git wd diff for project {}", project_id))?;
    Ok(diff)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_remote_branches(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<git::RemoteBranchName>, Error> {
    let app = handle.state::<app::App>();
    let branches = app.git_remote_branches(project_id).with_context(|| {
        format!(
            "failed to get remote git branches for project {}",
            project_id
        )
    })?;
    Ok(branches)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_remote_branches_data(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<virtual_branches::RemoteBranch>, Error> {
    let app = handle.state::<app::App>();
    let branches = app
        .git_remote_branches_data(project_id)
        .with_context(|| format!("failed to get git branches for project {}", project_id))?;

    let branches = join_all(
        branches
            .into_iter()
            .map(|branch| {
                let proxy = handle.state::<assets::Proxy>();
                async move {
                    virtual_branches::RemoteBranch {
                        commits: join_all(
                            branch
                                .commits
                                .into_iter()
                                .map(|commit| {
                                    let proxy = proxy.clone();
                                    async move {
                                        virtual_branches::RemoteCommit {
                                            author: virtual_branches::Author {
                                                gravatar_url: proxy
                                                    .proxy(&commit.author.gravatar_url)
                                                    .await
                                                    .unwrap_or_else(|e| {
                                                        tracing::error!(
                                                            "failed to proxy gravatar url {}: {:#}",
                                                            commit.author.gravatar_url,
                                                            e
                                                        );
                                                        commit.author.gravatar_url.clone()
                                                    }),
                                                ..commit.author.clone()
                                            },
                                            ..commit.clone()
                                        }
                                    }
                                })
                                .collect::<Vec<_>>(),
                        )
                        .await,
                        ..branch
                    }
                }
            })
            .collect::<Vec<_>>(),
    )
    .await;
    Ok(branches)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_head(handle: tauri::AppHandle, project_id: &str) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let head = app
        .git_head(project_id)
        .with_context(|| format!("failed to get git head for project {}", project_id))?;
    Ok(head)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn delete_all_data(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.delete_all_data()
        .await
        .context("failed to delete all data")?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn upsert_bookmark(
    handle: tauri::AppHandle,
    project_id: String,
    timestamp_ms: u64,
    note: String,
    deleted: bool,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    let now = time::UNIX_EPOCH
        .elapsed()
        .context("failed to get time")?
        .as_millis();
    let bookmark = bookmarks::Bookmark {
        project_id,
        timestamp_ms: timestamp_ms
            .try_into()
            .context("failed to convert timestamp")?,
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        note,
        deleted,
    };
    app.upsert_bookmark(&bookmark)
        .await
        .context("failed to upsert bookmark")?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn list_bookmarks(
    handle: tauri::AppHandle,
    project_id: &str,
    range: Option<ops::Range<u128>>,
) -> Result<Vec<bookmarks::Bookmark>, Error> {
    let app = handle.state::<app::App>();
    let bookmarks = app
        .list_bookmarks(project_id, range)
        .context("failed to list bookmarks")?;
    Ok(bookmarks)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn fetch_from_target(handle: tauri::AppHandle, project_id: &str) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.fetch_from_target(project_id)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn mark_resolved(
    handle: tauri::AppHandle,
    project_id: &str,
    path: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.mark_resolved(project_id, path)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_set_global_config(
    handle: tauri::AppHandle,
    key: &str,
    value: &str,
) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let result = app.git_set_global_config(key, value)?;
    Ok(result)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn git_get_global_config(
    handle: tauri::AppHandle,
    key: &str,
) -> Result<Option<String>, Error> {
    let app = handle.state::<app::App>();
    let result = app.git_get_global_config(key)?;
    Ok(result)
}
