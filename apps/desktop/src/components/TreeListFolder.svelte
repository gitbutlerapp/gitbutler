<script lang="ts">
	import { nodePath, type TreeNode } from '$lib/files/filetreeV3';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { FolderListItem } from '@gitbutler/ui';

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

	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const selectionStatus = $derived(uncommittedService.folderCheckStatus(stackId, nodePath(node)));

	function handleCheck(checked: boolean) {
		if (checked) {
			uncommittedService.checkDir(stackId || null, nodePath(node));
		} else {
			uncommittedService.uncheckDir(stackId || null, nodePath(node));
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
