//! In place of commands.rs
use anyhow::Result;
use but_api_macros::but_api;
use but_ctx::Context;
use gitbutler_repo::{GitRemote, RepoCommands};
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn list_remotes(ctx: &Context) -> Result<Vec<GitRemote>> {
    ctx.remotes()
}

#[but_api]
#[instrument(err(Debug))]
pub fn add_remote(ctx: &Context, name: String, url: String) -> Result<()> {
    ctx.add_remote(&name, &url)
}
