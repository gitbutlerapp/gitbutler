mod agent;
mod capture;
mod capture_lock;
mod cli;
mod environment;
mod gitmeta;
pub mod projection;
mod redaction;
mod skim;
mod transcript;

pub use agent::Agent;
pub use cli::{Command, RelatedSessionTarget, run_from_dir};

#[cfg(test)]
pub(crate) fn hide_global_git_config() {
    use but_testsupport::gix_testtools::Env;
    static HIDDEN: std::sync::LazyLock<Env<'static>> = std::sync::LazyLock::new(|| {
        Env::new().set("GIT_CONFIG_NOSYSTEM", "1").set(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
    });
    std::sync::LazyLock::force(&HIDDEN);
}
