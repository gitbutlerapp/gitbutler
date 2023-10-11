#![forbid(unsafe_code)]
#![deny(
    clippy::all,
    clippy::perf,
    clippy::correctness,
    clippy::complexity,
    clippy::style,
    clippy::pedantic
)]
#![allow(
    clippy::missing_errors_doc,
    clippy::used_underscore_binding,
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::implicit_hasher,
    clippy::if_not_else,
    clippy::return_self_not_must_use,
    clippy::inconsistent_struct_constructor,
    clippy::match_wildcard_for_single_variants,
    clippy::unnested_or_patterns,
    //TODO: should probably be cleaned up as any of these could lead to panics or unexpected behaviour (the cast-ones)
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::match_same_arms,
    clippy::similar_names
)]

pub mod analytics;
pub mod app;
pub mod assets;
pub mod bookmarks;
pub mod commands;
pub mod database;
pub mod dedup;
pub mod deltas;
pub mod error;
pub mod events;
pub mod fs;
pub mod gb_repository;
pub mod git;
pub mod github;
pub mod keys;
pub mod lock;
pub mod logs;
pub mod paths;
pub mod project_repository;
pub mod projects;
pub mod reader;
pub mod search;
pub mod sessions;
pub mod storage;
pub mod users;
pub mod virtual_branches;
pub mod watcher;
pub mod writer;
pub mod zip;

#[cfg(test)]
pub mod test_utils;
