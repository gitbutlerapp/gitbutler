<script lang="ts">
	import FileTreeFolder from "$components/files/FileTreeFolder.svelte";
	import Self from "$components/files/FileTreeNode.svelte";
	import { getAllChanges } from "$lib/files/filetreeV3";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { pathIsLocallyIgnored } from "$lib/worktree/worktreeService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";
	import type { TreeNode } from "$lib/files/filetreeV3";
	import type { SelectionId } from "$lib/selection/key";
	import type { TreeChange } from "@gitbutler/but-sdk";
	import type { Snippet } from "svelte";

	type Props = {
		projectId: string;
		stackId?: string;
		selectionId: SelectionId;
		node: TreeNode;
		isRoot?: boolean;
		showCheckboxes?: boolean;
		draggableFiles?: boolean;
		changes: TreeChange[];
		depth?: number;
		initiallyExpanded?: boolean;
		fileTemplate: Snippet<[TreeChange, number, number]>;
		localIgnoredPaths?: string[];
		active?: boolean;
	};

	let {
		projectId,
		stackId,
		selectionId,
		node,
		isRoot = false,
		showCheckboxes,
		draggableFiles,
		changes,
		depth = 0,
		fileTemplate,
		localIgnoredPaths = [],
		active,
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);

	// Local state to track whether the folder is expanded
	let isExpanded = $state<boolean>(true);

	// Flag to suppress keyboard-nav selection when a mouse click is in progress
	let mouseClickPending = false;
	const folderChanges = $derived(node.kind === "dir" ? getAllChanges(node) : []);
	const selectableFolderChanges = $derived(
		folderChanges.filter((change) => !pathIsLocallyIgnored(change.path, localIgnoredPaths)),
	);
	const folderLocallyIgnored = $derived(
		folderChanges.length > 0 && selectableFolderChanges.length === 0,
	);

	// Handler for toggling the folder
	function handleToggle() {
		isExpanded = !isExpanded;
	}

	// Selects all files nested under this folder node
	function selectFolderContents(addToSelection = false) {
		if (node.kind !== "dir") return;
		if (selectableFolderChanges.length === 0) return;

		const indexMap = new Map(changes.map((c, i) => [c.path, i]));

		if (!addToSelection) {
			idSelection.clear(selectionId);
		}

		const last = selectableFolderChanges.at(-1)!;
		const lastIndex = indexMap.get(last.path) ?? 0;
		idSelection.addMany(
			selectableFolderChanges.map((c) => c.path),
			selectionId,
			{ path: last.path, index: lastIndex },
		);
	}

	// Handler for clicking a folder — respects modifier keys for multi-select
	function handleFolderClick(e: MouseEvent) {
		selectFolderContents(e.ctrlKey || e.metaKey || e.shiftKey);
	}

	// Set pending flag on mousedown so onActive skips selection during mouse clicks
	function handleFolderMouseDown() {
		mouseClickPending = true;
		setTimeout(() => {
			mouseClickPending = false;
		}, 0);
	}

	// Handles arrow-key navigation away from a folder by updating file selection
	// before FocusManager moves focus to the next/prev item.
	function handleFolderKeyDown(e: KeyboardEvent): boolean {
		if (selectableFolderChanges.length === 0) return false;

		if ((e.key === "ArrowDown" || e.key === "j") && !e.shiftKey) {
			// FocusManager will focus the first file in this folder next.
			const firstFile = selectableFolderChanges[0]!;
			const idx = changes.findIndex((c) => c.path === firstFile.path);
			if (idx !== -1) {
				idSelection.set(firstFile.path, selectionId, idx);
			}
		} else if ((e.key === "ArrowUp" || e.key === "k") && !e.shiftKey) {
			// FocusManager will focus the item before this folder next.
			const firstFile = selectableFolderChanges[0]!;
			const idx = changes.findIndex((c) => c.path === firstFile.path);
			if (idx > 0) {
				const prevFile = changes[idx - 1]!;
				idSelection.set(prevFile.path, selectionId, idx - 1);
			}
		}
		return false; // Let FocusManager handle the actual focus movement
	}
</script>

{#if isRoot}
	<!-- Node is a root and should only render children! -->
	{#each node.children as childNode (childNode.name)}
		<Self
			{projectId}
			{stackId}
			{selectionId}
			{depth}
			node={childNode}
			{showCheckboxes}
			{draggableFiles}
			{changes}
			{fileTemplate}
			{localIgnoredPaths}
			{active}
		/>
	{/each}
{:else if node.kind === "file"}
	{@render fileTemplate(node.change, node.index, depth)}
{:else}
	<FileTreeFolder
		{projectId}
		{stackId}
		{selectionId}
		testId={TestId.FileListTreeFolder}
		{depth}
		{isExpanded}
		showCheckbox={showCheckboxes && !folderLocallyIgnored}
		draggable={draggableFiles && !folderLocallyIgnored}
		locallyIgnored={folderLocallyIgnored}
		{node}
		{active}
		focusableOpts={{
			focusable: !folderLocallyIgnored,
			onAction: () => selectFolderContents(),
			onActive: (isActive) => {
				if (!folderLocallyIgnored && isActive && !mouseClickPending) selectFolderContents();
			},
			onKeydown: handleFolderKeyDown,
		}}
		onmousedown={handleFolderMouseDown}
		onclick={handleFolderClick}
		ontoggle={handleToggle}
	/>

	{#if isExpanded}
		{#each node.children as childNode (childNode.name)}
			<Self
				{projectId}
				{stackId}
				{selectionId}
				depth={depth + 1}
				node={childNode}
				{showCheckboxes}
				{draggableFiles}
				{changes}
				{fileTemplate}
				{localIgnoredPaths}
				{active}
			/>
		{/each}
	{/if}
{/if}
