use std::{collections::HashMap, fs, io::Write, path};

use anyhow::{Context, Result};
use pretty_assertions::assert_eq;
#[cfg(target_family = "unix")]
use std::{
    fs::Permissions,
    os::unix::{fs::symlink, prelude::*},
};

use crate::{
    gb_repository, git, project_repository, reader, sessions,
    tests::{self, empty_bare_repository, Case, Suite},
    virtual_branches::errors::CommitError,
};

use super::*;
use branch::{BranchCreateRequest, BranchOwnershipClaims};

pub fn set_test_target(
    gb_repo: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<()> {
    let remote_repo = empty_bare_repository();
    let mut remote = project_repository
        .git_repository
        .remote(
            "origin",
            &remote_repo.path().to_str().unwrap().parse().unwrap(),
        )
        .expect("failed to add remote");
    remote.push(&["refs/heads/master:refs/heads/master"], None)?;

    target::Writer::new(gb_repo, project_repository.project().path.as_path())?
        .write_default(&target::Target {
            branch: "refs/remotes/origin/master".parse().unwrap(),
            remote_url: remote_repo.path().to_str().unwrap().parse().unwrap(),
            sha: remote_repo.head().unwrap().target().unwrap(),
        })
        .expect("failed to write target");

    super::integration::update_gitbutler_integration(gb_repo, project_repository)
        .expect("failed to update integration");

    Ok(())
}

#[test]
fn test_commit_on_branch_then_change_file_then_get_status() -> Result<()> {
    let Case {
        project,
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([
        (
            path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\n",
        ),
        (
            path::PathBuf::from("test2.txt"),
            "line5\nline6\nline7\nline8\n",
        ),
    ]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
        std::path::Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
        None,
        None,
        false,
    )?;

    // status (no files)
    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.commits.len(), 1);

    std::fs::write(
        std::path::Path::new(&project.path).join("test2.txt"),
        "line5\nline6\nlineBLAH\nline7\nline8\n",
    )?;

    // should have just the last change now, the other line is committed
    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.commits.len(), 1);

    Ok(())
}

#[test]
fn test_signed_commit() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = suite.new_case_with_files(HashMap::from([
        (
            path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\n",
        ),
        (
            path::PathBuf::from("test2.txt"),
            "line5\nline6\nline7\nline8\n",
        ),
    ]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
        std::path::Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let mut config = project_repository
        .git_repository
        .config()
        .with_context(|| "failed to get config")?;
    config.set_str("gitbutler.signCommits", "true")?;

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
        Some(suite.keys.get_or_create()?).as_ref(),
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository).unwrap();
    let commit_id = &branches[0].commits[0].id;
    let commit_obj = project_repository.git_repository.find_commit(*commit_id)?;
    // check the raw_header contains the string "SSH SIGNATURE"
    assert!(commit_obj.raw_header().unwrap().contains("SSH SIGNATURE"));

    Ok(())
}

#[test]
fn test_track_binary_files() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case();

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
    tests::commit_all(&project_repository.git_repository);

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
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

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
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
        &gb_repository,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
        None,
        None,
        false,
    )?;

    // status (no files)
    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository).unwrap();
    let commit_id = &branches[0].commits[0].id;
    let commit_obj = project_repository.git_repository.find_commit(*commit_id)?;
    let tree = commit_obj.tree()?;
    let files = tree_to_entry_list(&project_repository.git_repository, &tree);
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
        &gb_repository,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository).unwrap();
    let commit_id = &branches[0].commits[0].id;
    // get tree from commit_id
    let commit_obj = project_repository.git_repository.find_commit(*commit_id)?;
    let tree = commit_obj.tree()?;
    let files = tree_to_entry_list(&project_repository.git_repository, &tree);

    assert_eq!(files[0].0, "image.bin");
    assert_eq!(files[0].3, "ea6901a04d1eed6ebf6822f4360bda9f008fa317");

    Ok(())
}

#[test]
fn test_create_branch_with_ownership() -> Result<()> {
    let Case {
        project,
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\n",
    )
    .unwrap();

    let branch0 = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");

    get_status_by_branch(&gb_repository, &project_repository).expect("failed to get status");

    let current_session = gb_repository.get_or_create_current_session().unwrap();
    let current_session_reader = sessions::Reader::open(&gb_repository, &current_session).unwrap();
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch0 = branch_reader.read(&branch0.id).unwrap();

    let branch1 = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest {
            ownership: Some(branch0.ownership),
            ..Default::default()
        },
    )
    .expect("failed to create virtual branch");

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch0.id].len(), 0);
    assert_eq!(files_by_branch_id[&branch1.id].len(), 1);

    Ok(())
}

#[test]
fn test_create_branch_in_the_middle() -> Result<()> {
    let Case {
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");
    create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");
    create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest {
            order: Some(1),
            ..Default::default()
        },
    )
    .expect("failed to create virtual branch");

    let current_session = gb_repository.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)?;

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
    let Case {
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");

    let current_session = gb_repository.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)?;

    let branches = iterator::BranchIterator::new(&current_session_reader)?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .expect("failed to read branches");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "Virtual branch");
    assert!(branches[0].applied);
    assert_eq!(branches[0].ownership, BranchOwnershipClaims::default());
    assert_eq!(branches[0].order, 0);

    Ok(())
}

#[test]
fn test_hunk_expantion() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\n",
    )?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    // even though selected branch has changed
    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch1_id,
            order: Some(1),
            ..Default::default()
        },
    )?;
    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id,
            order: Some(0),
            ..Default::default()
        },
    )?;

    // a slightly different hunk should still go to the same branch
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\n",
    )?;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    Ok(())
}

#[test]
fn test_get_status_files_by_branch_no_hunks_no_branches() -> Result<()> {
    let Case {
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;

    assert_eq!(statuses.len(), 0);

    Ok(())
}

#[test]
fn test_get_status_files_by_branch() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\n",
    )?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    Ok(())
}

#[test]
fn test_move_hunks_multiple_sources() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
    )]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch3_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
            std::path::Path::new(&project.path).join("test.txt"),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;

    let current_session = gb_repository.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(&gb_repository, project.path.as_path())?;
    let mut branch2 = branch_reader.read(&branch2_id)?;
    branch2.ownership = BranchOwnershipClaims {
        claims: vec!["test.txt:1-5".parse()?],
    };
    branch_writer.write(&mut branch2)?;
    let mut branch1 = branch_reader.read(&branch1_id)?;
    branch1.ownership = BranchOwnershipClaims {
        claims: vec!["test.txt:11-15".parse()?],
    };
    branch_writer.write(&mut branch1)?;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 3);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    // assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 1);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 1);
    // assert_eq!(files_by_branch_id[&branch2_id][0].hunks.len(), 1);
    assert_eq!(files_by_branch_id[&branch3_id].len(), 0);

    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch3_id,
            ownership: Some("test.txt:1-5,11-15".parse()?),
            ..Default::default()
        },
    )?;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 3);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 0);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);
    assert_eq!(files_by_branch_id[&branch3_id].len(), 1);
    assert_eq!(
        files_by_branch_id[&branch3_id][std::path::Path::new("test.txt")].len(),
        2
    );
    assert_eq!(
        files_by_branch_id[&branch3_id][std::path::Path::new("test.txt")][0].diff,
        "@@ -1,3 +1,4 @@\n+line0\n line1\n line2\n line3\n"
    );
    assert_eq!(
        files_by_branch_id[&branch3_id][std::path::Path::new("test.txt")][1].diff,
        "@@ -10,3 +11,4 @@ line9\n line10\n line11\n line12\n+line13\n"
    );
    Ok(())
}

#[test]
fn test_move_hunks_partial_explicitly() -> Result<()> {
    let Case {
        project_repository,
        project,
    gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
    )]));

    set_test_target(&gb_repository, &project_repository)?;

    std::fs::write(
            std::path::Path::new(&project.path).join("test.txt"),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\n",
        )?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    // assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 2);
    assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id,
            ownership: Some("test.txt:1-5".parse()?),
            ..Default::default()
        },
    )?;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
    assert_eq!(
        files_by_branch_id[&branch1_id][std::path::Path::new("test.txt")].len(),
        1
    );
    assert_eq!(
        files_by_branch_id[&branch1_id][std::path::Path::new("test.txt")][0].diff,
        "@@ -11,3 +12,4 @@ line10\n line11\n line12\n line13\n+line14\n"
    );

    assert_eq!(files_by_branch_id[&branch2_id].len(), 1);
    assert_eq!(
        files_by_branch_id[&branch2_id][std::path::Path::new("test.txt")].len(),
        1
    );
    assert_eq!(
        files_by_branch_id[&branch2_id][std::path::Path::new("test.txt")][0].diff,
        "@@ -1,3 +1,4 @@\n+line0\n line1\n line2\n line3\n"
    );

    Ok(())
}

#[test]
fn test_add_new_hunk_to_the_end() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline13\nline14\n",
    )]));

    set_test_target(&gb_repository, &project_repository)?;

    std::fs::write(
            std::path::Path::new(&project.path).join("test.txt"),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
        )?;

    create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;
    assert_eq!(
        statuses[0].1[std::path::Path::new("test.txt")][0].diff,
        "@@ -11,5 +11,5 @@ line10\n line11\n line12\n line13\n-line13\n line14\n+line15\n"
    );

    std::fs::write(
            std::path::Path::new(&project.path).join("test.txt"),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
        )?;

    let statuses = get_status_by_branch(&gb_repository, &project_repository)
        .expect("failed to get status")
        .0;

    assert_eq!(
        statuses[0].1[std::path::Path::new("test.txt")][0].diff,
        "@@ -11,5 +12,5 @@ line10\n line11\n line12\n line13\n-line13\n line14\n+line15\n"
    );
    assert_eq!(
        statuses[0].1[std::path::Path::new("test.txt")][1].diff,
        "@@ -1,3 +1,4 @@\n+line0\n line1\n line2\n line3\n"
    );

    Ok(())
}

#[test]
fn test_merge_vbranch_upstream_clean_rebase() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = suite.new_case();

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    tests::commit_all(&project_repository.git_repository);
    let target_oid = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    tests::commit_all(&project_repository.git_repository);
    let last_push = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();

    // coworker adds some work
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\ncoworker work\n",
    )?;

    tests::commit_all(&project_repository.git_repository);
    let coworker_work = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    project_repository.git_repository.reference(
        &"refs/remotes/origin/master".parse().unwrap(),
        coworker_work,
        true,
        "update target",
    )?;

    // revert to our file
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;

    set_test_target(&gb_repository, &project_repository)?;
    target::Writer::new(&gb_repository, project_repository.project().path.as_path())?
        .write_default(&target::Target {
            branch: "refs/remotes/origin/master".parse().unwrap(),
            remote_url: "origin".to_string(),
            sha: target_oid,
        })?;

    // add some uncommitted work
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "file2\n",
    )?;

    let remote_branch: git::RemoteRefname = "refs/remotes/origin/master".parse().unwrap();
    let branch_writer = branch::Writer::new(&gb_repository, project.path.as_path())?;
    let mut branch = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");
    branch.upstream = Some(remote_branch.clone());
    branch.head = last_push;
    branch_writer
        .write(&mut branch)
        .context("failed to write target branch after push")?;

    // create the branch
    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches[0];
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.commits.len(), 1);
    // assert_eq!(branch1.upstream.as_ref().unwrap().commits.len(), 1);

    merge_virtual_branch_upstream(
        &gb_repository,
        &project_repository,
        &branch1.id,
        Some(suite.keys.get_or_create()?).as_ref(),
        None,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches[0];

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nupstream\ncoworker work\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("file2\n", String::from_utf8(contents)?);
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.commits.len(), 2);
    // assert_eq!(branch1.upstream.as_ref().unwrap().commits.len(), 0);

    Ok(())
}

#[test]
fn test_merge_vbranch_upstream_conflict() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case();

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    tests::commit_all(&project_repository.git_repository);
    let target_oid = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    tests::commit_all(&project_repository.git_repository);
    let last_push = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();

    // coworker adds some work
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\ncoworker work\n",
    )?;

    tests::commit_all(&project_repository.git_repository);
    let coworker_work = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    project_repository.git_repository.reference(
        &"refs/remotes/origin/master".parse().unwrap(),
        coworker_work,
        true,
        "update target",
    )?;

    // revert to our file
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;

    set_test_target(&gb_repository, &project_repository)?;
    target::Writer::new(&gb_repository, project.path.as_path())?.write_default(
        &target::Target {
            branch: "refs/remotes/origin/master".parse().unwrap(),
            remote_url: "origin".to_string(),
            sha: target_oid,
        },
    )?;

    // add some uncommitted work
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nother side\n",
    )?;

    let remote_branch: git::RemoteRefname = "refs/remotes/origin/master".parse().unwrap();
    let branch_writer = branch::Writer::new(&gb_repository, project.path.as_path())?;
    let mut branch = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");
    branch.upstream = Some(remote_branch.clone());
    branch.head = last_push;
    branch_writer
        .write(&mut branch)
        .context("failed to write target branch after push")?;

    // create the branch
    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches[0];

    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.commits.len(), 1);
    // assert_eq!(branch1.upstream.as_ref().unwrap().commits.len(), 1);

    merge_virtual_branch_upstream(&gb_repository, &project_repository, &branch1.id, None, None)?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches[0];
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;

    assert_eq!(
        "line1\nline2\nline3\nline4\nupstream\n<<<<<<< ours\nother side\n=======\ncoworker work\n>>>>>>> theirs\n",
        String::from_utf8(contents)?
    );

    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.commits.len(), 1);
    assert!(branch1.conflicted);

    // fix the conflict
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nother side\ncoworker work\n",
    )?;

    // make gb see the conflict resolution
    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    assert!(branches[0].conflicted);

    // commit the merge resolution
    commit(
        &gb_repository,
        &project_repository,
        &branch1.id,
        "fix merge conflict",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches[0];
    assert!(!branch1.conflicted);
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.commits.len(), 3);

    // make sure the last commit was a merge commit (2 parents)
    let last_id = &branch1.commits[0].id;
    let last_commit = project_repository.git_repository.find_commit(*last_id)?;
    assert_eq!(last_commit.parent_count(), 2);

    Ok(())
}

#[test]
fn test_unapply_ownership_partial() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\n",
    )]));

    set_test_target(&gb_repository, &project_repository)?;

    std::fs::write(
        std::path::Path::new(&project.path).join("test.txt"),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;

    create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].ownership.claims.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
    assert_eq!(branches[0].ownership.claims[0].hunks.len(), 1);
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&project.path).join("test.txt"))?,
        "line1\nline2\nline3\nline4\nbranch1\n"
    );

    unapply_ownership(
        &gb_repository,
        &project_repository,
        &"test.txt:2-6".parse().unwrap(),
    )
    .unwrap();

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[0].ownership.claims.len(), 0);
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&project.path).join("test.txt"))?,
        "line1\nline2\nline3\nline4\n"
    );

    Ok(())
}

#[test]
fn test_apply_unapply_branch() -> Result<()> {
    let Case {
        project,
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    tests::commit_all(&project_repository.git_repository);

    set_test_target(&gb_repository, &project_repository)?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "line5\nline6\n",
    )?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    update_branch(
        &gb_repository,
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

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    unapply_branch(&gb_repository, &project_repository, &branch1_id)?;

    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(!branch.active);

    apply_branch(&gb_repository, &project_repository, &branch1_id, None, None)?;
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nbranch1\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    Ok(())
}

#[test]
fn test_apply_unapply_added_deleted_files() -> Result<()> {
    let Case {
        project,
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

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
    tests::commit_all(&project_repository.git_repository);

    set_test_target(&gb_repository, &project_repository)?;

    // rm file_path2, add file3
    std::fs::remove_file(std::path::Path::new(&project.path).join(file_path2))?;
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\n",
    )?;

    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch3_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id,
            ownership: Some("test2.txt:0-0".parse()?),
            ..Default::default()
        },
    )?;
    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch3_id,
            ownership: Some("test3.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    unapply_branch(&gb_repository, &project_repository, &branch2_id)?;
    // check that file2 is back
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
    assert_eq!("file2\n", String::from_utf8(contents)?);

    unapply_branch(&gb_repository, &project_repository, &branch3_id)?;
    // check that file3 is gone
    assert!(!std::path::Path::new(&project.path)
        .join(file_path3)
        .exists());

    apply_branch(&gb_repository, &project_repository, &branch2_id, None, None)?;
    // check that file2 is gone
    assert!(!std::path::Path::new(&project.path)
        .join(file_path2)
        .exists());

    apply_branch(&gb_repository, &project_repository, &branch3_id, None, None)?;
    // check that file3 is back
    let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path3))?;
    assert_eq!("file3\n", String::from_utf8(contents)?);

    Ok(())
}

#[test]
fn test_detect_mergeable_branch() -> Result<()> {
    let Case {
        project,
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

    // create a commit and set the target
    let file_path = std::path::Path::new("test.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    tests::commit_all(&project_repository.git_repository);

    set_test_target(&gb_repository, &project_repository)?;

    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;
    let file_path4 = std::path::Path::new("test4.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path4),
        "line5\nline6\n",
    )?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    let current_session = gb_repository.get_or_create_current_session()?;
    let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(&gb_repository, project.path.as_path())?;

    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id,
            ownership: Some("test4.txt:1-3".parse()?),
            ..Default::default()
        },
    )
    .expect("failed to update branch");

    // unapply both branches and create some conflicting ones
    unapply_branch(&gb_repository, &project_repository, &branch1_id)?;
    unapply_branch(&gb_repository, &project_repository, &branch2_id)?;

    project_repository
        .git_repository
        .set_head(&"refs/heads/master".parse().unwrap())?;
    project_repository
        .git_repository
        .checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // create an upstream remote conflicting commit
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    tests::commit_all(&project_repository.git_repository);
    let up_target = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();
    project_repository.git_repository.reference(
        &"refs/remotes/origin/remote_branch".parse().unwrap(),
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
    tests::commit_all(&project_repository.git_repository);
    let up_target = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();
    project_repository.git_repository.reference(
        &"refs/remotes/origin/remote_branch2".parse().unwrap(),
        up_target,
        true,
        "update target",
    )?;
    // remove file_path3
    std::fs::remove_file(std::path::Path::new(&project.path).join(file_path3))?;

    project_repository
        .git_repository
        .set_head(&"refs/heads/gitbutler/integration".parse().unwrap())?;
    project_repository
        .git_repository
        .checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // create branches that conflict with our earlier branches
    create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch");
    let branch4_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
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

    let mut branch4 = branch_reader.read(&branch4_id)?;
    branch4.ownership = BranchOwnershipClaims {
        claims: vec!["test2.txt:1-6".parse()?],
    };
    branch_writer.write(&mut branch4)?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    assert_eq!(branches.len(), 4);

    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
    assert!(!branch1.active);
    assert!(
        !is_virtual_branch_mergeable(&gb_repository, &project_repository, &branch1.id).unwrap()
    );

    let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
    assert!(!branch2.active);
    assert!(is_virtual_branch_mergeable(&gb_repository, &project_repository, &branch2.id).unwrap());

    let remotes =
        list_remote_branches(&gb_repository, &project_repository).expect("failed to list remotes");
    let _remote1 = &remotes
        .iter()
        .find(|b| b.name.to_string() == "refs/remotes/origin/remote_branch")
        .unwrap();
    assert!(!is_remote_branch_mergeable(
        &gb_repository,
        &project_repository,
        &"refs/remotes/origin/remote_branch".parse().unwrap()
    )
    .unwrap());
    // assert_eq!(remote1.commits.len(), 1);

    let _remote2 = &remotes
        .iter()
        .find(|b| b.name.to_string() == "refs/remotes/origin/remote_branch2")
        .unwrap();
    assert!(is_remote_branch_mergeable(
        &gb_repository,
        &project_repository,
        &"refs/remotes/origin/remote_branch2".parse().unwrap()
    )
    .unwrap());
    // assert_eq!(remote2.commits.len(), 2);

    Ok(())
}

#[test]
fn test_upstream_integrated_vbranch() -> Result<()> {
    // ok, we need a vbranch with some work and an upstream target that also includes that work, but the base is behind
    // plus a branch with work not in upstream so we can see that it is not included in the vbranch

    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([
        (path::PathBuf::from("test.txt"), "file1\n"),
        (path::PathBuf::from("test2.txt"), "file2\n"),
        (path::PathBuf::from("test3.txt"), "file3\n"),
    ]));

    let base_commit = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();

    std::fs::write(
        std::path::Path::new(&project.path).join("test.txt"),
        "file1\nversion2\n",
    )?;
    tests::commit_all(&project_repository.git_repository);

    let upstream_commit = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();
    project_repository.git_repository.reference(
        &"refs/remotes/origin/master".parse().unwrap(),
        upstream_commit,
        true,
        "update target",
    )?;

    target::Writer::new(&gb_repository, project_repository.path())?.write_default(
        &target::Target {
            branch: "refs/remotes/origin/master".parse().unwrap(),
            remote_url: "http://origin.com/project".to_string(),
            sha: base_commit,
        },
    )?;
    project_repository
        .git_repository
        .remote("origin", &"http://origin.com/project".parse().unwrap())?;
    super::integration::update_gitbutler_integration(&gb_repository, &project_repository)?;

    // create vbranches, one integrated, one not
    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch2_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;
    let branch3_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
        std::path::Path::new(&project.path).join("test2.txt"),
        "file2\nversion2\n",
    )?;

    std::fs::write(
        std::path::Path::new(&project.path).join("test3.txt"),
        "file3\nversion2\n",
    )?;

    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch1_id,
            name: Some("integrated".to_string()),
            ownership: Some("test.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch2_id,
            name: Some("not integrated".to_string()),
            ownership: Some("test2.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    update_branch(
        &gb_repository,
        &project_repository,
        branch::BranchUpdateRequest {
            id: branch3_id,
            name: Some("not committed".to_string()),
            ownership: Some("test3.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    // create a new virtual branch from the remote branch
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "integrated commit",
        None,
        None,
        None,
        false,
    )?;
    commit(
        &gb_repository,
        &project_repository,
        &branch2_id,
        "non-integrated commit",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;

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
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
            std::path::Path::new(&project.path).join("test.txt"),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "first commit to test.txt",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
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
            std::path::Path::new(&project.path).join("test.txt"),
            "line1\nPATCH1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1, "one file should be changed");
    assert_eq!(branch.commits.len(), 1, "commit is still there");

    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "second commit to test.txt",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
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
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
            std::path::Path::new(&project.path).join("test.txt"),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "first commit to test.txt",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
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
            std::path::Path::new(&project.path).join("file.txt"),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\npatch2\nline11\nline12\n",
        )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1, "one file should be changed");
    assert_eq!(branch.commits.len(), 1, "commit is still there");

    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "second commit to test.txt",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
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
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
            std::path::Path::new(&project.path).join("test.txt"),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\npatch2\nline11\nline12\n",
        )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 2);
    assert_eq!(branch.commits.len(), 0);

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "first commit to test.txt",
        Some(&"test.txt:1-6".parse::<BranchOwnershipClaims>().unwrap()),
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.commits.len(), 1);
    assert_eq!(branch.commits[0].files.len(), 1);
    assert_eq!(branch.commits[0].files[0].hunks.len(), 1);

    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "second commit to test.txt",
        Some(&"test.txt:16-22".parse::<BranchOwnershipClaims>().unwrap()),
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
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
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([
        (path::PathBuf::from("test.txt"), "file1\n"),
        (path::PathBuf::from("test2.txt"), "file2\n"),
    ]));

    let commit1_oid = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();
    let commit1 = project_repository
        .git_repository
        .find_commit(commit1_oid)
        .unwrap();

    set_test_target(&gb_repository, &project_repository)?;

    // remove file
    std::fs::remove_file(std::path::Path::new(&project.path).join("test2.txt"))?;
    // add new file
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\n",
    )?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "branch1 commit",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit2 = &branch1.commits[0].id;
    let commit2 = project_repository
        .git_repository
        .find_commit(*commit2)
        .expect("failed to get commit object");

    let tree = commit1.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&project_repository.git_repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

    // get the tree
    let tree = commit2.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&project_repository.git_repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

    Ok(())
}

#[test]
fn test_commit_add_and_delete_files() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([
        (path::PathBuf::from("test.txt"), "file1\n"),
        (path::PathBuf::from("test2.txt"), "file2\n"),
    ]));

    let commit1_oid = project_repository
        .git_repository
        .head()
        .unwrap()
        .target()
        .unwrap();
    let commit1 = project_repository
        .git_repository
        .find_commit(commit1_oid)
        .unwrap();

    set_test_target(&gb_repository, &project_repository)?;

    // remove file
    std::fs::remove_file(std::path::Path::new(&project.path).join("test2.txt"))?;
    // add new file
    let file_path3 = std::path::Path::new("test3.txt");
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path3),
        "file3\n",
    )?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "branch1 commit",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit2 = &branch1.commits[0].id;
    let commit2 = project_repository
        .git_repository
        .find_commit(*commit2)
        .expect("failed to get commit object");

    let tree = commit1.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&project_repository.git_repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

    // get the tree
    let tree = commit2.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(&project_repository.git_repository, &tree);
    assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

    Ok(())
}

#[test]
#[cfg(target_family = "unix")]
fn test_commit_executable_and_symlinks() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([
        (path::PathBuf::from("test.txt"), "file1\n"),
        (path::PathBuf::from("test2.txt"), "file2\n"),
    ]));

    set_test_target(&gb_repository, &project_repository)?;

    // add symlinked file
    let file_path3 = std::path::Path::new("test3.txt");
    let src = std::path::Path::new(&project.path).join("test2.txt");
    let dst = std::path::Path::new(&project.path).join(file_path3);
    symlink(src, dst)?;

    // add executable
    let file_path4 = std::path::Path::new("test4.bin");
    let exec = std::path::Path::new(&project.path).join(file_path4);
    std::fs::write(&exec, "exec\n")?;
    let permissions = fs::metadata(&exec)?.permissions();
    let new_permissions = Permissions::from_mode(permissions.mode() | 0o111); // Add execute permission
    fs::set_permissions(&exec, new_permissions)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    // commit
    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "branch1 commit",
        None,
        None,
        None,
        false,
    )?;

    let (branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

    let commit = &branch1.commits[0].id;
    let commit = project_repository
        .git_repository
        .find_commit(*commit)
        .expect("failed to get commit object");

    let tree = commit.tree().expect("failed to get tree");

    let list = tree_to_entry_list(&project_repository.git_repository, &tree);
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
    tree.walk(|_, entry| {
        let path = entry.name().unwrap();
        let entry = tree.get_path(std::path::Path::new(path)).unwrap();
        let object = entry.to_object(repository).unwrap();
        if object.kind() == Some(git2::ObjectType::Blob) {
            file_list.push(path.to_string());
        }
        git::TreeWalkResult::Continue
    })
    .expect("failed to walk tree");
    file_list
}

fn tree_to_entry_list(
    repository: &git::Repository,
    tree: &git::Tree,
) -> Vec<(String, String, String, String)> {
    let mut file_list = Vec::new();
    tree.walk(|_root, entry| {
        let path = entry.name().unwrap();
        let entry = tree.get_path(std::path::Path::new(path)).unwrap();
        let object = entry.to_object(repository).unwrap();
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
        git::TreeWalkResult::Continue
    })
    .expect("failed to walk tree");
    file_list
}

#[test]
fn test_verify_branch_commits_to_integration() -> Result<()> {
    let Case {
        project_repository,
        project,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    integration::verify_branch(&gb_repository, &project_repository).unwrap();

    //  write two commits
    let file_path2 = std::path::Path::new("test2.txt");
    std::fs::write(std::path::Path::new(&project.path).join(file_path2), "file")?;
    tests::commit_all(&project_repository.git_repository);
    std::fs::write(
        std::path::Path::new(&project.path).join(file_path2),
        "update",
    )?;
    tests::commit_all(&project_repository.git_repository);

    // verify puts commits onto the virtual branch
    integration::verify_branch(&gb_repository, &project_repository).unwrap();

    // one virtual branch with two commits was created
    let (virtual_branches, _, _) = list_virtual_branches(&gb_repository, &project_repository)?;
    assert_eq!(virtual_branches.len(), 1);

    let branch = &virtual_branches.first().unwrap();
    assert_eq!(branch.commits.len(), 2);
    assert_eq!(branch.commits.len(), 2);

    Ok(())
}

#[test]
fn test_verify_branch_not_integration() -> Result<()> {
    let Case {
        project_repository,
        gb_repository,
        ..
    } = Suite::default().new_case();

    set_test_target(&gb_repository, &project_repository)?;

    integration::verify_branch(&gb_repository, &project_repository).unwrap();

    project_repository
        .git_repository
        .set_head(&"refs/heads/master".parse().unwrap())?;

    let verify_result = integration::verify_branch(&gb_repository, &project_repository);
    assert!(verify_result.is_err());
    assert_eq!(
        verify_result.unwrap_err().to_string(),
        "head is refs/heads/master"
    );

    Ok(())
}

#[test]
fn test_pre_commit_hook_rejection() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = suite.new_case_with_files(HashMap::from([
        (
            path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\n",
        ),
        (
            path::PathBuf::from("test2.txt"),
            "line5\nline6\nline7\nline8\n",
        ),
    ]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
        std::path::Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let hook = b"#!/bin/sh
    echo 'rejected'
    exit 1
            ";

    git2_hooks::create_hook(
        (&project_repository.git_repository).into(),
        git2_hooks::HOOK_PRE_COMMIT,
        hook,
    );

    let res = commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
        Some(suite.keys.get_or_create()?).as_ref(),
        None,
        true,
    );

    let error = res.unwrap_err();

    assert!(matches!(error, CommitError::CommitHookRejected(_)));

    let CommitError::CommitHookRejected(output) = error else {
        unreachable!()
    };

    assert_eq!(&output, "rejected\n");

    Ok(())
}

#[test]
fn test_post_commit_hook() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = suite.new_case_with_files(HashMap::from([
        (
            path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\n",
        ),
        (
            path::PathBuf::from("test2.txt"),
            "line5\nline6\nline7\nline8\n",
        ),
    ]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
        std::path::Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let hook = b"#!/bin/sh
    touch hook_ran
            ";

    git2_hooks::create_hook(
        (&project_repository.git_repository).into(),
        git2_hooks::HOOK_POST_COMMIT,
        hook,
    );

    let hook_ran_proof = project_repository
        .git_repository
        .path()
        .parent()
        .unwrap()
        .join("hook_ran");

    assert!(!hook_ran_proof.exists());

    commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
        Some(suite.keys.get_or_create()?).as_ref(),
        None,
        true,
    )?;

    assert!(hook_ran_proof.exists());

    Ok(())
}

#[test]
fn test_commit_msg_hook_rejection() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        project_repository,
        ..
    } = suite.new_case_with_files(HashMap::from([
        (
            path::PathBuf::from("test.txt"),
            "line1\nline2\nline3\nline4\n",
        ),
        (
            path::PathBuf::from("test2.txt"),
            "line5\nline6\nline7\nline8\n",
        ),
    ]));

    set_test_target(&gb_repository, &project_repository)?;

    let branch1_id = create_virtual_branch(
        &gb_repository,
        &project_repository,
        &BranchCreateRequest::default(),
    )
    .expect("failed to create virtual branch")
    .id;

    std::fs::write(
        std::path::Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let hook = b"#!/bin/sh
    echo 'rejected'
    exit 1
            ";

    git2_hooks::create_hook(
        (&project_repository.git_repository).into(),
        git2_hooks::HOOK_COMMIT_MSG,
        hook,
    );

    let res = commit(
        &gb_repository,
        &project_repository,
        &branch1_id,
        "test commit",
        None,
        Some(suite.keys.get_or_create()?).as_ref(),
        None,
        true,
    );

    let error = res.unwrap_err();

    assert!(matches!(error, CommitError::CommitMsgHookRejected(_)));

    let CommitError::CommitMsgHookRejected(output) = error else {
        unreachable!()
    };

    assert_eq!(&output, "rejected\n");

    Ok(())
}
