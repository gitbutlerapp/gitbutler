<!-- This is a V3 replacement for `FileListItemWrapper.svelte` -->
<script lang="ts">
	import FileContextMenu from '$components/v3/FileContextMenu.svelte';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { getFilename } from '$lib/files/utils';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { key, type SelectionId } from '$lib/selection/key';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getContext } from '@gitbutler/shared/context';
	import FileListItemV3 from '@gitbutler/ui/file/FileListItemV3.svelte';
	import FileViewHeader from '@gitbutler/ui/file/FileViewHeader.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import type { TreeChange } from '$lib/hunks/change';
	import type { Rename } from '$lib/hunks/change';
	import type { UnifiedDiff } from '$lib/hunks/diff';

	interface Props {
		projectId: string;
		change: TreeChange;
		diff?: UnifiedDiff;
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
		onCloseClick?: () => void;
	}

	const {
		change,
		diff,
		selectionId,
		projectId,
		selected,
		isHeader,
		listActive,
		isLast,
		listMode,
		depth,
		showCheckbox,
		onclick,
		onkeydown,
		onCloseClick
	}: Props = $props();

	const idSelection = getContext(IdSelection);
	const changeSelection = getContext(ChangeSelectionService);
	const diffService = getContext(DiffService);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();

	const selection = $derived(changeSelection.getById(change.path));
	const indeterminate = $derived(selection.current && selection.current.type === 'partial');
	const selectedChanges = $derived(idSelection.treeChanges(projectId, selectionId));
	const diffResult = $derived(diffService.getDiff(projectId, change));

	const isBinary = $derived(diffResult.current.data?.type === 'Binary');
	const isUncommitted = $derived(selectionId?.type === 'worktree');

	const previousTooltipText = $derived(
		(change.status.subject as Rename).previousPath
			? `${(change.status.subject as Rename).previousPath} →\n${change.path}`
			: undefined
	);

	const lineChangesStat = $derived.by(() => {
		if (diff && diff.type === 'Patch') {
			return {
				added: diff.subject.linesAdded,
				removed: diff.subject.linesRemoved
			};
		}
		return undefined;
	});

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
	use:stickyHeader={{
		disabled: !isHeader
	}}
	class="filelistitem-wrapper"
	class:filelistitem-header={isHeader}
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new ChangeDropData(change, idSelection, selectionId),
		viewportId: 'board-viewport',
		selector: '.selected-draggable',
		disabled: showCheckbox,
		chipType: 'file'
	}}
>
	<FileContextMenu
		bind:this={contextMenu}
		trigger={draggableEl}
		{isUncommitted}
		{isBinary}
		{unSelectChanges}
	/>

	{#if isHeader}
		<FileViewHeader
			filePath={change.path}
			fileStatus={computeChangeStatus(change)}
			draggable={!showCheckbox}
			linesAdded={lineChangesStat?.added}
			linesRemoved={lineChangesStat?.removed}
			fileStatusTooltip={previousTooltipText}
			oncontextmenu={(e) => {
				e.stopPropagation();
				e.preventDefault();
				onContextMenu(e);
			}}
			oncloseclick={onCloseClick}
		/>
	{:else}
		<FileListItemV3
			id={key({ ...selectionId, path: change.path })}
			filePath={change.path}
			fileStatus={computeChangeStatus(change)}
			{selected}
			{showCheckbox}
			fileStatusTooltip={previousTooltipText}
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
			{onclick}
			oncheck={onCheck}
			oncontextmenu={onContextMenu}
		/>
	{/if}
</div>

<style lang="postcss">
	.filelistitem-wrapper {
		display: flex;
		flex-direction: column;
	}
	.filelistitem-header {
		z-index: var(--z-lifted);
	}
</style>
