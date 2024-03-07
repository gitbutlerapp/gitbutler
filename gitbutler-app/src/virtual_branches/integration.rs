use std::io::{Read, Write};

use anyhow::{Context, Result};
use lazy_static::lazy_static;

use crate::{
    gb_repository,
    git::{self},
    project_repository::{self, LogUntil},
    reader, sessions,
    virtual_branches::branch::BranchCreateRequest,
};

use super::errors;

lazy_static! {
    pub static ref GITBUTLER_INTEGRATION_REFERENCE: git::LocalRefname =
        git::LocalRefname::new("gitbutler/integration", None);
}

const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME: &str = "GitButler";
const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";

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
        &GITBUTLER_INTEGRATION_REFERENCE.clone().into(),
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
    if prev_head != GITBUTLER_INTEGRATION_REFERENCE.to_string() {
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
    repo.set_head(&GITBUTLER_INTEGRATION_REFERENCE.clone().into())
        .context("failed to set head")?;

    let latest_session = gb_repository
        .get_latest_session()
        .context("failed to get latest session")?
        .context("latest session not found")?;
    let session_reader = sessions::Reader::open(gb_repository, &latest_session)
        .context("failed to open current session")?;

    // get all virtual branches, we need to try to update them all
    let all_virtual_branches = super::iterator::BranchIterator::new(&session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<super::branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;

    let applied_virtual_branches = all_virtual_branches
        .iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    let base_tree = target_commit.tree()?;
    let mut final_tree = target_commit.tree()?;
    for branch in &applied_virtual_branches {
        // merge this branches tree with our tree
        let branch_head = repo.find_commit(branch.head)?;
        let branch_tree = branch_head.tree()?;
        if let Ok(mut result) = repo.merge_trees(&base_tree, &final_tree, &branch_tree) {
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
        message.push_str(format!(" ({})", &branch.refname()).as_str());
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

    let committer = git::Signature::now(
        GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME,
        GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL,
    )?;

    repo.commit(
        Some(&"refs/heads/gitbutler/integration".parse().unwrap()),
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

        repo.reference(
            &branch.refname().into(),
            branch_head.id(),
            true,
            "update virtual branch",
        )?;
    }

    Ok(())
}

pub fn verify_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<(), errors::VerifyError> {
    verify_head_is_set(project_repository)?;
    verify_head_is_clean(gb_repository, project_repository)?;
    Ok(())
}

fn verify_head_is_clean(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<(), errors::VerifyError> {
    let head_commit = project_repository
        .git_repository
        .head()
        .context("failed to get head")?
        .peel_to_commit()
        .context("failed to peel to commit")?;

    let mut extra_commits = project_repository
        .log(
            head_commit.id(),
            LogUntil::When(Box::new(|commit| Ok(is_integration_commit(commit)))),
        )
        .context("failed to get log")?;

    let integration_commit = extra_commits.pop();

    if integration_commit.is_none() {
        // no integration commit found
        return Err(errors::VerifyError::NoIntegrationCommit);
    }

    if extra_commits.is_empty() {
        // no extra commits found, so we're good
        return Ok(());
    }

    project_repository
        .git_repository
        .reset(
            integration_commit.as_ref().unwrap(),
            git2::ResetType::Soft,
            None,
        )
        .context("failed to reset to integration commit")?;

    let mut new_branch = super::create_virtual_branch(
        gb_repository,
        project_repository,
        &BranchCreateRequest {
            name: extra_commits
                .last()
                .unwrap()
                .message()
                .map(ToString::to_string),
            ..Default::default()
        },
    )
    .context("failed to create virtual branch")?;

    // rebasing the extra commits onto the new branch
    let writer = super::branch::Writer::new(gb_repository).context("failed to create writer")?;
    extra_commits.reverse();
    let mut head = new_branch.head;
    for commit in extra_commits {
        let new_branch_head = project_repository
            .git_repository
            .find_commit(head)
            .context("failed to find new branch head")?;

        let rebased_commit_oid = project_repository
            .git_repository
            .commit(
                None,
                &commit.author(),
                &commit.committer(),
                commit.message().unwrap(),
                &commit.tree().unwrap(),
                &[&new_branch_head],
            )
            .context(format!(
                "failed to rebase commit {} onto new branch",
                commit.id()
            ))?;

        let rebased_commit = project_repository
            .git_repository
            .find_commit(rebased_commit_oid)
            .context(format!(
                "failed to find rebased commit {}",
                rebased_commit_oid
            ))?;

        new_branch.head = rebased_commit.id();
        new_branch.tree = rebased_commit.tree_id();
        writer
            .write(&mut new_branch)
            .context("failed to write branch")?;

        head = rebased_commit.id();
    }
    Ok(())
}

fn verify_head_is_set(
    project_repository: &project_repository::Repository,
) -> Result<(), errors::VerifyError> {
    match project_repository
        .get_head()
        .context("failed to get head")
        .map_err(errors::VerifyError::Other)?
        .name()
    {
        Some(refname) if refname.to_string() == GITBUTLER_INTEGRATION_REFERENCE.to_string() => {
            Ok(())
        }
        None => Err(errors::VerifyError::DetachedHead),
        Some(head_name) => Err(errors::VerifyError::InvalidHead(head_name.to_string())),
    }
}

fn is_integration_commit(commit: &git::Commit) -> bool {
    is_integration_commit_author(commit) && is_integration_commit_message(commit)
}

fn is_integration_commit_author(commit: &git::Commit) -> bool {
    is_integration_commit_author_email(commit) && is_integration_commit_author_name(commit)
}

fn is_integration_commit_author_email(commit: &git::Commit) -> bool {
    commit.author().email().map_or(false, |email| {
        email == GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL
    })
}

fn is_integration_commit_author_name(commit: &git::Commit) -> bool {
    commit.author().name().map_or(false, |name| {
        name == GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME
    })
}

fn is_integration_commit_message(commit: &git::Commit) -> bool {
    commit.message().map_or(false, |message| {
        message.starts_with("GitButler Integration Commit")
    })
}
