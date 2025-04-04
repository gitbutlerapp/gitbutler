<!-- This is a V3 replacement for `FileListItemWrapper.svelte` -->
<script lang="ts">
	import FileContextMenu from '$components/v3/FileContextMenu.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { getFilename } from '$lib/files/utils';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { key, type SelectionId } from '$lib/selection/key';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getContext, maybeGetContextStore } from '@gitbutler/shared/context';
	import FileListItemV3 from '@gitbutler/ui/file/FileListItemV3.svelte';
	import FileViewHeader from '@gitbutler/ui/file/FileViewHeader.svelte';
	import type { TreeChange } from '$lib/hunks/change';

	interface Props {
		projectId: string;
		change: TreeChange;
		selectionId: SelectionId;
		selected?: boolean;
		isHeader?: boolean;
		listActive?: boolean;
		isLast?: boolean;
		listMode: 'list' | 'tree';
		linesAdded?: number;
		linesRemoved?: number;
		depth?: number;
		showCheckbox?: boolean;
		onclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
	}

	const {
		change,
		selectionId,
		projectId,
		selected,
		isHeader,
		listActive,
		isLast,
		listMode,
		linesAdded,
		linesRemoved,
		depth,
		showCheckbox,
		onclick,
		onkeydown
	}: Props = $props();

	const stack = maybeGetContextStore(BranchStack);
	const stackId = $derived($stack?.id);
	const idSelection = getContext(IdSelection);
	const changeSelection = getContext(ChangeSelectionService);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();

	const selection = $derived(changeSelection.getById(change.path));
	const indeterminate = $derived(selection.current && selection.current.type === 'partial');
	const selectedChanges = $derived(idSelection.treeChanges(projectId, selectionId));

	function onCheck() {
		if (selection.current) {
			changeSelection.remove(change.path);
		} else {
			const { path, pathBytes } = change;
			changeSelection.add({
				type: 'full',
				path,
				pathBytes
			});
		}
	}

	function onContextMenu(e: MouseEvent) {
		if (selectedChanges.current.isSuccess && idSelection.has(change.path, selectionId)) {
			const changes: TreeChange[] = selectedChanges.current.data;
			contextMenu?.open(e, { changes });
			return;
		}

		contextMenu?.open(e, { changes: [change] });
	}

	function unSelectChanges(changes: TreeChange[]) {
		for (const change of changes) {
			idSelection.remove(change.path, selectionId);
			changeSelection.remove(change.path);
		}
	}
</script>

<div
	class="filelistitem-wrapper"
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new ChangeDropData(stackId || '', change, idSelection, selectionId),
		viewportId: 'board-viewport',
		selector: '.selected-draggable',
		disabled: showCheckbox
	}}
>
	<FileContextMenu
		bind:this={contextMenu}
		trigger={draggableEl}
		isUnapplied={false}
		isBinary={false}
		{unSelectChanges}
	/>
	{#if isHeader}
		<FileViewHeader
			filePath={change.path}
			fileStatus={computeChangeStatus(change)}
			draggable={!showCheckbox}
			{linesAdded}
			{linesRemoved}
			oncontextmenu={(e) => {
				e.stopPropagation();
				e.preventDefault();
				onContextMenu(e);
			}}
		/>
	{:else}
		<FileListItemV3
			id={key({ ...selectionId, path: change.path })}
			filePath={change.path}
			fileStatus={computeChangeStatus(change)}
			{selected}
			{showCheckbox}
			{listMode}
			checked={!!selection.current}
			{listActive}
			{indeterminate}
			{isLast}
			{depth}
			draggable={!showCheckbox}
			{onkeydown}
			locked={false}
			conflicted={false}
			onclick={(e) => {
				onclick?.(e);
			}}
			oncheck={onCheck}
			oncontextmenu={onContextMenu}
		/>
	{/if}
</div>
