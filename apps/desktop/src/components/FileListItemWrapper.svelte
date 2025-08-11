<!-- This is a V3 replacement for `FileListItemWrapper.svelte` -->
<script lang="ts">
	import FileContextMenu from '$components/FileContextMenu.svelte';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { DRAG_STATE_SERVICE } from '$lib/dragging/dragStateService.svelte';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import { getFilename } from '$lib/files/utils';
	import { type TreeChange } from '$lib/hunks/change';
	import { ID_SELECTION } from '$lib/selection/idSelection.svelte';
	import { key, type SelectionId } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/shared/context';
	import { FileListItem, FileViewHeader, TestId } from '@gitbutler/ui';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import type { ConflictEntriesObj } from '$lib/files/conflicts';
	import type { Rename } from '$lib/hunks/change';
	import type { UnifiedDiff } from '$lib/hunks/diff';

	interface Props {
		projectId: string;
		stackId?: string;
		change: TreeChange;
		diff?: UnifiedDiff;
		selectionId: SelectionId;
		selected?: boolean;
		isHeader?: boolean;
		active?: boolean;
		listMode: 'list' | 'tree';
		linesAdded?: number;
		linesRemoved?: number;
		depth?: number;
		executable?: boolean;
		showCheckbox?: boolean;
		draggable?: boolean;
		transparent?: boolean;
		onclick?: (e: MouseEvent) => void;
		ondblclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		onCloseClick?: () => void;
		conflictEntries?: ConflictEntriesObj;
		hideBorder?: boolean;
	}

	const {
		change,
		diff,
		selectionId,
		projectId,
		stackId,
		selected,
		isHeader,
		active,
		listMode,
		depth,
		executable,
		showCheckbox,
		conflictEntries,
		draggable,
		transparent,
		onclick,
		ondblclick,
		onkeydown,
		onCloseClick,
		hideBorder
	}: Props = $props();

	const idSelection = inject(ID_SELECTION);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();

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

	function onCheck(checked: boolean) {
		if (checked) {
			uncommittedService.checkFile(stackId || null, change.path);
		} else {
			uncommittedService.uncheckFile(stackId || null, change.path);
		}
	}

	const checkStatus = $derived(uncommittedService.fileCheckStatus(stackId, change.path));

	async function onContextMenu(e: MouseEvent) {
		const changes = await idSelection.treeChanges(projectId, selectionId);
		if (idSelection.has(change.path, selectionId)) {
			contextMenu?.open(e, { changes });
			return;
		}
		contextMenu?.open(e, { changes: [change] });
	}

	const conflict = $derived(conflictEntries ? conflictEntries.entries[change.path] : undefined);
	const draggableDisabled = $derived(!draggable || showCheckbox);

	let timeoutId: any;

	$effect(() => {
		if (selected && draggableEl && active) {
			if (timeoutId) {
				clearTimeout(timeoutId);
			}
			timeoutId = setTimeout(() => {
				draggableEl?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
			}, 50);
		}
	});
</script>

<div
	data-testid={TestId.FileListItem}
	use:stickyHeader={{
		disabled: !isHeader
	}}
	class="filelistitem-wrapper"
	data-remove-from-panning
	class:filelistitem-header={isHeader}
	class:transparent
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new ChangeDropData(projectId, change, idSelection, selectionId, stackId || null),
		viewportId: 'board-viewport',
		selector: '.selected-draggable',
		disabled: draggableDisabled,
		chipType: 'file',
		dropzoneRegistry,
		dragStateService
	}}
>
	<FileContextMenu
		bind:this={contextMenu}
		{projectId}
		{stackId}
		trigger={draggableEl}
		{selectionId}
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
		<FileListItem
			id={key({ ...selectionId, path: change.path })}
			filePath={change.path}
			fileStatus={computeChangeStatus(change)}
			{selected}
			{showCheckbox}
			fileStatusTooltip={previousTooltipText}
			{listMode}
			checked={checkStatus.current === 'checked' || checkStatus.current === 'indeterminate'}
			{active}
			indeterminate={checkStatus.current === 'indeterminate'}
			{depth}
			{executable}
			draggable={!draggableDisabled}
			{onkeydown}
			{hideBorder}
			locked={false}
			conflicted={!!conflict}
			conflictHint={conflict ? conflictEntryHint(conflict) : undefined}
			{onclick}
			{ondblclick}
			oncheck={(e) => onCheck(e.currentTarget.checked)}
			oncontextmenu={onContextMenu}
		/>
	{/if}
</div>

<style lang="postcss">
	.filelistitem-wrapper {
		/* We have two nested divs for one file, but a block within a block
		   seems fine. It seems we cannot have them both be flex boxes, it
		   makes any :hover css trigger excessive layout passes, thus making
		   the interface super slow. */
		display: block;

		&.transparent {
			background-color: transparent;
		}
	}
	.filelistitem-header {
		z-index: var(--z-lifted);
		background-color: var(--clr-bg-1);
	}
</style>
