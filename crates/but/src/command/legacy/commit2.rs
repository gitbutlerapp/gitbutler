use serde::Serialize;

use crate::{
    CliResult,
    args::commit2::Platform,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

#[derive(Serialize)]
#[must_use]
pub struct CommitOutcome {}

impl CliOutputHuman for CommitOutcome {
    fn on_human(self, _out: &mut dyn WriteWithUtils, _theme: &Theme) -> anyhow::Result<()> {
        Ok(())
    }
}

impl CliOutput for CommitOutcome {
    fn on_shell(self, _out: &mut dyn WriteWithUtils) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_json(self) -> impl serde::Serialize {
        self
    }
}

pub fn commit(
    _ctx: &mut but_ctx::Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<CommitOutcome> {
    let Platform {} = args;
    Ok(CommitOutcome {})
}
