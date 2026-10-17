use std::path::PathBuf;

use but_api::watcher::WatcherEvent;
use gitbutler_operating_modes::OperatingMode;
use gitbutler_watcher::Change;

fn worktree_changes() -> but_hunk_assignment::WorktreeChanges {
    but_hunk_assignment::WorktreeChanges {
        worktree_changes: but_core::ui::WorktreeChanges {
            changes: vec![],
            ignored_changes: vec![],
            modification_times: Default::default(),
        },
        assignments: vec![],
        assignments_error: None,
        dependencies: None,
        dependencies_error: None,
    }
}

#[test]
fn every_change_becomes_an_sdk_event() -> anyhow::Result<()> {
    let project_id: but_ctx::ProjectHandleOrLegacyProjectId =
        "00000000-0000-0000-0000-000000000001".parse()?;
    let changed_paths: std::sync::Arc<[PathBuf]> = vec![PathBuf::from("src/main.rs")].into();
    let changes = vec![
        Change::GitFetch(project_id.clone()),
        Change::GitHead {
            project_id: project_id.clone(),
            head: Some("refs/heads/main".into()),
            operating_mode: OperatingMode::OpenWorkspace,
        },
        Change::GitActivity {
            project_id: project_id.clone(),
            head_sha: "abc123".into(),
        },
        Change::WorkspaceActivity {
            project_id: project_id.clone(),
        },
        Change::ExternalInvalidation {
            project_id: project_id.clone(),
            tags: vec!["Workspace".into(), "NotATag".into()],
        },
        Change::WorktreeChanges {
            project_id: project_id.clone(),
            changes: worktree_changes(),
            changed_paths: changed_paths.clone(),
        },
        Change::LinkedWorktreeChanges {
            project_id,
            worktree: "feature".into(),
            changes: worktree_changes(),
            changed_paths,
        },
    ];
    let events: Vec<_> = changes.into_iter().map(WatcherEvent::from).collect();

    // Hosts deliver these as-is: changed paths survive and unknown tags are dropped.
    snapbox::assert_data_eq!(
        serde_json::to_string_pretty(&events)?,
        snapbox::str![[r#"
[
  {
    "name": "project://00000000-0000-0000-0000-000000000001/git/fetch",
    "payload": {
      "type": "gitFetch",
      "subject": null
    }
  },
  {
    "name": "project://00000000-0000-0000-0000-000000000001/git/head",
    "payload": {
      "type": "gitHead",
      "subject": {
        "head": "refs/heads/main",
        "operatingMode": {
          "type": "OpenWorkspace"
        }
      }
    }
  },
  {
    "name": "project://00000000-0000-0000-0000-000000000001/git/activity",
    "payload": {
      "type": "gitActivity",
      "subject": {
        "headSha": "abc123"
      }
    }
  },
  {
    "name": "project://00000000-0000-0000-0000-000000000001/workspace-activity",
    "payload": {
      "type": "workspaceActivity",
      "subject": null
    }
  },
  {
    "name": "project://00000000-0000-0000-0000-000000000001/external-invalidation",
    "payload": {
      "type": "externalInvalidation",
      "subject": {
        "tags": [
          "Workspace"
        ]
      }
    }
  },
  {
    "name": "project://00000000-0000-0000-0000-000000000001/worktree_changes",
    "payload": {
      "type": "worktreeChanges",
      "subject": {
        "changedPaths": [
          "src/main.rs"
        ],
        "changes": {
          "changes": [],
          "ignoredChanges": [],
          "modificationTimes": {},
          "assignments": [],
          "assignmentsError": null,
          "dependencies": null,
          "dependenciesError": null
        }
      }
    }
  },
  {
    "name": "project://00000000-0000-0000-0000-000000000001/linked_worktree_changes",
    "payload": {
      "type": "linkedWorktreeChanges",
      "subject": {
        "worktree": "feature",
        "changedPaths": [
          "src/main.rs"
        ],
        "changes": {
          "changes": [],
          "ignoredChanges": [],
          "modificationTimes": {},
          "assignments": [],
          "assignmentsError": null,
          "dependencies": null,
          "dependenciesError": null
        }
      }
    }
  }
]
"#]]
    );
    Ok(())
}
