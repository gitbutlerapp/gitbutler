use std::{fs, path, str::FromStr};

use gitbutler_core::{
    git, keys,
    projects::{self, ProjectId},
    users,
    virtual_branches::{branch, controller::ControllerError, errors, Controller},
};
use tempfile::TempDir;

use crate::shared::{paths, TestProject, VAR_NO_CLEANUP};

struct Test {
    repository: TestProject,
    project_id: ProjectId,
    projects: projects::Controller,
    controller: Controller,
    data_dir: Option<TempDir>,
}

impl Drop for Test {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.data_dir.take().unwrap().into_path();
        }
    }
}

impl Default for Test {
    fn default() -> Self {
        let data_dir = paths::data_dir();
        let keys = keys::Controller::from_path(&data_dir);
        let projects = projects::Controller::from_path(&data_dir);
        let users = users::Controller::from_path(&data_dir);
        let helper = git::credentials::Helper::from_path(&data_dir);

        let test_project = TestProject::default();
        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        Self {
            repository: test_project,
            project_id: project.id,
            controller: Controller::new(
                data_dir.path().into(),
                projects.clone(),
                users,
                keys,
                helper,
            ),
            projects,
            data_dir: Some(data_dir),
        }
    }
}

mod amend;
mod apply_virtual_branch;
mod cherry_pick;
mod create_commit;
mod create_virtual_branch_from_branch;
mod delete_virtual_branch;
mod fetch_from_target;
mod init;
mod move_commit_to_vbranch;
mod references;
mod reset_virtual_branch;
mod selected_for_changes;
mod set_base_branch;
mod squash;
mod unapply;
mod unapply_ownership;
mod update_base_branch;
mod update_commit_message;
mod upstream;

#[tokio::test]
async fn resolve_conflict_flow() {
    let Test {
        repository,
        project_id,
        controller,
        ..
    } = &Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        fs::write(repository.path().join("file.txt"), "first").unwrap();
        let first_commit_oid = repository.commit_all("first");
        fs::write(repository.path().join("file.txt"), "second").unwrap();
        repository.commit_all("second");
        repository.push();
        repository.reset_hard(Some(first_commit_oid));
    }

    controller
        .set_base_branch(project_id, &"refs/remotes/origin/master".parse().unwrap())
        .await
        .unwrap();

    let branch1_id = {
        // make a branch that conflicts with the remote branch, but doesn't know about it yet
        let branch1_id = controller
            .create_virtual_branch(project_id, &branch::BranchCreateRequest::default())
            .await
            .unwrap();
        fs::write(repository.path().join("file.txt"), "conflict").unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(branches[0].active);

        branch1_id
    };

    {
        // fetch remote
        controller.update_base_branch(project_id).await.unwrap();

        // there is a conflict now, so the branch should be inactive
        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(!branches[0].active);
    }

    {
        // when we apply conflicted branch, it has conflict
        controller
            .apply_virtual_branch(project_id, &branch1_id)
            .await
            .unwrap();

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(branches[0].active);
        assert!(branches[0].conflicted);

        // and the conflict markers are in the file
        assert_eq!(
            fs::read_to_string(repository.path().join("file.txt")).unwrap(),
            "<<<<<<< ours\nconflict\n=======\nsecond\n>>>>>>> theirs\n"
        );
    }

    {
        // can't commit conflicts
        assert!(matches!(
            controller
                .create_commit(project_id, &branch1_id, "commit conflicts", None, false)
                .await,
            Err(ControllerError::Action(errors::CommitError::Conflicted(_)))
        ));
    }

    {
        // fixing the conflict removes conflicted mark
        fs::write(repository.path().join("file.txt"), "resolved").unwrap();
        let commit_oid = controller
            .create_commit(project_id, &branch1_id, "resolution", None, false)
            .await
            .unwrap();

        let commit = repository.find_commit(commit_oid).unwrap();
        assert_eq!(commit.parent_count(), 2);

        let (branches, _, _) = controller.list_virtual_branches(project_id).await.unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].id, branch1_id);
        assert!(branches[0].active);
        assert!(!branches[0].conflicted);
    }
}
