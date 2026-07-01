//! Download and verification logic

use std::{fs::File, io::Write, path::Path};

use anyhow::{Context, Result, anyhow, bail};

use crate::http::create_client;

/// Download a URL and return its contents as a string.
#[cfg(target_os = "linux")]
pub(crate) fn download_to_string(url: &str) -> Result<String> {
    let mut easy = create_client()?;

    easy.url(url)
        .with_context(|| format!("Failed to set URL: {url}"))?;

    let buf = std::cell::RefCell::new(Vec::new());

    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                buf.borrow_mut().extend_from_slice(data);
                Ok(data.len())
            })
            .context("Failed to set write function")?;

        transfer
            .perform()
            .with_context(|| format!("Failed to download from {url}"))?;
    }

    let response_code = easy
        .response_code()
        .context("Failed to get response code")?;
    if response_code != 200 {
        bail!("Download of {url} failed with HTTP status: {response_code}");
    }

    let effective_url = easy
        .effective_url()
        .context("Failed to get effective URL")?
        .ok_or_else(|| anyhow!("Effective URL is missing"))?;

    crate::release::validate_download_url(effective_url)
        .with_context(|| format!("Download was redirected to an untrusted URL: {effective_url}"))?;

    String::from_utf8(buf.into_inner()).context("Signature file is not valid UTF-8")
}

pub(crate) fn download_file(url: &str, dest: &Path) -> Result<()> {
    let mut easy = create_client()?;

    easy.url(url)
        .with_context(|| format!("Failed to set URL: {url}"))?;

    // Enable libcurl's built-in progress reporting
    easy.progress(true)?;

    let file = File::create(dest).context("Failed to create download file")?;
    let file = std::cell::RefCell::new(file);

    // Capture write errors so we can propagate them after transfer fails
    let write_error: std::cell::RefCell<Option<std::io::Error>> = std::cell::RefCell::new(None);

    {
        let mut transfer = easy.transfer();

        // Write downloaded data to file
        transfer
            .write_function(|data| {
                match file.borrow_mut().write_all(data) {
                    Ok(_) => Ok(data.len()),
                    Err(e) => {
                        // Store the error for later propagation
                        *write_error.borrow_mut() = Some(e);
                        // Abort the transfer by returning 0 bytes written
                        // (returning a size != data.len() signals an error to libcurl)
                        Ok(0)
                    }
                }
            })
            .context("Failed to set write function")?;

        // Use libcurl's progress callback for simple output
        transfer
            .progress_function(|total_download, downloaded, _, _| {
                if total_download > 0.0 {
                    let percent = (downloaded / total_download * 100.0) as u32;
                    let msg = format!(
                        "\rDownloading: {}% ({:.1}MB / {:.1}MB)",
                        percent,
                        downloaded / 1_000_000.0,
                        total_download / 1_000_000.0
                    );
                    crate::ui::print(&msg);
                } else if downloaded > 0.0 {
                    let msg = format!("\rDownloaded: {:.1}MB", downloaded / 1_000_000.0);
                    crate::ui::print(&msg);
                }
                true
            })
            .context("Failed to set progress function")?;

        let perform_result = transfer.perform();

        // Check if we had a write error
        if let Some(io_err) = write_error.borrow_mut().take() {
            // Clear progress line before showing error
            crate::ui::println_empty();
            return Err(io_err).context("Failed to write downloaded data");
        }

        // If perform failed for other reasons, propagate that error
        perform_result.with_context(|| format!("Failed to download from {url}"))?;
    }

    // Clear progress line
    crate::ui::println_empty();

    let response_code = easy
        .response_code()
        .context("Failed to get response code")?;
    if response_code == 403 {
        bail!(
            "Download failed, the download artifact could not be found. Most likely, the but CLI has not been published for the requested version."
        )
    } else if response_code != 200 {
        bail!("Download failed with HTTP status: {response_code}");
    }

    // Validate the effective URL after following redirects
    // This protects against malicious redirects to untrusted domains or insecure protocols
    let effective_url = easy
        .effective_url()
        .context("Failed to get effective URL")?
        .ok_or_else(|| anyhow!("Effective URL is missing"))?;

    crate::release::validate_download_url(effective_url)
        .with_context(|| format!("Download was redirected to an untrusted URL: {effective_url}"))?;

    Ok(())
}
