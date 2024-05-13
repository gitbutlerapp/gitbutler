<script lang="ts">
	import Button from './Button.svelte';
	import FileCard from './FileCard.svelte';
	import FileStatusCircle from './FileStatusCircle.svelte';
	import Icon from './Icon.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import SnapshotAttachment from './SnapshotAttachment.svelte';
	import SnapshotCard from './SnapshotCard.svelte';
	import Tag from './Tag.svelte';
	import { invoke } from '$lib/backend/ipc';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { getContext } from '$lib/utils/context';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import is from 'date-fns/locale/is';
	import { onMount, onDestroy } from 'svelte';
	import type { AnyFile } from '$lib/vbranches/types';

	export let projectId: string;

	const vbranchService = getContext(VirtualBranchService);
	vbranchService.activeBranches.subscribe(() => {
		// whenever virtual branches change, we need to reload the snapshots
		// TODO: if the list has results from more pages, merge into it?
		listSnapshots(projectId).then((rsp) => {
			snapshots = rsp;
			listElement?.scrollTo(0, 0);
		});
	});

	type Trailer = {
		key: string;
		value: string;
	};
	type Operation =
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
	type SnapshotDetails = {
		title: string;
		operation: Operation;
		body: string | undefined;
		trailers: Trailer[];
	};
	type Snapshot = {
		id: string;
		linesAdded: number;
		linesRemoved: number;
		filesChanged: string[];
		details: SnapshotDetails | undefined;
		createdAt: number;
	};
	let snapshots: Snapshot[] = [];
	async function listSnapshots(projectId: string, sha?: string) {
		const resp = await invoke<Snapshot[]>('list_snapshots', {
			projectId: projectId,
			limit: 32,
			sha: sha
		});
		return resp;
	}
	async function getSnapshotDiff(projectId: string, sha: string) {
		const resp = await invoke<{ [key: string]: AnyFile }>('snapshot_diff', {
			projectId: projectId,
			sha: sha
		});
		// console.log(resp);
		return resp;
	}

	// async function restoreSnapshot(projectId: string, sha: string) {
	// 	await invoke<string>('restore_snapshot', {
	// 		projectId: projectId,
	// 		sha: sha
	// 	});
	// 	// TODO: is there a better way to update all the state?
	// 	await goto(window.location.href, { replaceState: true });
	// }

	function onLastInView() {
		if (!listElement) return;
		if (listElement.scrollTop + listElement.clientHeight >= listElement.scrollHeight) {
			listSnapshots(projectId, snapshots[snapshots.length - 1].id).then((rsp) => {
				snapshots = [...snapshots, ...rsp.slice(1)];
			});
		}
	}
	let listElement: HTMLElement | undefined = undefined;

	// Handle formatting
	function createdOnDay(dateNumber: number) {
		const d = new Date(dateNumber);
		const t = new Date();
		return `${t.toDateString() == d.toDateString() ? 'Today' : d.toLocaleDateString('en-US', { weekday: 'short' })}, ${d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`;
	}

	function mapOperationToText(snapshotDetails: SnapshotDetails) {
		// console.log('snapshotDetails', snapshotDetails);

		switch (snapshotDetails.operation) {
			case 'CreateCommit':
				return 'Commit';
			case 'CreateBranch':
				return 'Create branch';
			case 'SetBaseBranch':
				return 'Set base branch';
			case 'MergeUpstream':
				return 'Merge upstream';
			case 'UpdateWorkspaceBase':
				return 'Update workspace base';
			case 'MoveHunk':
				return 'Move hunk';
			case 'UpdateBranchName':
				return `Branch "${snapshotDetails.trailers.find((t) => t.key === 'before')?.value}" renamed to "${snapshotDetails.trailers.find((t) => t.key === 'after')?.value}"`;
			case 'UpdateBranchNotes':
				return 'Update branch notes';
			case 'ReorderBranches':
				return 'Reorder branches';
			case 'SelectDefaultVirtualBranch':
				return 'Select default virtual branch';
			case 'UpdateBranchRemoteName':
				return 'Update branch remote name';
			case 'GenericBranchUpdate':
				return 'Generic branch update';
			case 'DeleteBranch':
				return `Delete branch "${snapshotDetails.trailers.find((t) => t.key == 'name')?.value}"`;
			case 'ApplyBranch':
				return 'Apply branch';
			case 'DiscardHunk':
				return 'Discard hunk';
			case 'DiscardFile':
				return 'Discard file';
			case 'AmendCommit':
				return 'Amend commit';
			case 'UndoCommit':
				return 'Undo commit';
			case 'UnapplyBranch':
				return 'Unapply branch';
			case 'CherryPick':
				return 'Cherry pick';
			case 'SquashCommit':
				return 'Squash commit';
			case 'UpdateCommitMessage':
				return 'Update commit message';
			case 'MoveCommit':
				return 'Move commit';
			case 'RestoreFromSnapshot':
				return 'Restore from snapshot';
			case 'ReorderCommit':
				return 'Reorder commit';
			case 'InsertBlankCommit':
				return 'Insert blank commit';
			case 'MoveCommitFile':
				return 'Move commit file';
			case 'FileChanges':
				return 'File changes';
			default:
				return snapshotDetails.operation;
		}
	}

	onMount(async () => {
		listSnapshots(projectId).then((rsp) => {
			snapshots = rsp;
		});
		if (listElement) listElement.addEventListener('scroll', onLastInView, true);
	});
	onDestroy(() => {
		listElement?.removeEventListener('scroll', onLastInView, true);
	});

	// let currentFilePreview: AnyFile | undefined = undefined;

	$: console.log(snapshots);
</script>

<aside class="sideview">
	<!-- <div class="file-preview">
		{#if currentFilePreview}
			<FileCard
				conflicted={false}
				file={currentFilePreview}
				isUnapplied={false}
				readonly={true}
				on:close={() => {
					currentFilePreview = undefined;
				}}
			/>
		{/if}
	</div> -->

	<div class="sideview-wrap">
		<div class="sideview__header">
			<i class="clock-icon">
				<div class="clock-pointers" />
			</i>
			<h3 class="sideview__header-title text-base-15 text-bold">Time machine</h3>
			<Button
				style="ghost"
				icon="cross"
				on:click={() => {
					console.log('close');
				}}
			/>
		</div>

		<ScrollableContainer>
			<div class="container" bind:this={listElement}>
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
							entry={{
								isCurrent: idx == 0,
								createdAt: entry.createdAt,
								id: entry.id,
								filesChanged: entry.filesChanged,
								title: mapOperationToText(entry.details)
							}}
						/>
						<!-- <div class="snapshot-card">
							<span class="snapshot-time text-base-12">
								{createdOnTime(entry.createdAt)}
							</span>

							<div class="snapshot-line">
								<Icon name="commit" />
							</div>

							<div class="snapshot-content">
								<div class="snapshot-details">
									{#if idx == 0}
										<Tag style="pop" kind="soft">Current</Tag>
									{/if}

									<div class="snapshot-title-wrap">
										<h4 class="snapshot-title text-base-body-13 text-semibold">
											<span>{mapOperationToText(entry.details)}</span>
											<span class="snapshot-sha">
												• #{entry.id.slice(0, 6)}</span
											>
										</h4>

										<div class="restore-btn"><Tag>Resotre</Tag></div>
									</div>
								</div>

								{#if entry.filesChanged.length > 0}
									<SnapshotAttachment
										foldable={entry.filesChanged.length > 2}
										foldedAmount={entry.filesChanged.length - 2}
									>
										<div class="changed-files-list">
											{#each entry.filesChanged as filePath}
												<button
													class="snapshot-file"
													on:click={async () => {
														const allDiffs = await getSnapshotDiff(
															projectId,
															entry.id
														);

														// get the file by path
														currentFilePreview = allDiffs[filePath];
														console.log(currentFilePreview);
													}}
												>
													<img
														draggable="false"
														class="file-icon"
														src={getVSIFileIcon(filePath)}
														alt=""
													/>
													<div class="text-base-12 file-path-and-name">
														<span class="file-name">
															{filePath.split('/').pop()}
														</span>
														<span class="file-path">
															{filePath.replace(/\/[^/]+$/, '')}
														</span>
													</div>
												</button>
											{/each}
										</div>
									</SnapshotAttachment>
								{/if}
							</div>
						</div> -->
					{/if}
				{/each}
			</div>
		</ScrollableContainer>
	</div>
</aside>

<style lang="postcss">
	.sideview {
		position: relative;
		display: flex;
		height: 100%;
	}

	.sideview-wrap {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-1);
		border-left: 1px solid var(--clr-border-2);
		min-width: 420px;
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
		flex: 1;
	}

	.clock-icon {
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
			transform: translate(-50%, -50%) rotate(-900deg);
			animation: hour-pointer 1.5s forwards;
		}

		&::after {
			transform: translate(-50%, -50%) rotate(-90deg);
			animation: minute-pointer 1.5s forwards;
		}
	}

	@keyframes hour-pointer {
		0% {
			transform: translate(-50%, -50%) rotate(0deg);
		}
		100% {
			transform: translate(-50%, -50%) rotate(360deg);
		}
	}

	@keyframes minute-pointer {
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
		margin-top: -1px;

		& h4 {
			color: var(--clr-text-3);
		}
	}

	/* FILE PREVIEW */
	/* .file-preview {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
		padding: var(--size-14);
		background-color: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
	} */
</style>
