<script lang="ts" context="module">
	export type Trailer = {
		key: string;
		value: string;
	};
	export type Operation =
		| 'CreateCommit'
		| 'CreateBranch'
		| 'SetBaseBranch'
		| 'MergeUpstream'
		| 'UpdateWorkspaceBase'
		| 'MoveHunk'
		| 'UpdateBranchName'
		| 'UpdateBranchNotes'
		| 'ReorderBranches'
		| 'SelectDefaultVirtualBranch'
		| 'UpdateBranchRemoteName'
		| 'GenericBranchUpdate'
		| 'DeleteBranch'
		| 'ApplyBranch'
		| 'DiscardHunk'
		| 'DiscardFile'
		| 'AmendCommit'
		| 'UndoCommit'
		| 'UnapplyBranch'
		| 'CherryPick'
		| 'SquashCommit'
		| 'UpdateCommitMessage'
		| 'MoveCommit'
		| 'RestoreFromSnapshot'
		| 'ReorderCommit'
		| 'InsertBlankCommit'
		| 'MoveCommitFile'
		| 'FileChanges';
	export type SnapshotDetails = {
		title: string;
		operation: Operation;
		body: string | undefined;
		trailers: Trailer[];
	};
	export type Snapshot = {
		id: string;
		linesAdded: number;
		linesRemoved: number;
		filesChanged: string[];
		details: SnapshotDetails | undefined;
		createdAt: number;
	};

	export function createdOnDay(dateNumber: number) {
		const d = new Date(dateNumber);
		const t = new Date();
		return `${t.toDateString() == d.toDateString() ? 'Today' : d.toLocaleDateString('en-US', { weekday: 'short' })}, ${d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`;
	}
	export type SnapshotDiff = {
		binary: boolean;
		hunks: RemoteHunk[];
		newPath: string;
		newSizeBytes: number;
		oldPath: string;
		oldSizeBytes: number;
		skipped: boolean;
	};
</script>

<script lang="ts">
	import Button from './Button.svelte';
	import EmptyStatePlaceholder from './EmptyStatePlaceholder.svelte';
	import FileCard from './FileCard.svelte';
	import FullviewLoading from './FullviewLoading.svelte';
	import Icon from './Icon.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import SnapshotCard from './SnapshotCard.svelte';
	import emptyFolderSvg from '$lib/assets/empty-state/empty-folder.svg?raw';
	import { invoke, listen } from '$lib/backend/ipc';
	import { clickOutside } from '$lib/clickOutside';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContext, getContextStoreBySymbol } from '$lib/utils/context';
	import { type RemoteHunk, RemoteFile } from '$lib/vbranches/types';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { plainToInstance } from 'class-transformer';
	import { onMount, onDestroy } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { goto } from '$app/navigation';

	export let projectId: string;
	let currentFilePreview: RemoteFile | undefined = undefined;

	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	const vbranchService = getContext(VirtualBranchService);

	let listElement: HTMLElement | undefined = undefined;

	// TODO: Fires multiple times nad cause uninitialized variable error
	vbranchService.activeBranches.subscribe(() => {
		// whenever virtual branches change, we need to reload the snapshots
		// TODO: if the list has results from more pages, merge into it?
		listSnapshots(projectId)
			.then((rsp) => {
				snapshots = rsp;
			})
			.catch((error) => {
				console.error('Error occurred while listing snapshots:', error);
			});
	});

	let snapshots: Snapshot[] = [];
	let isSnapshotsLoading = false;

	async function listSnapshots(projectId: string, sha?: string) {
		isSnapshotsLoading = true;

		const resp = await invoke<Snapshot[]>('list_snapshots', {
			projectId: projectId,
			limit: 32,
			sha: sha
		});

		isSnapshotsLoading = false;
		return resp;
	}

	async function getSnapshotDiff(projectId: string, sha: string) {
		const resp = await invoke<{ [key: string]: SnapshotDiff }>('snapshot_diff', {
			projectId: projectId,
			sha: sha
		});
		return resp;
	}

	async function restoreSnapshot(projectId: string, sha: string) {
		await invoke<string>('restore_snapshot', {
			projectId: projectId,
			sha: sha
		});

		await listSnapshots(projectId).then((rsp) => {
			snapshots = rsp;
		});

		// TODO: is there a better way to update all the state?
		await goto(window.location.href, { replaceState: true });
	}

	function onLastInView() {
		if (!listElement) return;
		if (listElement.scrollTop + listElement.clientHeight >= listElement.scrollHeight) {
			listSnapshots(projectId, snapshots[snapshots.length - 1].id).then((rsp) => {
				snapshots = [...snapshots, ...rsp.slice(1)];
			});
		}
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

	function closeView() {
		userSettings.update((s) => ({
			...s,
			showHistoryView: false
		}));
	}

	onMount(async () => {
		if (listElement) listElement.addEventListener('scroll', onLastInView, true);
	});
	onMount(() => {
		const unsubscribe = listen<string>('menu://view/history/clicked', () => {
			userSettings.update((s) => ({
				...s,
				showHistoryView: !$userSettings.showHistoryView
			}));
		});

		return () => {
			unsubscribe();
		};
	});
	onDestroy(() => {
		listElement?.removeEventListener('scroll', onLastInView, true);
	});

	// optimisation: don't fetch snapshots if the view is not visible
	$: if (!$userSettings.showHistoryView) {
		snapshots = [];
		currentFilePreview = undefined;
		selectedFile = undefined;
	} else {
		listSnapshots(projectId).then((rsp) => {
			snapshots = rsp;
		});
	}

	let snapshotFilesTempStore:
		| { entryId: string; diffs: { [key: string]: SnapshotDiff } }
		| undefined = undefined;
	let selectedFile: { entryId: string; path: string } | undefined = undefined;

	$: if (snapshotFilesTempStore) {
		console.log(snapshotFilesTempStore);
	}
</script>

{#if $userSettings.showHistoryView}
	<aside class="sideview-container" class:show-view={$userSettings.showHistoryView}>
		<div
			class="sideview-content-wrap"
			use:clickOutside={{
				handler: () => {
					closeView();
				}
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
						<div class="clock-pointers" />
					</i>
					<h3 class="sideview__header-title text-base-15 text-bold">Project history</h3>
					<Button
						style="ghost"
						icon="cross"
						on:click={() => {
							closeView();
						}}
					/>
				</div>

				<!-- EMPTY STATE -->
				{#if snapshots.length == 0}
					<EmptyStatePlaceholder image={emptyFolderSvg}>
						<svelte:fragment slot="title">No snapshots yet</svelte:fragment>
						<svelte:fragment slot="caption">
							Gitbutler saves your work, including file changes, so your progress is always secure.
							Adjust snapshot settings in project settings.
						</svelte:fragment>
					</EmptyStatePlaceholder>
				{/if}

				{#if isSnapshotsLoading}
					<FullviewLoading />
				{/if}

				<!-- SNAPSHOTS -->
				{#if snapshots.length > 0 && !isSnapshotsLoading}
					<ScrollableContainer on:bottomReached={onLastInView}>
						<div class="container" bind:this={listElement}>
							<!-- SNAPSHOTS FEED -->
							{#each snapshots as entry, idx}
								{#if idx === 0 || createdOnDay(entry.createdAt) != createdOnDay(snapshots[idx - 1].createdAt)}
									<div class="sideview__date-header">
										<h4 class="text-base-12 text-semibold">
											{createdOnDay(entry.createdAt)}
										</h4>
									</div>
								{/if}

								{#if entry.details}
									<SnapshotCard
										{entry}
										isCurrent={idx == 0}
										on:restoreClick={() => {
											restoreSnapshot(projectId, entry.id);
										}}
										{selectedFile}
										on:diffClick={async (filePath) => {
											const path = filePath.detail;

											if (snapshotFilesTempStore?.entryId == entry.id) {
												updateFilePreview(entry, path);
											} else {
												snapshotFilesTempStore = {
													entryId: entry.id,
													diffs: await getSnapshotDiff(projectId, entry.id)
												};
												updateFilePreview(entry, path);
											}
										}}
									/>
								{/if}
							{/each}

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
						</div>
					</ScrollableContainer>
				{/if}
			</div>
		</div>
	</aside>
{/if}

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
		width: 26rem;
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

		&::before,
		&::after {
			content: '';
			position: absolute;
			bottom: -0.125rem;
			left: 50%;
			transform: translate(-50%, -50%);
			transform-origin: bottom;
			width: 0.125rem;
			height: calc(var(--size-12) / 2);
			background-color: #000;
		}

		&::before {
			transform: translate(-50%, -50%) rotate(120deg);
			animation: minute-pointer 1s forwards;
		}

		&::after {
			transform: translate(-50%, -50%) rotate(0deg);
			animation: hour-pointer 1.5s forwards;
		}
	}

	@keyframes minute-pointer {
		0% {
			transform: translate(-50%, -50%) rotate(120deg);
		}
		100% {
			transform: translate(-50%, -50%) rotate(360deg);
		}
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
		padding: var(--size-14) var(--size-14) var(--size-8) 5.25rem;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);

		& h4 {
			color: var(--clr-text-3);
		}

		&:first-child {
			border-top: none;
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
		padding: var(--size-12) var(--size-16) var(--size-16) 3.7rem;
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

	/* MODIFIERS */
	.show-view {
		animation: view-fade-in 0.2s forwards;

		& .sideview-content-wrap {
			animation: view-slide-in 0.25s cubic-bezier(0.23, 1, 0.32, 1) forwards;
			animation-delay: 0.05s;
		}
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
