//! Download and verification logic

use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;

use crate::{http, release::validate_download_url};

/// Download a URL and return its contents as a string.
#[cfg(target_os = "linux")]
pub(crate) fn download_to_string(url: &str) -> Result<String> {
    let response = http::get(url).with_context(|| format!("Failed to download from {url}"))?;

    let status = response.status();
    if status != StatusCode::OK {
        bail!("Download of {url} failed with HTTP status: {status}");
    }

    let effective_url = response.url().as_str();
    validate_download_url(effective_url)
        .with_context(|| format!("Download was redirected to an untrusted URL: {effective_url}"))?;

    let body = response
        .bytes()
        .with_context(|| format!("Failed to download from {url}"))?;
    String::from_utf8(body.to_vec()).context("Signature file is not valid UTF-8")
}

pub(crate) fn download_file(url: &str, dest: &Path) -> Result<()> {
    let response = http::get(url).with_context(|| format!("Failed to download from {url}"))?;

    let status = response.status();
    if status == StatusCode::FORBIDDEN {
        bail!(
            "Download failed, the download artifact could not be found. Most likely, the but CLI has not been published for the requested version."
        )
    } else if status != StatusCode::OK {
        bail!("Download failed with HTTP status: {status}");
    }

    // Validate the effective URL after following redirects
    // This protects against malicious redirects to untrusted domains or insecure protocols
    let effective_url = response.url().as_str();
    validate_download_url(effective_url)
        .with_context(|| format!("Download was redirected to an untrusted URL: {effective_url}"))?;

    let file = File::create(dest).context("Failed to create download file")?;
    let total = response.content_length();
    let copied = copy_with_progress(response, file, total);

    // Clear progress line
    crate::ui::println_empty();

    copied.with_context(|| format!("Failed to download from {url}"))
}

fn copy_with_progress(mut body: impl Read, mut dest: impl Write, total: Option<u64>) -> Result<()> {
    let megabytes = |bytes: u64| bytes as f64 / 1_000_000.0;
    let mut chunk = [0; 64 * 1024];
    let mut downloaded = 0;

    loop {
        let len = body
            .read(&mut chunk)
            .context("Failed to read response body")?;
        if len == 0 {
            return Ok(());
        }
        dest.write_all(&chunk[..len])
            .context("Failed to write downloaded data")?;
        downloaded += len as u64;

        match total {
            Some(total) if total > 0 => crate::ui::print(&format!(
                "\rDownloading: {}% ({:.1}MB / {:.1}MB)",
                downloaded * 100 / total,
                megabytes(downloaded),
                megabytes(total)
            )),
            _ => crate::ui::print(&format!("\rDownloaded: {:.1}MB", megabytes(downloaded))),
        }
    }
}
