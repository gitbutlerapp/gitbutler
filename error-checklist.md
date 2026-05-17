# Error Checklist — v0.19.10 & v0.19.12 toast:show_error

## v0.19.10 Summary
Based on PostHog `toast:show_error` events for `appVersion = 0.19.10`.
Total: 3891 events across 60 error types. Last refreshed: 2026-05-14.

With `PreconditionFailed` code on the backend, the frontend now shows
these as warnings instead of errors.

## v0.19.12 Summary
91 events, 37 users. Last refreshed: 2026-05-17.

---

## Upstream Integration

### v0.19.10 (1368 events, 35.2%)

- [x] `integrate_upstream` — 1264 events (32.5%), 210 users
  - Majority were "Branches are all up to date" — backend fix in `b56a2785b1`, frontend shows as warning via `PreconditionFailed` code.
  - Some were permission errors (`could not open ... for writing: Permission denied`) — user-side filesystem issue.
  - Some were inter-stack tree conflicts ("conflicts with other applied stacks") — correct error behavior.
- [ ] `integrate_upstream_commits` — 54 events (1.4%), 28 users
- [ ] `upstream_integration_statuses` — 50 events (1.3%), 23 users

### v0.19.12
- [ ] `integrate_upstream` — 10 events, 6 users
  - "Chosen resolutions do not match quantity of applied virtual branches" (5 events) — possible race/stale UI state
  - "The new head names do not match the current heads" (2 events)
  - "merge conflict when computing workspace tree" (1 event)
  - Stack conflicts with other applied stacks (2 events) — correct error behavior

---

## Branch Operations

### v0.19.10 (762 events, 19.6%)

- [ ] `create_virtual_branch_from_branch` — 275 events (7.1%), 101 users
- [ ] `create_virtual_branch` — 147 events (3.8%), 67 users
- [ ] `delete_local_branch` — 131 events (3.4%), 23 users
- [ ] `unapply_stack` — 107 events (2.7%), 44 users
- [ ] `switch_back_to_workspace` — 89 events (2.3%), 48 users
- [ ] `update_branch_name` — 46 events (1.2%), 21 users
- [ ] `discard_worktree_changes` — 43 events (1.1%), 16 users
- [ ] `tear_off_branch` — 14 events (0.4%), 7 users
- [ ] `stash_into_branch` — 12 events (0.3%), 6 users
- [ ] `commit_move` — 12 events (0.3%), 8 users
- [ ] `move_branch` — 10 events (0.3%), 6 users
- [ ] `create_branch` — 5 events (0.1%), 4 users
- [ ] `update_stack_order` — 5 events (0.1%), 5 users
- [ ] `remove_branch` — 3 events (0.1%), 3 users

### v0.19.12
- [ ] `create_virtual_branch` — 11 events, 4 users
  - "target commit already belongs to another branch" (10 events) — should be `PreconditionFailed`/warning
  - "database is locked" (1 event)
- [ ] `switch_back_to_workspace` — 9 events, 5 users
  - "Shared index checksum mismatch" (5 events, 1 user) — corrupted git index
  - `<project-conflict>` / workspace conflicts (4 events, 4 users) — correct error behavior
- [ ] `create_virtual_branch_from_branch` — 7 events, 6 users
  - "Worktree changes would be overwritten by checkout" (4 events) — should be warning
  - BUG errors in branch lookup (2 events)
- [ ] `unapply_stack` — 2 events, 2 users
  - `<verification-failed>` (1 event) — user on wrong branch
  - branch not found (1 event)
- [ ] `delete_local_branch` — 2 events, 2 users
  - "Refusing to delete a branch that is checked out" (2 events) — should be warning

---

## Commit Operations

### v0.19.10 (407 events, 10.5%)

- [ ] `commit_create` — 210 events (5.4%), 63 users
- [ ] `commit_amend` — 103 events (2.6%), 34 users
- [ ] `commit_uncommit` — 55 events (1.4%), 26 users
- [ ] `commit_uncommit_changes` — 14 events (0.4%), 5 users
- [ ] `commit_move_changes_between` — 13 events (0.3%), 7 users
- [ ] `commit_squash` — 12 events (0.3%), 7 users

### v0.19.12
- [ ] `commit_uncommit` — 2 events, 1 user — "failed to uncommit commits"
- [ ] `commit_create` — 2 events, 2 users — submodule error + failed commit signing
- [ ] `commit_squash` — 2 events, 2 users — commit became conflicted + commit not found in rebase editor

---

## Git Operations

### v0.19.10 (360 events, 9.2%)

- [ ] Git push failed — 227 events (5.8%), 83 users
- [ ] Git hook failed — 130 events (3.3%), 27 users
- [ ] Commit message hook failed — 3 events (0.1%), 3 users

### v0.19.12
- [ ] Git push failed — 14 events, 7 users
  - pre-push hook failures (5 events) — user-side hooks
  - auth/credential errors (4 events) — user-side
  - `git push` non-zero exit (5 events) — various

---

## Network / Connectivity

### v0.19.10 (180 events, 4.6%)

- [ ] Failed to fetch — 180 events (4.6%), 90 users

---

## Frontend / Client

### v0.19.10 (153 events, 3.9%)

- [ ] `git_get_global_config` — 178 events (4.6%), 1 user
- [x] TypeError (`i.type`) — 112 events (2.9%), 37 users
  - Root cause: unhandled promise rejection in `IntegrateUpstreamModal.svelte:127`
  - Fixed: added `.catch(console.error)` to `upstreamStatuses()` call
- [ ] Unhandled exception — 24 events (0.6%), 7 users
- [ ] Error — 17 events (0.4%), 4 users

### v0.19.12
- [ ] TypeError (`i.type`) — 8 events, 2 users — fix not yet picked up by these users

---

## GitHub / Remote

### v0.19.10 (70 events, 1.8%)

- [ ] GitHub API error: pulls/create — 45 events (1.2%), 27 users
- [ ] GitHub API error: pulls/get — 11 events (0.3%), 6 users
- [ ] `update_branch_pr_number` — 6 events (0.2%), 5 users
- [ ] File upload failed — 5 events (0.1%), 3 users
- [ ] GitHub API error: pulls/merge — 4 events (0.1%), 3 users
- [ ] GitHub API error: pulls/update — 4 events (0.1%), 4 users

---

## Edit Mode

### v0.19.10 (76 events, 2.0%)

- [ ] `save_edit_and_return_to_workspace` — 57 events (1.5%), 15 users
- [ ] `enter_edit_mode` — 19 events (0.5%), 13 users

---

## Other

### v0.19.10 (337 events, 8.7%)

- [ ] `set_project_active` — 49 events (1.3%), 16 users
- [ ] `irc_auto_leave` — 18 events (0.5%), 2 users — "Command not found"
- [ ] `irc_auto_join` — 18 events (0.5%), 2 users — "Command not found"
- [ ] `assign_hunk` — 15 events (0.4%), 13 users
- [ ] `integrate_branch_with_steps` — 13 events (0.3%), 9 users
- [ ] `stacks` — 13 events (0.3%), 8 users
- [ ] `get_user_profile` — 12 events (0.3%), 7 users
- [ ] `set_base_branch` — 11 events (0.3%), 4 users
- [ ] `login_with_token` — 10 events (0.3%), 7 users
- [ ] `absorption_plan` — 7 events (0.2%), 6 users
- [ ] `restore_snapshot` — 6 events (0.2%), 4 users
- [ ] `pr_template` — 5 events (0.1%), 1 user
- [ ] `target_commits` — 3 events (0.1%), 2 users
- [ ] `claude_update_permission_request` — 3 events (0.1%), 1 user
- [ ] Failed to open terminal — 3 events (0.1%), 3 users
- [ ] `push_stack` — 2 events (0.1%), 1 user
- [ ] `pre_commit_hook_diffspecs` — 2 events (0.1%), 2 users
- [ ] `create_workspace_rule` — 2 events (0.1%), 1 user
- [ ] `get_logs_archive_path` — 1 event (0.0%), 1 user
- [ ] Error occurred while logging in — 1 event (0.0%), 1 user
- [ ] Failed to delete project — 1 event (0.0%), 1 user

### v0.19.12
- [ ] `irc_auto_join` / `irc_auto_leave` / `irc_start_working_files_broadcast` — 8 events, 1 user — "Command not found"
- [ ] `set_project_active` — 7 events, 5 users
  - "OS file watch limit reached" (4 events, 1 user) — Linux `inotify` limit
  - "Shared index checksum mismatch" (3 events, 1 user) — corrupted git index
- [ ] `target_commits` — 3 events, 1 user — "database is locked"
- [ ] `update_stack_order` — 1 event — "Requires open workspace mode"
- [ ] `stacks` — 1 event — "Found 58 commit(s) on top of workspace commit" (git state corruption)
- [ ] `discard_worktree_changes` — 1 event — "Access is denied" (Windows, os error 5)
- [ ] `commit_message_hook_failed` — 1 event — user-side husky hook
- [ ] `Run cargo build -p gitbutler-git` — 1 event — missing askpass binary (dev build)
