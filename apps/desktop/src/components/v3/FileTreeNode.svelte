<script lang="ts">
	import Self from '$components/v3/FileTreeNode.svelte';
	import TreeListFolder from '$components/v3/TreeListFolder.svelte';
	import type { TreeNode } from '$lib/files/filetreeV3';
	import type { TreeChange } from '$lib/hunks/change';
	import type { Snippet } from 'svelte';

	type Props = {
		node: TreeNode;
		isRoot?: boolean;
		showCheckboxes?: boolean;
		changes: TreeChange[];
		depth?: number;
		fileTemplate: Snippet<[TreeChange, number, number]>;
	};

	let { node, isRoot = false, showCheckboxes, changes, depth = 0, fileTemplate }: Props = $props();

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
		<Self {depth} node={childNode} {showCheckboxes} {changes} {fileTemplate} />
	{/each}
{:else if node.kind === 'file'}
	{@render fileTemplate(node.change, node.index, depth)}
{:else}
	<TreeListFolder
		{depth}
		{isExpanded}
		showCheckbox={showCheckboxes}
		{node}
		ontoggle={handleToggle}
	/>

	{#if isExpanded}
		{#each node.children as childNode}
			<Self depth={depth + 1} node={childNode} {showCheckboxes} {changes} {fileTemplate} />
		{/each}
	{/if}
{/if}
