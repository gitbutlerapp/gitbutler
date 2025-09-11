<script lang="ts">
	import Drawer from '$components/Drawer.svelte';
	import FileList from '$components/FileList.svelte';
	import FileListMode from '$components/FileListMode.svelte';
	import Resizer from '$components/Resizer.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { readStableSelectionKey, stableSelectionKey, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/core/context';
	import { Badge, EmptyStatePlaceholder, LineStats } from '@gitbutler/ui';

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
		bottomBorder?: boolean;
		resizer?: Partial<ComponentProps<typeof Resizer>>;
		autoselect?: boolean;
		ancestorMostConflictedCommitId?: string;
		ontoggle?: (collapsed: boolean) => void;
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
		grow,
		noshrink,
		bottomBorder,
		resizer,
		autoselect,
		ancestorMostConflictedCommitId,
		ontoggle
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

<Drawer {grow} {ontoggle} {resizer} {noshrink} bottomBorder={changes.length > 0}>
	{#snippet header()}
		<h4 class="text-14 text-semibold truncate">{title}</h4>
		<div class="text-11 header-stats">
			<Badge>{changes.length}</Badge>
			{#if stats}
				<LineStats linesAdded={stats.linesAdded} linesRemoved={stats.linesRemoved} />
			{/if}
		</div>
	{/snippet}
	{#snippet extraActions()}
		<FileListMode bind:mode={listMode} persist="committed" />
	{/snippet}

	<div class="filelist-wrapper" class:bottom-border={bottomBorder}>
		{#if changes.length > 0}
			<FileList
				{selectionId}
				{projectId}
				{stackId}
				{changes}
				{listMode}
				{conflictEntries}
				{draggableFiles}
				{ancestorMostConflictedCommitId}
				hideLastFileBorder={false}
			/>
		{:else}
			<EmptyStatePlaceholder image={emptyFolderSvg} width={180} gap={4}>
				{#snippet caption()}
					No files changed
				{/snippet}
			</EmptyStatePlaceholder>
		{/if}
	</div>
</Drawer>

<style lang="postcss">
	.header-stats {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.filelist-wrapper {
		display: flex;
		flex-direction: column;
		margin-bottom: 16px;
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}
</style>
