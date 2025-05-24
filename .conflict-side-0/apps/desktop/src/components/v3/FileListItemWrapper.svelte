<!-- This is a V3 replacement for `FileListItemWrapper.svelte` -->
<script lang="ts">
	import FileContextMenu from '$components/v3/FileContextMenu.svelte';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { getFilename } from '$lib/files/utils';
	import { previousPathBytesFromTreeChange, type TreeChange } from '$lib/hunks/change';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { key, type SelectionId } from '$lib/selection/key';
	import { TestId } from '$lib/testing/testIds';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getContext } from '@gitbutler/shared/context';
	import FileListItemV3 from '@gitbutler/ui/file/FileListItemV3.svelte';
	import FileViewHeader from '@gitbutler/ui/file/FileViewHeader.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import type { ConflictEntriesObj } from '$lib/files/conflicts';
	import type { Rename } from '$lib/hunks/change';
	import type { UnifiedDiff } from '$lib/hunks/diff';

	interface Props {
		projectId: string;
		stackId?: string;
		change: TreeChange;
		allChanges?: TreeChange[];
		diff?: UnifiedDiff;
		selectionId: SelectionId;
		selected?: boolean;
		isHeader?: boolean;
		active?: boolean;
		isLast?: boolean;
		listMode: 'list' | 'tree';
		linesAdded?: number;
		linesRemoved?: number;
		depth?: number;
		executable?: boolean;
		showCheckbox?: boolean;
		draggable: boolean;
		onclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		onCloseClick?: () => void;
		conflictEntries?: ConflictEntriesObj;
	}

	const {
		change,
		allChanges,
		diff,
		selectionId,
		projectId,
		stackId,
		selected,
		isHeader,
		active,
		isLast,
		listMode,
		depth,
		executable,
		showCheckbox,
		conflictEntries,
		draggable,
		onclick,
		onkeydown,
		onCloseClick
	}: Props = $props();

	const idSelection = getContext(IdSelection);
	const changeSelection = getContext(ChangeSelectionService);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();

	const selection = $derived(changeSelection.getById(change.path));
	const indeterminate = $derived(selection.current && selection.current.type === 'partial');
	const selectedChanges = $derived(idSelection.treeChanges(projectId, selectionId));
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
			changeSelection.upsert({
				type: 'full',
				path,
				pathBytes,
				previousPathBytes: previousPathBytesFromTreeChange(change)
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

	const conflict = $derived(conflictEntries ? conflictEntries.entries[change.path] : undefined);
	const draggableDisabled = $derived(!draggable || showCheckbox || selectionId.type === 'branch');
</script>

<div
	data-testid={TestId.FileListItem}
	use:stickyHeader={{
		disabled: !isHeader
	}}
	class="filelistitem-wrapper"
	class:filelistitem-header={isHeader}
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new ChangeDropData(change, idSelection, allChanges ?? [change], selectionId, stackId),
		viewportId: 'board-viewport',
		selector: '.selected-draggable',
		disabled: draggableDisabled,
		chipType: 'file'
	}}
>
	<FileContextMenu
		bind:this={contextMenu}
		trigger={draggableEl}
		{isUncommitted}
		{unSelectChanges}
	/>

	{#if isHeader}
		<FileViewHeader
			filePath={change.path}
			fileStatus={computeChangeStatus(change)}
			draggable={!showCheckbox && draggable}
			linesAdded={lineChangesStat?.added}
			linesRemoved={lineChangesStat?.removed}
			fileStatusTooltip={previousTooltipText}
			{executable}
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
			{active}
			{indeterminate}
			{isLast}
			{depth}
			{executable}
			draggable={!draggableDisabled}
			{onkeydown}
			locked={false}
			conflicted={!!conflict}
			conflictHint={conflict ? conflictEntryHint(conflict) : undefined}
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
