<script lang="ts">
	import FileCard from '$components/FileCard.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import ScrollableContainer from '$components/ScrollableContainer.svelte';
	import SnapshotCard from '$components/SnapshotCard.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { HistoryService, createdOnDay } from '$lib/history/history';
	import { Project } from '$lib/project/projects';
	import { RemoteFile } from '$lib/vbranches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { plainToInstance } from 'class-transformer';
	import type { Snapshot, SnapshotDiff } from '$lib/history/types';

	interface Props {
		onHide: () => void;
	}

	const { onHide }: Props = $props();

	const project = getContext(Project);
	const historyService = getContext(HistoryService);
	const snapshots = historyService.snapshots;

	const loading = historyService.loading;
	const isAllLoaded = historyService.isAllLoaded;

	let currentFilePreview: RemoteFile | undefined = $state(undefined);

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

	async function onLastInView() {
		if (!$loading && !$isAllLoaded) await historyService.loadMore();
	}

	function updateFilePreview(entry: Snapshot, path: string) {
		if (!snapshotFilesTempStore) return;

		const file = snapshotFilesTempStore.diffs[path];
		if (!file) return;

		selectedFile = {
			entryId: entry.id,
			path: path
		};

		currentFilePreview = plainToInstance(RemoteFile, {
			path: path,
			hunks: file.hunks,
			binary: file.binary
		});
	}

	let snapshotFilesTempStore:
		| { entryId: string; diffs: { [key: string]: SnapshotDiff } }
		| undefined = $state(undefined);
	let selectedFile: { entryId: string; path: string } | undefined = $state(undefined);

	const withinRestoreItems = $derived(findRestorationRanges($snapshots));
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape') {
			onHide?.();
		}
	}}
/>

<aside class="sideview-container show-view">
	<div
		class="sideview-content-wrap show-sideview"
		use:clickOutside={{
			handler: () => onHide?.()
		}}
	>
		{#if currentFilePreview}
			<div class="file-preview" class:show-file-view={currentFilePreview}>
				<FileCard
					isCard={false}
					conflicted={false}
					file={currentFilePreview}
					isUnapplied={false}
					readonly={true}
					onClose={() => {
						currentFilePreview = undefined;
						selectedFile = undefined;
					}}
				/>
			</div>
		{/if}

		<div class="sideview">
			<div class="sideview__header">
				<i class="clock-icon">
					<div class="clock-pointers">
						<div class="clock-pointer clock-pointer-minute"></div>
						<div class="clock-pointer clock-pointer-hour"></div>
					</div>
				</i>
				<h3 class="sideview__header-title text-15 text-bold">Project history</h3>
				<Button kind="ghost" icon="cross" onclick={onHide} />
			</div>

			<!-- EMPTY STATE -->
			{#if $snapshots.length === 0 && !$loading}
				<EmptyStatePlaceholder image={emptyFolderSvg} bottomMargin={48}>
					{#snippet title()}
						No snapshots yet
					{/snippet}
					{#snippet caption()}
						Gitbutler saves your work, including file changes, so your progress is always secure.
						Adjust snapshot settings in project settings.
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
					<div class="container">
						<!-- SNAPSHOTS FEED -->
						<LazyloadContainer
							minTriggerCount={30}
							ontrigger={() => {
								onLastInView();
							}}
						>
							{#each $snapshots as entry, idx (entry.id)}
								{#if idx === 0 || createdOnDay(entry.createdAt) !== createdOnDay($snapshots[idx - 1]?.createdAt ?? new Date())}
									<div class="sideview__date-header">
										<h4 class="text-13 text-semibold">
											{createdOnDay(entry.createdAt)}
										</h4>
									</div>
								{/if}

								{#if entry.details}
									<SnapshotCard
										isWithinRestore={withinRestoreItems.includes(entry.id)}
										{entry}
										onRestoreClick={() => {
											historyService.restoreSnapshot(project.id, entry.id);
											// In some cases, restoring the snapshot doesnt update the UI correctly
											// Until we have that figured out, we need to reload the page.
											location.reload();
										}}
										{selectedFile}
										onDiffClick={async (path) => {
											if (snapshotFilesTempStore?.entryId === entry.id) {
												if (selectedFile?.path === path) {
													currentFilePreview = undefined;
													selectedFile = undefined;
												} else {
													updateFilePreview(entry, path);
												}
											} else {
												snapshotFilesTempStore = {
													entryId: entry.id,
													diffs: await historyService.getSnapshotDiff(project.id, entry.id)
												};
												updateFilePreview(entry, path);
											}
										}}
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
						{#if !$loading && $isAllLoaded}
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
		</div>
	</div>
</aside>

<!-- TODO: HANDLE LOADING STATE -->

<style lang="postcss">
	.sideview-container {
		z-index: var(--z-modal);
		position: fixed;
		top: 0;
		right: 0;
		display: flex;
		justify-content: flex-end;
		height: 100%;
		width: 100%;
		background-color: var(--clr-overlay-bg);
	}

	.sideview-content-wrap {
		transform: translateX(100%);
		display: flex;
	}

	.sideview {
		position: relative;
		z-index: var(--z-lifted);
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-1);
		border-left: 1px solid var(--clr-border-2);
		width: 448px;
	}

	/* SIDEVIEW HEADER */
	.sideview__header {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 10px 10px 12px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.sideview__header-title {
		pointer-events: none;
		flex: 1;
	}

	.clock-icon {
		pointer-events: none;
		position: relative;
		width: 20px;
		height: 20px;
		background-color: #ffcf88;
		border-radius: var(--radius-s);
	}

	.clock-pointers {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		border-radius: 100%;
		width: 2px;
		height: 2px;
		background-color: #000;
	}

	.clock-pointer {
		position: absolute;
		bottom: -2px;
		left: 50%;
		transform-origin: bottom;
		width: 2px;
		height: 6px;
		background-color: #000;
	}

	.clock-pointer-minute {
		transform: translate(-50%, -50%) rotate(120deg);
		animation: minute-pointer 1s forwards;
	}

	@keyframes minute-pointer {
		0% {
			transform: translate(-50%, -50%) rotate(120deg);
		}
		100% {
			transform: translate(-50%, -50%) rotate(360deg);
		}
	}

	.clock-pointer-hour {
		transform: translate(-50%, -50%) rotate(0deg);
		animation: hour-pointer 1.5s forwards;
	}

	@keyframes hour-pointer {
		0% {
			transform: translate(-50%, -50%) rotate(0deg);
		}
		100% {
			transform: translate(-50%, -50%) rotate(90deg);
		}
	}

	/* DATE HEADER */
	.sideview__date-header {
		padding: 20px 14px 14px 114px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		margin-top: 12px;

		& h4 {
			color: var(--clr-text-3);
		}

		&:first-child {
			border-top: none;
			margin-top: 0;
		}
	}

	/* FILE PREVIEW */
	.file-preview {
		position: relative;
		z-index: var(--z-ground);
		display: flex;
		flex-direction: column;
		width: 512px;
		border-left: 1px solid var(--clr-border-2);
	}

	/* WELCOME POINT */
	.welcome-point {
		display: flex;
		gap: 10px;
		padding: 12px 16px 32px 84px;
	}

	.welcome-point__content {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 4px;
	}

	.welcome-point__caption {
		color: var(--clr-text-3);
	}

	.load-more {
		display: flex;
		justify-content: center;
		padding: 24px 14px;
	}

	.load-more span {
		color: var(--clr-text-3);
	}

	/* MODIFIERS */
	.show-view {
		animation: view-fade-in 0.3s forwards;
	}

	.show-sideview {
		animation: view-slide-in 0.35s cubic-bezier(0.23, 1, 0.32, 1) forwards;
		animation-delay: 0.05s;
	}

	@keyframes view-fade-in {
		0% {
			opacity: 0;
		}
		100% {
			opacity: 1;
		}
	}

	@keyframes view-slide-in {
		0% {
			transform: translateX(100%);
		}
		100% {
			transform: translateX(0);
		}
	}

	.show-file-view {
		animation: file-view-slide-in 0.25s cubic-bezier(0.23, 1, 0.32, 1) forwards;
	}

	@keyframes file-view-slide-in {
		0% {
			transform: translateX(100%);
		}
		100% {
			transform: translateX(0);
		}
	}
</style>
