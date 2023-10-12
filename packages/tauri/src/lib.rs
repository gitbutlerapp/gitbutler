#![forbid(unsafe_code, dead_code)]
#![deny(
    clippy::all,
    clippy::perf,
    clippy::correctness,
    clippy::complexity,
    clippy::style,
    clippy::pedantic
)]
// clippy::restriction (see https://rust-lang.github.io/rust-clippy/master/index.html#/?groups=restriction)
// it is recommended to enable individually based on style and requirements
#![deny(
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    clippy::clone_on_ref_ptr,
    clippy::create_dir,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::default_numeric_fallback,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::exit,
    clippy::filetype_is_file,
    clippy::float_cmp_const,
    clippy::fn_to_numeric_cast_any,
    clippy::format_push_string,
    clippy::get_unwrap,
    clippy::integer_division,
    clippy::lossy_float_literal,
    clippy::mem_forget,
    clippy::mixed_read_write_in_expression,
    clippy::mutex_atomic,
    clippy::needless_raw_strings,
    clippy::non_ascii_literal,
    clippy::panic,
    clippy::print_stderr,
    clippy::pub_without_shorthand,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::redundant_type_annotations,
    clippy::ref_patterns,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::string_add,
    clippy::string_lit_chars_any,
    clippy::string_slice,
    clippy::string_to_string,
    clippy::suspicious_xor_used_as_pow,
    clippy::todo,
    clippy::try_err,
    clippy::unimplemented,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern,
    clippy::unseparated_literal_suffix,
    //TODO: 
    // clippy::if_then_some_else_none
    // clippy::partial_pub_fields
    // clippy::print_stdout
    // clippy::unwrap_in_result
    // clippy::unwrap_used
    // clippy::use_debug
)]
// reason = "noise and or false-positives"
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
    clippy::unnested_or_patterns
)]
//TODO: should probably be cleaned up as any of these could lead to panics or unexpected behaviour (the cast-ones)
#![allow(
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
