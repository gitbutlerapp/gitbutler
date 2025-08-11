<script lang="ts">
	import AsyncRender from '$components/AsyncRender.svelte';
	import Self from '$components/FileTreeNode.svelte';
	import TreeListFolder from '$components/TreeListFolder.svelte';
	import { TestId } from '@gitbutler/ui';
	import type { TreeNode } from '$lib/files/filetreeV3';
	import type { TreeChange } from '$lib/hunks/change';
	import type { Snippet } from 'svelte';

	type Props = {
		stackId?: string;
		node: TreeNode;
		isRoot?: boolean;
		showCheckboxes?: boolean;
		changes: TreeChange[];
		depth?: number;
		initiallyExpanded?: boolean;
		fileTemplate: Snippet<[TreeChange, number, number]>;
	};

	let {
		stackId,
		node,
		isRoot = false,
		showCheckboxes,
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
		<Self {stackId} {depth} node={childNode} {showCheckboxes} {changes} {fileTemplate} />
	{/each}
{:else if node.kind === 'file'}
	{@render fileTemplate(node.change, node.index, depth)}
{:else}
	<TreeListFolder
		{stackId}
		testId={TestId.FileListTreeFolder}
		{depth}
		{isExpanded}
		showCheckbox={showCheckboxes}
		{node}
		ontoggle={handleToggle}
	/>

	{#if isExpanded}
		<AsyncRender>
			{#each node.children as childNode (childNode.name)}
				<Self
					{stackId}
					depth={depth + 1}
					node={childNode}
					{showCheckboxes}
					{changes}
					{fileTemplate}
				/>
			{/each}
		</AsyncRender>
	{/if}
{/if}
