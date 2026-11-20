//! Git's credential store, which pushes from `but` read since the CLI turns askpass off. Giving
//! it the token is what makes `but push` and `git push` to GitButler Code work without a prompt.

use std::io::Write as _;
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result, bail};
use but_secret::Sensitive;

use crate::base_url;

/// Store `token` as `username`'s password for GitButler Code, setting up a credential helper for
/// it first if Git has none.
pub(crate) fn approve(username: &str, token: &Sensitive<String>) -> Result<()> {
    ensure_helper()?;
    run("approve", username, token)
}

/// Remove `token` from Git's credential store.
pub(crate) fn reject(username: &str, token: &Sensitive<String>) -> Result<()> {
    run("reject", username, token)
}

fn git() -> Command {
    let mut cmd = Command::new(gix::path::env::exe_invocation());
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

fn run(action: &str, username: &str, token: &Sensitive<String>) -> Result<()> {
    let url = url::Url::parse(&base_url()).context("GitButler Code's URL is not a URL")?;
    let host = crate::host()?;
    let mut child = git()
        .args(["credential", action])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to run git credential")?;
    let input = format!(
        "protocol={}\nhost={host}\nusername={username}\npassword={}\n\n",
        url.scheme(),
        token.0
    );
    child
        .stdin
        .take()
        .context("git credential has no input")?
        .write_all(input.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "git credential {action} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Git drops what `approve` hands it when no helper is configured. With none for GitButler
/// Code, set the platform's keychain helper for its URL only, leaving other hosts alone.
fn ensure_helper() -> Result<()> {
    let base = base_url();
    let configured = git()
        .args(["config", "--get-urlmatch", "credential.helper", &base])
        .output()
        .context("Failed to read Git's credential helper")?;
    if configured.status.success() && !configured.stdout.trim_ascii().is_empty() {
        return Ok(());
    }
    let helper = if cfg!(target_os = "macos") {
        "osxkeychain"
    } else if cfg!(windows) {
        "manager"
    } else {
        bail!(
            "Git has no credential helper to keep the GitButler Code token in. Set one, for example \
             `git config --global credential.helper libsecret`, and sign in again."
        )
    };
    let set = git()
        .args([
            "config",
            "--global",
            &format!("credential.{base}.helper"),
            helper,
        ])
        .output()
        .context("Failed to set Git's credential helper")?;
    if !set.status.success() {
        bail!(
            "Failed to set Git's credential helper: {}",
            String::from_utf8_lossy(&set.stderr).trim()
        );
    }
    Ok(())
}
