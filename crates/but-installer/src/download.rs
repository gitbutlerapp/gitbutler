//! Download and verification logic

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};

use crate::{http::create_client, ui::success};

pub(crate) fn download_file(url: &str, dest: &Path) -> Result<()> {
    let mut easy = create_client()?;

    easy.url(url)
        .with_context(|| format!("Failed to set URL: {}", url))?;

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
        perform_result.with_context(|| format!("Failed to download from {}", url))?;
    }

    // Clear progress line
    crate::ui::println_empty();

    let response_code = easy
        .response_code()
        .context("Failed to get response code")?;
    if response_code != 200 {
        bail!("Download failed with HTTP status: {}", response_code);
    }

    // Validate the effective URL after following redirects
    // This protects against malicious redirects to untrusted domains or insecure protocols
    let effective_url = easy
        .effective_url()
        .context("Failed to get effective URL")?
        .ok_or_else(|| anyhow!("Effective URL is missing"))?;

    crate::release::validate_download_url(effective_url).with_context(|| {
        format!(
            "Download was redirected to an untrusted URL: {}",
            effective_url
        )
    })?;

    Ok(())
}

pub(crate) fn validate_tarball(path: &Path) -> Result<()> {
    // Check if file is not empty
    let metadata = fs::metadata(path).context("Failed to read tarball metadata")?;
    if metadata.len() == 0 {
        bail!("Downloaded file is empty");
    }

    // Check for gzip magic bytes (1f 8b)
    if !is_gzip_file(path)? {
        bail!("Downloaded file does not appear to be a valid gzip archive");
    }

    Ok(())
}

/// Check if a file has valid gzip magic bytes
pub(crate) fn is_gzip_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path).context("Failed to open file")?;
    let mut magic_bytes = [0u8; 2];

    // If we can't read 2 bytes, it's definitely not a valid gzip file
    match file.read_exact(&mut magic_bytes) {
        Ok(_) => Ok(magic_bytes == [0x1f, 0x8b]),
        Err(_) => Ok(false),
    }
}

pub(crate) fn verify_signature(tarball: &Path, signature_b64: &str, temp_dir: &Path) -> Result<()> {
    crate::ui::info("Verifying download signature...");

    // Validate signature is not empty - this is a security requirement
    if signature_b64.trim().is_empty() {
        bail!("Signature is empty - refusing to verify without a valid signature");
    }

    // GitButler's minisign public key
    let pubkey_str = "RWTrOEI+im1XYA9RBwyxnzFN/evFzJhU1lbQ70LVayWH3WRo7xQnRLD2";

    // Parse the public key
    let public_key = minisign_verify::PublicKey::from_base64(pubkey_str)
        .context("Failed to parse public key")?;

    // Decode signature from base64 and write to file
    // The signature format from the API is base64-encoded minisign signature file content
    use base64::{Engine, engine::general_purpose::STANDARD};
    let signature_bytes = STANDARD
        .decode(signature_b64)
        .context("Failed to decode signature from base64")?;

    // Write signature to temp file for minisign to parse
    let signature_file = temp_dir.join("signature.minisig");
    fs::write(&signature_file, &signature_bytes)?;

    // Read it back as a string (minisign signatures are text files)
    let signature_str =
        fs::read_to_string(&signature_file).context("Failed to read signature file as string")?;

    // Parse the signature
    let signature =
        minisign_verify::Signature::decode(&signature_str).context("Failed to parse signature")?;

    // Use streaming verification to avoid loading entire file into memory
    let mut file = File::open(tarball).context("Failed to open tarball for verification")?;
    let mut verifier = public_key
        .verify_stream(&signature)
        .map_err(|e| anyhow!("Failed to initialize signature verifier: {}", e))?;

    // Read and verify file in 64KB chunks
    let mut buffer = [0u8; 65536];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        verifier.update(&buffer[..bytes_read]);
    }

    // Finalize verification
    verifier.finalize().map_err(|e| {
        anyhow!(
            "Signature verification failed - the download may have been tampered with: {}",
            e
        )
    })?;

    success("Signature verification passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_is_gzip_file_with_valid_gzip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.gz");

        // Write gzip magic bytes
        let mut file = File::create(&test_file).unwrap();
        file.write_all(&[0x1f, 0x8b, 0x08, 0x00]).unwrap();

        assert!(is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_is_gzip_file_with_invalid_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // Write non-gzip content
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Hello, World!").unwrap();

        assert!(!is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_is_gzip_file_with_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("empty.gz");

        // Create empty file
        File::create(&test_file).unwrap();

        // Empty file can't be a valid gzip
        assert!(!is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_is_gzip_file_with_single_byte() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("single.gz");

        // Write only one byte
        let mut file = File::create(&test_file).unwrap();
        file.write_all(&[0x1f]).unwrap();

        // Not enough bytes to be valid gzip
        assert!(!is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_validate_tarball_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("empty.tar.gz");

        // Create empty file
        File::create(&test_file).unwrap();

        // Should fail because file is empty
        assert!(validate_tarball(&test_file).is_err());
    }

    #[test]
    fn test_validate_tarball_non_gzip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.tar.gz");

        // Write non-gzip content
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Not a gzip file").unwrap();

        // Should fail because it's not gzip
        assert!(validate_tarball(&test_file).is_err());
    }

    #[test]
    fn test_verify_signature_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.tar.gz");

        // Create a dummy file
        let mut file = File::create(&test_file).unwrap();
        file.write_all(&[0x1f, 0x8b, 0x08, 0x00]).unwrap();

        // Empty signature should fail with clear error
        let result = verify_signature(&test_file, "", temp_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));

        // Whitespace-only signature should also fail
        let result = verify_signature(&test_file, "   ", temp_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }
}
