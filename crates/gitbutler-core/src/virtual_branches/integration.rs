use std::{path::PathBuf, vec};

use anyhow::{anyhow, bail, Context, Result};
use bstr::ByteSlice;
use lazy_static::lazy_static;

use super::VirtualBranchesHandle;
use crate::git::RepositoryExt;
use crate::virtual_branches::errors::Marker;
use crate::{
    git::{self, CommitExt},
    project_repository::{self, conflicts, LogUntil},
    virtual_branches::branch::BranchCreateRequest,
};

lazy_static! {
    pub static ref GITBUTLER_INTEGRATION_REFERENCE: git::LocalRefname =
        git::LocalRefname::new("gitbutler/integration", None);
}

const WORKSPACE_HEAD: &str = "Workspace Head";
pub const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME: &str = "GitButler";
pub const GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";

pub fn get_integration_commiter<'a>() -> Result<git2::Signature<'a>> {
    Ok(git2::Signature::now(
        GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME,
        GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL,
    )?)
}

// Creates and returns a merge commit of all active branch heads.
//
// This is the base against which we diff the working directory to understand
// what files have been modified.
pub fn get_workspace_head(
    vb_state: &VirtualBranchesHandle,
    project_repo: &project_repository::Repository,
) -> Result<git2::Oid> {
    let target = vb_state
        .get_default_target()
        .context("failed to get target")?;
    let repo: &git2::Repository = project_repo.repo();
    let vb_state = project_repo.project().virtual_branches();

    let all_virtual_branches = vb_state.list_branches()?;
    let branches = all_virtual_branches.iter().collect::<Vec<_>>();

    let target_commit = repo.find_commit(target.sha)?;
    let mut workspace_tree = target_commit.tree()?;

    if conflicts::is_conflicting(project_repo, None)? {
        let merge_parent =
            conflicts::merge_parent(project_repo)?.ok_or(anyhow!("No merge parent"))?;
        let first_branch = branches.first().ok_or(anyhow!("No branches"))?;

        let merge_base = repo.merge_base(first_branch.head, merge_parent)?;
        workspace_tree = repo.find_commit(merge_base)?.tree()?;
    } else {
        for branch in &branches {
            let branch_tree = repo.find_commit(branch.head)?.tree()?;
            let merge_tree = repo.find_commit(target.sha)?.tree()?;
            let mut index = repo.merge_trees(&merge_tree, &workspace_tree, &branch_tree, None)?;

            if !index.has_conflicts() {
                workspace_tree = repo.find_tree(index.write_tree_to(repo)?)?;
            } else {
                return Err(anyhow!("Merge conflict between base and {:?}", branch.name));
            }
        }
    }

    let branch_heads = branches
        .iter()
        .map(|b| repo.find_commit(b.head))
        .collect::<Result<Vec<_>, _>>()?;
    let branch_head_refs = branch_heads.iter().collect::<Vec<_>>();

    // If no branches are applied then the workspace head is the target.
    if branch_head_refs.is_empty() {
        return Ok(target_commit.id());
    }

    // TODO(mg): Can we make this a constant?
    let committer = get_integration_commiter()?;

    let mut heads: Vec<git2::Commit<'_>> = branches
        .iter()
        .filter(|b| b.head != target.sha)
        .map(|b| repo.find_commit(b.head))
        .filter_map(Result::ok)
        .collect();

    if heads.is_empty() {
        heads = vec![target_commit]
    }

    // TODO: Why does commit only accept a slice of commits? Feels like we
    // could make use of AsRef with the right traits.
    let head_refs: Vec<&git2::Commit<'_>> = heads.iter().collect();

    let workspace_head_id = repo.commit(
        None,
        &committer,
        &committer,
        WORKSPACE_HEAD,
        &workspace_tree,
        head_refs.as_slice(),
    )?;
    Ok(workspace_head_id)
}

// Before switching the user to our gitbutler integration branch we save
// the current branch into a text file. It is used in generating the commit
// message for integration branch, as a helpful hint about how to get back
// to where you were.
struct PreviousHead {
    head: String,
    sha: String,
}

fn read_integration_file(path: &PathBuf) -> Result<Option<PreviousHead>> {
    if let Ok(prev_data) = std::fs::read_to_string(path) {
        let parts: Vec<&str> = prev_data.split(':').collect();
        let prev_head = parts[0].to_string();
        let prev_sha = parts[1].to_string();
        Ok(Some(PreviousHead {
            head: prev_head,
            sha: prev_sha,
        }))
    } else {
        Ok(None)
    }
}

fn write_integration_file(head: &git2::Reference, path: PathBuf) -> Result<()> {
    let sha = head.target().unwrap().to_string();
    std::fs::write(path, format!(":{}", sha))?;
    Ok(())
}
pub fn update_gitbutler_integration(
    vb_state: &VirtualBranchesHandle,
    project_repository: &project_repository::Repository,
) -> Result<git2::Oid> {
    let target = vb_state
        .get_default_target()
        .context("failed to get target")?;

    let repo: &git2::Repository = project_repository.repo();

    // get commit object from target.sha
    let target_commit = repo.find_commit(target.sha)?;

    // get current repo head for reference
    let head_ref = repo.head()?;
    let integration_filepath = repo.path().join("integration");
    let mut prev_branch = read_integration_file(&integration_filepath)?;
    if let Some(branch) = &prev_branch {
        if branch.head != GITBUTLER_INTEGRATION_REFERENCE.to_string() {
            // we are moving from a regular branch to our gitbutler integration branch, write a file to
            // .git/integration with the previous head and name
            write_integration_file(&head_ref, integration_filepath)?;
            prev_branch = Some(PreviousHead {
                head: head_ref.target().unwrap().to_string(),
                sha: head_ref.target().unwrap().to_string(),
            });
        }
    }

    let vb_state = project_repository.project().virtual_branches();

    // get all virtual branches, we need to try to update them all
    let all_virtual_branches = vb_state
        .list_branches()
        .context("failed to list virtual branches")?;

    let integration_commit =
        repo.find_commit(get_workspace_head(&vb_state, project_repository)?)?;
    let integration_tree = integration_commit.tree()?;

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
    for branch in &all_virtual_branches {
        message.push_str(" - ");
        message.push_str(branch.name.as_str());
        message.push_str(format!(" ({})", &branch.refname()).as_str());
        message.push('\n');

        if branch.head != target.sha {
            message.push_str("   branch head: ");
            message.push_str(&branch.head.to_string());
            message.push('\n');
        }
        for file in &branch.ownership.claims {
            message.push_str("   - ");
            message.push_str(&file.file_path.display().to_string());
            message.push('\n');
        }
    }
    if let Some(prev_branch) = prev_branch {
        message.push_str("\nYour previous branch was: ");
        message.push_str(&prev_branch.head);
        message.push_str("\n\n");
        message.push_str("The sha for that commit was: ");
        message.push_str(&prev_branch.sha);
        message.push_str("\n\n");
    }
    message.push_str("For more information about what we're doing here, check out our docs:\n");
    message.push_str("https://docs.gitbutler.com/features/virtual-branches/integration-branch\n");

    let committer = get_integration_commiter()?;

    // It would be nice if we could pass an `update_ref` parameter to this function, but that
    // requires committing to the tip of the branch, and we're mostly replacing the tip.
    let final_commit = repo.commit(
        None,
        &committer,
        &committer,
        &message,
        &integration_commit.tree()?,
        &[&target_commit],
    )?;

    // Create or replace the integration branch reference, then set as HEAD.
    repo.reference(
        &GITBUTLER_INTEGRATION_REFERENCE.clone().to_string(),
        final_commit,
        true,
        "updated integration commit",
    )?;
    repo.set_head(&GITBUTLER_INTEGRATION_REFERENCE.clone().to_string())?;

    let mut index = repo.index()?;
    index.read_tree(&integration_tree)?;
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
                // None,
            )?;
            branch_head = repo.find_commit(branch_head_oid)?;
        }

        repo.reference(
            &branch.refname().to_string(),
            branch_head.id(),
            true,
            "update virtual branch",
        )?;
    }

    Ok(final_commit)
}

pub fn verify_branch(project_repository: &project_repository::Repository) -> Result<()> {
    project_repository
        .verify_current_branch_name()
        .and_then(|me| me.verify_head_is_set())
        .and_then(|me| me.verify_head_is_clean())
        .context(Marker::VerificationFailure)?;
    Ok(())
}

impl project_repository::Repository {
    fn verify_head_is_set(&self) -> Result<&Self> {
        match self.get_head().context("failed to get head")?.name() {
            Some(refname) if *refname == GITBUTLER_INTEGRATION_REFERENCE.to_string() => Ok(self),
            Some(head_name) => Err(invalid_head_err(head_name)),
            None => Err(anyhow!(
                "project in detached head state. Please checkout {} to continue",
                GITBUTLER_INTEGRATION_REFERENCE.branch()
            )),
        }
    }

    // Returns an error if repo head is not pointing to the integration branch.
    fn verify_current_branch_name(&self) -> Result<&Self> {
        match self.get_head()?.name() {
            Some(head) => {
                let head_name = head.to_string();
                if head_name != GITBUTLER_INTEGRATION_REFERENCE.to_string() {
                    return Err(invalid_head_err(&head_name));
                }
                Ok(self)
            }
            None => Err(anyhow!("Repo HEAD is unavailable")),
        }
    }

    fn verify_head_is_clean(&self) -> Result<&Self> {
        let head_commit = self
            .repo()
            .head()
            .context("failed to get head")?
            .peel_to_commit()
            .context("failed to peel to commit")?;

        let vb_handle = VirtualBranchesHandle::new(self.project().gb_dir());
        let default_target = vb_handle
            .get_default_target()
            .context("failed to get default target")?;

        let mut extra_commits = self
            .log(head_commit.id(), LogUntil::Commit(default_target.sha))
            .context("failed to get log")?;

        let integration_commit = extra_commits.pop();

        if integration_commit.is_none() {
            // no integration commit found
            bail!("gibButler's integration commit not found on head");
        }

        if extra_commits.is_empty() {
            // no extra commits found, so we're good
            return Ok(self);
        }

        self.repo()
            .reset(
                integration_commit.as_ref().unwrap().as_object(),
                git2::ResetType::Soft,
                None,
            )
            .context("failed to reset to integration commit")?;

        let mut new_branch = super::create_virtual_branch(
            self,
            &BranchCreateRequest {
                name: extra_commits
                    .last()
                    .map(|commit| commit.message_bstr().to_string()),
                ..Default::default()
            },
        )
        .context("failed to create virtual branch")?;

        // rebasing the extra commits onto the new branch
        let vb_state = self.project().virtual_branches();
        extra_commits.reverse();
        let mut head = new_branch.head;
        for commit in extra_commits {
            let new_branch_head = self
                .repo()
                .find_commit(head)
                .context("failed to find new branch head")?;

            let rebased_commit_oid = self
                .repo()
                .commit_with_signature(
                    None,
                    &commit.author(),
                    &commit.committer(),
                    &commit.message_bstr().to_str_lossy(),
                    &commit.tree().unwrap(),
                    &[&new_branch_head],
                    None,
                )
                .context(format!(
                    "failed to rebase commit {} onto new branch",
                    commit.id()
                ))?;

            let rebased_commit = self
                .repo()
                .find_commit(rebased_commit_oid)
                .context(format!(
                    "failed to find rebased commit {}",
                    rebased_commit_oid
                ))?;

            new_branch.head = rebased_commit.id();
            new_branch.tree = rebased_commit.tree_id();
            vb_state
                .set_branch(new_branch.clone())
                .context("failed to write branch")?;

            head = rebased_commit.id();
        }
        Ok(self)
    }
}

fn invalid_head_err(head_name: &str) -> anyhow::Error {
    anyhow!(
        "project is on {head_name}. Please checkout {} to continue",
        GITBUTLER_INTEGRATION_REFERENCE.branch()
    )
}
