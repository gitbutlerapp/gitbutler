use std::io::Write;

mod output_channel;
pub use output_channel::OutputChannel;

pub mod metrics;
#[cfg(feature = "legacy")]
pub use metrics::types::BackgroundMetrics;
pub use metrics::types::OneshotMetricsContext;

/// Utilities attached to `anyhow::Result<impl serde::Serialize>`.
pub trait ResultJsonExt {
    /// Write this value as pretty `JSON` to stdout if `json` is `true`.
    ///
    /// This style is great if you don't want to forget that JSON must be implemented.
    /// Note that "null" isn't printed and silently dropped.
    fn output_json(self, json: bool) -> anyhow::Result<()>;
}

pub trait ResultErrorExt {
    fn show_root_cause_error_then_exit_without_destructors(self, out: OutputChannel) -> !;
}

impl ResultErrorExt for anyhow::Result<()> {
    fn show_root_cause_error_then_exit_without_destructors(self, out: OutputChannel) -> ! {
        // Trigger the pager to be flushed before exiting early, or destructors aren't called.
        drop(out);
        let code = if let Err(e) = &self {
            writeln!(std::io::stderr(), "{} {}", e, e.root_cause()).ok();
            1
        } else {
            0
        };
        std::process::exit(code);
    }
}

/// Utilities attached to `anyhow::Result<T>`.
pub trait ResultMetricsExt {
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> anyhow::Result<()>;
}

impl<T> ResultJsonExt for anyhow::Result<T>
where
    T: serde::Serialize,
{
    fn output_json(self, json: bool) -> anyhow::Result<()> {
        if json && let Ok(value) = &self {
            json_pretty_to_stdout(value)?;
        }
        self.map(|_| ())
    }
}

fn json_pretty_to_stdout(value: &impl serde::Serialize) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    let value = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    if value != "null" {
        stdout.write_all(value.as_bytes())?;
        stdout.write_all(b"\n").ok();
    }
    Ok(())
}

/// Utilities specifically for testing the private API.
#[cfg(test)]
pub(crate) mod tests {
    use but_testsupport::gix_testtools::tempfile;

    /// Obtain an isolated `Context` from the `tests/fixtures/$name.sh` script, with in-memory objects.
    // TODO: use this more once we don't need to post-adjust the scenarios, `legacy_minit` isn't needed by modern code.
    #[expect(unused)]
    pub fn read_only_in_memory_scenario(name: &str) -> anyhow::Result<but_ctx::Context> {
        but_testsupport::read_only_in_memory_scenario(name)?.try_into()
    }

    /// Obtain a `(ctx, tmp)` where `tmp` is a copy of the `tests/fixtures/scenario/$name.sh` script.
    pub fn writable_scenario(name: &str) -> (but_ctx::Context, tempfile::TempDir) {
        let (repo, tmp) = but_testsupport::writable_scenario(name);
        (
            repo.try_into().expect("valid repo yields valid context"),
            tmp,
        )
    }

    /// Minimally initialise a legacy project so old code doesn't fail outright.
    /// This will *write* to the repository at `ctx`.
    #[cfg(feature = "legacy")]
    pub fn legacy_minit(ctx: &but_ctx::Context) -> anyhow::Result<()> {
        use anyhow::Context;
        let guard = ctx.shared_worktree_access();
        // This is testing, and the permission system doesn't enforce mutability isn't abused,
        // no need to wait for write locks in parallel tests, fixtures have exclusive access.
        let mut meta = ctx.legacy_meta(guard.read_permission())?;
        let repo = ctx.repo.get()?;

        let target_ref: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
        let target_sha = repo
            .find_reference(target_ref.as_ref())
            .context(
                "Old code basically doesn't work without a Target, need 'origin/main' to be setup",
            )?
            .id()
            .detach();
        meta.data_mut().default_target = Some(but_meta::virtual_branches_legacy_types::Target {
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            remote_url: "https://example.org/should-not-be-needed-in-our-tests".to_string(),
            sha: target_sha,
            push_remote_name: None,
        });
        meta.set_changed_to_necessitate_write();
        Ok(())
    }
}
