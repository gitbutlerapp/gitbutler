//! Environment variables used by but.

/// Selects `human` or `json` output when `--json` is not passed.
///
/// Unknown values are ignored. This is publicly documented in the CLI help via [`ALL_ENVS`].
pub const BUT_OUTPUT_FORMAT: &str = "BUT_OUTPUT_FORMAT";
pub const BUT_OUTPUT_FORMAT_DESCRIPTION: &str =
    "Sets the output format when --json is not passed. Options: human, json.";

pub const BUT_PAGER: &str = "BUT_PAGER";
pub const BUT_PAGER_DESCRIPTION: &str = "Sets the pager for large outputs. [default: less]";

pub const BUT_THEME: &str = "BUT_THEME";
pub const BUT_THEME_DESCRIPTION: &str = "Sets the theme for but. Options: dark, light. [default: detected from the terminal, falling back to dark]";

#[cfg(target_os = "linux")]
pub const BUT_CREDENTIALS_DIRECTORY: &str = "BUT_CREDENTIALS_DIRECTORY";
#[cfg(target_os = "linux")]
pub const BUT_CREDENTIALS_DIRECTORY_DESCRIPTION: &str = "CAUTION: Store credentials as UNENCRYPTED plaintext files in this directory instead of using system keychain.";

pub const ALL_ENVS: &[(&str, &str)] = &[
    (BUT_OUTPUT_FORMAT, BUT_OUTPUT_FORMAT_DESCRIPTION),
    (BUT_PAGER, BUT_PAGER_DESCRIPTION),
    (BUT_THEME, BUT_THEME_DESCRIPTION),
    #[cfg(target_os = "linux")]
    (
        BUT_CREDENTIALS_DIRECTORY,
        BUT_CREDENTIALS_DIRECTORY_DESCRIPTION,
    ),
];
