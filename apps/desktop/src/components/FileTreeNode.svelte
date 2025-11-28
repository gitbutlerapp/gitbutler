<script lang="ts">
	import Self from '$components/FileTreeNode.svelte';
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import TreeListFolder from '$components/TreeListFolder.svelte';
	import { chunk } from '$lib/utils/array';
	import { TestId } from '@gitbutler/ui';
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

	// For root nodes, use chunking to lazily load children for better performance
	let currentDisplayIndex = $state(0);
	const childChunks = $derived(isRoot ? chunk(node.children, 100) : []);
	const visibleChildren = $derived(
		isRoot ? childChunks.slice(0, currentDisplayIndex + 1).flat() : node.children
	);

	function loadMore() {
		if (currentDisplayIndex + 1 >= childChunks.length) return;
		currentDisplayIndex += 1;
	}

	// Handler for toggling the folder
	function handleToggle() {
		isExpanded = !isExpanded;
	}
</script>

{#if isRoot}
	<!-- Node is a root and should only render children with lazy loading! -->
	<LazyloadContainer
		minTriggerCount={80}
		itemCount={visibleChildren.length}
		ontrigger={() => {
			loadMore();
		}}
	>
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
	</LazyloadContainer>
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
