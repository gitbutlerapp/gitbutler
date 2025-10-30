<script lang="ts">
	import ChangedFilesContextMenu from '$components/ChangedFilesContextMenu.svelte';
	import { getAllChanges, nodePath, type TreeNode } from '$lib/files/filetreeV3';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { FolderListItem } from '@gitbutler/ui';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		stackId?: string;
		selectionId: SelectionId;
		node: TreeNode & { kind: 'dir' };
		depth: number;
		showCheckbox?: boolean;
		isExpanded?: boolean;
		onclick?: (e: MouseEvent) => void;
		ontoggle?: (expanded: boolean) => void;
		testId?: string;
	};

	const {
		projectId,
		stackId,
		selectionId,
		node,
		depth,
		showCheckbox,
		isExpanded,
		onclick,
		ontoggle,
		testId
	}: Props = $props();

	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const selectionStatus = $derived(uncommittedService.folderCheckStatus(stackId, nodePath(node)));

	let contextMenu: ReturnType<typeof ChangedFilesContextMenu>;
	let draggableEl: HTMLDivElement | undefined = $state();

	function handleCheck(checked: boolean) {
		if (checked) {
			uncommittedService.checkDir(stackId || null, nodePath(node));
		} else {
			uncommittedService.uncheckDir(stackId || null, nodePath(node));
		}
	}

	function onContextMenu(e: MouseEvent) {
		const item = {
			path: nodePath(node),
			changes: getAllChanges(node)
		};
		contextMenu?.open(e, item);
	}
</script>

<div bind:this={draggableEl}>
	<ChangedFilesContextMenu
		bind:this={contextMenu}
		{projectId}
		{stackId}
		trigger={draggableEl}
		{selectionId}
	/>

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
		oncontextmenu={onContextMenu}
	/>
</div>
