<!--
	Compound component that renders the file list (list or tree mode).
	Must be a child of <FileListProvider>.

	Usage:
	```svelte
	<FileListProvider {changes} {selectionId}>
		<FileListItems mode="list" draggable />
	</FileListProvider>
	```
-->
<script lang="ts">
	import FileListItemContainer from "$components/files/FileListItemContainer.svelte";
	import FileTreeNode from "$components/files/FileTreeNode.svelte";
	import FileTreeFolder from "$components/files/FileTreeFolder.svelte";
	import LazyList from "$components/shared/LazyList.svelte";
	import { DEPENDENCY_SERVICE } from "$lib/dependencies/dependencyService.svelte";
	import { abbreviateFolders, changesToFileTree, getAllChanges, nodePath } from "$lib/files/filetreeV3";
	import { isExecutableStatus } from "$lib/hunks/change";
	import { getLockedCommitIds, getLockedTargets, isFileLocked } from "$lib/hunks/dependencies";
	import {
		getFileListContext,
		type FileListKeyHandler,
	} from "$lib/selection/fileListController.svelte";
	import { pathIsLocallyIgnored } from "$lib/worktree/worktreeService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { TestId, VirtualList } from "@gitbutler/ui";
	import { FOCUS_MANAGER } from "@gitbutler/ui/focus/focusManager";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import type { ConflictEntriesObj } from "$lib/files/conflicts";
	import type { ScrollbarVisilitySettings } from "@gitbutler/ui/components/scroll/Scrollbar.svelte";
	import type { TreeChange } from "@gitbutler/but-sdk";
	import type { TreeNode } from "$lib/files/filetreeV3";

	type Props = {
		projectId: string;
		stackId?: string;
		mode: "list" | "tree";
		showCheckboxes?: boolean;
		draggable?: boolean;
		showLockedIndicator?: boolean;
		visibleRange?: { start: number; end: number };
		/** nick → file paths mapping from IRC working files broadcast */
		ircWorkingFiles?: Record<string, string[]>;
		/** Per-file conflict hints (rendered inline on each item) */
		conflictEntries?: ConflictEntriesObj;
		localIgnoredPaths?: string[];
		virtualized?: boolean;
		scrollbarVisibility?: ScrollbarVisilitySettings;
		onvisiblechange?: (range: { start: number; end: number } | undefined) => void;
		dataTestId?: string;
		/** Called when a file is selected (click, Enter/Space/l, or arrow navigation). */
		onselect?: (change: TreeChange, index: number) => void;
		/** Extra keyboard handlers injected by the consumer (e.g. AI shortcuts). */
		extraKeyHandlers?: FileListKeyHandler[];
	};

	const {
		projectId,
		stackId,
		mode,
		showCheckboxes,
		draggable,
		showLockedIndicator = false,
		visibleRange,
		ircWorkingFiles,
		conflictEntries,
		localIgnoredPaths = [],
		virtualized = false,
		scrollbarVisibility = "hover",
		onvisiblechange,
		dataTestId,
		onselect,
		extraKeyHandlers,
	}: Props = $props();

	const controller = getFileListContext();
	const dependencyService = inject(DEPENDENCY_SERVICE);
	const focusManager = inject(FOCUS_MANAGER);

	/** Invert nick→paths map to path→nicks for per-file lookup. */
	const ircWorkingUsersByPath = $derived.by(() => {
		if (!ircWorkingFiles) return undefined;
		const map = new Map<string, string[]>();
		for (const [nick, paths] of Object.entries(ircWorkingFiles)) {
			for (const p of paths) {
				const nicks = map.get(p);
				if (nicks) {
					nicks.push(nick);
				} else {
					map.set(p, [nick]);
				}
			}
		}
		return map;
	});

	const filePaths = $derived(controller.changes.map((change) => change.path));
	const fileDependenciesQuery = $derived(
		showLockedIndicator ? dependencyService.filesDependencies(projectId, filePaths, stackId) : null,
	);
	const fileDependencies = $derived(fileDependenciesQuery?.result.data || []);
	const tree = $derived.by(() => abbreviateFolders(changesToFileTree(controller.changes)));
	const localIgnoredPathSet = $derived(new Set(localIgnoredPaths));
	let expandedFolders = $state.raw<Record<string, boolean>>({});

	type FileListRow =
		| { type: "list"; change: TreeChange; index: number; depth: number }
		| { type: "file"; change: TreeChange; index: number; depth: number }
		| {
				type: "folder";
				node: TreeNode & { kind: "dir" };
				path: string;
				depth: number;
				changes: TreeChange[];
		  };

	const flatTreeRows = $derived.by(() => flattenTreeRows(tree));
	const flatListRows = $derived.by(() =>
		controller.changes.map((change, index) => ({
			type: "list" as const,
			change,
			index,
			depth: 0,
		})),
	);
	const virtualItems = $derived.by(() => (mode === "tree" ? flatTreeRows : flatListRows));

	function isLocallyIgnored(path: string): boolean {
		if (localIgnoredPathSet.has(path)) return true;
		return pathIsLocallyIgnored(path, localIgnoredPaths);
	}

	function flattenTreeRows(root: TreeNode): FileListRow[] {
		const rows: FileListRow[] = [];

		function visit(node: TreeNode, depth: number) {
			if (node.kind === "file") {
				rows.push({ type: "file", change: node.change, index: node.index, depth });
				return;
			}

			if (node.parent) {
				const path = nodePath(node);
				const changes = getAllChanges(node);
				rows.push({ type: "folder", node, path, depth, changes });
				if (expandedFolders[path] === false) return;
			}

			for (const child of node.children) {
				visit(child, node.parent ? depth + 1 : depth);
			}
		}

		visit(root, 0);
		return rows;
	}

	function setFolderExpanded(path: string, expanded: boolean) {
		expandedFolders = { ...expandedFolders, [path]: expanded };
	}

	function selectFolderContents(e: MouseEvent, row: Extract<FileListRow, { type: "folder" }>) {
		const selectableChanges = row.changes.filter((change) => !isLocallyIgnored(change.path));
		if (selectableChanges.length === 0) return;

		const indexMap = new Map(controller.changes.map((change, index) => [change.path, index]));
		if (!(e.ctrlKey || e.metaKey || e.shiftKey)) {
			controller.selection.clear(controller.selectionId);
		}

		const last = selectableChanges.at(-1)!;
		const lastIndex = indexMap.get(last.path) ?? 0;
		controller.selection.addMany(
			selectableChanges.map((change) => change.path),
			controller.selectionId,
			{ path: last.path, index: lastIndex },
		);
	}
</script>

{#snippet fileTemplate(change: TreeChange, idx: number, depth: number = 0, isLast: boolean = false)}
	{@const isExecutable = isExecutableStatus(change.status)}
	{@const locallyIgnored = isLocallyIgnored(change.path)}
	{@const selected = controller.isSelected(change.path)}
	{@const locked = showLockedIndicator && isFileLocked(change.path, fileDependencies)}
	{@const lockedCommitIds = showLockedIndicator
		? getLockedCommitIds(change.path, fileDependencies)
		: []}
	{@const lockedTargets = showLockedIndicator
		? getLockedTargets(change.path, fileDependencies)
		: []}
	<FileListItemContainer
		selectionId={controller.selectionId}
		{change}
		{projectId}
		{stackId}
		{selected}
		listMode={mode}
		{depth}
		active={controller.active}
		{locked}
		{lockedCommitIds}
		{lockedTargets}
		{isLast}
		notched={controller.hasSelectionInList &&
			visibleRange !== undefined &&
			idx >= visibleRange.start &&
			idx < visibleRange.end}
		draggable={draggable && !locallyIgnored}
		executable={isExecutable}
		showCheckbox={showCheckboxes && !locallyIgnored}
		ircWorkingUsers={ircWorkingUsersByPath?.get(change.path)}
		{locallyIgnored}
		focusableOpts={{
			onKeydown: (e) => {
				if (locallyIgnored) return false;
				// 1. Activation keys (Enter/Space/l)
				if (controller.handleActivation(change, idx, e)) {
					onselect?.(change, idx);
					return true;
				}
				// 2. Extra handlers (e.g. AI shortcuts)
				if (extraKeyHandlers) {
					for (const handler of extraKeyHandlers) {
						if (handler(change, idx, e)) return true;
					}
				}
				// 3. Arrow/vim navigation.
				// In tree mode with shift held: use flat-array multi-select (handleNavigation)
				// so shift+arrows extend the selection across files, skipping folders.
				// In tree mode without shift: let FM navigate naturally through folder
				// headers too — file selection happens via onActive below.
				// In list mode: always intercept and drive selection ourselves.
				if (mode === "tree") {
					if (e.shiftKey) {
						const navigatedIndex = controller.handleNavigation(e);
						if (navigatedIndex !== undefined) {
							const navigatedChange = controller.changes[navigatedIndex];
							if (navigatedChange) {
								onselect?.(navigatedChange, navigatedIndex);
							}
							return true;
						}
					}
					return false;
				}
				const navigatedIndex = controller.handleNavigation(e);
				if (navigatedIndex !== undefined && navigatedIndex !== idx) {
					const navigatedChange = controller.changes[navigatedIndex];
					if (navigatedChange) {
						onselect?.(navigatedChange, navigatedIndex);
					}
					return true;
				}
			},
			// In tree mode, FM fires onActive when plain arrow keys land on a file
			// item. We use this to drive single-file selection so folders are
			// navigable. Shift+arrows are handled in onKeydown above instead.
			// Guard: skip when controller.isKeyboardSelecting is true — that means
			// shift-range-select called focusByElement to move the ring, and we
			// must not overwrite the multi-select with a single-file set().
			onActive:
				mode === "tree" && !locallyIgnored
					? (active) => {
							if (active && focusManager.isKeyboardNavigation && !controller.isKeyboardSelecting) {
								controller.selectSingle(change, idx);
								onselect?.(change, idx);
							}
						}
					: undefined,
			focusable: true,
		}}
		onclick={(e) => {
			if (locallyIgnored) return;
			e.stopPropagation();
			controller.select(e, change, idx);
			if (controller.isSelected(change.path)) {
				onselect?.(change, idx);
			}
		}}
		{conflictEntries}
	/>
{/snippet}

{#snippet folderTemplate(row: Extract<FileListRow, { type: "folder" }>)}
	{@const locallyIgnored =
		row.changes.length > 0 && row.changes.every((change) => isLocallyIgnored(change.path))}
	<FileTreeFolder
		{projectId}
		{stackId}
		selectionId={controller.selectionId}
		testId={TestId.FileListTreeFolder}
		depth={row.depth}
		isExpanded={expandedFolders[row.path] !== false}
		showCheckbox={showCheckboxes && !locallyIgnored}
		draggable={draggable && !locallyIgnored}
		locallyIgnored={locallyIgnored}
		node={row.node}
		active={controller.active}
		focusableOpts={{
			focusable: true,
			onAction: () => {
				const syntheticEvent = new MouseEvent("click");
				selectFolderContents(syntheticEvent, row);
			},
		}}
		onclick={(e) => selectFolderContents(e, row)}
		ontoggle={(expanded) => setFolderExpanded(row.path, expanded)}
	/>
{/snippet}

{#snippet virtualTemplate(row: FileListRow)}
	{#if row.type === "folder"}
		{@render folderTemplate(row)}
	{:else}
		{@render fileTemplate(row.change, row.index, row.depth)}
	{/if}
{/snippet}

<div
	data-testid={dataTestId}
	class="file-list"
	class:virtualized
	use:focusable={{
		vertical: true,
		onActive: (value) => (controller.active = value),
	}}
>
	{#if controller.changes.length > 0}
		{#if virtualized}
			<VirtualList
				grow
				items={virtualItems}
				defaultHeight={30}
				visibility={scrollbarVisibility}
				renderDistance={180}
				onVisibleChange={onvisiblechange}
				getId={(row) => (row.type === "folder" ? `folder:${row.path}` : `file:${row.change.path}`)}
			>
				{#snippet template(row)}
					{@render virtualTemplate(row)}
				{/snippet}
			</VirtualList>
		{:else if mode === "tree"}
			<FileTreeNode
				isRoot
				{projectId}
				selectionId={controller.selectionId}
				{stackId}
				node={tree}
				{showCheckboxes}
				draggableFiles={draggable}
				changes={controller.changes}
				{fileTemplate}
				active={controller.active}
				{localIgnoredPaths}
			/>
		{:else}
			<LazyList items={controller.changes} chunkSize={100}>
				{#snippet template(change, context)}
					<!--
						There is a bug here related to the reactivity of `idSelection.has`,
						affecting somehow the first item in the list of files.. but only where
						used for the "assigned files" of the workspace.

						This unused variable is a workaround, while present the reactivity
						works as expected.

						TODO: Bisect this issue, it was introduced between nightly version
						0.5.1705 and 0.5.1783.
						-->
					{@const _selected = controller.isSelected(change.path)}
					{@render fileTemplate(change, context.index, 0, context.last)}
				{/snippet}
			</LazyList>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.file-list {
		display: flex;
		flex-direction: column;

		&.virtualized {
			flex: 1;
			min-height: 0;
		}
	}
</style>
