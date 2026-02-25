use bstr::ByteSlice as _;
use but_core::{ChangeState, DiffSpec, HunkHeader, UnifiedPatch};
use but_testsupport::{CommandExt, git};
use but_workspace::tree_manipulation::{ChangesSource, create_tree_without_diff};

use crate::utils::writable_scenario;

const CONTEXT_LINES: u32 = 3;

#[test]
fn create_tree_without_diff_removes_single_hunk_modification_from_commit() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("plain-modifications");

    git(&repo).args(["add", "all-modified"]).run();
    git(&repo)
        .args(["commit", "-m", "modify all-modified"])
        .run();

    let head_id = repo.rev_parse_single("HEAD")?.detach();
    let head_commit = repo.find_commit(head_id)?;
    let parent_id = head_commit
        .parent_ids()
        .next()
        .expect("single-parent commit");
    let parent_tree_id = repo.find_commit(parent_id)?.tree_id()?.detach();

    let hunk_header = single_hunk_header_for_file_in_commit(&repo, head_id, "all-modified")?;

    let (tree_without_hunk_id, dropped) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: head_id },
        [DiffSpec {
            previous_path: None,
            path: "all-modified".into(),
            hunk_headers: vec![hunk_header],
        }],
        CONTEXT_LINES,
    )?;

    assert!(
        dropped.is_empty(),
        "hunk diff spec should not be rejected for a matching single-hunk modification"
    );
    assert_eq!(
        tree_without_hunk_id, parent_tree_id,
        "removing the only hunk in the commit restores the parent tree"
    );

    Ok(())
}

fn single_hunk_header_for_file_in_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    path: &str,
) -> anyhow::Result<HunkHeader> {
    let commit = repo.find_commit(commit_id)?;
    let parent_id = commit.parent_ids().next().expect("single-parent commit");
    let parent = repo.find_commit(parent_id)?;

    let before_tree = parent.tree()?;
    let after_tree = commit.tree()?;
    let before_entry = before_tree
        .lookup_entry(path.split('/'))?
        .expect("fixture has file in commit parent");
    let after_entry = after_tree
        .lookup_entry(path.split('/'))?
        .expect("fixture has file in commit");

    let diff = UnifiedPatch::compute(
        repo,
        path.as_bytes().as_bstr(),
        Some(path.as_bytes().as_bstr()),
        ChangeState {
            id: after_entry.id().detach(),
            kind: after_entry.mode().kind(),
        },
        ChangeState {
            id: before_entry.id().detach(),
            kind: before_entry.mode().kind(),
        },
        CONTEXT_LINES,
    )?
    .expect("all-modified is a text patch");

    let UnifiedPatch::Patch { hunks, .. } = diff else {
        unreachable!("all-modified yields a patch")
    };
    assert_eq!(hunks.len(), 1, "fixture yields a single hunk");
    Ok((&hunks[0]).into())
}
