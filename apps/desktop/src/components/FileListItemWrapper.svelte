<script lang="ts">
	import ChangedFilesContextMenu from '$components/ChangedFilesContextMenu.svelte';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { draggableChips } from '$lib/dragging/draggable';
	import { FileChangeDropData } from '$lib/dragging/draggables';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import { getFilename } from '$lib/files/utils';
	import { type TreeChange } from '$lib/hunks/change';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { key, type SelectionId } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/core/context';
	import { FileListItem, TestId } from '@gitbutler/ui';
	import { DRAG_STATE_SERVICE } from '@gitbutler/ui/drag/dragStateService.svelte';
	import { type FocusableOptions } from '@gitbutler/ui/focus/focusManager';
	import type { ConflictEntriesObj } from '$lib/files/conflicts';
	import type { Rename } from '$lib/hunks/change';

	interface Props {
		projectId: string;
		stackId?: string;
		change: TreeChange;
		selectionId: SelectionId;
		selected?: boolean;
		listMode: 'list' | 'tree';
		depth?: number;
		executable?: boolean;
		focusableOpts?: FocusableOptions;
		showCheckbox?: boolean;
		draggable?: boolean;
		active?: boolean;
		locked?: boolean;
		lockedCommitIds?: string[];
		lockedStackIds?: string[];
		onclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		conflictEntries?: ConflictEntriesObj;
		hideBorder?: boolean;
	}

	const {
		change,
		selectionId,
		projectId,
		stackId,
		selected,
		listMode,
		depth,
		executable,
		showCheckbox,
		conflictEntries,
		draggable,
		focusableOpts,
		active,
		locked,
		lockedCommitIds = [],
		lockedStackIds = [],
		onclick,
		onkeydown,
		hideBorder
	}: Props = $props();
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const userSettings = inject(SETTINGS);

	let contextMenu = $state<ReturnType<typeof ChangedFilesContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();

	const previousTooltipText = $derived(
		(change.status.subject as Rename).previousPath
			? `${(change.status.subject as Rename).previousPath} →\n${change.path}`
			: undefined
	);

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
		if (idSelection.has(change.path, selectionId) && changes.length > 0) {
			contextMenu?.open(e, { changes });
			return;
		}
		contextMenu?.open(e, { changes: [change] });
	}

	const conflict = $derived(conflictEntries ? conflictEntries.entries[change.path] : undefined);
	const draggableDisabled = $derived(!draggable || showCheckbox);

	const lockText = $derived.by(() => {
		if (!locked || lockedStackIds.length === 0) return undefined;

		const stacks = stackService.stacks(projectId).result.data ?? [];
		const stackNames = stacks
			.filter((stack) => stack.id && lockedStackIds.includes(stack.id))
			.map(getStackName);

		return stackNames.length === 1
			? `Depends on changes in:\n '${stackNames[0]}'`
			: `Depends on changes in:\n ${stackNames.join(', ')}`;
	});

	function handleLockHover() {
		lockedCommitIds.forEach((commitId) => {
			const commitRows = document.querySelectorAll(`[data-commit-id="${commitId}"]`);
			commitRows.forEach((row) => {
				row.classList.add('dependency-highlighted');
			});
		});
	}

	function handleLockUnhover() {
		const highlighted = document.querySelectorAll('.dependency-highlighted');
		highlighted.forEach((row) => {
			row.classList.remove('dependency-highlighted');
		});
	}
</script>

<div
	data-testid={TestId.FileListItem}
	class="filelistitem-wrapper"
	data-remove-from-panning
	bind:this={draggableEl}
	use:draggableChips={{
		label: getFilename(change.path),
		filePath: change.path,
		data: new FileChangeDropData(projectId, change, idSelection, selectionId, stackId || undefined),
		disabled: draggableDisabled,
		chipType: 'file',
		dropzoneRegistry,
		dragStateService
	}}
>
	<ChangedFilesContextMenu
		bind:this={contextMenu}
		{projectId}
		{stackId}
		trigger={draggableEl}
		{selectionId}
	/>
	<FileListItem
		id={key({ ...selectionId, path: change.path })}
		filePath={change.path}
		fileStatus={computeChangeStatus(change)}
		{selected}
		{showCheckbox}
		fileStatusTooltip={previousTooltipText}
		pathFirst={$userSettings.pathFirst}
		{listMode}
		checked={checkStatus.current === 'checked' || checkStatus.current === 'indeterminate'}
		{active}
		indeterminate={checkStatus.current === 'indeterminate'}
		{depth}
		{executable}
		draggable={!draggableDisabled}
		{onkeydown}
		{hideBorder}
		locked={locked || false}
		{lockText}
		onlockhover={handleLockHover}
		onlockunhover={handleLockUnhover}
		conflicted={!!conflict}
		conflictHint={conflict ? conflictEntryHint(conflict) : undefined}
		{onclick}
		oncheck={(e) => onCheck(e.currentTarget.checked)}
		oncontextmenu={onContextMenu}
		actionOpts={focusableOpts}
	/>
</div>

<style lang="postcss">
	.filelistitem-wrapper {
		/* We have two nested divs for one file, but a block within a block
		   seems fine. It seems we cannot have them both be flex boxes, it
		   makes any :hover css trigger excessive layout passes, thus making
		   the interface super slow. */
		display: block;
	}
</style>
