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
		fileWrapper: Snippet<[TreeChange, number]>;
	};

	let { stackId, node, isRoot = false, showCheckboxes, changes, fileWrapper }: Props = $props();
</script>

{#if isRoot}
	<!-- Node is a root and should only render children! -->
	<div class="text-13">
		{#each node.children as childNode}
			<Self {stackId} node={childNode} {showCheckboxes} {changes} {fileWrapper} />
		{/each}
	</div>
{:else if node.kind === 'file'}
	{@render fileWrapper(node.change, node.index)}
{:else}
	<TreeListFolder showCheckbox={showCheckboxes} {node} />

	<div class="nested">
		<div class="line-wrapper">
			<div class="line"></div>
		</div>
		<div class="files-list">
			{#each node.children as childNode}
				<Self {stackId} node={childNode} {showCheckboxes} {changes} {fileWrapper} />
			{/each}
		</div>
	</div>
{/if}

<style lang="postcss">
	.nested {
		display: flex;
		width: 100%;
		overflow: hidden;
	}
	.line-wrapper {
		position: relative;
		padding-left: 7px;
		padding-right: 7px;
		&:hover .line {
			background-color: var(--clr-scale-ntrl-60);
		}
	}
	.line {
		width: 1px;
		height: 100%;
		background-color: var(--clr-scale-ntrl-80);
	}
	.files-list {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		width: 100%;
	}
</style>
