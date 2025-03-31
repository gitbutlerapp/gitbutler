<script lang="ts">
	import Self from '$components/v3/FileTreeNode.svelte';
	import TreeListFolder from '$components/v3/TreeListFolder.svelte';
	import type { TreeNode } from '$lib/files/filetreeV3';
	import type { TreeChange } from '$lib/hunks/change';
	import type { Snippet } from 'svelte';

	type Props = {
		stackId?: string;
		node: TreeNode;
		isRoot?: boolean;
		showCheckboxes: boolean;
		changes: TreeChange[];
		depth?: number;
		fileWrapper: Snippet<[TreeChange, number, number]>;
		onFolderClick: (e: MouseEvent) => void;
	};

	let {
		stackId,
		node,
		isRoot = false,
		showCheckboxes,
		changes,
		depth = 0,
		fileWrapper,
		onFolderClick
	}: Props = $props();

	// Local state to track whether the folder is expanded
	let isExpanded = $state(true);

	// Handler for toggling the folder
	function handleToggle() {
		isExpanded = !isExpanded;
	}
</script>

{#if isRoot}
	<!-- Node is a root and should only render children! -->
	{#each node.children as childNode}
		<Self
			{depth}
			{stackId}
			node={childNode}
			{showCheckboxes}
			{changes}
			{fileWrapper}
			{onFolderClick}
		/>
	{/each}
{:else if node.kind === 'file'}
	{@render fileWrapper(node.change, node.index, depth)}
{:else}
	<TreeListFolder
		{depth}
		{isExpanded}
		showCheckbox={showCheckboxes}
		{node}
		onclick={onFolderClick}
		ontoggle={handleToggle}
	/>

	{#if isExpanded}
		{#each node.children as childNode}
			<Self
				depth={depth + 1}
				{stackId}
				node={childNode}
				{showCheckboxes}
				{changes}
				{fileWrapper}
				{onFolderClick}
			/>
		{/each}
	{/if}
{/if}
