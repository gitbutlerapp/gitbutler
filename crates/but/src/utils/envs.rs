//! Environment variables used by but.

pub const BUT_PAGER: &str = "BUT_PAGER";
pub const BUT_PAGER_DESCRIPTION: &str = "Sets the pager for large outputs. [default: less]";

pub const BUT_THEME: &str = "BUT_THEME";
pub const BUT_THEME_DESCRIPTION: &str =
    "Sets the theme for but. Options: dark, light. [default: dark]";

pub const ALL_ENVS: [(&str, &str); 2] = [
    (BUT_PAGER, BUT_PAGER_DESCRIPTION),
    (BUT_THEME, BUT_THEME_DESCRIPTION),
];
