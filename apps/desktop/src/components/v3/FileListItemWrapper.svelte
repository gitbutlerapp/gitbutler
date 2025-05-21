<!-- This is a V3 replacement for `FileListItemWrapper.svelte` -->
<script lang="ts">
	import FileContextMenu from '$components/v3/FileContextMenu.svelte';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { draggableChips } from '$lib/dragging/draggable';
	import { ChangeDropData } from '$lib/dragging/draggables';
	import { getFilename } from '$lib/files/utils';
	import { previousPathBytesFromTreeChange, type TreeChange } from '$lib/hunks/change';
	import {
		DiffService,
		hunkGroupToKey,
		type HunkAssignments,
		type HunkGroup
	} from '$lib/hunks/diffService.svelte';
	import { hunkHeaderEquals, type HunkHeader } from '$lib/hunks/hunk';
	import { ChangeSelectionService, type SelectedHunk } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { key, type SelectionId } from '$lib/selection/key';
	import { TestId } from '$lib/testing/testIds';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
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
		group?: HunkGroup;
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
		group,
		onclick,
		onkeydown,
		onCloseClick
	}: Props = $props();

	const idSelection = getContext(IdSelection);
	const changeSelection = getContext(ChangeSelectionService);
	const worktreeService = getContext(WorktreeService);
	const diffService = getContext(DiffService);

	let contextMenu = $state<ReturnType<typeof FileContextMenu>>();
	let draggableEl: HTMLDivElement | undefined = $state();

	const changesKeyResult = $derived(worktreeService.getChangesKey(projectId));
	const assignments = $derived.by(() => {
		if (selectionId.type !== 'worktree') return;

		return changesKeyResult.current
			? diffService.hunkAssignments(projectId, changesKeyResult.current)
			: undefined;
	});
	const selection = $derived(changeSelection.getById(change.path));
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

	function allAssignedToCurrentGroup(
		assignments: HunkAssignments,
		selectionId: SelectionId & { type: 'worktree' }
	): boolean {
		for (const [key, value] of assignments.entries()) {
			if (key === hunkGroupToKey(selectionId.group)) continue;

			if (value.has(change.path)) return false;
		}

		return true;
	}

	function allHunksForPath(assignments: HunkAssignments, except?: HunkGroup): HunkHeader[] {
		const headers = [];

		for (const [key, value] of assignments.entries()) {
			if (except) {
				if (key === hunkGroupToKey(except)) continue;
			}

			const assignments = value.get(change.path);
			if (!assignments) continue;
			headers.push(...assignments.map((assignment) => assignment.hunkHeader));
		}

		return headers;
	}

	function relevantHunkHeaders(
		assignments: HunkAssignments,
		selectionId: SelectionId & { type: 'worktree' }
	): HunkHeader[] {
		const stackGroup = assignments.get(hunkGroupToKey(selectionId.group));
		if (!stackGroup) return [];
		const hunkAssignments = stackGroup.get(change.path);

		return hunkAssignments?.map((assignment) => assignment.hunkHeader) ?? [];
	}

	function onCheck() {
		// Currently selection is only implemented for the worktree changes.
		if (selectionId.type !== 'worktree') return;
		if (!assignments?.current?.data) return;

		// TODO: wtf do you do in the case where a whole group is selected, but
		// a differently grouped hunk appears due to a disk change????
		if (allAssignedToCurrentGroup(assignments.current.data, selectionId)) {
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
		} else {
			// Handle selection/deselection when not all the hunks are assigned
			// to the one group.
			const relevantHeaders = relevantHunkHeaders(assignments.current.data, selectionId);
			const { path, pathBytes } = change;
			if (selection.current?.type === 'full') {
				// Currently, all the hunks are selected; we want to replace the
				// current selection with one that contains all the hunk hunk
				// headers for this path _except_ for those in the current group
				const hunkHeadersWithoutGroup = allHunksForPath(
					assignments.current.data,
					selectionId.group
				);

				changeSelection.upsert({
					type: 'partial',
					path,
					pathBytes,
					previousPathBytes: previousPathBytesFromTreeChange(change),
					hunks: hunkHeadersWithoutGroup.map((header) => ({ ...header, type: 'full' }))
				});
			} else if (selection.current?.type === 'partial') {
				const selectedHunks = selection.current.hunks;
				// Handle selection/deselection when _some_ of the changes are
				// selected for committing. We want to deselect if there are any
				// hunks associated to the group already selected.

				// TODO: Look at optimizing some of these N+1s
				const includesRelevantHunks = selectedHunks.some((hunk) =>
					relevantHeaders.some((header) => hunkHeaderEquals(header, hunk))
				);

				if (includesRelevantHunks) {
					// We have found some selected hunks that are in the current
					// group, so we want to filter them out of the current
					// selection.
					const selectionWithoutOwnedHunks = selectedHunks.filter(
						(hunk) => !relevantHeaders.some((header) => hunkHeaderEquals(header, hunk))
					);
					if (selectionWithoutOwnedHunks.length === 0) {
						// If there are no hunks left in the selection, then we
						// can just clear the selection.
						changeSelection.remove(path);
					} else {
						changeSelection.upsert({
							type: 'partial',
							path,
							pathBytes,
							previousPathBytes: previousPathBytesFromTreeChange(change),
							hunks: selectionWithoutOwnedHunks
						});
					}
				} else {
					// We want to add the hunks that are associated with the
					// current group to the selection.
					const hunks: SelectedHunk[] = relevantHeaders.map((header) => ({
						...header,
						type: 'full'
					}));
					const combinedHunks = [...selection.current.hunks, ...hunks];
					const allHunks = allHunksForPath(assignments.current.data);

					if (combinedHunks.length === allHunks.length) {
						// If all the hunks have ended up selected, let's
						// replace it with a "full" selection.
						changeSelection.upsert({
							type: 'full',
							path,
							pathBytes,
							previousPathBytes: previousPathBytesFromTreeChange(change)
						});
					} else {
						changeSelection.upsert({
							type: 'partial',
							path,
							pathBytes,
							previousPathBytes: previousPathBytesFromTreeChange(change),
							hunks: [...selection.current.hunks, ...hunks]
						});
					}
				}
			} else {
				// There is no existing selection so we can simply select all
				// the hunks belonging to the group
				changeSelection.upsert({
					type: 'partial',
					path,
					pathBytes,
					previousPathBytes: previousPathBytesFromTreeChange(change),
					hunks: relevantHeaders.map((header) => ({ ...header, type: 'full' }))
				});
			}
		}
	}

	const checkStatus = $derived.by((): 'checked' | 'indeterminate' | 'unchecked' => {
		// Currently selection is only implemented for the worktree changes.
		if (selectionId.type !== 'worktree') return 'unchecked';
		if (!assignments?.current?.data) return 'unchecked';
		const currentSelection = selection.current;
		if (!currentSelection) return 'unchecked';

		if (currentSelection.type === 'full') {
			// If the selection type is "full", then we can assume that since
			// this is rendered in the first place that it should indeed be
			// checked.
			return 'checked';
		}

		const relevantHeaders = relevantHunkHeaders(assignments.current.data, selectionId);

		const includesRelevantHunks = currentSelection.hunks.some((hunk) =>
			relevantHeaders.some((header) => hunkHeaderEquals(header, hunk))
		);

		if (includesRelevantHunks) {
			const includesAllRelevantHunks = relevantHeaders.every((hunk) =>
				currentSelection.hunks.some((header) => hunkHeaderEquals(header, hunk))
			);
			if (includesAllRelevantHunks) {
				return 'checked';
			} else {
				return 'indeterminate';
			}
		}

		return 'unchecked';
	});

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
		data: new ChangeDropData(
			change,
			idSelection,
			allChanges ?? [change],
			selectionId,
			stackId,
			group
		),
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
			checked={checkStatus === 'checked' || checkStatus === 'indeterminate'}
			{active}
			indeterminate={checkStatus === 'indeterminate'}
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
