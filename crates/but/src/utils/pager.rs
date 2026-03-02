pub(crate) enum Pager {
    /// An external pager process (e.g. `less`)
    External(std::process::Child, std::process::ChildStdin),
    /// The built-in fallback pager.
    Builtin(minus::Pager),
}

/// We use less as the default pager as it's an excellent pager that is ubiquitous in UNIX-likes.
const DEFAULT_PAGER: &str = "less";

/// This default configuration makes less behave nicely for a CLI. See the less manual for details.
/// The options are:
/// - `F`: Quit if content fits on screen.
/// - `X`: Don't clear the screen on exit.
/// - `R`: Recognize ANSI color escape sequences and use them verbatim to *keep* styling.
/// - `S`: Don't wrap long lines. We don't want wrapping as we control the layout carefully.
const DEFAULT_PAGER_ARGS: &str = "FXRS";
const DEFAULT_PAGER_ENV_VAR: &str = "LESS";

/// Attempt to initialize a new pager.
///
/// We first try to spawn an external pager, and if that fails, we fall back on the builtin
/// one. If that _also_ fails we simply move on with our lives without a pager, as the app does
/// function without it. It's just less great.
///
/// # Safety
/// This function should never be called when a pager is running, especially not an external
/// one. That can lead to some _very_ funky things in the terminal. There is currently no use
/// case for initializing a pager more than once per CLI invocation, but if that need arises
/// this implementation needs to be extended to account for the fact that there may already be
/// a pager running that needs to be dropped before a new one is initialized.
pub(crate) fn try_init_pager() -> Option<Pager> {
    if let Some((child, stdin)) = try_spawn_external_pager() {
        Some(Pager::External(child, stdin))
    } else {
        match try_spawn_builtin_pager() {
            Ok(pager) => Some(Pager::Builtin(pager)),
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "Failed to initialize builtin pager"
                );
                None
            }
        }
    }
}

fn try_spawn_external_pager() -> Option<(std::process::Child, std::process::ChildStdin)> {
    use std::process::{Command, Stdio};

    let pager_override = std::env::var("BUT_PAGER").ok();
    let (program, is_default) = if let Some(cmd) = &pager_override {
        (cmd.as_ref(), false)
    } else if Command::new(DEFAULT_PAGER)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        (DEFAULT_PAGER, true)
    } else {
        return None;
    };

    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::piped());

    if is_default {
        // If we use the default pager, we're going to forcefully set our intended default
        // configuration for it. If a user wishes to have different configuration, they must
        // explicitly set the pager with e.g. `BUT_PAGER` (even if that explicitly makes it the
        // default pager).
        //
        // This could be considered a user configuration issue, but as it seems like macOS systems
        // tend to have some rather exotic configuration for less which messes with our intended
        // experience, we'll just set our own preference here.
        cmd.env(DEFAULT_PAGER_ENV_VAR, DEFAULT_PAGER_ARGS);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            tracing::debug!(?cmd, "Launched external pager");
            let stdin = child.stdin.take()?;
            Some((child, stdin))
        }
        Err(err) => {
            tracing::warn!(
                pager = program,
                error = %err,
                "Failed to start pager"
            );
            None
        }
    }
}

fn try_spawn_builtin_pager() -> Result<minus::Pager, minus::MinusError> {
    let pager = minus::Pager::new();
    pager.set_exit_strategy(minus::ExitStrategy::PagerQuit)?;
    pager.set_prompt("GitButler")?;
    Ok(pager)
}
