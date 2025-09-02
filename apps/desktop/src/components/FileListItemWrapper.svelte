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
	import { inject } from '@gitbutler/core/context';
	import { FileListItem, FileViewHeader, TestId } from '@gitbutler/ui';
	import { type FocusableOptions } from '@gitbutler/ui/focus/focusManager';
	import { sticky as stickyAction } from '@gitbutler/ui/utils/sticky';
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
		focusableOpts?: FocusableOptions;
		showCheckbox?: boolean;
		draggable?: boolean;
		transparent?: boolean;
		sticky?: boolean;
		onclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		onCloseClick?: () => void;
		conflictEntries?: ConflictEntriesObj;
		scrollContainer?: HTMLDivElement;
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
		focusableOpts,
		onclick,
		onkeydown,
		onCloseClick,
		hideBorder,
		scrollContainer
	}: Props = $props();

	const idSelection = inject(ID_SELECTION);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();
	let isStuck = $state(false);

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

	const checkStatus = $derived(
		uncommittedService.fileCheckStatus(stackId || undefined, change.path)
	);

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

	// This was causing other scrolling problems. I'm really not sure if this
	// scroll behaviour belongs here.
	// let timeoutId: any;

	// $effect(() => {
	// 	if (selected && draggableEl && active) {
	// 		if (timeoutId) {
	// 			clearTimeout(timeoutId);
	// 		}
	// 		timeoutId = setTimeout(() => {
	// 			draggableEl?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
	// 		}, 50);
	// 	}
	// });
</script>

<div
	data-testid={TestId.FileListItem}
	class="filelistitem-wrapper"
	data-remove-from-panning
	class:filelistitem-header={isHeader}
	class:transparent
	class:stuck={isHeader && isStuck}
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new ChangeDropData(projectId, change, idSelection, selectionId, stackId || undefined),
		viewportId: 'board-viewport',
		selector: '.selected-draggable',
		disabled: draggableDisabled,
		chipType: 'file',
		dropzoneRegistry,
		dragStateService
	}}
	use:stickyAction={{
		enabled: isHeader,
		scrollContainer,
		onStuck: (stuck) => {
			isStuck = stuck;
		}
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
			oncheck={(e) => onCheck(e.currentTarget.checked)}
			oncontextmenu={onContextMenu}
			actionOpts={focusableOpts}
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

		&.stuck {
			border-bottom: 1px solid var(--clr-border-2);
			background-color: var(--clr-bg-1);
			box-shadow: 0 1px 8px rgba(0, 0, 0, 0.1);
		}
	}
	.filelistitem-header {
		z-index: var(--z-lifted);
	}
</style>
