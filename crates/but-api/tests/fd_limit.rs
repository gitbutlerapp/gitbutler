//! Integration test for the public `but_api::fd_limit` API.
//!
//! Unix-only: the helper is a no-op elsewhere and `rlimit` is a Unix-only dev
//! dependency, so the whole file compiles away on other platforms.
#![cfg(unix)]

use rlimit::Resource;

/// `raise_soft_limit` is best-effort. We assert only its safety contract - it
/// must not panic and must never *lower* the open-file soft limit - not that a
/// specific target is reached: the OS-permitted maximum is platform-specific
/// (on macOS `kern.maxfilesperproc`, well below `RLIM_INFINITY`), so the
/// achieved limit can legitimately fall short of the target.
#[test]
fn raise_soft_limit_never_lowers_and_is_repeatable() {
    let (soft_before, _hard) = Resource::NOFILE.get().expect("read NOFILE limit");

    but_api::fd_limit::raise_soft_limit();
    let (soft_after, _hard) = Resource::NOFILE.get().expect("read NOFILE limit");
    assert!(
        soft_after >= soft_before,
        "raise_soft_limit must never lower the limit: {soft_after} < {soft_before}"
    );

    // Called once at startup in production, but it must stay safe to call again:
    // a second invocation must likewise never lower the limit.
    but_api::fd_limit::raise_soft_limit();
    let (soft_again, _hard) = Resource::NOFILE.get().expect("read NOFILE limit");
    assert!(
        soft_again >= soft_after,
        "a repeated raise_soft_limit call must not lower the limit: {soft_again} < {soft_after}"
    );
}
