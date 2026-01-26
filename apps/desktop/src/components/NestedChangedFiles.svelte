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
		onselect?: (change: TreeChange, index: number) => void;
		allowUnselect?: boolean;
		persistId?: string;
		limitPreview?: boolean;
	};

	const INITIAL_VISIBLE_COUNT = 3;

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
		persistId = 'default',
		limitPreview = false
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	// Turn the selection key into a string so it can be watched reactively in a consistent way.
	const stringSelectionKey = $derived(stableSelectionKey(selectionId));
	// Derive the path of the first changed file, so it can be watched reactively in a consistent way.
	const firstChangePath = $derived(changes.at(0)?.path);

	let listMode: 'list' | 'tree' = $state('tree');
	let folded = $state(false);
	let showAll = $state(false);

	const hasConflicts = $derived(conflictEntries && Object.keys(conflictEntries).length > 0);
	const hasMore = $derived(limitPreview && changes.length > INITIAL_VISIBLE_COUNT);
	const displayedChanges = $derived(
		limitPreview && !showAll ? changes.slice(0, INITIAL_VISIBLE_COUNT) : changes
	);

	$effect(() => {
		const id = readStableSelectionKey(stringSelectionKey);
		const selection = idSelection.getById(id);
		if (firstChangePath && autoselect && selection.entries.size === 0) {
			idSelection.set(firstChangePath, selectionId, 0);
		}
	});

	function handleSelect(change: TreeChange, index: number) {
		if (hasMore) {
			showAll = true;
		}
		onselect?.(change, index);
	}
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
					topBottomPadding={4}
					bottomMargin={24}
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
					changes={displayedChanges}
					{listMode}
					{conflictEntries}
					{draggableFiles}
					{ancestorMostConflictedCommitId}
					{allowUnselect}
					onselect={handleSelect}
				/>
				{#if hasMore}
					<button
						type="button"
						class="show-all-button text-11"
						class:showing-all={showAll}
						onclick={() => {
							showAll = !showAll;
						}}
					>
						<span>{showAll ? 'Show less' : 'Show all'}</span>
						<div class="show-all-button__chevron">
							<Icon name="chevron-down-small" />
						</div>
					</button>
				{/if}
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

	.show-all-button {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		margin-top: -12px;
		padding: 8px 10px;
		border-top: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		color: var(--clr-text-2);
		transition: color var(--transition-fast);
		&:hover {
			color: var(--clr-text-2);

			& .show-all-button__chevron {
				color: var(--clr-text-2);
			}
		}

		&:after {
			position: absolute;
			top: -1px;
			left: 0;
			width: 100%;
			height: 24px;
			transform: translateY(-100%);
			background: linear-gradient(to top, var(--clr-bg-1), transparent);
			content: '';
		}

		&.showing-all {
			margin-top: 0;

			&::after {
				display: none;
			}

			& .show-all-button__chevron {
				transform: rotate(180deg);
			}
		}
	}

	.show-all-button__chevron {
		display: flex;
		color: var(--clr-text-3);
		transition:
			transform var(--transition-fast),
			color var(--transition-fast);
	}
</style>
