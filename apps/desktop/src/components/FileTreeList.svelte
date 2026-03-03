<script lang="ts">
	import { abbreviateFolders, changesToFileTree } from "$lib/files/filetreeV3";
	import { isExecutableStatus } from "$lib/hunks/change";
	import { computeChangeStatus } from "$lib/utils/fileStatus";
	import { FileListItem, FolderListItem } from "@gitbutler/ui";
	import { SvelteMap } from "svelte/reactivity";
	import type { TreeNode } from "$lib/files/filetreeV3";
	import type { TreeChange } from "$lib/hunks/change";
	import type { FocusableOptions } from "@gitbutler/ui/focus/focusManager";

	type Props = {
		changes: TreeChange[];
		selectedIndex?: number;
		visibleRange?: { start: number; end: number };
		listMode?: "list" | "tree";
		getItemFocusableOpts?: (index: number) => FocusableOptions;
		onFileClick?: (index: number) => void;
		onFileContextMenu?: (e: MouseEvent, change: TreeChange) => void;
	};

	const {
		changes,
		selectedIndex,
		visibleRange,
		listMode = "list",
		getItemFocusableOpts,
		onFileClick,
		onFileContextMenu,
	}: Props = $props();

	const tree = $derived(abbreviateFolders(changesToFileTree(changes)));

	// Track folder expanded state reactively, keyed by folder path
	const folderExpanded = new SvelteMap<string, boolean>();

	function isFolderExpanded(path: string): boolean {
		return folderExpanded.get(path) ?? true;
	}
</script>

{#snippet treeNodes(node: TreeNode, depth: number)}
	{#if node.kind === "file"}
		<FileListItem
			filePath={node.change.path}
			fileStatus={computeChangeStatus(node.change)}
			executable={isExecutableStatus(node.change.status)}
			selected={selectedIndex === node.index}
			active={selectedIndex === node.index}
			notched={visibleRange !== undefined &&
				node.index >= visibleRange.start &&
				node.index < visibleRange.end}
			listMode="tree"
			{depth}
			actionOpts={getItemFocusableOpts?.(node.index)}
			onclick={() => onFileClick?.(node.index)}
			oncontextmenu={(e) => onFileContextMenu?.(e, node.change)}
		/>
	{:else if node.parent === undefined}
		{#each node.children as child (child.name)}
			{@render treeNodes(child, depth)}
		{/each}
	{:else}
		<FolderListItem
			name={node.name}
			isExpanded={isFolderExpanded(node.name)}
			{depth}
			ontoggle={(v) => folderExpanded.set(node.name, v)}
		/>
		{#if isFolderExpanded(node.name)}
			{#each node.children as child (child.name)}
				{@render treeNodes(child, depth + 1)}
			{/each}
		{/if}
	{/if}
{/snippet}

{#if listMode === "tree"}
	{@render treeNodes(tree, 0)}
{:else}
	{#each changes as change, index}
		<FileListItem
			filePath={change.path}
			fileStatus={computeChangeStatus(change)}
			executable={isExecutableStatus(change.status)}
			selected={selectedIndex === index}
			active={selectedIndex === index}
			notched={visibleRange !== undefined &&
				index >= visibleRange.start &&
				index < visibleRange.end}
			listMode="list"
			isLast={index === changes.length - 1}
			actionOpts={getItemFocusableOpts?.(index)}
			onclick={() => onFileClick?.(index)}
			oncontextmenu={(e) => onFileContextMenu?.(e, change)}
		/>
	{/each}
{/if}
