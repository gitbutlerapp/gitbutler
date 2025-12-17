mod apply;
use crate::args::branch::Subcommands;
use crate::utils::OutputChannel;
pub use apply::apply;
use but_ctx::Context;

pub fn handle(
    cmd: Option<Subcommands>,
    ctx: Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    match cmd {
        None => {
            todo!("implement list and call recursively")
        }
        Some(cmd) => match cmd {
            Subcommands::Apply { branch_name } => apply(ctx, &branch_name, out),
        },
    }
}
