use std::io::{Read, Write};

use anyhow::{Context, Result};

use crate::{gb_repository, project_repository, reader, sessions};

pub const GITBUTLER_INTEGRATION_BRANCH_NAME: &str = "gitbutler/integration";
pub const GITBUTLER_INTEGRATION_REFERENCE: &str = "refs/heads/gitbutler/integration";

pub fn update_gitbutler_integration(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<()> {
    let target = gb_repository
        .default_target()
        .context("failed to get target")?
        .context("no target set")?;

    let repo = &project_repository.git_repository;

    // write the currrent target sha to a temp branch as a parent
    repo.reference(
        GITBUTLER_INTEGRATION_REFERENCE,
        target.sha,
        true,
        "update target",
    )?;

    // get commit object from target.sha
    let target_commit = repo.find_commit(target.sha)?;

    // get current repo head for reference
    let head = repo.head()?;
    let mut prev_head = head.name().unwrap().to_string();
    let mut prev_sha = head.target().unwrap().to_string();
    let integration_file = repo.path().join("integration");
    if prev_head != GITBUTLER_INTEGRATION_REFERENCE {
        // we are moving from a regular branch to our gitbutler integration branch, save the original
        // write a file to .git/integration with the previous head and name
        let mut file = std::fs::File::create(integration_file)?;
        prev_head.push(':');
        prev_head.push_str(&prev_sha);
        file.write_all(prev_head.as_bytes())?;
    } else {
        // read the .git/integration file
        if let Ok(mut integration_file) = std::fs::File::open(integration_file) {
            let mut prev_data = String::new();
            integration_file.read_to_string(&mut prev_data)?;
            let parts: Vec<&str> = prev_data.split(':').collect();

            prev_head = parts[0].to_string();
            prev_sha = parts[1].to_string();
        }
    }

    // commit index to temp head for the merge
    repo.set_head(GITBUTLER_INTEGRATION_REFERENCE)
        .context("failed to set head")?;

    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    // get all virtual branches, we need to try to update them all
    let all_virtual_branches = super::iterator::BranchIterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<super::branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;

    let applied_virtual_branches = all_virtual_branches
        .iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    let merge_options = git2::MergeOptions::new();
    let base_tree = target_commit.tree()?;
    let mut final_tree = target_commit.tree()?;
    for branch in &applied_virtual_branches {
        // merge this branches tree with our tree
        let branch_head = repo.find_commit(branch.head)?;
        let branch_tree = branch_head.tree()?;
        if let Ok(mut result) =
            repo.merge_trees(&base_tree, &final_tree, &branch_tree, Some(&merge_options))
        {
            if !result.has_conflicts() {
                let final_tree_oid = result.write_tree_to(repo)?;
                final_tree = repo.find_tree(final_tree_oid)?;
            }
        }
    }

    // message that says how to get back to where they were
    let mut message = "GitButler Integration Commit".to_string();
    message.push_str("\n\n");
    message.push_str(
        "This is an integration commit for the virtual branches that GitButler is tracking.\n\n",
    );
    message.push_str(
        "Due to GitButler managing multiple virtual branches, you cannot switch back and\n",
    );
    message.push_str("forth between git branches and virtual branches easily. \n\n");

    message.push_str("If you switch to another branch, GitButler will need to be reinitialized.\n");
    message.push_str("If you commit on this branch, GitButler will throw it away.\n\n");
    message.push_str("Here are the branches that are currently applied:\n");
    for branch in &applied_virtual_branches {
        message.push_str(" - ");
        message.push_str(branch.name.as_str());
        let branch_name = super::name_to_branch(branch.name.as_str());
        message.push_str(format!(" (gitbutler/{})", &branch_name).as_str());
        message.push('\n');

        if branch.head != target.sha {
            message.push_str("   branch head: ");
            message.push_str(&branch.head.to_string());
            message.push('\n');
        }
        for file in &branch.ownership.files {
            message.push_str("   - ");
            message.push_str(&file.file_path.display().to_string());
            message.push('\n');
        }
    }
    message.push_str("\nYour previous branch was: ");
    message.push_str(&prev_head);
    message.push_str("\n\n");
    message.push_str("The sha for that commit was: ");
    message.push_str(&prev_sha);
    message.push_str("\n\n");
    message.push_str("For more information about what we're doing here, check out our docs:\n");
    message.push_str("https://docs.gitbutler.com/features/virtual-branches/integration-branch\n");

    let committer = git2::Signature::now("GitButler", "gitbutler@gitbutler.com")?;

    repo.commit(
        Some("HEAD"),
        &committer,
        &committer,
        &message,
        &final_tree,
        &[&target_commit],
    )?;

    // write final_tree as the current index
    let mut index = repo.index()?;
    index.read_tree(&final_tree)?;
    index.write()?;

    // finally, update the refs/gitbutler/ heads to the states of the current virtual branches
    for branch in &all_virtual_branches {
        let wip_tree = repo.find_tree(branch.tree)?;
        let mut branch_head = repo.find_commit(branch.head)?;
        let head_tree = branch_head.tree()?;

        // create a wip commit if there is wip
        if head_tree.id() != wip_tree.id() {
            let mut message = "GitButler WIP Commit".to_string();
            message.push_str("\n\n");
            message.push_str("This is a WIP commit for the virtual branch '");
            message.push_str(branch.name.as_str());
            message.push_str("'\n\n");
            message.push_str("This commit is used to store the state of the virtual branch\n");
            message.push_str("while you are working on it. It is not meant to be used for\n");
            message.push_str("anything else.\n\n");
            let branch_head_oid = repo.commit(
                None,
                &committer,
                &committer,
                &message,
                &wip_tree,
                &[&branch_head],
            )?;
            branch_head = repo.find_commit(branch_head_oid)?;
        }

        let branch_name = super::name_to_branch(branch.name.as_str());
        let branch_ref = format!("refs/gitbutler/{}", branch_name);
        repo.reference(&branch_ref, branch_head.id(), true, "update virtual branch")?;
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("head is detached")]
    DetachedHead,
    #[error("head is {0}")]
    InvalidHead(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn verify_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<(), VerifyError> {
    match project_repository
        .get_head()
        .context("failed to get head")
        .map_err(VerifyError::Other)?
        .name()
    {
        Some(GITBUTLER_INTEGRATION_REFERENCE) => Ok(()),
        None => {
            super::vbranch::mark_all_unapplied(gb_repository).map_err(VerifyError::Other)?;
            Err(VerifyError::DetachedHead)
        }
        Some(head_name) => {
            super::vbranch::mark_all_unapplied(gb_repository).map_err(VerifyError::Other)?;
            Err(VerifyError::InvalidHead(head_name.to_string()))
        }
    }
}
