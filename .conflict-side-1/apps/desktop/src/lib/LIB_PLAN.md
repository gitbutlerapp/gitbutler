# `src/lib/` Refactoring — Completed

> Reorganized `src/lib/` to eliminate import cycles between directories.
> **Result: 30 → 2 cycles** (remaining 2 are test-only: `ai <-> testing`, `forge <-> testing`).

## Changes Made

### Phase 1: Move domain-specific utils to feature modules

- `utils/branch.ts` → `branches/branchUtils.ts`
- `utils/commitMessage.ts` → `commits/commitMessage.ts`
- `utils/fileStatus.ts` → `files/fileStatus.ts`
- `utils/codegenTools.ts` → `codegen/codegenTools.ts`
- `utils/theme.ts` → `config/theme.ts`
- `conflictEntryPresence.ts` → `files/conflictEntryPresence.ts`
- `rxjs.ts` → `utils/rxjs.ts`
- `tabs.ts` → `utils/tabs.ts`

### Phase 2: Consolidate drop handlers into `dragging/dropHandlers/`

All feature-module drop handlers moved into `dragging/dropHandlers/`:

- `branches/dropHandler.ts` → `dragging/dropHandlers/branchDropHandler.ts`
- `commits/dropHandler.ts` → `dragging/dropHandlers/commitDropHandler.ts`
- `hunks/dropHandler.ts` → `dragging/dropHandlers/hunkDropHandler.ts`
- `stacks/dropHandler.ts` → `dragging/dropHandlers/stackDropHandler.ts`
- `codegen/dropzone.ts` → `dragging/dropHandlers/codegenDropzone.ts`

**Eliminated 10 cycles** involving dragging ↔ branches/commits/hunks/stacks/codegen.

### Phase 3: Untangle `state/` hub

- **`reduxError.ts`**: Moved from `state/` to `error/` (it's an error type, not state infrastructure).
- **`messageQueueSlice.ts`**: Moved from `codegen/` to `state/` (it's a Redux slice consumed by clientState).
- **`uiState.svelte.ts`**: Inlined shared type definitions (`ThinkingLevel`, `ModelType`, `PermissionMode`, `GeneralSettingsPageId`, `ProjectSettingsPageId`, `RejectionReason`) that were previously imported from feature modules. Feature modules now re-export from state.
- **Stale state updaters**: Moved `replaceBranchInExclusiveAction`, `replaceBranchInStackSelection`, `updateStaleStackState`, `updateStaleProjectState` from `state/uiState.svelte.ts` to `stacks/staleStateUpdaters.ts` (only caller is stackService).
- **`clientState.svelte.ts`**: Replaced concrete forge client type imports (`GitHubClient`, `GitLabClient`) with opaque structural types to break `state <-> forge` cycle.

**Eliminated 6 cycles** involving state ↔ codegen/stacks/settings/forge/error/backend.

### Phase 4: Merge small coupled modules

- `stores/tokenMemoryService.ts` → `user/tokenMemoryService.ts` (eliminated `stores/` directory)
- `dependencies/` merged into `hunks/` (eliminated `dependencies/` directory)
- `stack/stackController.svelte.ts` → `stacks/stackController.svelte.ts` (eliminated `stack/` directory)
- `editMode/editPatchUtils.ts` → `mode/editPatchUtils.ts` (eliminated `editMode/` directory)

### Phase 5: Fix remaining service-level cycles

- **`codegen <-> soup`**: Moved `codegenAnalytics.ts` from `soup/` to `codegen/`.
- **`error <-> notifications`**: Extracted `showError` from `notifications/toasts.ts` into `error/showError.ts`. Updated all 18 importers.
- **`backend <-> error`**: Made `logErrorToFile` injectable via `setLogErrorToFile()` in `error/logError.ts`, wired in `hooks.client.ts`.
- **`analytics <-> user`**: Replaced `User` type import in `analytics/sentry.ts` with structural type.
- **`branches <-> forge`**: Changed `forge/prContents.ts` to import `Workspace.Commit` from `@gitbutler/core/api` directly.
- **`forge <-> stacks`**: Changed `forge/shared/prFooter.ts` to import `Workspace.BranchDetails` from `@gitbutler/core/api` directly.
- **`forge <-> project`**: Inlined `ForgeName` type in `project/project.ts`.
- **`hunks <-> worktree`**: Used structural type for `WorktreeService` in `hunks/dependencyService.svelte.ts`.
- **`selection <-> stacks`**: Used structural `StackServiceLike` interface in `selection/fileSelectionManager.svelte.ts`.
- **`backend <-> utils`**: Moved `utils/url.ts` (URLService) to `backend/url.ts`.

## Validation

- **0 type errors** (`svelte-check`)
- **204/204 tests pass**
- **2 remaining cycles** (test-only, exempt by convention):
  - `ai <-> testing`
  - `forge <-> testing`

## Directory structure changes

Directories removed: `stores/`, `dependencies/`, `stack/`, `editMode/`
New subdirectory: `dragging/dropHandlers/`
New file: `error/showError.ts`, `stacks/staleStateUpdaters.ts`
Current count: 46 directories, 279 files
