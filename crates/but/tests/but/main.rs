mod command;
// TODO: Id tests can be on integration level, but shouldn't involve the CLI
#[cfg(feature = "legacy")]
mod id;
#[cfg(feature = "legacy")]
mod journey;
pub mod utils;
