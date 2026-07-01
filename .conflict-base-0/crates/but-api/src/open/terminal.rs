use serde::{Deserialize, Serialize};

/// Configuration for a terminal application.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOption {
    /// The name of the process/program to run.
    pub identifier: String,
    /// Human-readable terminal name shown to users in picker and settings UI.
    pub display_name: String,
    /// Operating-system family this option applies to: `macos`, `windows`, or
    /// `linux`.
    pub platform: String,
}

// TODO: this list was in the frontend before, now it could be backend-integrated
//       more so they don't get out of sync, when moving this out of legacy.
/// Terminal options ordered by preference.
/// In [`get_recommended_terminal_for_platform()`], platform binaries that are mentioned first will be returned first
/// if once they are discovered in `PATH`.
///
/// # WARNING: keep in sync with [crate::open::open_in_terminal()].
#[cfg(feature = "legacy")]
const ALL_TERMINALS: &[(&str, &str, &str)] = &[
    // macOS
    ("terminal", "Terminal", "macos"),
    ("iterm2", "iTerm2", "macos"),
    ("ghostty", "Ghostty", "macos"),
    ("warp", "Warp", "macos"),
    ("alacritty-mac", "Alacritty", "macos"),
    ("wezterm-mac", "WezTerm", "macos"),
    ("hyper", "Hyper", "macos"),
    ("kitty", "Kitty", "macos"),
    // Windows — wt first (detected via which), falls back to always-present powershell/cmd
    ("wt", "Windows Terminal", "windows"),
    ("powershell", "PowerShell", "windows"),
    ("cmd", "Command Prompt", "windows"),
    // Linux
    ("gnome-terminal", "GNOME Terminal", "linux"),
    ("konsole", "Konsole", "linux"),
    ("xfce4-terminal", "XFCE Terminal", "linux"),
    ("alacritty", "Alacritty", "linux"),
    ("ghostty", "Ghostty", "linux"),
    ("warp", "Warp", "linux"),
    ("hyper", "Hyper", "linux"),
    ("wezterm", "WezTerm", "linux"),
    ("kitty", "Kitty", "linux"),
    ("cosmic-term", "COSMIC Terminal", "linux"),
    ("ptyxis", "Ptyxis", "linux"),
];

/// Resolves a terminal identifier to the binary name used to detect/launch it.
/// This mapping is shared between terminal detection and the open-in-terminal logic.
pub(super) fn terminal_binary(identifier: &str) -> &str {
    match identifier {
        "warp" => "warp-terminal",
        other => other,
    }
}

/// Returns all available terminal options for the given platform.
///
/// The list is empty if the `platform` isn't one of `linux`, `macos` or `windows`, case-sensitive.
///
/// ## Why Legacy?
///
/// It's born as a port from the frontend, which means it's frontend centric and serves mostly that right now.
/// But it could be generalised as more consumers join in.
/// Big question is if the backend shouldn't just know the platform… If it is needed, it should be an enum here.
#[cfg(feature = "legacy")]
#[but_api_macros::but_api]
#[tracing::instrument(err(Debug))]
pub fn get_terminal_options_for_platform(platform: String) -> anyhow::Result<Vec<TerminalOption>> {
    Ok(ALL_TERMINALS
        .iter()
        .filter(|(_, _, p)| *p == platform)
        .map(|(id, name, p)| TerminalOption {
            identifier: id.to_string(),
            display_name: name.to_string(),
            platform: p.to_string(),
        })
        .collect())
}

/// Returns the recommended terminal for the `platform` by probing what is installed.
/// The list is empty if the `platform` isn't one of `linux`, `macos` or `windows`, case-sensitive.
///
/// On macOS, Terminal.app is always present so it is returned immediately.
/// On Linux and Windows the ordered list is walked and the first installed one wins.
///
/// ## Why Legacy?
///
/// It's born as a port from the frontend, which means it's frontend centric and serves mostly that right now.
/// `platform` can be inferred.
#[cfg(feature = "legacy")]
#[but_api_macros::but_api]
#[tracing::instrument(err(Debug))]
pub fn get_recommended_terminal_for_platform(
    platform: String,
) -> anyhow::Result<Option<TerminalOption>> {
    // macOS is special-cased because Terminal.app is always available on every Mac and is the system default
    if platform == "macos" {
        return Ok(Some(TerminalOption {
            identifier: "terminal".to_string(),
            display_name: "Terminal".to_string(),
            platform: "macos".to_string(),
        }));
    }

    Ok(get_terminal_options_for_platform(platform)?
        .into_iter()
        .find(|t| which::which(terminal_binary(&t.identifier)).is_ok()))
}
