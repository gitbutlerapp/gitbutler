use super::*;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::{
    create_commit, create_virtual_branch, list_virtual_branches, set_base_branch,
};

// This test ensures hunk lock detection works when a lines are shifted.
//
// The idea is you introduce change A, then you shift it down in a a different
// (default) branch with commit B, so that new line numbers no longer intersect
// with those applied to the isolated branch.
#[tokio::test]
async fn hunk_locking_confused_by_line_number_shift() -> anyhow::Result<()> {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();
    let mut lines = repository.gen_file("file.txt", 9);
    repository.commit_all("initial commit");
    repository.push();

    set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap()).unwrap();

    // Introduce a change that should lock the last change to the first branch.
    lines[8] = "modification 1 to line 8".to_string();
    repository.write_file("file.txt", &lines);

    // We're forced to call this before making a second commit.
    let (branches, _) = list_virtual_branches(project).unwrap();
    create_commit(project, branches[0].id, "first commit", None, false)?;

    let (branches, _) = list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[0].commits.len(), 1);

    // Commit some changes to the second branch that will push the first
    // changes down when diffing workspace head against the default target.
    create_virtual_branch(
        project,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    let new_lines: Vec<_> = (0_i32..5_i32)
        .map(|i| format!("inserted line {}", i))
        .collect();
    lines.splice(0..0, new_lines);
    repository.write_file("file.txt", &lines);

    // We're forced to call this before making a second commit.
    let (branches, _) = list_virtual_branches(project).unwrap();
    create_commit(project, branches[1].id, "second commit", None, false)?;

    // At this point we expect no uncommitted files, and one commit per branch.
    let (branches, _) = list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].commits.len(), 1);
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[1].commits.len(), 1);
    assert_eq!(branches[1].files.len(), 0);

    // Now we change line we already changed in the first commit.
    lines[13] = "modification 2 to original line 8".to_string();
    repository.write_file("file.txt", &lines);

    // And ensure that the new change is assigned to the first branch, despite the second
    // branch being default for new changes.
    let (branches, _) = list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].files.len(), 1);

    // For good measure, let's ensure the hunk lock points to the right branch.
    let branch = &branches[0];
    let file = &branch.files[0];
    let hunk_locks = file.hunks[0].locked_to.clone().unwrap();
    assert_eq!(hunk_locks[0].branch_id, branch.id);
    Ok(())
}

// This test ensures hunk lock detection works when a lines are shifted.
//
// The idea is you introduce change A, then you shift it down in a a different
// (default) branch with commit B, so that new line numbers no longer intersect
// with those applied to the isolated branch.
#[tokio::test]
async fn hunk_locking_with_deleted_lines_only() -> anyhow::Result<()> {
    let Test {
        repository,
        project,
        ..
    } = &Test::default();
    repository.gen_file("file.txt", 3);
    repository.commit_all("initial commit");
    repository.push();

    set_base_branch(project, &"refs/remotes/origin/master".parse().unwrap()).unwrap();

    // Introduce a change that should lock the last change to the first branch.
    let mut lines = repository.gen_file("file.txt", 2);
    lines[1] = "line 2".to_string();
    repository.write_file("file.txt", &lines);

    // We have to do this before creating a commit.
    let (branches, _) = list_virtual_branches(project).unwrap();
    create_commit(project, branches[0].id, "first commit", None, false)?;

    let (branches, _) = list_virtual_branches(project).unwrap();
    assert_eq!(branches[0].commits.len(), 1);
    assert_eq!(branches[0].files.len(), 0);

    // Commit some changes to the second branch that will push the first changes
    // down when diffing workspace head against the default target.
    create_virtual_branch(
        project,
        &BranchCreateRequest {
            selected_for_changes: Some(true),
            ..Default::default()
        },
    )
    .unwrap();

    lines[1] = "modified line 2".to_string();
    repository.write_file("file.txt", &lines);

    let (branches, _) = list_virtual_branches(project)?;
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[1].files.len(), 0);
    Ok(())
}
