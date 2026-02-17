//! Platform-agnostic installation logic

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, anyhow, bail};

use crate::ui::success;

/// Returns the path to the `but` CLI binary for the given home directory.
pub(crate) fn but_binary_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".local/bin/but")
}

/// Validate that but can be executed
pub(crate) fn validate_installed_binary(path: &Path) -> bool {
    Command::new(path)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Verify the signature for a CLI installable
pub(crate) fn verify_signature(installable: &Path, signature_b64: &str, temp_dir: &Path) -> Result<()> {
    crate::ui::info("Verifying download signature...");

    // Validate signature is not empty - this is a security requirement
    if signature_b64.trim().is_empty() {
        bail!("Signature is empty - refusing to verify without a valid signature");
    }

    // GitButler's minisign public key
    let pubkey_str = "RWTrOEI+im1XYA9RBwyxnzFN/evFzJhU1lbQ70LVayWH3WRo7xQnRLD2";

    // Parse the public key
    let public_key = minisign_verify::PublicKey::from_base64(pubkey_str).context("Failed to parse public key")?;

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
    let signature_str = fs::read_to_string(&signature_file).context("Failed to read signature file as string")?;

    // Parse the signature
    let signature = minisign_verify::Signature::decode(&signature_str).context("Failed to parse signature")?;

    // Use streaming verification to avoid loading entire file into memory
    let mut file = File::open(installable).context("Failed to open installable for verification")?;
    let mut verifier = public_key
        .verify_stream(&signature)
        .map_err(|e| anyhow!("Failed to initialize signature verifier: {e}"))?;

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
    verifier
        .finalize()
        .map_err(|e| anyhow!("Signature verification failed - the download may have been tampered with: {e}"))?;

    success("Signature verification passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

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
