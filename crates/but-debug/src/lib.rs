//! Debugging utilities exposed as a dedicated CLI.
#![deny(unsafe_code)]

use std::{ffi::OsString, io};

use anyhow::Result;
use clap::Parser;

pub mod args;
pub(crate) mod command;
mod metadata;
mod setup;
mod trace;

use args::{Args, Subcommands};

/// Parse CLI arguments and dispatch the requested subcommand.
pub fn handle_args(
    args: impl Iterator<Item = OsString>,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let args = Args::parse_from(args);
    trace::init(args.trace)?;

    let _span = tracing::info_span!("run").entered();
    match &args.cmd {
        Subcommands::Api(api_args) => command::api::run(&args, api_args, out, err),
        Subcommands::Dump(dump_args) => command::dump::run(&args, dump_args, out, err),
        Subcommands::Graph(graph_args) => command::graph::run(&args, graph_args, out, err),
        Subcommands::Apply(apply_args) => command::workspace::apply(&args, apply_args, out, err),
        Subcommands::Unapply(unapply_args) => {
            command::workspace::unapply(&args, unapply_args, out, err)
        }
        Subcommands::Revision(revision_args) => command::revision::run(&args, revision_args, out),
    }
}
