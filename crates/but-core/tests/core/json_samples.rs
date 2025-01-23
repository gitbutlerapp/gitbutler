//! Assure our JSON serialization doesn't break unknowingly - after all downstream may depend on it.
//!
use but_core::UnifiedDiff;

#[test]
fn worktree_changes_unified_diffs_json_example() -> anyhow::Result<()> {
    let repo = repo("many-in-worktree")?;
    let diffs: Vec<UnifiedDiff> = but_core::diff::worktree_status(&repo)?
        .changes
        .iter()
        .map(|tree_change| tree_change.unified_diff(&repo))
        .collect::<std::result::Result<_, _>>()?;
    let actual = serde_json::to_string_pretty(&diffs)?;
    insta::assert_snapshot!(actual, @r#"
    [
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+content\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+link-target\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+content not to add to the index\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+change-in-index\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+change-in-worktree\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+worktree-change\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      }
    ]
    "#);
    Ok(())
}

pub fn repo(name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("status-repo.sh")
        .map_err(anyhow::Error::from_boxed)?;
    Ok(gix::open_opts(
        root.join(name),
        gix::open::Options::isolated(),
    )?)
}
