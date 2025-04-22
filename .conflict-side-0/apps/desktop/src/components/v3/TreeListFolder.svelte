<script lang="ts">
	import { countLeafNodes, getAllChanges, nodePath, type TreeNode } from '$lib/files/filetreeV3';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import FolderListItem from '@gitbutler/ui/file/FolderListItem.svelte';

	type Props = {
		node: TreeNode & { kind: 'dir' };
		depth: number;
		showCheckbox?: boolean;
		isExpanded?: boolean;
		onclick?: (e: MouseEvent) => void;
		ontoggle?: (expanded: boolean) => void;
	};

	const { node, depth, showCheckbox, isExpanded, onclick, ontoggle }: Props = $props();

	const selectionService = getContext(ChangeSelectionService);
	const selection = $derived(selectionService.getByPrefix(nodePath(node)));
	const selectionCount = $derived(selection.current.length);
	const fileCount = $derived(countLeafNodes(node));

	const indeterminate = $derived.by(() => {
		if (!showCheckbox) return false;
		return selectionCount !== 0 && selectionCount !== fileCount;
	});

	const checked = $derived.by(() => {
		if (!showCheckbox) return false;
		return selectionCount === fileCount;
	});

	function handleCheck(e: Event) {
		const changes = getAllChanges(node);
		for (const change of changes) {
			if ((e.currentTarget as HTMLInputElement)?.checked) {
				selectionService.add({
					type: 'full',
					path: change.path,
					pathBytes: change.pathBytes
				});
			} else {
				selectionService.remove(change.path);
			}
		}
	}
</script>

<FolderListItem
	name={node.name}
	{depth}
	{isExpanded}
	{showCheckbox}
	{checked}
	{indeterminate}
	oncheck={handleCheck}
	{onclick}
	{ontoggle}
/>
