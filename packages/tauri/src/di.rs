use tauri::AppHandle;

use crate::{assets, zip};
use anyhow::{Context, Error, Result};
use std::fs;

pub fn zipper_from(handle: &AppHandle) -> Result<zip::Zipper, Error> {
    let cache_dir = handle
        .path_resolver()
        .app_cache_dir()
        .context("failed to get cache dir")?;
    fs::create_dir_all(&cache_dir).context("failed to create cache dir")?;
    let cache = cache_dir.join("archives");
    Ok(zip::Zipper::from(&cache))
}

pub fn proxy_from(handle: &AppHandle) -> Result<assets::Proxy, Error> {
    let app_cache_dir = handle
        .path_resolver()
        .app_cache_dir()
        .context("failed to get cache dir")?;
    fs::create_dir_all(&app_cache_dir).context("failed to create cache dir")?;
    let cache_dir = app_cache_dir.join("images");
    Ok(assets::Proxy::from(&cache_dir))
}
