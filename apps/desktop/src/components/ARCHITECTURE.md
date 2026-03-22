# Svelte Component Architecture

> This document describes the preferred component architecture for complex UI in `apps/desktop`. It supplements the patterns in `.github/copilot-instructions.md`.

## Folder Organization

Components are organized into three tiers based on abstraction level:

### Tier 1 — Shared utilities (`shared/`)

Generic, reusable components with no domain-specific imports. Anything in `shared/` must only import from other `shared/` components or external packages. Think of these as the app's internal component library.

**ESLint enforces:** max 2 non-shared domain imports (i.e., shared components should not depend on domain folders).

### Tier 2 — Domain components (`branch/`, `commit/`, `diff/`, `files/`, `forge/`, etc.)

Components that own one concept within a domain. A domain component imports from `shared/`, `$lib/`, and its own domain folder. Cross-domain imports are a signal the component may be in the wrong folder.

**ESLint warns:** when a component imports from more than 2 distinct `$components/` domains (excluding `shared/`).

### Tier 3 — Composition / view layer (`views/`)

Components that compose multiple domain components into complete UI surfaces. These are the roots of the component tree for each page or major panel. Cross-domain imports are expected here.

Includes app shell (`Chrome`, `ChromeHeader`, `ChromeSidebar`), page-level views (`WorkspaceView`, `BranchesView`), and panel composites (`StackView`, `BranchView`, `StackDetails`).

**ESLint:** cross-domain rule disabled for this folder.

---

## The Problem

Complex Svelte components tend toward one of two failure modes:

- **Monolith**: a single component with many boolean props (`showConflicts`, `showCheckboxes`, `allowUnselect`, …) that internally branches into large, hard-to-reason-about trees.
- **Prop explosion**: shared state distributed across unrelated props and callbacks, with no clear ownership.

## The Pattern: Compound Components

Split any complex component into three layers:

### 1. Controller (`.svelte.ts`)

A reactive class that owns shared state. No rendering. No callbacks. Exposes focused primitives with meaningful return values.

```ts
export class FileListController {
  readonly active = $state(false)
  readonly selectedPaths = $derived(...)

  select(e: MouseEvent, change: TreeChange, index: number) { ... }
  handleActivation(change: TreeChange, idx: number, e: KeyboardEvent) { ... }
  handleNavigation(e: KeyboardEvent) { ... }
}

const FILE_LIST_KEY = Symbol('FileList')
export function setFileListContext(c: FileListController) { setContext(FILE_LIST_KEY, c) }
export function getFileListContext(): FileListController { return getContext(FILE_LIST_KEY) }
```

Key rules:

- Uses `inject()` for dependency injection and `$effect()` for reactive side effects.
- Reactive constructor params are wrapped in closures (`changes: () => changes`) so the class reads current values on access rather than capturing a snapshot.
- State is accessed via getters; mutations happen via methods.

### 2. Provider (`.svelte`)

A thin wrapper that instantiates the controller, sets context, and renders `{@render children()}`. Its only job is creating shared state and making it available to the subtree.

```svelte
<script lang="ts">
	const { changes, selectionId, children } = $props();
	const controller = new FileListController({
		changes: () => changes,
		selectionId: () => selectionId,
	});
	setFileListContext(controller);
</script>

{@render children()}
```

Props are limited to shared state (data + identity). No callbacks, no rendering concerns.

**When to skip the Provider**: if the compound component has only one consumer, the consumer itself can act as both provider and controller host (like `StackView` does). A separate Provider is warranted when multiple independent consumers each need their own controller instance.

### 3. Compound children (`.svelte`)

Individual components that read the controller from context and own their specific rendering and interaction concerns.

```svelte
<script lang="ts">
	const { projectId, onselect, extraKeyHandlers } = $props();
	const controller = getFileListContext();
</script>
```

- Callbacks (`onselect`, `onFileClick`) and extension points (`extraKeyHandlers`) live **on the child that triggers them**, not in the controller or provider.
- Props specific to a rendering concern go here (`projectId`, `showCheckboxes`, `draggable`), not on the provider.
- No boolean props controlling component trees. Render `<FileListConflicts>` or don't — don't pass `showConflicts={true}` to a monolith.

## Key Principles

| Principle                                                           | Description                                                                                                                                                         |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Controllers expose primitives, components compose behavior          | A controller exposes `select()`, `handleActivation()`, `handleNavigation()`. The component's keydown handler composes them in sequence — extra handlers in between. |
| Callbacks belong on the child that triggers them                    | The component that fires `onselect` receives the prop. The controller and provider know nothing about it.                                                           |
| Props that must be reactive across the tree go through the Provider | `changes`, `selectionId`. Props specific to a rendering concern go directly on the child.                                                                           |
| Getter functions for reactive class params                          | Wrap reactive values in closures so the class reads current values on access.                                                                                       |
| No boolean tree-control props                                       | Prefer structural composition — render or don't render a child component.                                                                                           |

## Reference Implementations

### FileList — multi-consumer compound component

Used by four independent consumers; requires a separate Provider.

| File                                             | Role                                                                        |
| ------------------------------------------------ | --------------------------------------------------------------------------- |
| `src/lib/selection/fileListController.svelte.ts` | Controller: selection state, keyboard/mouse handling                        |
| `src/components/files/FileListProvider.svelte`   | Provider: instantiates controller, sets context                             |
| `src/components/files/FileListItems.svelte`      | Rendering + keyboard/mouse interaction; owns `onselect`, `extraKeyHandlers` |
| `src/components/files/FileListConflicts.svelte`  | Optional conflict display; reads context for `changes` and `active`         |

Consumer examples showing different compositions:

```svelte
<!-- IrcCommit.svelte — minimal: just items -->
<FileListProvider {changes} {selectionId}>
	<FileListItems {projectId} mode="list" onselect={(_, i) => (selectedIndex = i)} />
</FileListProvider>

<!-- NestedChangedFiles.svelte — with optional conflict display -->
<FileListProvider {changes} {selectionId} {allowUnselect}>
	<FileListConflicts {projectId} {conflictEntries} {ancestorMostConflictedCommitId} />
	<FileListItems {projectId} mode={listMode} {conflictEntries} {onselect} />
</FileListProvider>

<!-- WorktreeChanges.svelte — with extra key handlers -->
<FileListProvider {changes} {selectionId}>
	<FileListItems
		{projectId}
		mode={listMode}
		showCheckboxes={isCommitting}
		extraKeyHandlers={aiKeyHandlers}
		onselect={onFileClick}
	/>
</FileListProvider>
```

### StackView — singleton compound component

Only one consumer; `StackView.svelte` acts as its own provider.

| File                                       | Role                                                                     |
| ------------------------------------------ | ------------------------------------------------------------------------ |
| `src/lib/stack/stackController.svelte.ts`  | Controller: selection state, diff view registration, focus management    |
| `src/components/views/StackView.svelte`    | Composition root: creates controller, sets context, manages panel layout |
| `src/components/views/StackPanel.svelte`   | Left panel: worktree changes, commit flow, branch list                   |
| `src/components/views/StackDetails.svelte` | Right panel: commit/branch/file detail views                             |

## Decision Guide

**Use a separate Provider when:**

- Multiple independent consumers each need their own controller instance (FileList pattern).

**Skip the Provider (act as own provider) when:**

- There is exactly one consumer / the component is a singleton (StackView pattern).

**Add a new compound child when:**

- A rendering concern is optional (render it or don't).
- A rendering concern has its own props that shouldn't pollute the parent's interface.
- Two different consumers need meaningfully different rendering for the same slot.

**Keep something in the controller when:**

- State or logic needs to be shared across two or more children.
- It is a pure state transformation (no rendering decisions, no callbacks).

**Keep something in the child when:**

- It is a callback, event handler, or rendering extension point.
- It is a prop with no cross-tree visibility requirement.
