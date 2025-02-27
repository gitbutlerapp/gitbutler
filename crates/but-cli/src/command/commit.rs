use crate::command::debug_print;
use anyhow::bail;
use but_core::TreeChange;
use but_workspace::commit_engine::DiffSpec;
use gitbutler_project::Project;

pub fn commit(
    repo: gix::Repository,
    project: Option<Project>,
    message: Option<&str>,
    amend: bool,
    parent_revspec: Option<&str>,
    stack_segment_ref: Option<&str>,
) -> anyhow::Result<()> {
    if message.is_none() && !amend {
        bail!("Need a message when creating a new commit");
    }
    let parent_id = parent_revspec
        .map(|revspec| repo.rev_parse_single(revspec).map_err(anyhow::Error::from))
        .unwrap_or_else(|| Ok(repo.head_id()?))?
        .detach();
    debug_print(
        but_workspace::commit_engine::create_commit_and_update_refs_with_project(
            &repo,
            project.as_ref().map(|p| (p, None)),
            if amend {
                if message.is_some() {
                    bail!("Messages aren't used when amending");
                }
                but_workspace::commit_engine::Destination::AmendCommit(parent_id)
            } else {
                but_workspace::commit_engine::Destination::NewCommit {
                    parent_commit_id: Some(parent_id),
                    message: message.unwrap_or_default().to_owned(),
                    stack_segment_ref: stack_segment_ref.map(|rn| rn.try_into()).transpose()?,
                }
            },
            None,
            to_whole_file_diffspec(but_core::diff::worktree_changes(&repo)?.changes),
            0, /* context-lines */
        )?,
    )?;
    Ok(())
}

fn to_whole_file_diffspec(changes: Vec<TreeChange>) -> Vec<DiffSpec> {
    changes
        .into_iter()
        .map(|change| DiffSpec {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path,
            hunk_headers: Vec::new(),
        })
        .collect()
}
