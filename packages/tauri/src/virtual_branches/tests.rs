use std::{
    collections::HashMap,
    fs::{self, Permissions},
    io::Write,
    os::unix::fs::{symlink, PermissionsExt},
    path, thread,
    time::Duration,
};

use anyhow::{Context, Result};
use git2::TreeWalkResult;

use crate::{
    gb_repository, git, keys, project_repository, projects, reader, sessions, test_utils, users,
};

use super::branch::{Branch, BranchCreateRequest, Ownership};
use super::*;

pub struct TestDeps {
    repository: git::Repository,
    project: projects::Project,
    gb_repo: gb_repository::Repository,
    gb_repo_path: path::PathBuf,
    user_store: users::Storage,
    project_store: projects::Storage,
    keys_controller: keys::Storage,
}

fn new_test_deps() -> Result<TestDeps> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = test_utils::temp_dir();
    let local_data_dir = test_utils::temp_dir();
    let user_store = users::Storage::from(&local_data_dir);
    let project_store = projects::Storage::from(&local_data_dir);
    let keys_controller = keys::Storage::from(&local_data_dir);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        &project.id,
        project_store.clone(),
        user_store.clone(),
    )?;
    Ok(TestDeps {
        repository,
        project,
        gb_repo,
        gb_repo_path,
        user_store,
        project_store,
        keys_controller,
    })
}

fn set_test_target(
    gb_repo: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    repository: &git::Repository,
) -> Result<()> {
    target::Writer::new(gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    repository.reference(
        "refs/remotes/origin/master",
        repository.head().unwrap().target().unwrap(),
        true,
        "update target",
    )?;
    repository.remote("origin", "http://origin.com/project")?;
    super::integration::update_gitbutler_integration(gb_repo, project_repository)?;
    Ok(())
}

#[test]
fn test_commit_on_branch_then_change_file_then_get_status() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

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
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    set_test_target(&gb_repo, &project_repository, &repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
    )?;

    // status (no files)
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits.len(), 1);

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nlineBLAH\nline7\nline8\n",
    )?;

    // should have just the last change now, the other line is committed
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1);

    Ok(())
}

#[test]
fn test_signed_commit() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        keys_controller,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    set_test_target(&gb_repo, &project_repository, &repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let mut config = repository
        .config()
        .with_context(|| "failed to get config")?;
    config.set_str("gitbutler.signCommits", "true")?;

    // commit
    commit_signed(
        &keys_controller.get_or_create()?,
        &gb_repo,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository).unwrap();
    let commit_id = &branches[0].commits[0].id;
    let commit_obj = repository.find_commit(commit_id.parse().unwrap())?;
    // check the raw_header contains the string "SSH SIGNATURE"
    assert!(commit_obj.raw_header().unwrap().contains("SSH SIGNATURE"));

    Ok(())
}

#[test]
fn test_track_binary_files() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

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
    // add a binary file
    let image_data: [u8; 12] = [
        255, 0, 0, // Red pixel
        0, 0, 255, // Blue pixel
        255, 255, 0, // Yellow pixel
        0, 255, 0, // Green pixel
    ];
    let mut file = fs::File::create(std::path::Path::new(&project.path).join("image.bin"))?;
    file.write_all(&image_data)?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // test file change
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\nline9\n",
    )?;

    // add a binary file
    let image_data: [u8; 12] = [
        255, 0, 0, // Red pixel
        0, 255, 0, // Green pixel
        0, 0, 255, // Blue pixel
        255, 255, 0, // Yellow pixel
    ];
    let mut file = fs::File::create(std::path::Path::new(&project.path).join("image.bin"))?;
    file.write_all(&image_data)?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 2);
    let img_file = &branch
        .files
        .iter()
        .find(|b| b.path.as_os_str() == "image.bin")
        .unwrap();
    assert!(img_file.binary);
    assert_eq!(
        img_file.hunks[0].diff,
        "944996dd82015a616247c72b251e41661e528ae1"
    );

    // commit
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
    )?;

    // status (no files)
    let branches = list_virtual_branches(&gb_repo, &project_repository).unwrap();
    let commit_id = &branches[0].commits[0].id;
    let commit_obj = repository.find_commit(commit_id.parse().unwrap())?;
    let tree = commit_obj.tree()?;
    let files = tree_to_entry_list(&repository, &tree);
    assert_eq!(files[0].0, "image.bin");
    assert_eq!(files[0].3, "944996dd82015a616247c72b251e41661e528ae1");

    let image_data: [u8; 12] = [
        0, 255, 0, // Green pixel
        255, 0, 0, // Red pixel
        255, 255, 0, // Yellow pixel
        0, 0, 255, // Blue pixel
    ];
    let mut file = fs::File::create(std::path::Path::new(&project.path).join("image.bin"))?;
    file.write_all(&image_data)?;

    // commit
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository).unwrap();
    let commit_id = &branches[0].commits[0].id;
    // get tree from commit_id
    let commit_obj = repository.find_commit(commit_id.parse().unwrap())?;
    let tree = commit_obj.tree()?;
    let files = tree_to_entry_list(&repository, &tree);

    assert_eq!(files[0].0, "image.bin");
    assert_eq!(files[0].3, "ea6901a04d1eed6ebf6822f4360bda9f008fa317");

    Ok(())
}

#[test]
fn test_create_branch_with_ownership() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
    assert_eq!(branches[0].name, "Virtual branch");
    assert_eq!(branches[1].name, "Virtual branch 2");
    assert_eq!(branches[2].name, "Virtual branch 1");

    Ok(())
}

#[test]
fn test_create_branch_no_arguments() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch");

    let current_session = gb_repo.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

    let branches = iterator::BranchIterator::new(&current_session_reader)?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .expect("failed to read branches");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "Virtual branch");
    assert!(branches[0].applied);
    assert_eq!(branches[0].ownership, Ownership::default());
    assert_eq!(branches[0].order, 0);

    Ok(())
}

#[test]
fn test_name_to_branch() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    set_test_target(&gb_repo, &project_repository, &repository)?;

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

    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    // even though selected branch has changed
    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch1_id.clone(),
            name: Some("branch1".to_string()),
            order: Some(1),
            ..Default::default()
        },
    )?;
    let result = update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            name: Some("branch1".to_string()),
            order: Some(0),
            ..Default::default()
        },
    );
    assert!(result.is_err());
    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            name: Some("branch2".to_string()),
            order: Some(0),
            ..Default::default()
        },
    )?;

    let mut references = Vec::new();
    for reference in repository.references()? {
        references.push(reference?.name().unwrap().to_string());
    }
    assert!(references.contains(&"refs/gitbutler/branch1".to_string()));
    assert!(references.contains(&"refs/gitbutler/branch2".to_string()));

    Ok(())
}

#[test]
fn test_hunk_expantion() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch1_id.clone(),
            order: Some(1),
            ..Default::default()
        },
    )?;
    update_branch(
        &gb_repo,
        &project_repository,
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
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    let statuses =
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

    assert_eq!(statuses.len(), 0);

    Ok(())
}

#[test]
fn test_get_status_files_by_branch() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
    )?;
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
    assert_eq!(files, vec!["test.txt:11-15,1-5".parse()?]);
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
        vec!["test2.txt:1-2".parse()?, "test.txt:11-15,1-5".parse()?]
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
        vec!["test.txt:1-6,11-15".parse()?, "test2.txt:1-2".parse()?,]
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
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
    )?;
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
            files: vec!["test.txt:1-5".parse()?],
        },
        ..branch2
    })?;
    let branch1 = branch_reader.read(&branch1_id)?;
    branch_writer.write(&branch::Branch {
        ownership: Ownership {
            files: vec!["test.txt:11-15".parse()?],
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch3_id.clone(),
            ownership: Some("test.txt:1-5,11-15".parse()?),
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
        vec!["test.txt:1-5,11-15".parse()?]
    );

    Ok(())
}

#[test]
fn test_move_hunks_partial_explicitly() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some("test.txt:1-5".parse()?),
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
        vec!["test.txt:12-16".parse()?]
    );
    assert_eq!(
        branch_reader.read(&branch2_id)?.ownership.files,
        vec!["test.txt:1-5".parse()?]
    );

    Ok(())
}

#[test]
fn test_add_new_hunk_to_the_end() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\n",
        )?;
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
fn test_update_base_branch_base() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

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
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;
    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    test_utils::commit_all(&repository);
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

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
    )?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\nlocal\nmore local\n",
    )?;

    // add something to the branch
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1);

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(String::from_utf8(contents)?, "line1\nline2\nline3\nline4\n");

    // update the target branch
    // this should leave the work on file2, but update the contents of file1
    // and the branch diff should only be on file2
    update_base_branch(&gb_repo, &project_repository).unwrap();

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        String::from_utf8(contents)?,
        "line1\nline2\nline3\nline4\nupstream\n"
    );

    // assert that the vbranch target is updated
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1); // branch commit, rebased
    let head_sha = branch.commits[0].id.parse::<git::Oid>()?;

    let head_commit = repository.find_commit(head_sha)?;
    let parent = head_commit.parent(0)?;
    assert_eq!(parent.id(), up_target);

    Ok(())
}

#[test]
fn test_update_base_branch_detect_integrated_branches() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    test_utils::commit_all(&repository);
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

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
    )?;

    // add something to the branch
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits.len(), 1);

    // update the target branch
    // this should notice that the trees are the same after the merge, so it should unapply the branch
    update_base_branch(&gb_repo, &project_repository)?;

    // integrated branch should be deleted
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert!(!branches.iter().any(|b| b.id == branch1_id));

    Ok(())
}

#[test]
fn test_update_base_branch_detect_integrated_branches_with_more_work() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    test_utils::commit_all(&repository);

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    // create a vbranch
    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    test_utils::commit_all(&repository);
    let up_target = repository.head().unwrap().target().unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    repository.reference(
        "refs/remotes/origin/master",
        up_target,
        true,
        "update target",
    )?;

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
    )?;

    // add some uncommitted work
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "local\nline1\nline2\nline3\nline4\nupstream\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1);

    // update the target branch
    // this should notice that the trees are the same after the merge, but there are files on the branch, so do a merge and then leave the files there
    update_base_branch(&gb_repo, &project_repository)?;

    // there should be a new vbranch created, but nothing is on it
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 2);

    Ok(())
}

#[test]
fn test_update_base_branch_no_commits_no_conflict() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo_path,
        user_store,
        project_store,
        ..
    } = new_test_deps()?;

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    // create a vbranch
    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    test_utils::commit_all(&repository);
    let up_target = repository.head().unwrap().target().unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    repository.reference(
        "refs/remotes/origin/master",
        up_target,
        true,
        "update target",
    )?;

    // revert this file
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    // add some uncommitted work
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;

    unapply_branch(&gb_repo, &project_repository, &branch1_id)?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches[0];
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.commits.len(), 0);

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);

    update_base_branch(&gb_repo, &project_repository)?;

    // this should bring back the branch, with the same file changes, but merged into the upstream work
    apply_branch(&gb_repo, &project_repository, &branch1_id)?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches[0];
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.commits.len(), 0);

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nupstream\n",
        String::from_utf8(contents)?
    );

    Ok(())
}

#[test]
fn test_update_target_with_conflicts_in_vbranches() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;

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
    test_utils::commit_all(&repository);

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: repository.head().unwrap().target().unwrap(),
    })?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

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
    test_utils::commit_all(&repository);
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch7_id.clone(),
            name: Some("Situation 7".to_string()),
            ownership: Some("test.txt:1-5".parse()?),
            ..Default::default()
        },
    )?;
    commit(
        &gb_repo,
        &project_repository,
        &branch7_id,
        "integrated commit",
        None,
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch1_id.clone(),
            name: Some("Situation 1".to_string()),
            ownership: Some("test.txt:1-5".parse()?),
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            name: Some("Situation 2".to_string()),
            ownership: Some("test.txt:1-5".parse()?),
            ..Default::default()
        },
    )?;
    commit(
        &gb_repo,
        &project_repository,
        &branch2_id,
        "commit conflicts",
        None,
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "sit2.fixed in uncomitted\nline1\nline2\nline3\nline4\nupstream\n",
    )?;
    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some("test.txt:1-6".parse()?),
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch3_id.clone(),
            name: Some("Situation 3".to_string()),
            ownership: Some("test2.txt:1-5".parse()?),
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch5_id.clone(),
            name: Some("Situation 5".to_string()),
            ownership: Some("test3.txt:1-4".parse()?),
            ..Default::default()
        },
    )?;
    commit(
        &gb_repo,
        &project_repository,
        &branch5_id,
        "broken, but will fix",
        None,
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "test\nline1\nline2\nfile3\nupstream\n",
    )?;
    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch5_id.clone(),
            ownership: Some("test3.txt:1-5".parse()?),
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch4_id.clone(),
            name: Some("Situation 4".to_string()),
            ownership: Some("test.txt:1-5".parse()?),
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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch6_id.clone(),
            name: Some("Situation 6".to_string()),
            ownership: Some("test2.txt:1-5".parse()?),
            ..Default::default()
        },
    )?;

    // update the target branch
    update_base_branch(&gb_repo, &project_repository).unwrap();

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;

    // 1. unapplied branch, uncommitted conflicts
    let branch = branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert!(!branch.active);
    assert!(!is_virtual_branch_mergeable(&gb_repo, &project_repository, &branch.id).unwrap());
    assert!(!branch.base_current);

    // 2. unapplied branch, committed conflicts but not uncommitted
    let branch = branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert!(!branch.active);
    assert!(is_virtual_branch_mergeable(&gb_repo, &project_repository, &branch.id).unwrap());
    assert!(branch.base_current);
    assert_eq!(branch.commits.len(), 2);

    // 3. unapplied branch, no conflicts
    let branch = branches.iter().find(|b| b.id == branch3_id).unwrap();
    assert!(!branch.active);
    assert!(is_virtual_branch_mergeable(&gb_repo, &project_repository, &branch.id).unwrap());
    assert!(branch.base_current);

    // 4. applied branch, uncommitted conflicts
    let branch = branches.iter().find(|b| b.id == branch4_id).unwrap();
    assert!(!branch.active);
    assert!(!is_virtual_branch_mergeable(&gb_repo, &project_repository, &branch.id).unwrap());
    assert!(!branch.base_current);

    // 5. applied branch, committed conflicts but not uncommitted
    let branch = branches.iter().find(|b| b.id == branch5_id).unwrap();
    assert!(!branch.active); // cannot merge history into new target
    assert!(!is_virtual_branch_mergeable(&gb_repo, &project_repository, &branch.id).unwrap());
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
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id,
            ownership: Some("test2.txt:1-3".parse()?),
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

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    unapply_branch(&gb_repo, &project_repository, &branch1_id)?;

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(!branch.active);

    apply_branch(&gb_repo, &project_repository, &branch1_id)?;
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nbranch1\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    Ok(())
}

#[test]
fn test_apply_unapply_added_deleted_files() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some("test2.txt:0-0".parse()?),
            ..Default::default()
        },
    )?;
    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch3_id.clone(),
            ownership: Some("test3.txt:1-2".parse()?),
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
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            ownership: Some("test4.txt:1-3".parse()?),
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
    test_utils::commit_all(&repository);
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
    test_utils::commit_all(&repository);
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
            files: vec!["test2.txt:1-6".parse()?],
        },
        ..branch4
    })?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(branches.len(), 4);

    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert!(!branch1.active);
    assert!(!is_virtual_branch_mergeable(&gb_repo, &project_repository, &branch1.id).unwrap());

    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert!(!branch2.active);
    assert!(is_virtual_branch_mergeable(&gb_repo, &project_repository, &branch2.id).unwrap());

    let remotes =
        list_remote_branches(&gb_repo, &project_repository).expect("failed to list remotes");
    let remote1 = &remotes
        .iter()
        .find(|b| b.name == "refs/remotes/origin/remote_branch")
        .unwrap();
    assert!(!is_remote_branch_mergeable(
        &gb_repo,
        &project_repository,
        &"refs/remotes/origin/remote_branch".parse().unwrap()
    )
    .unwrap());
    assert_eq!(remote1.commits.len(), 1);

    let remote2 = &remotes
        .iter()
        .find(|b| b.name == "refs/remotes/origin/remote_branch2")
        .unwrap();
    assert!(is_remote_branch_mergeable(
        &gb_repo,
        &project_repository,
        &"refs/remotes/origin/remote_branch2".parse().unwrap()
    )
    .unwrap());
    assert_eq!(remote2.commits.len(), 2);

    Ok(())
}

#[test]
fn test_detect_remote_commits() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
        None,
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
        None,
    )?;

    // push the commit upstream
    let branch1 = branch_reader.read(&branch1_id)?;
    let up_target = branch1.head;
    let remote_branch: git::RemoteBranchName = "refs/remotes/origin/remote_branch".parse().unwrap();
    repository.reference(&remote_branch.to_string(), up_target, true, "update target")?;
    // set the upstream reference
    branch_writer.write(&Branch {
        upstream: Some(remote_branch),
        ..branch1
    })?;

    // create another commit that is not pushed up
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nmore upstream\nmore work\n",
    )?;

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "local commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
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
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch\n",
    )?;
    test_utils::commit_all(&repository);

    let upstream: git::BranchName = "refs/remotes/origin/branch1".parse().unwrap();

    repository.reference(
        &upstream.to_string(),
        repository.head().unwrap().target().unwrap(),
        true,
        "update target",
    )?;

    repository.set_head(&upstream.to_string())?;
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

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 0);

    // create a new virtual branch from the remote branch
    let branch2_id =
        create_virtual_branch_from_branch(&gb_repo, &project_repository, &upstream, None)?.id;

    // shouldn't be anything on either of our branches
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
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

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
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

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert_eq!(branch2.files.len(), 1);
    assert!(branch2.active);

    // add to another file so it goes to the default one
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert_eq!(branch2.files.len(), 2);
    assert!(branch2.active);

    Ok(())
}

#[test]
fn test_create_vbranch_from_behind_remote_branch() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    let base_commit = repository.head().unwrap().target().unwrap();

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
    test_utils::commit_all(&repository);
    let remote_commit = repository.head().unwrap().target().unwrap();

    let remote_branch: git::BranchName = "refs/remotes/origin/branch1".parse().unwrap();
    repository.reference(
        &remote_branch.to_string(),
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
    let branch1_id =
        create_virtual_branch_from_branch(&gb_repo, &project_repository, &remote_branch, None)?.id;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
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

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branches.len(), 1);
    // our branch still no hunks
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 2); // a merge commit too

    Ok(())
}

#[test]
fn test_upstream_integrated_vbranch() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    // ok, we need a vbranch with some work and an upstream target that also includes that work, but the base is behind
    // plus a branch with work not in upstream so we can see that it is not included in the vbranch

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
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\n",
    )?;
    test_utils::commit_all(&repository);

    let base_commit = repository.head().unwrap().target().unwrap();

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "file1\nversion2\n",
    )?;
    test_utils::commit_all(&repository);

    let upstream_commit = repository.head().unwrap().target().unwrap();
    repository.reference(
        "refs/remotes/origin/master",
        upstream_commit,
        true,
        "update target",
    )?;

    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "http://origin.com/project".to_string(),
        sha: base_commit,
    })?;
    repository.remote("origin", "http://origin.com/project")?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    // create vbranches, one integrated, one not
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
        std::path::Path::new(&project.path).join(file_path2),
        "file2\nversion2\n",
    )?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\nversion2\n",
    )?;

    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch1_id.clone(),
            name: Some("integrated".to_string()),
            ownership: Some("test.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id.clone(),
            name: Some("not integrated".to_string()),
            ownership: Some("test2.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    update_branch(
        &gb_repo,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch3_id.clone(),
            name: Some("not committed".to_string()),
            ownership: Some("test3.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    // create a new virtual branch from the remote branch
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "integrated commit",
        None,
    )?;
    commit(
        &gb_repo,
        &project_repository,
        &branch2_id,
        "non-integrated commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;

    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert!(branch1.commits.iter().any(|c| c.is_integrated));
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 1);

    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert!(!branch2.commits.iter().any(|c| c.is_integrated));
    assert_eq!(branch2.files.len(), 0);
    assert_eq!(branch2.commits.len(), 1);

    let branch3 = &branches.iter().find(|b| b.id == branch3_id).unwrap();
    assert!(!branch3.commits.iter().any(|c| c.is_integrated));
    assert_eq!(branch3.files.len(), 1);
    assert_eq!(branch3.commits.len(), 0);

    Ok(())
}

#[test]
fn test_commit_same_hunk_twice() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "first commit to test.txt",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 0, "no files expected");

    assert_eq!(branch.commits.len(), 1, "file should have been commited");
    assert_eq!(branch.commits[0].files.len(), 1, "hunks expected");
    assert_eq!(
        branch.commits[0].files[0].hunks.len(),
        1,
        "one hunk should have been commited"
    );

    // update same lines

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nPATCH1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1, "one file should be changed");
    assert_eq!(branch.commits.len(), 1, "commit is still there");

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "second commit to test.txt",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(
        branch.files.len(),
        0,
        "all changes should have been commited"
    );

    assert_eq!(branch.commits.len(), 2, "two commits expected");
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[0].files[0].hunks.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 1);
    assert_eq!(branch.commits[1].files[0].hunks.len(), 1);

    Ok(())
}

#[test]
fn test_commit_same_file_twice() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "first commit to test.txt",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 0, "no files expected");

    assert_eq!(branch.commits.len(), 1, "file should have been commited");
    assert_eq!(branch.commits[0].files.len(), 1, "hunks expected");
    assert_eq!(
        branch.commits[0].files[0].hunks.len(),
        1,
        "one hunk should have been commited"
    );

    // add second patch

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\npatch2\nline11\nline12\n",
        )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1, "one file should be changed");
    assert_eq!(branch.commits.len(), 1, "commit is still there");

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "second commit to test.txt",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(
        branch.files.len(),
        0,
        "all changes should have been commited"
    );

    assert_eq!(branch.commits.len(), 2, "two commits expected");
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[0].files[0].hunks.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 1);
    assert_eq!(branch.commits[1].files[0].hunks.len(), 1);

    Ok(())
}

#[test]
fn test_commit_partial_by_hunk() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\npatch2\nline11\nline12\n",
        )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 2);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "first commit to test.txt",
        Some(&"test.txt:1-6".parse::<Ownership>().unwrap()),
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[0].files[0].hunks.len(), 1);

    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "second commit to test.txt",
        Some(&"test.txt:16-22".parse::<Ownership>().unwrap()),
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits.len(), 2);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[0].files[0].hunks.len(), 1);
    assert_eq!(branch.commits[1].files.len(), 1);
    assert_eq!(branch.commits[1].files[0].hunks.len(), 1);

    Ok(())
}

#[test]
fn test_commit_partial_by_file() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    let commit1_oid = repository.head().unwrap().target().unwrap();
    let commit1 = repository.find_commit(commit1_oid).unwrap();

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "branch1 commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit2 = &branch1.commits[0].id;
    let commit2 = commit2
        .parse::<git::Oid>()
        .expect("failed to parse commit id");
    let commit2 = repository
        .find_commit(commit2)
        .expect("failed to get commit object");

    let tree = commit1.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

    // get the tree
    let tree = commit2.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

    Ok(())
}

#[test]
fn test_commit_add_and_delete_files() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    let commit1_oid = repository.head().unwrap().target().unwrap();
    let commit1 = repository.find_commit(commit1_oid).unwrap();

    set_test_target(&gb_repo, &project_repository, &repository)?;

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
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "branch1 commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit2 = &branch1.commits[0].id;
    let commit2 = commit2
        .parse::<git::Oid>()
        .expect("failed to parse commit id");
    let commit2 = repository
        .find_commit(commit2)
        .expect("failed to get commit object");

    let tree = commit1.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

    // get the tree
    let tree = commit2.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

    Ok(())
}

#[test]
fn test_commit_executable_and_symlinks() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    // add symlinked file
    let file_path3 = std::path::Path::new("test3.txt");
    let src = std::path::Path::new(&project.path).join(file_path2);
    let dst = std::path::Path::new(&project.path).join(file_path3);
    symlink(src, dst)?;

    // add executable
    let file_path4 = std::path::Path::new("test4.bin");
    let exec = std::path::Path::new(&project.path).join(file_path4);
    std::fs::write(&exec, "exec\n")?;
    let permissions = fs::metadata(&exec)?.permissions();
    let new_permissions = Permissions::from_mode(permissions.mode() | 0o111); // Add execute permission
    fs::set_permissions(&exec, new_permissions)?;

    let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
        .expect("failed to create virtual branch")
        .id;

    // commit
    commit(
        &gb_repo,
        &project_repository,
        &branch1_id,
        "branch1 commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    let commit = &branch1.commits[0].id;
    let commit = commit
        .parse::<git::Oid>()
        .expect("failed to parse commit id");
    let commit = repository
        .find_commit(commit)
        .expect("failed to get commit object");

    let tree = commit.tree().expect("failed to get tree");

    let list = tree_to_entry_list(&repository, &tree);
    assert_eq!(list[0].0, "test.txt");
    assert_eq!(list[0].1, "100644");
    assert_eq!(list[1].0, "test2.txt");
    assert_eq!(list[1].1, "100644");
    assert_eq!(list[2].0, "test3.txt");
    assert_eq!(list[2].1, "120000");
    assert_eq!(list[2].2, "test2.txt");
    assert_eq!(list[3].0, "test4.bin");
    assert_eq!(list[3].1, "100755");

    Ok(())
}

fn tree_to_file_list(repository: &git::Repository, tree: &git::Tree) -> Vec<String> {
    let mut file_list = Vec::new();
    tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
        let path = entry.name().unwrap();
        let entry = tree
            .get_path(std::path::Path::new(path))
            .unwrap_or_else(|_| panic!("failed to get tree entry for path {}", path));
        let object = entry
            .to_object(repository)
            .unwrap_or_else(|_| panic!("failed to get object for tree entry {}", path));
        if object.kind() == Some(git2::ObjectType::Blob) {
            file_list.push(path.to_string());
        }
        TreeWalkResult::Ok
    })
    .expect("failed to walk tree");
    file_list
}

fn tree_to_entry_list(
    repository: &git::Repository,
    tree: &git::Tree,
) -> Vec<(String, String, String, String)> {
    let mut file_list = Vec::new();
    tree.walk(git2::TreeWalkMode::PreOrder, |_root, entry| {
        let path = entry.name().unwrap();
        let entry = tree
            .get_path(std::path::Path::new(path))
            .unwrap_or_else(|_| panic!("failed to get tree entry for path {}", path));
        let object = entry
            .to_object(repository)
            .unwrap_or_else(|_| panic!("failed to get object for tree entry {}", path));
        let blob = object.as_blob().expect("failed to get blob");
        // convert content to string
        let octal_mode = format!("{:o}", entry.filemode());
        if let Ok(content) =
            std::str::from_utf8(blob.content()).context("failed to convert content to string")
        {
            file_list.push((
                path.to_string(),
                octal_mode,
                content.to_string(),
                blob.id().to_string(),
            ));
        } else {
            file_list.push((
                path.to_string(),
                octal_mode,
                "BINARY".to_string(),
                blob.id().to_string(),
            ));
        }
        TreeWalkResult::Ok
    })
    .expect("failed to walk tree");
    file_list
}

#[test]
fn test_apply_out_of_date_vbranch() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    let base_commit = repository.head().unwrap().target().unwrap();
    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "http://origin.com/project".to_string(),
        sha: base_commit,
    })?;
    repository.remote("origin", "http://origin.com/project")?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // ok, pretend upstream was updated
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    test_utils::commit_all(&repository);

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
    test_utils::commit_all(&repository);
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
            files: vec!["test2.txt:1-2".parse()?],
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

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(branches.len(), 1); // just the unapplied one with it's one commit
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 1);

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\n");

    // update target, this will update the wd and add an empty default branch
    update_base_branch(&gb_repo, &project_repository)?;

    // updated the file
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\n");

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(branches.len(), 1);

    // apply branch which is now out of date
    // - it should merge the new target into it and update the wd and nothing is in files
    apply_branch(&gb_repo, &project_repository, branch_id)?;

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\nbranch");

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(branches.len(), 1); // one is there still
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 1);

    Ok(())
}

#[test]
fn test_apply_out_of_date_conflicting_vbranch() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    let base_commit = repository.head().unwrap().target().unwrap();
    target::Writer::new(&gb_repo).write_default(&target::Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "http://origin.com/project".to_string(),
        sha: base_commit,
    })?;
    repository.remote("origin", "http://origin.com/project")?;
    super::integration::update_gitbutler_integration(&gb_repo, &project_repository)?;

    repository.set_head("refs/heads/master")?;
    repository.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // ok, pretend upstream was updated
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    test_utils::commit_all(&repository);

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
    test_utils::commit_all(&repository);
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
            files: vec!["test2.txt:1-2".parse()?, "test.txt:1-5".parse()?],
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

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(branches.len(), 1); // just the unapplied one with it's one commit
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 1);

    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\n");

    // update target, this will update the wd and add an empty default branch
    update_base_branch(&gb_repo, &project_repository)?;

    // updated the file
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
    let contents = std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!(contents, "file2\n");

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(branches.len(), 1);
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert!(!is_virtual_branch_mergeable(
        &gb_repo,
        &project_repository,
        &branch1.id
    )?);
    assert!(!branch1.base_current);

    // apply branch which is now out of date and conflicting
    apply_branch(&gb_repo, &project_repository, branch_id)?;

    assert!(project_repository::conflicts::is_conflicting(
        &project_repository,
        None
    )?);

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.files.first().unwrap().hunks.len(), 1);
    assert!(branch1.files.first().unwrap().conflicted);
    assert_eq!(branch1.commits.len(), 1);
    assert!(branch1.conflicted);

    // try to commit, fail
    let result = commit(
        &gb_repo,
        &project_repository,
        branch_id,
        "resolve commit",
        None,
    );
    assert!(result.is_err());

    // fix the conflict and commit it
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nconflict\n",
    )?;

    // make sure the branch has that commit and that the parent is the target
    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.files.first().unwrap().hunks.len(), 1);
    assert!(!branch1.files.first().unwrap().conflicted);
    assert!(branch1.conflicted);
    assert!(branch1.active);

    // commit
    commit(
        &gb_repo,
        &project_repository,
        branch_id,
        "resolve commit",
        None,
    )?;

    let branches = list_virtual_branches(&gb_repo, &project_repository)?;
    let branch1 = &branches.iter().find(|b| &b.id == branch_id).unwrap();
    let last_commit = branch1.commits.first().unwrap();
    let last_commit_oid = last_commit.id.parse::<git::Oid>()?;
    let commit = gb_repo.git_repository.find_commit(last_commit_oid)?;
    assert!(!branch1.conflicted);
    assert_eq!(commit.parent_count(), 2);

    Ok(())
}

#[test]
fn test_apply_conflicting_vbranch() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
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
    test_utils::commit_all(&repository);

    set_test_target(&gb_repo, &project_repository, &repository)?;

    // write new unapplied virtual branch with other changes
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nconflict\n",
    )?;
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\nbranch\n",
    )?;
    test_utils::commit_all(&repository);
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
            files: vec!["test.txt:1-5".parse()?, "test2.txt:1-2".parse()?],
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

#[test]
fn test_verify_branch_commits_to_integration() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    set_test_target(&gb_repo, &project_repository, &repository)?;

    assert!(integration::verify_branch(&gb_repo, &project_repository).is_ok());

    //  write two commits
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(std::path::Path::new(&project.path).join(file_path2), "file")?;
    test_utils::commit_all(&repository);
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "update",
    )?;
    test_utils::commit_all(&repository);

    // verify puts commits onto the virtual branch
    assert!(integration::verify_branch(&gb_repo, &project_repository).is_ok());

    // one virtual branch with two commits was created
    let virtual_branches = list_virtual_branches(&gb_repo, &project_repository)?;
    assert_eq!(virtual_branches.len(), 1);

    let branch = &virtual_branches.first().unwrap();
    assert_eq!(branch.commits.len(), 2);
    assert_eq!(branch.commits.len(), 2);

    Ok(())
}

#[test]
fn test_verify_branch_not_integration() -> Result<()> {
    let TestDeps {
        repository,
        project,
        gb_repo,
        ..
    } = new_test_deps()?;
    let project_repository = project_repository::Repository::open(&project)?;

    set_test_target(&gb_repo, &project_repository, &repository)?;

    assert!(integration::verify_branch(&gb_repo, &project_repository).is_ok());

    project_repository
        .git_repository
        .set_head("refs/heads/master")?;

    let verify_result = integration::verify_branch(&gb_repo, &project_repository);
    assert!(verify_result.is_err());
    assert_eq!(
        verify_result.unwrap_err().to_string(),
        "head is refs/heads/master"
    );

    Ok(())
}
