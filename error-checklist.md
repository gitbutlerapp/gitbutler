# Error Checklist — v0.19.10 & v0.19.12 toast:show_error

## v0.19.10 Summary

Based on PostHog `toast:show_error` events for `appVersion = 0.19.10`.
Total: 3891 events across 60 error types. Last refreshed: 2026-05-14.

With `PreconditionFailed` code on the backend, the frontend now shows
these as warnings instead of errors.

## v0.19.12 Summary

704 events, 205 users. Last refreshed: 2026-05-19.

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

- [ ] `integrate_upstream` — 16 events, ~9 users
  - "The new head names do not match the current heads" (8 events) — likely stale state
  - "Chosen resolutions do not match quantity of applied virtual branches" (4 events) — race/stale UI
  - "merge conflict when computing workspace tree" (2 events) — correct error
  - Stack conflicts with other applied stacks (2 events) — correct error behavior
- [ ] `upstream_integration_statuses` — 16 events, ~3 users
  - "The rebase failed as a merge could not be repeated without conflicts" (14 events, 1 user) — probably correct error
  - "failed to get target reference name" (2 events)

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

- [ ] `switch_back_to_workspace` — 40 events, ~17 users
  - `<project-conflict>` / workspace conflicts (21 events, 14 users) — correct error behavior
  - "failed to checkout tree" / rmdir error (12 events, 1 user) — Windows OneDrive path issue
  - "Shared index checksum mismatch" (5 events, 1 user) — corrupted git index
  - Windows IO error on null device (2 events)
- [ ] `create_virtual_branch_from_branch` — 34 events, ~12 users
  - "Worktree changes would be overwritten by checkout" (13 events) — should be warning
  - "Couldn't find merge-base between segments" / disjoint commit graph (8 events)
  - "Cannot add target branch to its own workspace" (4 events)
  - `<verification-failed>` / workspace commit not found (3 events)
- [ ] `create_virtual_branch` — 25 events, ~9 users
  - "target commit already belongs to another branch" (23 events) — should be `PreconditionFailed`/warning
  - "database is locked" (1 event)
- [ ] `delete_local_branch` — 21 events, 2 users
  - "Refusing to delete a branch that is checked out" (21 events) — should be warning
- [ ] `unapply_stack` — 20 events, ~5 users
  - "branch with ID … not found" (14 events) — stale branch reference
  - IO error / process using file (6 events, Windows)
- [ ] `tear_off_branch` — 2 events, 1 user — "Worktree changes would be overwritten" — should be warning
- [ ] `remove_branch` — 2 events, 1 user — "Refusing to delete a checked-out branch" — should be warning

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

- [ ] `commit_create` — 26 events, ~5 users
  - Windows rmdir error on OneDrive path (17 events, 1 user) — Windows/OneDrive filesystem issue
  - "Worktree changes would be overwritten" (3 events) — should be warning
  - "Failed to sign commit" (3 events, 3 users) — user-side GPG config
  - "index is locked" (3 events) — concurrent process
- [ ] `commit_uncommit` — 7 events, 3 users — "Cannot discard" — likely correct error
- [ ] `commit_move_changes_between` — 3 events, 1 user — "Destination commit must not be conflicted"
- [ ] `commit_squash` — 2 events, 2 users — commit became conflicted + commit not found in rebase editor
- [ ] `commit_move` — 2 events, 1 user — cherry-pick merge base failure

---

## Git Operations

### v0.19.10 (360 events, 9.2%)

- [ ] Git push failed — 227 events (5.8%), 83 users
- [ ] Git hook failed — 130 events (3.3%), 27 users
- [ ] Commit message hook failed — 3 events (0.1%), 3 users

### v0.19.12

- [ ] Git push failed — 20 events, ~8 users
  - auth/credential errors (10 events) — user-side
  - `git push`/`fetch` non-zero exit (6 events) — various
  - failed to create askpass server / timeout (2 events)
  - `<verification-failed>` / workspace commit not found (2 events)
- [ ] `pre_commit_hook_diffspecs` — 6 events, 2 users — program not found (3e) + index locked (3e)

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

- [ ] TypeError (`i.type`) — 44 events, 12 users — root cause still unclear; Rust fix shipped but not fully resolving it
- [ ] TypeError (network) — 4 events, 2 users — `Load failed` (Ollama + app.gitbutler.com) — unrelated network errors surfacing as TypeError

---

## GitHub / Remote

### v0.19.10 (70 events, 1.8%)

- [ ] GitHub API error: pulls/create — 45 events (1.2%), 27 users
- [ ] GitHub API error: pulls/get — 11 events (0.3%), 6 users
- [ ] `update_branch_pr_number` — 6 events (0.2%), 5 users
- [ ] File upload failed — 5 events (0.1%), 3 users
- [ ] GitHub API error: pulls/merge — 4 events (0.1%), 3 users
- [ ] GitHub API error: pulls/update — 4 events (0.1%), 4 users

### v0.19.12

- [ ] `GitHub API error: pulls/create` — 5 events, 2 users — rate limit + org auth error

---

## Edit Mode

### v0.19.10 (76 events, 2.0%)

- [ ] `save_edit_and_return_to_workspace` — 57 events (1.5%), 15 users
- [ ] `enter_edit_mode` — 19 events (0.5%), 13 users

### v0.19.12

(moved to Other section)

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

- [ ] `set_project_active` — 7 events, ~3 users
  - "Permission denied" starting watcher (4 events) — Linux permissions issue
  - "Shared index checksum mismatch" (3 events, 1 user) — corrupted git index
- [ ] `irc_auto_join` / `irc_auto_leave` / `irc_start_working_files_broadcast` — 8 events, 1 user — "Command not found" (frontend calling unimplemented backend commands)
- [ ] `save_edit_and_return_to_workspace` — 5 events, ~5 users
  - "index is locked" (3 events) — concurrent process
  - "Edit mode may only be left while in edit mode" (2 events) — should be warning
- [ ] `enter_edit_mode` — 2 events, 2 users — "may only be done when workspace is open" — should be warning
- [ ] `target_commits` — 4 events, 2 users — "database is locked"
- [ ] `assign_hunk` — 3 events, 3 users — "database is locked"
- [ ] `discard_worktree_changes` — 3 events, 1 user — "Operation not permitted" (os error 1)
- [ ] `git_get_global_config` — 2 events, 1 user — "requires a repository"
- [ ] `update_branch_pr_number` — 2 events, 1 user — `<verification-failed>`
- [ ] `update_stack_order` — 1 event — "Requires open workspace mode"
