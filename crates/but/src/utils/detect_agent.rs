//! Agent detection now lives in `but-skill` so the desktop app can share it.
//!
//! Re-exported under the original path so the CLI's call sites — and the
//! `AGENT_ENVIRONMENT_VARIABLES` surface the integration tests rely on — keep
//! working unchanged.

pub use but_skill::detect::*;
