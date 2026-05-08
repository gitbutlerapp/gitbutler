# Error Checklist — v0.19.10 toast:show_error

Based on PostHog `toast:show_error` events for `appVersion = 0.19.10`.
Total: 1436 events across 55 error types. Last refreshed: 2026-05-11.

With `PreconditionFailed` code on the backend, the frontend now shows
these as warnings instead of errors.

## Upstream Integration (524 events)

- [x] `integrate_upstream` — 461 events, 99 users
  - Majority were "Branches are all up to date" — backend fix in `b56a2785b1`, frontend shows as warning via `PreconditionFailed` code.
  - Some were permission errors (`could not open ... for writing: Permission denied`) — user-side filesystem issue.
  - Some were inter-stack tree conflicts ("conflicts with other applied stacks") — correct error behavior.
- [ ] `upstream_integration_statuses` — 34 events, 16 users
- [ ] `integrate_upstream_commits` — 29 events, 12 users

## Commit Operations (188 events)

- [ ] `commit_create` — 99 events, 28 users
- [ ] `commit_amend` — 45 events, 16 users
- [ ] `commit_uncommit` — 24 events, 12 users
- [ ] `commit_uncommit_changes` — 14 events, 5 users
- [ ] `commit_squash` — 3 events, 2 users
- [ ] `commit_move_changes_between` — 3 events, 2 users

## Branch Operations (296 events)

- [ ] `create_virtual_branch_from_branch` — 94 events, 41 users
- [ ] `create_virtual_branch` — 75 events, 29 users
- [ ] `delete_local_branch` — 55 events, 16 users
- [ ] `switch_back_to_workspace` — 37 events, 15 users
- [ ] `unapply_stack` — 32 events, 19 users
- [ ] `update_branch_name` — 22 events, 12 users
- [ ] `stash_into_branch` — 3 events, 2 users
- [ ] `create_branch` — 3 events, 2 users
- [ ] `remove_branch` — 2 events, 2 users
- [ ] `update_stack_order` — 2 events, 2 users
- [ ] `tear_off_branch` — 2 events, 2 users
- [ ] `commit_move` — 2 events, 2 users
- [ ] `create_reference` — 1 event, 1 user
- [ ] `move_branch` — 1 event, 1 user

## Git Operations (144 events)

- [ ] Git push failed — 74 events, 37 users
- [ ] Git hook failed — 68 events, 18 users
- [ ] Commit message hook failed — 2 events, 2 users

## Frontend / Client (61 events)

- [ ] TypeError — 50 events, 17 users
- [ ] Unhandled exception — 10 events, 3 users
- [ ] Error — 1 event, 1 user

## Network / Connectivity (57 events)

- [ ] Failed to fetch — 57 events, 38 users

## GitHub / Remote (30 events)

- [ ] GitHub API error: pulls/create — 20 events, 12 users
- [ ] GitHub API error: pulls/get — 4 events, 3 users
- [ ] `update_branch_pr_number` — 3 events, 3 users
- [ ] GitHub API error: pulls/update — 2 events, 2 users
- [ ] GitHub API error: pulls/merge — 1 event, 1 user

## Edit Mode (38 events)

- [ ] `save_edit_and_return_to_workspace` — 30 events, 7 users
- [ ] `enter_edit_mode` — 7 events, 6 users
- [ ] `abort_edit_and_return_to_workspace` — 1 event, 1 user

## Other (98 events)

- [ ] `set_project_active` — 11 events, 7 users
- [ ] `integrate_branch_with_steps` — 7 events, 5 users
- [ ] `set_base_branch` — 6 events, 2 users
- [ ] `stacks` — 6 events, 3 users
- [ ] `assign_hunk` — 5 events, 5 users
- [ ] `pr_template` — 5 events, 1 user
- [ ] `absorption_plan` — 5 events, 4 users
- [ ] `restore_snapshot` — 3 events, 1 user
- [ ] `login_with_token` — 3 events, 3 users
- [ ] `get_user_profile` — 3 events, 3 users
- [ ] File upload failed — 2 events, 2 users
- [ ] `discard_worktree_changes` — 2 events, 2 users
- [ ] `init_github_device_oauth` — 1 event, 1 user
- [ ] `get_logs_archive_path` — 1 event, 1 user
- [ ] `update_user_profile` — 1 event, 1 user
- [ ] `pre_commit_hook_diffspecs` — 1 event, 1 user
- [ ] Error occurred while logging in — 1 event, 1 user
