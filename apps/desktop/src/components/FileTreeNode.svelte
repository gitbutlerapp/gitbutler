<script lang="ts">
	import Self from '$components/FileTreeNode.svelte';
	import TreeListFolder from '$components/TreeListFolder.svelte';
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
		initiallyExpanded?: boolean;
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
		fileTemplate
	}: Props = $props();

	// Local state to track whether the folder is expanded
	let isExpanded = $state<boolean>(true);

	// Handler for toggling the folder
	function handleToggle() {
		isExpanded = !isExpanded;
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
		/>
	{/each}
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
				{fileTemplate}
			/>
		{/each}
	{/if}
{/if}
