use std::env;

use anyhow::Result;

/// Initialize a pager if appropriate.
///
/// This function will:
/// - Skip on Windows (pager behavior is problematic on Windows)
/// - Skip if output is not to a terminal (default behavior of pager crate)
/// - Check the PAGER environment variable first (handled by pager crate)
/// - Read the `core.pager` git config value if PAGER is not set
/// - Fall back to `less` if neither is configured
/// - Respect the NOPAGER environment variable to disable paging (handled by pager crate)
pub fn setup_pager_if_appropriate() -> Result<()> {
    // Do not use pager on Windows
    #[cfg(target_os = "windows")]
    {
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        // If PAGER is already set, use the default pager behavior which will use it
        if env::var("PAGER").is_ok() {
            pager::Pager::new().setup();
            return Ok(());
        }

        // Try to get the pager from git config
        let pager_command = get_git_pager()?;

        // Set the pager program (will automatically skip if output is not to a terminal)
        pager::Pager::with_pager(&pager_command).setup();

        Ok(())
    }
}

/// Get the pager command from git config, falling back to "less" if not set.
fn get_git_pager() -> Result<String> {
    // Try to read core.pager from git config
    match git2::Config::open_default() {
        Ok(config) => {
            match config.get_string("core.pager") {
                Ok(pager) => Ok(pager),
                Err(e) if e.code() == git2::ErrorCode::NotFound => {
                    // core.pager not set, use default
                    Ok("less".to_string())
                }
                Err(e) => {
                    // Some other error, use default
                    eprintln!("Warning: Failed to read core.pager config: {}", e);
                    Ok("less".to_string())
                }
            }
        }
        Err(e) => {
            // Can't open git config, use default
            eprintln!("Warning: Failed to open git config: {}", e);
            Ok("less".to_string())
        }
    }
}
