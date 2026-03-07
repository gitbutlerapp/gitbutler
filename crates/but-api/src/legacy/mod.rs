#![allow(missing_docs)]
//! Legacy data structures and functionality tied to `gitbutler-*` crates.
//!

use but_ctx::ProjectHandleOrLegacyProjectId;
pub mod absorb;
pub mod askpass;
pub mod cherry_apply;
pub mod claude;
pub mod cli;
pub mod config;
pub mod diff;
pub mod forge;
pub mod git;
pub mod meta;
pub mod modes;
pub mod open;
pub mod oplog;
pub mod projects;
pub mod remotes;
pub mod repo;
pub mod rules;
pub mod secret;
pub mod settings;
pub mod stack;
pub mod users;
pub mod virtual_branches;
pub mod workspace;
pub mod worktree;

fn legacy_project(
    project_id: ProjectHandleOrLegacyProjectId,
) -> anyhow::Result<gitbutler_project::Project> {
    match project_id {
        ProjectHandleOrLegacyProjectId::ProjectHandle(handle) => {
            let ctx = but_ctx::Context::new_from_project_handle(handle)?;
            Ok(ctx.legacy_project)
        }
        ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id) => {
            gitbutler_project::get(ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id))
        }
    }
}
