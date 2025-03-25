use crate::discard::hunk::util::hunk_header;
use crate::utils::{
    CONTEXT_LINES, read_only_in_memory_scenario, to_change_specs_all_hunks, visualize_index,
    writable_scenario,
};
use but_testsupport::git_status;
use but_workspace::commit_engine::DiffSpec;
use but_workspace::discard_workspace_changes;

#[test]
fn non_modifications_trigger_error() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("deletion-addition-untracked")?;
    insta::assert_snapshot!(git_status(&repo)?, @r"
    A  added-to-index
     D to-be-deleted
    D  to-be-deleted-in-index
    ?? untracked
    ");

    let add_single_line = hunk_header("-1,0", "+1,1");
    let remove_single_line = hunk_header("-1,1", "+1,0");
    for (file_name, hunk) in [
        ("untracked", add_single_line),
        ("added-to-index", add_single_line),
        ("to-be-deleted", remove_single_line),
        ("to-be-deleted-in-index", remove_single_line),
    ] {
        let err = discard_workspace_changes(
            &repo,
            Some(
                DiffSpec {
                    previous_path: None,
                    path: file_name.into(),
                    hunk_headers: vec![hunk],
                }
                .into(),
            ),
            CONTEXT_LINES,
        )
        .unwrap_err();
        assert!(
            err.to_string().starts_with(
                "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead"
            ),
        );
    }
    Ok(())
}

#[test]
fn deletion_modification_addition_of_hunks_mixed_discard_all_in_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    // Note that one of these renames can't be detected by Git but is visible to us.
    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    M  file-in-index
    RM file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");

    // Show that we detect renames correctly, despite the rename + modification.
    let wt_changes = but_core::diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(wt_changes.changes, @r#"
    [
        TreeChange {
            path: "file",
            status: Modification {
                previous_state: ChangeState {
                    id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
                flags: None,
            },
        },
        TreeChange {
            path: "file-in-index",
            status: Modification {
                previous_state: ChangeState {
                    id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(cb89473a55c3443b5567e990e2a0293895c91a4a),
                    kind: Blob,
                },
                flags: None,
            },
        },
        TreeChange {
            path: "file-renamed",
            status: Rename {
                previous_path: "file-to-be-renamed",
                previous_state: ChangeState {
                    id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
                flags: None,
            },
        },
        TreeChange {
            path: "file-renamed-in-index",
            status: Rename {
                previous_path: "file-to-be-renamed-in-index",
                previous_state: ChangeState {
                    id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
                flags: None,
            },
        },
    ]
    "#);

    let specs = to_change_specs_all_hunks(&repo, wt_changes)?;
    let dropped =
        discard_workspace_changes(&repo, specs.into_iter().map(Into::into), CONTEXT_LINES)?;
    assert!(dropped.is_empty());

    // TODO: this most definitely isn't correct, figure out what's going on.
    // Notably, discarding all hunks leaves the renamed file in place, but without modifications.
    insta::assert_snapshot!(git_status(&repo)?, @r"
    MM file-in-index
    R  file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");
    // The index still only holds what was in the index before, but is representing the changed worktree.
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
    100644:3d3b36f file
    100644:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");

    // TODO: content checks.

    Ok(())
}

mod util {
    use but_workspace::commit_engine::HunkHeader;

    /// Choose a slightly more obvious, yet easy to type syntax than a function with 4 parameters.
    pub fn hunk_header(old: &str, new: &str) -> HunkHeader {
        fn parse_header(hunk_info: &str) -> (u32, u32) {
            let hunk_info = hunk_info.trim_start_matches(['-', '+'].as_slice());
            let parts: Vec<_> = hunk_info.split(',').collect();
            let start = parts[0].parse().unwrap();
            let lines = if parts.len() > 1 {
                parts[1].parse().unwrap()
            } else {
                1
            };
            (start, lines)
        }

        let (old_start, old_lines) = parse_header(old);
        let (new_start, new_lines) = parse_header(new);
        HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        }
    }
}
