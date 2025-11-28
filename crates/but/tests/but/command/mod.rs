//! Put command-specific tests here. They should be focused on what's important for each command.
//! Ideally they *show* the initial state, and the *post* state, to validate the commands actually do what they claim.
mod branch;
#[cfg(feature = "legacy")]
mod commit;
#[cfg(feature = "legacy")]
mod describe;
mod format;
mod help;
#[cfg(feature = "legacy")]
mod rub;
#[cfg(feature = "legacy")]
mod status;
