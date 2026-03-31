//! A place for each command, i.e. `but foo` as `pub mod foo` here.
#[cfg(feature = "legacy")]
pub mod legacy;

pub mod alias;
pub mod branch;
pub mod commit;
pub mod completions;
pub mod config;
pub mod eval_hook;
pub(crate) mod git_config;
pub mod gui;
pub mod help;
pub mod r#move;
pub mod onboarding;
pub mod push;
pub mod skill;
pub mod update;
