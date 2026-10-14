//! Raise the process's open-file-descriptor soft limit at startup.
//!
//! macOS ships a default `RLIMIT_NOFILE` soft limit of 256, which a large
//! rebase across many stacks can exhaust with `EMFILE` ("too many open
//! files"): each `gix`/`git2` repository memory-maps a potentially large
//! number of pack files on first object access, and several repositories can
//! be open at once. Hitting that limit mid-rebase can leave the workspace in
//! an inconsistent state, so raise the soft limit toward the hard limit up
//! front to avoid the crash entirely (#14260).

/// The soft `RLIMIT_NOFILE` we try to raise to. Capped rather than unbounded so
/// we don't request an enormous descriptor table on hosts with a very high
/// hard limit; this matches the `ulimit -n 65536` workaround that resolves the
/// issue in practice.
#[cfg(unix)]
const TARGET_NOFILE: u64 = 65536;

/// Raise the open-file soft limit toward the hard limit (up to `TARGET_NOFILE`).
///
/// Best-effort and safe to call once at startup: it never *lowers* the limit,
/// treats any failure as non-fatal (the process simply keeps the OS default),
/// and is a no-op on non-Unix platforms (Windows has no comparably low
/// default).
pub fn raise_soft_limit() {
    #[cfg(unix)]
    match rlimit::increase_nofile_limit(TARGET_NOFILE) {
        Ok(limit) => tracing::debug!("raised open-file soft limit to {limit}"),
        Err(err) => tracing::warn!("could not raise the open-file soft limit: {err}"),
    }
}
