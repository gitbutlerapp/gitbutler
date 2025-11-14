/// Initialize a pager if appropriate.
///
/// This function will:
/// - Skip on Windows (pager behavior is problematic on Windows)
/// - Skip if output is not to a terminal (default behavior of pager crate)
/// - Check the PAGER environment variable first (handled by pager crate)
/// - Read the `core.pager` git config value if PAGER is not set
/// - Fall back to `less` if neither is configured
/// - Respect the NOPAGER environment variable to disable paging (handled by pager crate)
#[cfg_attr(windows, allow(unused))]
pub fn from_env_or_git(directory: &std::path::Path) {
    #[cfg(windows)]
    {
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        // If PAGER is already set, use the default pager behavior which will use it
        if std::env::var("PAGER").is_ok() {
            pager::Pager::new().setup();
            return;
        }

        let git_pager = gix::discover(directory)
            .ok()
            .and_then(|repo| {
                repo.config_snapshot()
                    .trusted_program("core.pager")
                    .map(|p| p.into_owned())
            })
            .unwrap_or_else(|| std::ffi::OsString::from("less"));

        // Set the pager program (will automatically skip if output is not to a terminal)
        pager::Pager::with_pager(&git_pager.to_string_lossy()).setup();
    }
}
