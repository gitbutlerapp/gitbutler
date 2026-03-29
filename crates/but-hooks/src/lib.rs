//! Git hook management for GitButler.
//!
//! This crate provides two complementary modules:
//!
//! - [`hook_manager`] — detection and integration with external hook managers
//!   (e.g. prek). Used during `but setup` to decide whether GitButler should
//!   install its own hooks or defer to an existing manager.
//!
//! - [`managed_hooks`] — installation, update, and removal of GitButler's own
//!   managed hook scripts (pre-commit guard, post-checkout cleanup, pre-push
//!   guard).

pub mod hook_manager;
pub mod managed_hooks;
