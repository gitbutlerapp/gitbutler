<script lang="ts">
	import Self from '$components/FileTreeNode.svelte';
	import TreeListFolder from '$components/TreeListFolder.svelte';
	import { TestId, Button } from '@gitbutler/ui';
	import type { TreeNode } from '$lib/files/filetreeV3';
	import type { TreeChange } from '$lib/hunks/change';
	import type { SelectionId } from '$lib/selection/key';
	import type { Snippet } from 'svelte';

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
		/**
		 * Whether folders should be expanded by default.
		 * Set to false when there are 100+ files to avoid rendering performance issues.
		 */
		defaultExpanded?: boolean;
		fileTemplate: Snippet<[TreeChange, number, number]>;
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
		defaultExpanded = true,
		fileTemplate
	}: Props = $props();

	// Local state to track whether the folder is expanded
	let isExpanded = $state<boolean>(defaultExpanded);

	// For root nodes with many children, use pagination to avoid rendering everything at once
	const BATCH_SIZE = 100;
	let visibleCount = $state(BATCH_SIZE);
	const totalChildren = $derived(node.children.length);
	const hasMore = $derived(isRoot && visibleCount < totalChildren);
	const visibleChildren = $derived(isRoot ? node.children.slice(0, visibleCount) : node.children);

	function showMore() {
		visibleCount = Math.min(visibleCount + BATCH_SIZE, totalChildren);
	}

	// Handler for toggling the folder
	function handleToggle() {
		isExpanded = !isExpanded;
	}
</script>

{#if isRoot}
	<!-- Node is a root and should only render children! -->
	{#each visibleChildren as childNode (childNode.name)}
		<Self
			{projectId}
			{stackId}
			{selectionId}
			{depth}
			node={childNode}
			{showCheckboxes}
			{draggableFiles}
			{changes}
			{defaultExpanded}
			{fileTemplate}
		/>
	{/each}
	{#if hasMore}
		<div class="show-more">
			<Button kind="outline" onclick={showMore}>
				Show more ({totalChildren - visibleCount} remaining)
			</Button>
		</div>
	{/if}
{:else if node.kind === 'file'}
	{@render fileTemplate(node.change, node.index, depth)}
{:else}
	<TreeListFolder
		{projectId}
		{stackId}
		{selectionId}
		testId={TestId.FileListTreeFolder}
		{depth}
		{isExpanded}
		showCheckbox={showCheckboxes}
		draggable={draggableFiles}
		{node}
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
				{defaultExpanded}
				{fileTemplate}
			/>
		{/each}
	{/if}
{/if}

<style>
	.show-more {
		display: flex;
		justify-content: center;
		padding: 8px;
	}
</style>
