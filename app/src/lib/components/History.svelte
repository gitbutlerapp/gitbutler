<script lang="ts">
	import Button from './Button.svelte';
	import EmptyStatePlaceholder from './EmptyStatePlaceholder.svelte';
	import FileCard from './FileCard.svelte';
	import FullviewLoading from './FullviewLoading.svelte';
	import Icon from './Icon.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import SnapshotCard from './SnapshotCard.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { Project } from '$lib/backend/projects';
	import { clickOutside } from '$lib/clickOutside';
	import { HistoryService, createdOnDay } from '$lib/history/history';
	import { getContext } from '$lib/utils/context';
	import { RemoteFile } from '$lib/vbranches/types';
	import { plainToInstance } from 'class-transformer';
	import { createEventDispatcher } from 'svelte';
	import type { Snapshot, SnapshotDiff } from '$lib/history/types';

	const project = getContext(Project);
	const historyService = getContext(HistoryService);
	const snapshots = historyService.snapshots;
	const dispatch = createEventDispatcher<{ hide: any }>();

	const loading = historyService.loading;
	const isAllLoaded = historyService.isAllLoaded;

	let currentFilePreview: RemoteFile | undefined = undefined;

	async function onLastInView() {
		if (!$loading && !$isAllLoaded) await historyService.loadMore();
	}

	function updateFilePreview(entry: Snapshot, path: string) {
		if (!snapshotFilesTempStore) return;

		const file = snapshotFilesTempStore.diffs[path];

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
		| undefined = undefined;
	let selectedFile: { entryId: string; path: string } | undefined = undefined;
</script>

<aside class="sideview-container show-view">
	<div
		class="sideview-content-wrap show-sideview"
		use:clickOutside={{
			handler: () => dispatch('hide')
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
					on:close={() => {
						currentFilePreview = undefined;
						selectedFile = undefined;
					}}
				/>
			</div>
		{/if}

		<div class="sideview">
			<div class="sideview__header" data-tauri-drag-region>
				<i class="clock-icon">
					<div class="clock-pointers">
						<div class="clock-pointer clock-pointer-minute" />
						<div class="clock-pointer clock-pointer-hour" />
					</div>
				</i>
				<h3 class="sideview__header-title text-base-15 text-bold">Project history</h3>
				<Button
					style="ghost"
					icon="cross"
					on:click={() => {
						dispatch('hide');
					}}
				/>
			</div>

			<!-- EMPTY STATE -->
			{#if $snapshots.length == 0}
				<EmptyStatePlaceholder image={emptyFolderSvg}>
					<svelte:fragment slot="title">No snapshots yet</svelte:fragment>
					<svelte:fragment slot="caption">
						Gitbutler saves your work, including file changes, so your progress is always secure.
						Adjust snapshot settings in project settings.
					</svelte:fragment>
				</EmptyStatePlaceholder>
			{/if}

			<!-- INITIAL LOADING -->
			{#if $loading && $snapshots.length == 0}
				<FullviewLoading />
			{/if}

			<!-- SNAPSHOTS -->
			{#if $snapshots.length > 0}
				<ScrollableContainer on:bottomReached={onLastInView}>
					<div class="container">
						<!-- SNAPSHOTS FEED -->
						{#each $snapshots as entry, idx (entry.id)}
							{#if idx === 0 || createdOnDay(entry.createdAt) != createdOnDay($snapshots[idx - 1].createdAt)}
								<div class="sideview__date-header">
									<h4 class="text-base-13 text-semibold">
										{createdOnDay(entry.createdAt)}
									</h4>
								</div>
							{/if}

							{#if entry.details}
								<SnapshotCard
									{entry}
									on:restoreClick={() => {
										historyService.restoreSnapshot(project.id, entry.id);
										// In some cases, restoring the snapshot doesnt update the UI correctly
										// Until we have that figured out, we need to reload the page.
										location.reload();
									}}
									{selectedFile}
									on:diffClick={async (filePath) => {
										const path = filePath.detail;

										if (snapshotFilesTempStore?.entryId == entry.id) {
											if (selectedFile?.path == path) {
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

						<!-- LOAD MORE -->
						{#if $loading}
							<div class="load-more">
								<span class="text-base-body-13"> Loading more snapshots… </span>
							</div>
						{/if}

						<!-- ALL SNAPSHOTS LOADED -->
						{#if !$loading && $isAllLoaded}
							<div class="welcome-point">
								<div class="welcome-point__icon">
									<Icon name="finish" />
								</div>
								<div class="welcome-point__content">
									<p class="text-base-13 text-semibold">Welcome to history!</p>
									<p class="welcome-point__caption text-base-body-12">
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
		z-index: var(--z-floating);
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
		display: flex;
		transform: translateX(100%);
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
		width: 28rem;
	}

	/* SIDEVIEW HEADER */
	.sideview__header {
		display: flex;
		align-items: center;
		gap: var(--size-12);
		padding: var(--size-10) var(--size-10) var(--size-10) var(--size-12);
		border-bottom: 1px solid var(--clr-border-2);
	}

	.sideview__header-title {
		pointer-events: none;
		flex: 1;
	}

	.clock-icon {
		pointer-events: none;
		position: relative;
		width: var(--size-20);
		height: var(--size-20);
		background-color: #ffcf88;
		border-radius: var(--radius-s);
	}

	.clock-pointers {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		border-radius: 100%;
		width: 0.125rem;
		height: 0.125rem;
		background-color: #000;
	}

	.clock-pointer {
		position: absolute;
		bottom: -0.125rem;
		left: 50%;
		transform-origin: bottom;
		width: 0.125rem;
		height: calc(var(--size-12) / 2);
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
		padding: var(--size-20) var(--size-14) var(--size-14) 6.8rem;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		margin-top: var(--size-12);

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
		width: 32rem;
		border-left: 1px solid var(--clr-border-2);
	}

	/* WELCOME POINT */
	.welcome-point {
		display: flex;
		gap: var(--size-10);
		padding: var(--size-12) var(--size-16) var(--size-32) 5.3rem;
	}

	.welcome-point__content {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		margin-top: var(--size-4);
	}

	.welcome-point__caption {
		color: var(--clr-text-3);
	}

	.load-more {
		display: flex;
		justify-content: center;
		padding: var(--size-24) var(--size-14);
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
