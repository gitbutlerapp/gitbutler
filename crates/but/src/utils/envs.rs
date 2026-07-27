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
pub const BUT_THEME_DESCRIPTION: &str =
    "Sets the theme for but. Options: dark, light. [default: dark]";

pub const ALL_ENVS: [(&str, &str); 3] = [
    (BUT_OUTPUT_FORMAT, BUT_OUTPUT_FORMAT_DESCRIPTION),
    (BUT_PAGER, BUT_PAGER_DESCRIPTION),
    (BUT_THEME, BUT_THEME_DESCRIPTION),
];
