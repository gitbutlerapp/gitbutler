<script lang="ts">
	import FileList from '$components/FileList.svelte';
	import FileListMode from '$components/FileListMode.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { readStableSelectionKey, stableSelectionKey, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/core/context';
	import { Badge, LineStats, EmptyStatePlaceholder, Icon } from '@gitbutler/ui';

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
		autoselect?: boolean;
		ancestorMostConflictedCommitId?: string;
		onFileClick?: (index: number) => void;
		allowUnselect?: boolean;
		persistId?: string;
		foldedByDefault?: boolean;
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
		onFileClick,
		allowUnselect = true,
		persistId = 'default',
		foldedByDefault = false
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	// Turn the selection key into a string so it can be watched reactively in a consistent way.
	const stringSelectionKey = $derived(stableSelectionKey(selectionId));
	// Derive the path of the first changed file, so it can be watched reactively in a consistent way.
	const firstChangePath = $derived(changes.at(0)?.path);

	let listMode: 'list' | 'tree' = $state('tree');
	const hasConflicts = $derived(conflictEntries && Object.keys(conflictEntries).length > 0);
	let folded = $state(foldedByDefault);

	$effect(() => {
		if (changes.length === 0 && !hasConflicts) {
			folded = true;
		}
	});

	$effect(() => {
		const id = readStableSelectionKey(stringSelectionKey);
		const selection = idSelection.getById(id);
		if (firstChangePath && autoselect && selection.entries.size === 0) {
			idSelection.set(firstChangePath, selectionId, 0);
		}
	});
</script>

<div role="presentation" class="filelist-wrapper">
	<div class="filelist-container">
		<div
			class="filelist-header"
			class:folded
			role="presentation"
			ondblclick={() => {
				folded = !folded;
			}}
		>
			<div class="stack-h gap-4">
				<button
					type="button"
					class="filelist-header__chevron"
					class:rotated={folded}
					onclick={(e) => {
						e.stopPropagation();
						folded = !folded;
					}}
					aria-label="Toggle file list"
					aria-expanded={!folded}
				>
					<Icon name="chevron-down" />
				</button>
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

		{#if !folded}
			{#if changes.length === 0 && !hasConflicts}
				<EmptyStatePlaceholder
					image={emptyFolderSvg}
					gap={0}
					topBottomPadding={14}
					bottomMargin={20}
				>
					{#snippet caption()}
						No files changed
					{/snippet}
				</EmptyStatePlaceholder>
			{:else}
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
					{onFileClick}
				/>
			{/if}
		{/if}
	</div>
</div>

<style lang="postcss">
	.filelist-wrapper {
		width: 100%;
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
		padding: 10px 10px 6px 10px;
		gap: 12px;

		&.folded {
			padding-bottom: 10px;
		}
	}

	.filelist-header__chevron {
		display: flex;
		color: var(--clr-text-3);
		transition:
			color var(--transition-fast),
			transform var(--transition-medium);

		&.rotated {
			transform: rotate(-90deg);
		}

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
