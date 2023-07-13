use std::{thread, time::Duration};

use tempfile::tempdir;

use crate::{projects, storage, users};

use super::*;

fn commit_all(repository: &git2::Repository) -> Result<git2::Oid> {
    let mut index = repository.index()?;
    index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let oid = index.write_tree()?;
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    let commit_oid = repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "some commit",
        &repository.find_tree(oid)?,
        &[&repository.find_commit(repository.refname_to_id("HEAD")?)?],
    )?;
    Ok(commit_oid)
}

fn test_repository() -> Result<git2::Repository> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    let repository = git2::Repository::init(path)?;
    repository.remote_add_fetch("origin/master", "master")?;
    let mut index = repository.index()?;
    let oid = index.write_tree()?;
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &repository.find_tree(oid)?,
        &[],
    )?;
    Ok(repository)
}

#[test]
fn test_commit_on_branch_then_change_file_then_get_status() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\n",
    )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        remote_name: "origin".to_string(),
        branch_name: "master".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(&gb_repo, &project_repository, &branch1_id, "test commit")?;

    // status (no files)
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits.len(), 1);

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nlineBLAH\nline7\nline8\n",
    )?;

    // should have just the last change now, the other line is committed
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1);

    Ok(())
}

#[test]
fn test_create_branch_with_ownership() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        remote_name: "origin".to_string(),
        branch_name: "master".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\n",
    )
    .unwrap();

    let branch0 = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");

    get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    let current_session = gb_repo.get_or_create_current_session().unwrap();
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session).unwrap();
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch0 = branch_reader.read(&branch0.id).unwrap();

    let branch1 = create_virtual_branch(
        &gb_repo,
        &BranchCreateRequest {
            ownership: Some(branch0.ownership),
            ..Default::default()
        },
    )
    .expect("failed to create virtual branch");

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch0.id].len(), 0);
    assert_eq!(files_by_branch_id[&branch1.id].len(), 1);

    Ok(())
}

#[test]
fn test_create_branch_in_the_middle() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");
    create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");
    create_virtual_branch(
        &gb_repo,
        &BranchCreateRequest {
            order: Some(1),
            ..Default::default()
        },
    )
    .expect("failed to create virtual branch");

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

    let mut branches = iterator::BranchIterator::new(&current_session_reader)?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .expect("failed to read branches");
    branches.sort_by_key(|b| b.order);
    assert_eq!(branches.len(), 3);
    assert_eq!(branches[0].name, "Branch 1");
    assert_eq!(branches[1].name, "Branch 3");
    assert_eq!(branches[2].name, "Branch 2");

    Ok(())
}

#[test]
fn test_create_branch_no_arguments() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

    let branches = iterator::BranchIterator::new(&current_session_reader)?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .expect("failed to read branches");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "Branch 1");
    assert!(branches[0].applied);
    assert_eq!(branches[0].ownership, Ownership::default());
    assert_eq!(branches[0].order, 0);

    Ok(())
}

#[test]
fn test_hunk_expantion() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\n",
    )?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    // even though selected branch has changed
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch1_id.clone(),
            order: Some(1),
            ..Default::default()
        },
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            order: Some(0),
            ..Default::default()
        },
    )?;

    // a slightly different hunk should still go to the same branch
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\n",
    )?;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    Ok(())
}

#[test]
fn test_get_status_files_by_branch_no_hunks_no_branches() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    assert_eq!(statuses.len(), 0);

    Ok(())
}

#[test]
fn test_get_status_files_by_branch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\n",
    )?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    Ok(())
}

#[test]
fn test_updated_ownership_should_bubble_up() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
    )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // write first file
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;
    get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    let files = branch_reader.read(&branch1_id)?.ownership.files;
    assert_eq!(files, vec!["test.txt:11-15,1-5".try_into()?]);
    assert_eq!(
        files[0].hunks[0].timestam_ms(),
        files[0].hunks[1].timestam_ms(),
        "timestamps must be the same"
    );

    thread::sleep(Duration::from_millis(10)); // make sure timestamps are different

    // wriging a new file should put it on the top
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "hello",
    )?;

    get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    let files1 = branch_reader.read(&branch1_id)?.ownership.files;
    assert_eq!(
        files1,
        vec![
            "test2.txt:1-2".try_into()?,
            "test.txt:11-15,1-5".try_into()?
        ]
    );

    assert_ne!(
        files1[0].hunks[0].timestam_ms(),
        files1[1].hunks[0].timestam_ms(),
        "new file timestamp must be different"
    );

    assert_eq!(
        files[0].hunks[0].timestam_ms(),
        files1[1].hunks[0].timestam_ms(),
        "old file timestamp must not change"
    );

    thread::sleep(Duration::from_millis(10)); // make sure timestamps are different

    // update second hunk, it should make both file and hunk bubble up
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1update\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;
    get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    let files2 = branch_reader.read(&branch1_id)?.ownership.files;
    assert_eq!(
        files2,
        vec![
            "test.txt:1-6,11-15".try_into()?,
            "test2.txt:1-2".try_into()?,
        ]
    );

    assert_ne!(
        files2[0].hunks[0].timestam_ms(),
        files2[0].hunks[1].timestam_ms(),
        "new file timestamps must be different"
    );
    assert_eq!(
        files2[0].hunks[1].timestam_ms(),
        files1[1].hunks[0].timestam_ms(),
        "old file timestamp must not change"
    );
    assert_eq!(
        files2[1].hunks[0].timestam_ms(),
        files1[0].hunks[0].timestam_ms(),
        "old file timestamp must not change"
    );

    Ok(())
}

#[test]
fn test_move_hunks_multiple_sources() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
    )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch3_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(&gb_repo);
    let branch2 = branch_reader.read(&branch2_id)?;
    branch_writer.write(&branch::Branch {
        ownership: Ownership {
            files: vec!["test.txt:1-5".try_into()?],
        },
        ..branch2
    })?;
    let branch1 = branch_reader.read(&branch1_id)?;
    branch_writer.write(&branch::Branch {
        ownership: Ownership {
            files: vec!["test.txt:11-15".try_into()?],
        },
        ..branch1
    })?;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 3);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id][0].hunks.len(), 1);
    assert_eq!(files_by_branch_id[&branch3_id].len(), 0);

    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch3_id.clone(),
            ownership: Some(Ownership::try_from("test.txt:1-5,11-15")?),
            ..Default::default()
        },
    )?;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 3);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 0);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);
    assert_eq!(files_by_branch_id[&branch3_id][0].hunks.len(), 2);

    let branch_reader = branch::Reader::new(&current_session_reader);
    assert_eq!(branch_reader.read(&branch1_id)?.ownership.files, vec![]);
    assert_eq!(branch_reader.read(&branch2_id)?.ownership.files, vec![]);
    assert_eq!(
        branch_reader.read(&branch3_id)?.ownership.files,
        vec!["test.txt:1-5,11-15".try_into()?]
    );

    Ok(())
}

#[test]
fn test_move_hunks_partial_explicitly() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;

    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\n",
        )?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 2);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some(Ownership::try_from("test.txt:1-5")?),
            ..Default::default()
        },
    )?;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id.clone(), files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 1);

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    assert_eq!(
        branch_reader.read(&branch1_id)?.ownership.files,
        vec!["test.txt:12-16".try_into()?]
    );
    assert_eq!(
        branch_reader.read(&branch2_id)?.ownership.files,
        vec!["test.txt:1-5".try_into()?]
    );

    Ok(())
}

#[test]
fn test_add_new_hunk_to_the_end() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\n",
        )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;

    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
        )?;

    create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    assert_eq!(statuses[0].1[0].hunks[0].id, "12-16");

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
        )?;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
    assert!(statuses[0].1[0].hunks[0]
        .id
        .starts_with("13-17-ad6f6af93b494f66d4754e4806c7c1b4-"));
    assert_eq!(statuses[0].1[0].hunks[1].id, "1-5");

    Ok(())
}

#[test]
fn test_update_branch_target_base() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\n",
    )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;
    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    commit_all(&repository)?;
    let up_target = repository.head().unwrap().target().unwrap();
    //update repo ref refs/remotes/origin/master to up_target oid
    repository.reference(
        "refs/remotes/origin/master",
        up_target,
        true,
        "update target",
    )?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // revert content
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\nlocal\n",
    )?;

    // create a vbranch
    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    commit(&gb_repo, &project_repository, &branch1_id, "test commit")?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\nlocal\nmore local\n",
    )?;

    // add something to the branch
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1);

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(String::from_utf8(contents)?, "line1\nline2\nline3\nline4\n");

    // update the target branch
    // this should leave the work on file2, but update the contents of file1
    // and the branch diff should only be on file2
    update_branch_target(&gb_repo, &project_repository)?;

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        String::from_utf8(contents)?,
        "line1\nline2\nline3\nline4\nupstream\n"
    );

    // assert that the vbranch target is updated
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 2); // branch commit, merge commit

    Ok(())
}

#[test]
fn test_update_branch_target_detect_integrated_branches() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    commit_all(&repository)?;
    let up_target = repository.head().unwrap().target().unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    repository.reference(
        "refs/remotes/origin/master",
        up_target,
        true,
        "update target",
    )?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // create a vbranch
    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;

    commit(&gb_repo, &project_repository, &branch1_id, "test commit")?;

    // add something to the branch
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits.len(), 1);

    // update the target branch
    // this should notice that the trees are the same after the merge, so it should unapply the branch
    update_branch_target(&gb_repo, &project_repository)?;

    // integrated branch should be deleted
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert!(!branches.iter().any(|b| b.id == branch1_id));

    Ok(())
}

#[test]
fn test_update_branch_target_detect_integrated_branches_with_more_work() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    // create a vbranch
    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    commit_all(&repository)?;
    let up_target = repository.head().unwrap().target().unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    repository.reference(
        "refs/remotes/origin/master",
        up_target,
        true,
        "update target",
    )?;

    commit(&gb_repo, &project_repository, &branch1_id, "test commit")?;

    // add some uncommitted work
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "local\nline1\nline2\nline3\nline4\nupstream\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1);

    // update the target branch
    // this should notice that the trees are the same after the merge, but there are files on the branch, so do a merge and then leave the files there
    update_branch_target(&gb_repo, &project_repository)?;

    // there should be a new vbranch created, but nothing is on it
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 2);

    Ok(())
}

#[test]
fn test_update_target_with_conflicts_in_vbranches() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\n",
    )?;
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "line1\nline2\nfile3\n",
    )?;
    commit_all(&repository)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // add a commit to the target branch it's pointing to so there is something "upstream"
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "line1\nline2\nfile3\nupstream\n",
    )?;
    commit_all(&repository)?;
    let up_target = repository.head().unwrap().target().unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    repository.reference(
        "refs/remotes/origin/master",
        up_target,
        true,
        "update target",
    )?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // add a commit to the target branch it's pointing to so there is something "upstream"
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "line1\nline2\nfile3\nupstream\n",
    )?;

    // test all our situations
    // 1. unapplied branch, uncommitted conflicts
    // 2. unapplied branch, committed conflicts but not uncommitted
    // 3. unapplied branch, no conflicts
    // 4. applied branch, uncommitted conflicts
    // 5. applied branch, committed conflicts but not uncommitted
    // 6. applied branch, no conflicts
    // 7. applied branch with commits but everything is upstream, delete it

    // create a vbranch
    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch3_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch4_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch5_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch6_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch7_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // situation 7: everything is upstream, delete it
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "line1\nline2\nfile3\n",
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch7_id.clone(),
            name: Some("Situation 7".to_string()),
            ownership: Some(Ownership::try_from("test.txt:1-5")?),
            ..Default::default()
        },
    )?;
    commit(
        &gb_repo,
        &project_repository,
        &branch7_id,
        "integrated commit",
    )?;

    unapply_branch(&gb_repo, &project_repository, &branch7_id)?;

    // situation 1: unapplied branch, uncommitted conflicts
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nsit1.unapplied.conflict.uncommitted\n",
    )?;
    // reset other files
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\n",
    )?;

    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch1_id.clone(),
            name: Some("Situation 1".to_string()),
            ownership: Some(Ownership::try_from("test.txt:1-5")?),
            ..Default::default()
        },
    )?;
    unapply_branch(&gb_repo, &project_repository, &branch1_id)?;

    // situation 2: unapplied branch, committed conflicts but not uncommitted
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nsit2.unapplied.conflict.committed\n",
    )?;

    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            name: Some("Situation 2".to_string()),
            ownership: Some(Ownership::try_from("test.txt:1-5")?),
            ..Default::default()
        },
    )?;
    commit(
        &gb_repo,
        &project_repository,
        &branch2_id,
        "commit conflicts",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "sit2.fixed in uncomitted\nline1\nline2\nline3\nline4\nupstream\n",
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some(Ownership::try_from("test.txt:1-6")?),
            ..Default::default()
        },
    )?;
    unapply_branch(&gb_repo, &project_repository, &branch2_id)?;

    // situation 3: unapplied branch, no conflicts
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "sit3.no-conflict\nline5\nline6\nline7\nline8\n",
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch3_id.clone(),
            name: Some("Situation 3".to_string()),
            ownership: Some(Ownership::try_from("test2.txt:1-5")?),
            ..Default::default()
        },
    )?;
    unapply_branch(&gb_repo, &project_repository, &branch3_id)?;

    // situation 5: applied branch, committed conflicts but not uncommitted
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "line1\nline2\nfile3\nsit5.applied.conflict.committed\n",
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch5_id.clone(),
            name: Some("Situation 5".to_string()),
            ownership: Some(Ownership::try_from("test3.txt:1-4")?),
            ..Default::default()
        },
    )?;
    commit(
        &gb_repo,
        &project_repository,
        &branch5_id,
        "broken, but will fix",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "test\nline1\nline2\nfile3\nupstream\n",
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch5_id.clone(),
            ownership: Some(Ownership::try_from("test3.txt:1-5")?),
            ..Default::default()
        },
    )?;

    // situation 4: applied branch, uncommitted conflicts
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nsit4.applied.conflict.uncommitted\n",
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch4_id.clone(),
            name: Some("Situation 4".to_string()),
            ownership: Some(Ownership::try_from("test.txt:1-5")?),
            ..Default::default()
        },
    )?;

    // situation 6: applied branch, no conflicts
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\nsit6.no-conflict.applied\n",
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch6_id.clone(),
            name: Some("Situation 6".to_string()),
            ownership: Some(Ownership::try_from("test2.txt:1-5")?),
            ..Default::default()
        },
    )?;

    // update the target branch
    update_branch_target(&gb_repo, &project_repository)?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;

    // 1. unapplied branch, uncommitted conflicts
    let branch = branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert!(!branch.active);
    assert!(!branch.mergeable);
    assert!(!branch.base_current);

    // 2. unapplied branch, committed conflicts but not uncommitted
    let branch = branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert!(!branch.active);
    assert!(branch.mergeable);
    assert!(branch.base_current);
    assert_eq!(branch.commits.len(), 2);

    // 3. unapplied branch, no conflicts
    let branch = branches.iter().find(|b| b.id == branch3_id).unwrap();
    assert!(!branch.active);
    assert!(branch.mergeable);
    assert!(branch.base_current);

    // 4. applied branch, uncommitted conflicts
    let branch = branches.iter().find(|b| b.id == branch4_id).unwrap();
    assert!(!branch.active);
    assert!(!branch.mergeable);
    assert!(!branch.base_current);

    // 5. applied branch, committed conflicts but not uncommitted
    let branch = branches.iter().find(|b| b.id == branch5_id).unwrap();
    assert!(!branch.active); // cannot merge history into new target
    assert!(!branch.mergeable);
    assert!(!branch.base_current);

    // 6. applied branch, no conflicts
    let branch = branches.iter().find(|b| b.id == branch6_id).unwrap();
    assert!(branch.active); // still applied

    // 7. applied branch with commits but everything is upstream, delete it
    // branch7 was integrated and deleted
    let branch7 = branch_reader.read(&branch7_id);
    assert!(branch7.is_err());

    Ok(())
}

#[test]
fn test_apply_unapply_branch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(&repository)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\n",
    )?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch2_id,
            ownership: Some(Ownership::try_from("test2.txt:1-3")?),
            ..Default::default()
        },
    )?;

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nbranch1\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    unapply_branch(&gb_repo, &project_repository, &branch1_id)?;

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 0);
    assert!(!branch.active);

    apply_branch(&gb_repo, &project_repository, &branch1_id)?;
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nbranch1\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    Ok(())
}

#[test]
fn test_apply_unapply_added_deleted_files() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "file1\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;
    commit_all(&repository)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    // rm file_path2, add file3
    std::fs::remove_file(std::path::Path::new(&project.path).join(file_path2))?;
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\n",
    )?;

    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch3_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some(Ownership::try_from("test2.txt:0-0")?),
            ..Default::default()
        },
    )?;
    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch3_id.clone(),
            ownership: Some(Ownership::try_from("test3.txt:1-2")?),
            ..Default::default()
        },
    )?;

    unapply_branch(&gb_repo, &project_repository, &branch2_id)?;
    // check that file2 is back
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("file2\n", String::from_utf8(contents)?);

    unapply_branch(&gb_repo, &project_repository, &branch3_id)?;
    // check that file3 is gone
    assert!(!std::path::Path::new(&project.path)
        .join(file_path3)
        .exists());

    apply_branch(&gb_repo, &project_repository, &branch2_id)?;
    // check that file2 is gone
    assert!(!std::path::Path::new(&project.path)
        .join(file_path2)
        .exists());

    apply_branch(&gb_repo, &project_repository, &branch3_id)?;
    // check that file3 is back
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path3))?;
    assert_eq!("file3\n", String::from_utf8(contents)?);

    Ok(())
}

#[test]
fn test_detect_mergeable_branch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(&repository)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;
    let file_path4 = std::path::Path::new("test4.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path4),
        "line5\nline6\n",
    )?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(&gb_repo);

    update_branch(
        &gb_repo,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some("test4.txt:1-3".try_into()?),
            ..Default::default()
        },
    )
    .expect("failed to update branch");

    // unapply both branches and create some conflicting ones
    unapply_branch(&gb_repo, &project_repository, &branch1_id)?;
    unapply_branch(&gb_repo, &project_repository, &branch2_id)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // create an upstream remote conflicting commit
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    commit_all(&repository)?;
    let up_target = repository.head().unwrap().target().unwrap();
    repository.reference(
        "refs/remotes/origin/remote_branch",
        up_target,
        true,
        "update target",
    )?;

    // revert content and write a mergeable branch
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\n",
    )?;
    commit_all(&repository)?;
    let up_target = repository.head().unwrap().target().unwrap();
    repository.reference(
        "refs/remotes/origin/remote_branch2",
        up_target,
        true,
        "update target",
    )?;
    // remove file_path3
    std::fs::remove_file(std::path::Path::new(&project.path).join(file_path3))?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // create branches that conflict with our earlier branches
    create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");
    let branch4_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // branch3 conflicts with branch1 and remote_branch
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch3\n",
    )?;

    // branch4 conflicts with branch2
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line1\nline2\nline3\nline4\nbranch4\n",
    )?;

    let branch4 = branch_reader.read(&branch4_id)?;
    branch_writer.write(&Branch {
        ownership: Ownership {
            files: vec!["test2.txt:1-6".try_into()?],
        },
        ..branch4
    })?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 4);

    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert!(!branch1.active);
    assert!(!branch1.mergeable);
    assert_eq!(branch1.merge_conflicts.len(), 1);
    assert_eq!(branch1.merge_conflicts.first().unwrap(), "test.txt");

    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert!(!branch2.active);
    assert!(branch2.mergeable);
    assert_eq!(branch2.merge_conflicts.len(), 0);

    let remotes = remote_branches(&gb_repo, &project_repository)?;
    let remote1 = &remotes
        .iter()
        .find(|b| b.branch == "refs/remotes/origin/remote_branch")
        .unwrap();
    assert!(!remote1.mergeable);
    assert_eq!(remote1.ahead, 1);
    assert_eq!(remote1.merge_conflicts.len(), 1);
    assert_eq!(remote1.merge_conflicts.first().unwrap(), "test.txt");

    let remote2 = &remotes
        .iter()
        .find(|b| b.branch == "refs/remotes/origin/remote_branch2")
        .unwrap();
    assert!(remote2.mergeable);
    assert_eq!(remote2.ahead, 2);
    assert_eq!(remote2.merge_conflicts.len(), 0);

    Ok(())
}

#[test]
fn test_detect_remote_commits() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;
    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(&gb_repo);

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(&repository)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "http://origin.com/project".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let repo = &project_repository.git_repository;
    repo.remote("origin", "http://origin.com/project")?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // create a commit to push upstream
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "upstream commit 1",
    )?;

    // create another commit to push upstream
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nmore upstream\n",
    )?;

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "upstream commit 2",
    )?;

    // push the commit upstream
    let branch1 = branch_reader.read(&branch1_id)?;
    let up_target = branch1.head;
    repository.reference(
        "refs/remotes/origin/remote_branch",
        up_target,
        true,
        "update target",
    )?;
    // set the upstream reference
    branch_writer.write(&Branch {
        upstream: "remote_branch".to_string(),
        ..branch1
    })?;

    // create another commit that is not pushed up
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nmore upstream\nmore work\n",
    )?;

    commit(&gb_repo, &project_repository, &branch1_id, "local commit")?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 1);

    let branch = &branches.first().unwrap();
    assert_eq!(branch.commits.len(), 3);
    assert_eq!(branch.commits[0].description, "local commit");
    assert!(!branch.commits[0].is_remote);
    assert_eq!(branch.commits[1].description, "upstream commit 2");
    assert!(branch.commits[1].is_remote);
    assert!(branch.commits[2].is_remote);

    Ok(())
}

#[test]
fn test_create_vbranch_from_remote_branch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(&repository)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "http://origin.com/project".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    repository.remote("origin", "http://origin.com/project")?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch\n",
    )?;
    commit_all(&repository)?;
    repository.reference(
        "refs/remotes/branch1",
        repository.head().unwrap().target().unwrap(),
        true,
        "update target",
    )?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // reset the first file
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;

    // create a default branch. there should not be anything on this
    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 0);

    // create a new virtual branch from the remote branch
    let branch2_id =
        create_virtual_branch_from_branch(&gb_repo, &project_repository, "refs/remotes/branch1")?;

    // shouldn't be anything on either of our branches
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert!(branch1.active);
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert_eq!(branch2.files.len(), 0);
    assert!(!branch2.active);

    // file should still be the original
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);

    // this should bring in the branch change
    apply_branch(&gb_repo, &project_repository, &branch2_id)?;

    // file should be the branch version now
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nbranch\n",
        String::from_utf8(contents)?
    );

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert!(branch1.active);
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert_eq!(branch2.files.len(), 0);
    assert!(branch2.active);
    assert_eq!(branch2.commits.len(), 1);

    unapply_branch(&gb_repo, &project_repository, &branch1_id)?;

    // add to the applied file in the same hunk so it adds to the second branch
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch\nmore branch\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert_eq!(branch2.files.len(), 1);
    assert!(branch2.active);

    // add to another file so it goes to the default one
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert_eq!(branch2.files.len(), 2);
    assert!(branch2.active);

    Ok(())
}

#[test]
fn test_create_vbranch_from_behind_remote_branch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;
    commit_all(&repository)?;

    let base_commit = repository.head().unwrap().target().unwrap();

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    commit_all(&repository)?;

    let upstream_commit = repository.head().unwrap().target().unwrap();
    repository.reference(
        "refs/remotes/origin/master",
        upstream_commit,
        true,
        "update target",
    )?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        remote_name: "origin".to_string(),
        branch_name: "master".to_string(),
        remote_url: "http://origin.com/project".to_string(),
        sha: upstream_commit,
        behind: 0,
    })?;
    repository.remote("origin", "http://origin.com/project")?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // reset master to the base commit
    repository.reference("refs/heads/master", base_commit, true, "update target")?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\nremote",
    )?;
    commit_all(&repository)?;
    let remote_commit = repository.head().unwrap().target().unwrap();

    repository.reference(
        "refs/remotes/origin/branch1",
        remote_commit,
        true,
        "update target",
    )?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // reset wd
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;

    // create a new virtual branch from the remote branch
    let branch1_id = create_virtual_branch_from_branch(
        &gb_repo,
        &project_repository,
        "refs/remotes/origin/branch1",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branch1.files.len(), 0);

    // nothing has changed
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\n");

    apply_branch(&gb_repo, &project_repository, &branch1_id)?;

    // the file2 has been updated
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\nremote");

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branches.len(), 1);
    // our branch still no hunks
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 2); // a merge commit too

    Ok(())
}

#[test]
fn test_partial_commit() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;
    commit_all(&repository)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;
    let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // create a change with two hunks
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n",
        )?;

    // move hunk1 and hunk3 to branch2
    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(&gb_repo);
    let branch2 = branch_reader.read(&branch2_id)?;
    branch_writer.write(&branch::Branch {
        ownership: Ownership {
            files: vec!["test.txt:9-16".try_into()?],
        },
        ..branch2
    })?;
    let branch1 = branch_reader.read(&branch1_id)?;
    branch_writer.write(&branch::Branch {
        ownership: Ownership {
            files: vec!["test.txt:1-6".try_into()?, "test.txt:17-24".try_into()?],
        },
        ..branch1
    })?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch1.files[0].hunks.len(), 2);
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert_eq!(branch2.files[0].hunks.len(), 1);

    // commit
    commit(&gb_repo, &project_repository, &branch1_id, "branch1 commit")?;
    commit(&gb_repo, &project_repository, &branch2_id, "branch2 commit")?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit = &branch1.commits[0].id;
    let contents = commit_sha_to_contents(&repository, commit, "test.txt");
    assert_eq!(contents, "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n");

    // branch two test.txt has just the middle hunk applied
    let commit = &branch2.commits[0].id;
    let contents = commit_sha_to_contents(&repository, commit, "test.txt");
    assert_eq!(contents, "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n");

    // ok, now we're going to unapply branch1, which should remove the 1st and 3rd hunks
    unapply_branch(&gb_repo, &project_repository, &branch1_id)?;
    // read contents of test.txt
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n");

    // ok, now we're going to re-apply branch1, which adds hunk 1 and 3, then unapply branch2, which should remove the middle hunk
    apply_branch(&gb_repo, &project_repository, &branch1_id)?;
    unapply_branch(&gb_repo, &project_repository, &branch2_id)?;

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n");

    // finally, reapply the middle hunk on branch2, so we have all of them again
    apply_branch(&gb_repo, &project_repository, &branch2_id)?;

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n");

    Ok(())
}

fn commit_sha_to_contents(repository: &git2::Repository, commit: &str, path: &str) -> String {
    let commit = git2::Oid::from_str(commit).expect("failed to parse oid");
    let commit = repository
        .find_commit(commit)
        .expect("failed to get commit object");
    // get the tree
    let tree = commit.tree().expect("failed to get tree");
    // get the blob
    let tree_entry = tree
        .get_path(std::path::Path::new(path))
        .expect("failed to get blob");
    // blob from tree_entry
    let blob = tree_entry
        .to_object(repository)
        .unwrap()
        .peel_to_blob()
        .expect("failed to get blob");

    // get the contents
    let contents = blob.content();
    let contents = std::str::from_utf8(contents).expect("failed to convert to string");
    contents.to_string()
}

#[test]
fn test_commit_add_and_delete_files() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "file1\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;
    commit_all(&repository)?;

    let commit1_oid = repository.head().unwrap().target().unwrap();
    let commit1 = repository.find_commit(commit1_oid).unwrap();
    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "origin".to_string(),
        sha: commit1_oid,
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    // remove file
    std::fs::remove_file(std::path::Path::new(&project.path).join(file_path2))?;
    // add new file
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\n",
    )?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // commit
    commit(&gb_repo, &project_repository, &branch1_id, "branch1 commit")?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit2 = &branch1.commits[0].id;
    let commit2 = git2::Oid::from_str(commit2).expect("failed to parse oid");
    let commit2 = repository
        .find_commit(commit2)
        .expect("failed to get commit object");

    let tree = commit1.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&repository, &tree).unwrap();
    assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

    // get the tree
    let tree = commit2.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&repository, &tree).unwrap();
    assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

    Ok(())
}

fn tree_to_file_list(repository: &git2::Repository, tree: &git2::Tree) -> Result<Vec<String>> {
    let mut file_list = Vec::new();
    for entry in tree.iter() {
        let path = entry.name().unwrap();
        let entry = tree
            .get_path(std::path::Path::new(path))
            .context(format!("failed to get tree entry for path {}", path))?;
        let object = entry
            .to_object(repository)
            .context(format!("failed to get object for tree entry {}", path))?;
        if object.kind() == Some(git2::ObjectType::Blob) {
            file_list.push(path.to_string());
        }
    }
    Ok(file_list)
}

#[test]
fn test_apply_out_of_date_vbranch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;
    commit_all(&repository)?;

    let base_commit = repository.head().unwrap().target().unwrap();
    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "http://origin.com/project".to_string(),
        sha: base_commit,
        behind: 0,
    })?;
    repository.remote("origin", "http://origin.com/project")?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // ok, pretend upstream was updated
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    commit_all(&repository)?;

    let upstream_commit = repository.head().unwrap().target().unwrap();
    repository.reference(
        "refs/remotes/origin/master",
        upstream_commit,
        true,
        "update target",
    )?;

    // reset master to the base commit
    repository.reference("refs/heads/master", base_commit, true, "update target")?;

    // write new unapplied virtual branch with other changes
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\nbranch",
    )?;
    commit_all(&repository)?;
    let branch_commit = repository.head().unwrap().target().unwrap();
    let branch_commit_obj = repository.find_commit(branch_commit)?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    let mut branch = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");

    branch.head = branch_commit;
    branch.tree = branch_commit_obj.tree()?.id();
    branch.applied = false;
    let branch_id = &branch.id.clone();

    let branch_writer = branch::Writer::new(&gb_repo);
    branch_writer.write(&branch::Branch {
        ownership: Ownership {
            files: vec!["test2.txt:1-2".try_into()?],
        },
        ..branch
    })?;

    // reset wd
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 1); // just the unapplied one with it's one commit
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 1);

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\n");

    // update target, this will update the wd and add an empty default branch
    update_branch_target(&gb_repo, &project_repository)?;

    // updated the file
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\n");

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 1);

    // apply branch which is now out of date
    // - it should merge the new target into it and update the wd and nothing is in files
    apply_branch(&gb_repo, &project_repository, branch_id)?;

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\nbranch");

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 1); // one is there still
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 2);

    Ok(())
}

#[test]
fn test_apply_out_of_date_conflicting_vbranch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;
    commit_all(&repository)?;

    let base_commit = repository.head().unwrap().target().unwrap();
    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "http://origin.com/project".to_string(),
        sha: base_commit,
        behind: 0,
    })?;
    repository.remote("origin", "http://origin.com/project")?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // ok, pretend upstream was updated
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    commit_all(&repository)?;

    let upstream_commit = repository.head().unwrap().target().unwrap();
    repository.reference(
        "refs/remotes/origin/master",
        upstream_commit,
        true,
        "update target",
    )?;

    // reset master to the base commit
    repository.reference("refs/heads/master", base_commit, true, "update target")?;

    // write new unapplied virtual branch with other changes
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nconflict\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\nbranch",
    )?;
    commit_all(&repository)?;
    let branch_commit = repository.head().unwrap().target().unwrap();
    let branch_commit_obj = repository.find_commit(branch_commit)?;

    repository.set_head("refs/heads/gitbutler/integration")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    let mut branch = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");

    branch.head = branch_commit;
    branch.tree = branch_commit_obj.tree()?.id();
    branch.applied = false;
    let branch_id = &branch.id.clone();

    let branch_writer = branch::Writer::new(&gb_repo);
    branch_writer.write(&branch::Branch {
        name: "My Awesome Branch".to_string(),
        ownership: Ownership {
            files: vec!["test2.txt:1-2".try_into()?, "test.txt:1-5".try_into()?],
        },
        ..branch
    })?;

    // reset wd
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 1); // just the unapplied one with it's one commit
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 1);

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\n");

    // update target, this will update the wd and add an empty default branch
    update_branch_target(&gb_repo, &project_repository)?;

    // updated the file
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\n");

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    assert_eq!(branches.len(), 1);
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert!(!branch1.mergeable);
    assert!(!branch1.base_current);

    // apply branch which is now out of date and conflicting
    apply_branch(&gb_repo, &project_repository, branch_id)?;

    assert!(project_repository.is_conflicted(None)?);

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.files.first().unwrap().hunks.len(), 1);
    assert!(branch1.files.first().unwrap().conflicted);
    assert_eq!(branch1.commits.len(), 1);
    assert!(branch1.conflicted);

    // fix the conflict and commit it
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nconflict\n",
    )?;

    // try to commit, fail
    let result = commit(&gb_repo, &project_repository, branch_id, "resolve commit");
    assert!(result.is_err());

    // mark file as resolved
    project_repository.mark_resolved("test.txt".to_string())?;

    // make sure the branch has that commit and that the parent is the target
    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.files.first().unwrap().hunks.len(), 1);
    assert!(!branch1.files.first().unwrap().conflicted);
    assert!(!branch1.conflicted);
    assert!(branch1.active);

    // commit
    commit(&gb_repo, &project_repository, branch_id, "resolve commit")?;

    let branches = list_virtual_branches(&gb_repo, &project_repository, true)?;
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    let last_commit = branch1.commits.first().unwrap();
    let last_commit_oid = git2::Oid::from_str(&last_commit.id)?;
    let commit = gb_repo.git_repository.find_commit(last_commit_oid)?;
    assert_eq!(commit.parent_count(), 2);

    Ok(())
}

#[test]
fn test_apply_conflicting_vbranch() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store,
        user_store,
    )?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;
    commit_all(&repository)?;

    let base_commit = repository.head().unwrap().target().unwrap();
    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch_name: "origin/master".to_string(),
        remote_name: "origin".to_string(),
        remote_url: "http://origin.com/project".to_string(),
        sha: base_commit,
        behind: 0,
    })?;
    update_gitbutler_integration(&gb_repo, &project_repository)?;

    // write new unapplied virtual branch with other changes
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nconflict\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\nbranch\n",
    )?;
    commit_all(&repository)?;
    let branch_commit = repository.head().unwrap().target().unwrap();
    let branch_commit_obj = repository.find_commit(branch_commit)?;

    let mut branch = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");

    branch.head = branch_commit;
    branch.tree = branch_commit_obj.tree()?.id();
    branch.applied = false;
    let branch_id = &branch.id.clone();

    let branch_writer = branch::Writer::new(&gb_repo);
    branch_writer.write(&branch::Branch {
        name: "Our Awesome Branch".to_string(),
        ownership: Ownership {
            files: vec!["test.txt:1-5".try_into()?, "test2.txt:1-2".try_into()?],
        },
        ..branch
    })?;

    // update wd
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nworking\n",
    )?;

    // apply branch which is now out of date and conflicting, which fails
    let result = apply_branch(&gb_repo, &project_repository, branch_id);
    assert!(result.is_err());

    Ok(())
}
