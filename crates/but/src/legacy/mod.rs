use but_ctx::{Context, LegacyProject};

use crate::{args::Args, command, utils::OutputChannel};

pub mod commits;

pub fn get_or_init_non_bare_ctx(args: &Args) -> anyhow::Result<Context> {
    let repo = gix::discover(&args.current_dir)?;
    if let Some(path) = repo.workdir() {
        let project = match LegacyProject::find_by_worktree_dir(path) {
            Ok(p) => Ok(p),
            Err(_e) => {
                command::legacy::init::repo(
                    path,
                    &mut OutputChannel::new_without_pager_non_json(args.format),
                    false,
                )?;
                LegacyProject::find_by_worktree_dir(path)
            }
        }?;
        Context::new_from_legacy_project(project)
    } else {
        anyhow::bail!("Bare repositories are not supported.");
    }
}
