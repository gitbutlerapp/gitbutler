use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use gitbutler_core::{git, project_repository::LogUntil, projects};
use gitbutler_tauri::watcher::Handler;

use crate::watcher::handler::test_remote_repository;
use gitbutler_testsupport::{virtual_branches::set_test_target, Case, Suite};

fn log_walk(repo: &git2::Repository, head: git::Oid) -> Vec<git::Oid> {
    let mut walker = repo.revwalk().unwrap();
    walker.push(head.into()).unwrap();
    walker.map(|oid| oid.unwrap().into()).collect::<Vec<_>>()
}

#[tokio::test]
async fn push_error() -> Result<()> {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: String::new(),
        code_git_url: Some(String::new()),
        created_at: 0_i32.to_string(),
        updated_at: 0_i32.to_string(),
        sync: true,
    };

    suite
        .projects
        .update(&projects::UpdateRequest {
            id: project.id,
            api: Some(api_project.clone()),
            ..Default::default()
        })
        .await?;

    let res = Handler::push_project_to_gitbutler_pure(
        suite.local_app_data(),
        &suite.projects,
        &suite.users,
        project.id,
        100,
    )
    .await;

    res.unwrap_err();

    Ok(())
}

#[tokio::test]
async fn push_simple() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = &suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));

    suite.sign_in();

    set_test_target(gb_repository, project_repository).unwrap();

    let target_id = gb_repository.default_target().unwrap().unwrap().sha;

    let reference = project_repository.l(target_id, LogUntil::End).unwrap();

    let (cloud_code, _tmp) = test_remote_repository()?;

    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: String::new(),
        code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
        created_at: 0_i32.to_string(),
        updated_at: 0_i32.to_string(),
        sync: true,
    };

    suite
        .projects
        .update(&projects::UpdateRequest {
            id: project.id,
            api: Some(api_project.clone()),
            ..Default::default()
        })
        .await?;

    cloud_code.find_commit(target_id.into()).unwrap_err();

    {
        let res = Handler::push_project_to_gitbutler_pure(
            suite.local_app_data(),
            &suite.projects,
            &suite.users,
            project.id,
            10,
        )
        .await
        .unwrap();
        assert!(res.is_empty());
    }

    cloud_code.find_commit(target_id.into()).unwrap();

    let pushed = log_walk(&cloud_code, target_id);
    assert_eq!(reference.len(), pushed.len());
    assert_eq!(reference, pushed);

    assert_eq!(
        suite
            .projects
            .get(&project.id)
            .unwrap()
            .gitbutler_code_push_state
            .unwrap()
            .id,
        target_id
    );

    Ok(())
}

#[tokio::test]
async fn push_remote_ref() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = &suite.new_case();

    suite.sign_in();

    set_test_target(gb_repository, project_repository).unwrap();

    let (cloud_code, _tmp) = test_remote_repository()?;
    let cloud_code: git::Repository = cloud_code.into();

    let (remote_repo, _tmp) = test_remote_repository()?;
    let remote_repo: git::Repository = remote_repo.into();

    let last_commit = create_initial_commit(&remote_repo);

    remote_repo
        .reference(
            &git::Refname::Local(git::LocalRefname::new("refs/heads/testbranch", None)),
            last_commit,
            false,
            "",
        )
        .unwrap();

    let mut remote = project_repository
        .git_repository
        .remote("tr", &remote_repo.path().to_str().unwrap().parse().unwrap())
        .unwrap();

    remote
        .fetch(&["+refs/heads/*:refs/remotes/tr/*"], None)
        .unwrap();

    project_repository
        .git_repository
        .find_commit(last_commit)
        .unwrap();

    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: String::new(),
        code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
        created_at: 0_i32.to_string(),
        updated_at: 0_i32.to_string(),
        sync: true,
    };

    suite
        .projects
        .update(&projects::UpdateRequest {
            id: project.id,
            api: Some(api_project.clone()),
            ..Default::default()
        })
        .await?;

    {
        Handler::push_project_to_gitbutler_pure(
            suite.local_app_data(),
            &suite.projects,
            &suite.users,
            project.id,
            10,
        )
        .await
        .unwrap();
    }

    cloud_code.find_commit(last_commit).unwrap();

    Ok(())
}

fn create_initial_commit(repo: &git::Repository) -> git::Oid {
    let signature = git::Signature::now("test", "test@email.com").unwrap();

    let mut index = repo.index().unwrap();
    let oid = index.write_tree().unwrap();

    repo.commit(
        None,
        &signature,
        &signature,
        "initial commit",
        &repo.find_tree(oid).unwrap(),
        &[],
    )
    .unwrap()
}

fn create_test_commits(repo: &git::Repository, commits: usize) -> git::Oid {
    let signature = git::Signature::now("test", "test@email.com").unwrap();

    let mut last = None;

    for i in 0..commits {
        let mut index = repo.index().unwrap();
        let oid = index.write_tree().unwrap();
        let head = repo.head().unwrap();

        last = Some(
            repo.commit(
                Some(&head.name().unwrap()),
                &signature,
                &signature,
                format!("commit {i}").as_str(),
                &repo.find_tree(oid).unwrap(),
                &[&repo
                    .find_commit(repo.refname_to_id("HEAD").unwrap())
                    .unwrap()],
            )
            .unwrap(),
        );
    }

    last.unwrap()
}

#[tokio::test]
async fn push_batches() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = &suite.new_case();

    suite.sign_in();

    {
        let head: git::Oid = project_repository
            .get_head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id();

        let reference = project_repository.l(head, LogUntil::End).unwrap();
        assert_eq!(reference.len(), 2);

        let head = create_test_commits(&project_repository.git_repository, 10);

        let reference = project_repository.l(head, LogUntil::End).unwrap();
        assert_eq!(reference.len(), 12);
    }

    set_test_target(gb_repository, project_repository).unwrap();

    let target_id = gb_repository.default_target().unwrap().unwrap().sha;

    let reference = project_repository.l(target_id, LogUntil::End).unwrap();

    let (cloud_code, _tmp) = test_remote_repository()?;

    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: String::new(),
        code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
        created_at: 0_i32.to_string(),
        updated_at: 0_i32.to_string(),
        sync: true,
    };

    suite
        .projects
        .update(&projects::UpdateRequest {
            id: project.id,
            api: Some(api_project.clone()),
            ..Default::default()
        })
        .await?;

    {
        Handler::push_project_to_gitbutler_pure(
            suite.local_app_data(),
            &suite.projects,
            &suite.users,
            project.id,
            2,
        )
        .await
        .unwrap();
    }

    cloud_code.find_commit(target_id.into()).unwrap();

    let pushed = log_walk(&cloud_code, target_id);
    assert_eq!(reference.len(), pushed.len());
    assert_eq!(reference, pushed);

    assert_eq!(
        suite
            .projects
            .get(&project.id)
            .unwrap()
            .gitbutler_code_push_state
            .unwrap()
            .id,
        target_id
    );

    Ok(())
}

#[tokio::test]
async fn push_again_no_change() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = &suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));

    suite.sign_in();

    set_test_target(gb_repository, project_repository).unwrap();

    let target_id = gb_repository.default_target().unwrap().unwrap().sha;

    let reference = project_repository.l(target_id, LogUntil::End).unwrap();

    let (cloud_code, _tmp) = test_remote_repository()?;

    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: String::new(),
        code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
        created_at: 0_i32.to_string(),
        updated_at: 0_i32.to_string(),
        sync: true,
    };

    suite
        .projects
        .update(&projects::UpdateRequest {
            id: project.id,
            api: Some(api_project.clone()),
            ..Default::default()
        })
        .await?;

    cloud_code.find_commit(target_id.into()).unwrap_err();

    {
        let res = Handler::push_project_to_gitbutler_pure(
            suite.local_app_data(),
            &suite.projects,
            &suite.users,
            project.id,
            10,
        )
        .await
        .unwrap();
        assert!(res.is_empty());
    }

    cloud_code.find_commit(target_id.into()).unwrap();

    let pushed = log_walk(&cloud_code, target_id);
    assert_eq!(reference.len(), pushed.len());
    assert_eq!(reference, pushed);

    assert_eq!(
        suite
            .projects
            .get(&project.id)
            .unwrap()
            .gitbutler_code_push_state
            .unwrap()
            .id,
        target_id
    );

    Ok(())
}
