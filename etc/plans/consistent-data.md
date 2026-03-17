Consistent data is desirable for the same reason databases and Rust are desirable.
GitButler has various sources of data that should stay consistent and fit together.

# Data sources

GitButler has the following data sources, with a data source being defined as data that it reads and writes.

## Data sources per project

- Git repository via `gix::Repository` in the form of worktrees, git config, the object database and the reference database
  - The source of truth for most of what the user sees
- metadata at `.git/gitbutler/but.sqlite` via `but_db::DbHandle`
  - While relevant, it's only enriching the data stored in Git, and Git without any metadata is always a valid state.
- and still: **`.git/gitbutler/virtual-branches.toml`** via `gitbutler_stack::VirtualBranchesHandle`
  - _deprecated_, but the source of truth for the GitButler workspace (singular, there can only be one while this file exists)

## Data sources per application (dev, nightly, stable)

- `<app-support>/settings.json` for various settings
  - editable by the user, and watched for changes for live reloads
- cache via `but_db::AppCache` in `<app-support>/app-cache.sqlite`
  - entirely optional and there to speed up certain queries or avoid doing too many of them.
- recent projects and per-project information in `<app-support>/projects.json`
  - not editable by the user
  - _deprecated_ and to be moved into `but.sqlite` for per-project information, and `but_db::AppCache` for recently opened projects.
    - _deprecated_ `ProjectId` should be replaced by `but_ctx::ProjectHandle`
- `<app-support>/forge_settings.json`
  - Settings not to be edited by the user
  - _deprecated_ (?) and to be moved into `but_db::AppDb` - semantically this should never prevent the application from starting up.

# Caching data sources (performance)

Opening data sources for reading and writing isn't free, and interacting with Git objects isn't either, as decoding them is slow. This is why we should avoid duplicate work by:

- opening databases only when needed
- caching Git objects where it makes sense
- avoiding unnecessary worktree status checks

The `but_ctx::Context` is the solution to this problem, and is passed around as `&but_ctx::Context` and `&mut but_ctx::Context` to serve as storage for all information needed to serve an API request.

# Tasks

## Caching

- [x] `but_ctx::Context` is used everywhere via [#12064](https://github.com/gitbutlerapp/gitbutler/pull/12064)
  - [x] `but_db::DbHandle` via `db`
  - [x] `gix::Repository` via `repo`
  - [x] `git2::Repository` (semi-deprecated) via `git2_repo`
  - [x] `but_graph::projection::Workspace` via `ws`
  - [ ] `git_status` as one-time result of `but_core::diff::worktree_changes()`
    - [ ] allow initialization from in-memory cache _lazily_ kept up-to-date by something hooked into the filesystem watcher

## Consistency

### 1. Sync `.git/gitbutler/virtual-branches.toml` with database representation

- [x] https://github.com/gitbutlerapp/gitbutler/issues/12075

### 2. Port legacy virtual-branches consumers to `ctx.ws`

Track all remaining access paths to `.git/gitbutler/virtual-branches.toml`, including:

- `gitbutler_stack::VirtualBranchesHandle`
- `ctx.virtual_branches()`
- `but_meta::legacy_storage::read_synced_virtual_branches()`
- `but_meta::legacy_storage::write_virtual_branches_and_sync()`

The crate list below started from direct `VirtualBranchesHandle` references and should also cover
callers that only touch the same legacy state via `ctx.virtual_branches()` or the legacy
read/write helpers.

#### but-api (2)

`legacy_meta()` and `legacy_meta_mut()`, needed for stacks/details V3 and vb.toml reconciliation.

- [ ] `crates/but-api/src/legacy/workspace.rs`
- [ ] `crates/but-api/src/legacy/meta.rs`

#### but-claude (1)

- [ ] `crates/but-claude/src/session.rs`

#### but-tools (1)

- [ ] `crates/but-tools/src/workspace.rs`

#### but (10)

- [ ] `crates/but/src/command/legacy/branch/list.rs`
- [ ] `crates/but/src/command/legacy/branch/show.rs`
- [ ] `crates/but/src/command/legacy/mcp_internal/commit.rs`
- [ ] `crates/but/src/command/legacy/mcp_internal/stack.rs`
- [ ] `crates/but/src/command/legacy/pick.rs`
- [ ] `crates/but/src/command/legacy/push.rs`
- [ ] `crates/but/src/command/legacy/rub/commits.rs`
- [ ] `crates/but/src/command/legacy/rub/move.rs`
- [ ] `crates/but/src/command/legacy/rub/move_commit.rs`
- [ ] `crates/but/src/command/legacy/status/mod.rs`

#### but-workspace (7)

- [ ] `crates/but-workspace/src/legacy/commit_engine/mod.rs`
- [ ] `crates/but-workspace/src/legacy/head.rs`
- [ ] `crates/but-workspace/src/legacy/mod.rs`
- [ ] `crates/but-workspace/src/legacy/tree_manipulation/move_between_commits.rs`
- [ ] `crates/but-workspace/src/legacy/tree_manipulation/remove_changes_from_commit_in_stack.rs`
- [ ] `crates/but-workspace/src/legacy/tree_manipulation/split_branch.rs`
- [ ] `crates/but-workspace/src/legacy/tree_manipulation/split_commit.rs`

#### but-action (2)

- [ ] `crates/but-action/src/lib.rs`
- [ ] `crates/but-action/src/simple.rs`

#### but-cherry-apply (2)

- [ ] `crates/but-cherry-apply/src/lib.rs`
- [ ] `crates/but-cherry-apply/tests/cherry_apply/main.rs`

#### but-worktrees (2)

- [ ] `crates/but-worktrees/src/integrate.rs`
- [ ] `crates/but-worktrees/tests/worktree/main.rs`

#### Reconciliation for workspace-commit refresh

`VirtualBranchesHandle` is only a concurrency-safe handle to `virtual_branches.toml`, not an
in-memory snapshot of workspace state. The same applies to `ctx.virtual_branches()`, which merely
creates another handle to the same file. Passing a `VBH` around explicitly should therefore not be
treated as its own consistency boundary.

The special case in this migration is `gitbutler-branch-actions::update_workspace_commit()`.
Today it still reads applied stacks from `VirtualBranchesHandle` and delegates the actual merge
shape to `but_workspace::legacy::remerged_workspace_commit_v2()`. That means the relevant VBH
work is not "port this function to `ctx.ws` line-by-line", but rather:

- [ ] make `update_workspace_commit()` build the workspace commit via
      `WorkspaceCommit::from_new_merge_with_tips()`
- [ ] treat `crates/gitbutler-branch-actions/src/integration.rs` and
      `crates/but-workspace/src/legacy/head.rs` as one migration cluster
- [ ] only after that, remove leftover explicit `VirtualBranchesHandle` threading that existed
      solely to support workspace-commit refresh

Until that refactor lands, the `gitbutler-branch-actions` items below should be read as
"remaining legacy virtual-branches consumers", including `ctx.virtual_branches()` call sites,
not as independent stateful-handle flows.

#### gitbutler-cli (1)

- [ ] `crates/gitbutler-cli/src/command/vbranch.rs`

#### gitbutler-operating-modes (1)

- [ ] `crates/gitbutler-operating-modes/src/lib.rs`

#### gitbutler-testsupport (1)

- [ ] `crates/gitbutler-testsupport/src/lib.rs`

#### gitbutler-edit-mode (2)

- [x] `crates/gitbutler-edit-mode/src/lib.rs`
- [x] `crates/gitbutler-edit-mode/tests/edit_mode.rs`

#### gitbutler-oplog (2)

- [ ] `crates/gitbutler-oplog/src/oplog.rs`
- [ ] `crates/gitbutler-oplog/src/reflog.rs`

#### gitbutler-workspace (2)

- [ ] `crates/gitbutler-workspace/src/branch_trees.rs`
- [ ] `crates/gitbutler-workspace/src/lib.rs`

#### gitbutler-stack (5)

- [ ] `crates/gitbutler-stack/src/lib.rs`
- [ ] `crates/gitbutler-stack/src/stack.rs`
- [ ] `crates/gitbutler-stack/src/stack_branch.rs`
- [ ] `crates/gitbutler-stack/src/state.rs`
- [ ] `crates/gitbutler-stack/tests/mod.rs`

#### gitbutler-branch-actions (11)

- [ ] `crates/gitbutler-branch-actions/src/base.rs`
- [ ] `crates/gitbutler-branch-actions/src/branch_upstream_integration.rs`
- [ ] `crates/gitbutler-branch-actions/src/integration.rs`
- [ ] `crates/gitbutler-branch-actions/src/lib.rs`
- [ ] `crates/gitbutler-branch-actions/src/move_branch.rs`
- [ ] `crates/gitbutler-branch-actions/src/move_commits.rs`
- [ ] `crates/gitbutler-branch-actions/src/upstream_integration.rs`
- [ ] `crates/gitbutler-branch-actions/tests/reorder.rs`
- [ ] `crates/gitbutler-branch-actions/tests/squash.rs`
- [ ] `crates/gitbutler-branch-actions/tests/virtual_branches/oplog.rs`
- [ ] `crates/gitbutler-branch-actions/tests/virtual_branches/workspace_migration.rs`

### 3. Make `db.meta()` and remove `ctx.meta()`

Also write the database after _each change_ so it's semantically similar to how all other metadata is mutated. It's notable that this makes it compatible to be used with `but_db::Transaction` as well.

Sync the TOML file _on drop_ only, knowing well that this may write data that is going to be rolled back. The TOML sync is only for backward compatibility with older application versions.

- [ ] First migrate every remaining `ctx.legacy_meta()` caller to `ctx.meta()`
  - Current baseline:
    - `crates/but-action/src/lib.rs`
    - `crates/but-action/src/reword.rs`
    - `crates/but-cherry-apply/tests/cherry_apply/main.rs`
    - `crates/but-claude/src/hooks/mod.rs`
    - `crates/but-cursor/src/lib.rs`
    - `crates/but-testing/src/command/mod.rs`
    - `crates/but-tools/src/workspace.rs`
    - `crates/but-workspace/src/legacy/mod.rs`
    - `crates/but-worktrees/tests/worktree/main.rs`
    - `crates/but/src/command/legacy/mcp_internal/stack.rs`
    - `crates/but/src/legacy/commits.rs`
    - `crates/gitbutler-branch-actions/src/upstream_integration.rs`
    - `crates/gitbutler-branch-actions/tests/branch-actions/virtual_branches/mod.rs`
    - `crates/gitbutler-cli/src/command/vbranch.rs`
    - `crates/gitbutler-testsupport/src/lib.rs`
- [ ] Not started

### 4. Modernize workspace metadata schema

Migrate the existing workspace metadata to metadata that fits its purpose.
Remove `but-meta` in favor of a `but-db` implementation of the `RefMetadata` trait which is as minimal as it can be based on the data it actually reads and writes.

- [ ] Not started

## `ProjectId` to `ProjectHandle`

A transition from `ProjectId` to `ProjectHandle` to leverage the usability of `ProjectHandle` as `ProjectId`.
This means that all places that previously used `ProjectId` may now support `ProjectHandle` transparently
_as long_ as they are run through the `but-api` or use `ProjectHandleOrLegacyProjectId` explicitly.

- [x] implement `ProjectHandleOrLegacyProjectId`
- [x] Replace all manual mentions of `ProjectId` with `ProjectHandleOrLegacyProjectId`
- [x] Let `add_project` return a `ProjectHandle`

## DB for application data

### 5. Migrate application-wide metadata into `but-db`

Migrate app-support JSON metadata into SQLite-backed application data and remove remaining direct `*.json` reads/writes, all but `<app-support>/settings.json`.

- [ ] `<app-support/projects.json>` into `but_db::DbHandle`
  - [ ] recent projects as identified by `but_ctx::ProjectHandle` to `but_db::AppCache`
  - [ ] `but_ctx::LegacyProject` is removed
  - [ ] Replace `ProjectHandleOrLegacyProjectId` with `ProjectHandle` and remove the type.
- [ ] `<app-support>/forge_settings.json` to `but_db::AppDb` (`but_ctx::AppDb` also is guaranteed to be available just like a cache)
- [ ] `<app-general>/settings.json` should rather be per application-channel (nightly, stable, dev), i.e. in `<app-support>/settings.json`
