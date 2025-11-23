use crate::args::Args;
use crate::utils::OutputChannel;
use crate::{LegacyProject, command};

pub mod commits;
pub mod id;

pub fn get_or_init_non_bare_project(args: &Args) -> anyhow::Result<LegacyProject> {
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
        Ok(project)
    } else {
        anyhow::bail!("Bare repositories are not supported.");
    }
}
