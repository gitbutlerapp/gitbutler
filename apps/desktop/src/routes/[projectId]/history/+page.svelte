<script lang="ts">
	import { page } from '$app/state';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FilePreviewPlaceholder from '$components/FilePreviewPlaceholder.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import Resizer from '$components/Resizer.svelte';
	import SelectionView from '$components/SelectionView.svelte';
	import SnapshotCard from '$components/SnapshotCard.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { focusable } from '$lib/focus/focusable';
	import { HISTORY_SERVICE, createdOnDay } from '$lib/history/history';
	import { ID_SELECTION } from '$lib/selection/idSelection.svelte';
	import { createSnapshotSelection, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/shared/context';
	import { EmptyStatePlaceholder, Icon } from '@gitbutler/ui';
	import type { Snapshot } from '$lib/history/types';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);

	const MIN_SNAPSHOTS_TO_LOAD = 30;

	const idSelection = inject(ID_SELECTION);

	let sidebarEl = $state<HTMLElement>();

	const historyService = inject(HISTORY_SERVICE);
	const snapshotManager = $derived(historyService.snapshots(projectId));
	const snapshots = $derived(snapshotManager.snapshots);

	const loading = $derived(snapshotManager.loading);
	const isAllLoaded = $derived(snapshotManager.isAllLoaded);

	const withinRestoreItems = $derived(findRestorationRanges($snapshots));

	let currentSelectionId: SelectionId | undefined = $state(undefined);

	// Derive selectedFile from the selection service
	const selectedFile = $derived.by(() => {
		if (!currentSelectionId) return undefined;
		const selections = idSelection.values(currentSelectionId);
		return selections.length > 0 ? selections[0] : undefined;
	});

	async function onLastInView() {
		if (!$loading && !$isAllLoaded) await snapshotManager.loadMore();
	}

	function findRestorationRanges(snapshots: Snapshot[]) {
		if (snapshots.length === 0) return [];

		const idToIndexMap = new Map<string, number>();
		snapshots.forEach((snapshot, index) => idToIndexMap.set(snapshot.id, index));

		const ranges = snapshots.flatMap((snapshot, startIndex) => {
			if (snapshot.details?.operation === 'RestoreFromSnapshot') {
				const restoredId = snapshot.details?.trailers.find((t) => t.key === 'restored_from')?.value;
				if (restoredId !== undefined) {
					const endIndex = idToIndexMap.get(restoredId);
					if (endIndex !== undefined && startIndex <= endIndex) {
						return snapshots.slice(startIndex, endIndex + 1);
					}
				}
			}
			return []; // flatMap ignores empty arrays
		});

		return ranges.map((snapshot) => snapshot.id);
	}

	async function updateFilePreview(entry: Snapshot, path: string) {
		const selectionId = createSnapshotSelection({ snapshotId: entry.id });
		// Get the diff data to find the file index
		const diffs = await historyService.getSnapshotDiff(projectId, entry.id);
		const fileIndex = diffs.findIndex((tc) => tc.path === path);
		if (fileIndex === -1) return;

		idSelection.set(path, selectionId, fileIndex);
		currentSelectionId = selectionId;
	}

	let scrollContainer: HTMLDivElement | undefined = $state();
</script>

{#snippet historyEntries()}
	<!-- EMPTY STATE -->
	{#if $snapshots.length === 0 && !$loading}
		<EmptyStatePlaceholder image={emptyFolderSvg} bottomMargin={48}>
			{#snippet title()}
				No snapshots yet
			{/snippet}
			{#snippet caption()}
				Gitbutler saves your work, including file changes, so your progress is always secure. Adjust
				snapshot settings in project settings.
			{/snippet}
		</EmptyStatePlaceholder>
	{/if}

	<!-- INITIAL LOADING -->
	{#if $loading && $snapshots.length === 0}
		<FullviewLoading />
	{/if}

	<!-- SNAPSHOTS -->
	{#if $snapshots.length > 0}
		<ScrollableContainer>
			<div class="snapshots-wrapper">
				<!-- SNAPSHOTS FEED -->
				<LazyloadContainer
					minTriggerCount={MIN_SNAPSHOTS_TO_LOAD}
					ontrigger={() => {
						onLastInView();
					}}
				>
					{#each $snapshots as entry, idx (entry.id)}
						{#if idx === 0 || createdOnDay(entry.createdAt) !== createdOnDay($snapshots[idx - 1]?.createdAt ?? new Date())}
							<div class="history-view__snapshots__date-header">
								<h4 class="text-12 text-semibold">
									{createdOnDay(entry.createdAt)}
								</h4>
							</div>
						{/if}

						{#if entry.details}
							<SnapshotCard
								{projectId}
								isWithinRestore={withinRestoreItems.includes(entry.id)}
								{entry}
								onRestoreClick={() => {
									historyService.restoreSnapshot(projectId, entry.id);
									// In some cases, restoring the snapshot doesnt update the UI correctly
									// Until we have that figured out, we need to reload the page.
									location.reload();
								}}
								onDiffClick={(path) => {
									if (
										selectedFile?.path === path &&
										selectedFile?.type === 'snapshot' &&
										selectedFile.snapshotId === entry.id
									) {
										currentSelectionId = undefined;
									} else {
										updateFilePreview(entry, path);
									}
								}}
								selectedFile={currentSelectionId && selectedFile && selectedFile.type === 'snapshot'
									? { entryId: selectedFile.snapshotId, path: selectedFile.path }
									: undefined}
							/>
						{/if}
					{/each}
				</LazyloadContainer>

				<!-- LOAD MORE -->
				{#if $loading}
					<div class="load-more">
						<span class="text-13 text-body"> Loading more snapshots… </span>
					</div>
				{/if}

				<!-- ALL SNAPSHOTS LOADED -->
				{#if (!$loading && $isAllLoaded) || $snapshots.length <= MIN_SNAPSHOTS_TO_LOAD}
					<div class="welcome-point">
						<div class="welcome-point__icon">
							<Icon name="finish" />
						</div>
						<div class="welcome-point__content">
							<p class="text-13 text-semibold">Welcome to history!</p>
							<p class="welcome-point__caption text-12 text-body">
								Gitbutler saves your work, including file changes, so your progress is always
								secure. Adjust snapshot settings in project settings.
							</p>
						</div>
					</div>
				{/if}
			</div>
		</ScrollableContainer>
	{/if}
{/snippet}

<div class="history-view">
	<div class="relative overflow-hidden radius-ml">
		<div bind:this={sidebarEl} class="history-view__snapshots" use:focusable={{ list: true }}>
			<div class="history-view__snapshots-header">
				<h3 class="history-view__snapshots-header-title text-15 text-bold">Operations history</h3>
			</div>
			{@render historyEntries()}
		</div>

		<Resizer
			viewport={sidebarEl}
			direction="right"
			minWidth={14}
			persistId="resizer-historyWidth"
			defaultValue={30}
		/>
	</div>

	<div class="history-view__preview dotted-pattern" use:focusable>
		{#if selectedFile}
			<div class="history-view__preview-file">
				<ConfigurableScrollableContainer bind:viewport={scrollContainer}>
					<SelectionView {projectId} {scrollContainer} selectionId={currentSelectionId} />
				</ConfigurableScrollableContainer>
			</div>
		{:else}
			<FilePreviewPlaceholder />
		{/if}
	</div>
</div>

<style lang="postcss">
	.history-view {
		display: flex;
		width: 100%;
		height: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.history-view__snapshots {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		min-width: 360px;
		max-width: 620px;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	/* SIDEVIEW HEADER */
	.history-view__snapshots-header {
		display: flex;
		align-items: center;
		padding: 12px 14px;
		gap: 12px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.history-view__snapshots-header-title {
		flex: 1;
		pointer-events: none;
	}

	/* DATE HEADER */
	.history-view__snapshots__date-header {
		z-index: var(--z-ground);
		position: sticky;
		top: -1px;
		padding: 10px 14px 8px 86px;
		border-top: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);

		& h4 {
			color: var(--clr-text-2);
		}

		&:first-child {
			margin-top: 0;
			border-top: none;
		}
	}

	/* WELCOME POINT */
	.welcome-point {
		display: flex;
		padding: 12px 16px 32px 86px;
		gap: 12px;
	}

	.welcome-point__content {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.welcome-point__caption {
		color: var(--clr-text-3);
	}

	/* LOAD MORE */
	.load-more {
		display: flex;
		justify-content: center;
		padding: 24px 14px;
	}

	.load-more span {
		color: var(--clr-text-3);
	}

	/* PREVIEW */
	.history-view__preview {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.history-view__preview-file {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background-color: var(--clr-bg-1);
	}
</style>
