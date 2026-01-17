<script lang="ts">
	import FileList from '$components/FileList.svelte';
	import FileListMode from '$components/FileListMode.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { readStableSelectionKey, stableSelectionKey, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/core/context';
	import { Badge, LineStats } from '@gitbutler/ui';

	import { type ComponentProps } from 'svelte';
	import type { ConflictEntriesObj } from '$lib/files/conflicts';
	import type { TreeChange, TreeStats } from '$lib/hunks/change';

	type Props = {
		projectId: string;
		stackId?: string;
		selectionId: SelectionId;
		changes: TreeChange[];
		title: string;
		stats?: TreeStats;
		conflictEntries?: ConflictEntriesObj;
		draggableFiles?: boolean;
		grow?: boolean;
		noshrink?: boolean;
		maxHeight?: string;
		topBorder?: boolean;
		bottomBorder?: boolean;
		transparentHeader?: boolean;
		resizer?: Partial<ComponentProps<typeof Resizer>>;
		autoselect?: boolean;
		ancestorMostConflictedCommitId?: string;
		ontoggle?: (collapsed: boolean) => void;
		onselect?: (change: TreeChange, index: number) => void;
		allowUnselect?: boolean;
		persistId?: string;
	};

	const {
		projectId,
		stackId,
		selectionId,
		changes,
		title,
		stats,
		conflictEntries,
		draggableFiles,
		autoselect,
		ancestorMostConflictedCommitId,
		onselect,
		allowUnselect = true,
		persistId = 'default'
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	// Turn the selection key into a string so it can be watched reactively in a consistent way.
	const stringSelectionKey = $derived(stableSelectionKey(selectionId));
	// Derive the path of the first changed file, so it can be watched reactively in a consistent way.
	const firstChangePath = $derived(changes.at(0)?.path);

	let listMode: 'list' | 'tree' = $state('tree');

	$effect(() => {
		const id = readStableSelectionKey(stringSelectionKey);
		const selection = idSelection.getById(id);
		if (firstChangePath && autoselect && selection.entries.size === 0) {
			idSelection.set(firstChangePath, selectionId, 0);
		}
	});
</script>

<div class="filelist-wrapper">
	<div class="filelist-container">
		<div class="filelist-header">
			<div class="stack-h gap-4">
				<h4 class="text-14 text-semibold truncate">{title}</h4>
				<div class="text-11 header-stats">
					<Badge>{changes.length}</Badge>
					{#if stats}
						<LineStats linesAdded={stats.linesAdded} linesRemoved={stats.linesRemoved} />
					{/if}
				</div>
			</div>

			<FileListMode bind:mode={listMode} persistId={`changed-files-${persistId}`} />
		</div>

		<FileList
			{selectionId}
			{projectId}
			{stackId}
			{changes}
			{listMode}
			{conflictEntries}
			{draggableFiles}
			{ancestorMostConflictedCommitId}
			{allowUnselect}
			{onselect}
		/>
	</div>
</div>

<style lang="postcss">
	.filelist-wrapper {
		padding: 0 10px;
	}

	.filelist-container {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.header-stats {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.filelist-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 10px 6px 14px;
		gap: 12px;
	}
</style>
