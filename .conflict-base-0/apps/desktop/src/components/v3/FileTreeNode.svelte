<script lang="ts">
	import Self from '$components/v3/FileTreeNode.svelte';
	import TreeListFolder from '$components/v3/TreeListFolder.svelte';
	import { TestId } from '$lib/testing/testIds';
	import type { TreeNode } from '$lib/files/filetreeV3';
	import type { TreeChange } from '$lib/hunks/change';
	import type { Snippet } from 'svelte';

	const CHILD_THRESHOLD_FOR_AUTO_EXPAND = 8;

	type Props = {
		node: TreeNode;
		isRoot?: boolean;
		showCheckboxes?: boolean;
		changes: TreeChange[];
		depth?: number;
		initiallyExpanded?: boolean;
		fileTemplate: Snippet<[TreeChange, number, number]>;
	};

	let {
		node,
		isRoot = false,
		showCheckboxes,
		changes,
		depth = 0,
		fileTemplate,
		initiallyExpanded
	}: Props = $props();

	const hasAFewChildren = $derived(
		(node.kind === 'dir' || isRoot) && node.children.length <= CHILD_THRESHOLD_FOR_AUTO_EXPAND
	);
	const defaultIsExpanded = $derived(
		initiallyExpanded ?? (hasAFewChildren || node.kind === 'file')
	);

	let actionableIsExpanded = $state<boolean>();

	// Local state to track whether the folder is expanded
	const isExpanded = $derived(actionableIsExpanded ?? defaultIsExpanded);

	// Handler for toggling the folder
	function handleToggle() {
		actionableIsExpanded = !isExpanded;
	}
</script>

{#if isRoot}
	<!-- Node is a root and should only render children! -->
	{#each node.children as childNode (childNode.name)}
		<Self
			{depth}
			node={childNode}
			{showCheckboxes}
			{changes}
			{fileTemplate}
			initiallyExpanded={hasAFewChildren}
		/>
	{/each}
{:else if node.kind === 'file'}
	{@render fileTemplate(node.change, node.index, depth)}
{:else}
	<TreeListFolder
		testId={TestId.FileListTreeFolder}
		{depth}
		{isExpanded}
		showCheckbox={showCheckboxes}
		{node}
		ontoggle={handleToggle}
	/>

	{#if isExpanded}
		{#each node.children as childNode (childNode.name)}
			<Self depth={depth + 1} node={childNode} {showCheckboxes} {changes} {fileTemplate} />
		{/each}
	{/if}
{/if}
