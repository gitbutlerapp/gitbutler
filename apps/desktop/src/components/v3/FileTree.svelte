<script lang="ts">
	import FileTreeNode from '$components/v3/FileTreeNode.svelte';
	import { type TreeNode } from '$lib/files/filetreeV3';
	import type { TreeChange } from '$lib/hunks/change';
	import type { Snippet } from 'svelte';

	type Props = {
		stackId?: string;
		expanded: boolean;
		node: TreeNode;
		showCheckboxes: boolean;
		changes: TreeChange[];
		fileWrapper: Snippet<[TreeChange, number]>;
	};
	const props: Props = $props();
</script>

<div class="file-tree">
	<FileTreeNode isRoot {...props} />
</div>

<style lang="postcss">
	.file-tree {
		padding: 0 12px;
	}
</style>
