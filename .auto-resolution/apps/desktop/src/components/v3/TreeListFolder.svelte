<script lang="ts">
	import { getAllChanges, nodePath, type TreeNode } from '$lib/files/filetreeV3';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import FolderListItem from '@gitbutler/ui/file/FolderListItem.svelte';

	type Props = {
		stackId?: string;
		node: TreeNode & { kind: 'dir' };
		depth: number;
		showCheckbox?: boolean;
		isExpanded?: boolean;
		onclick?: (e: MouseEvent) => void;
		ontoggle?: (expanded: boolean) => void;
		testId?: string;
	};

	const { stackId, node, depth, showCheckbox, isExpanded, onclick, ontoggle, testId }: Props =
		$props();

	const uncommittedService = getContext(UncommittedService);
	const selectionStatus = $derived(uncommittedService.folderCheckStatus(stackId, nodePath(node)));

	function handleCheck(checked: boolean) {
		const changes = getAllChanges(node);
		for (const change of changes) {
			if (checked) {
				uncommittedService.checkFile(stackId || null, change.path);
			} else {
				uncommittedService.uncheckFile(stackId || null, change.path);
			}
		}
	}
</script>

<FolderListItem
	{testId}
	name={node.name}
	{depth}
	{isExpanded}
	{showCheckbox}
	checked={selectionStatus.current === 'checked'}
	indeterminate={selectionStatus.current === 'indeterminate'}
	oncheck={(e) => handleCheck(e.currentTarget.checked)}
	{onclick}
	{ontoggle}
/>
