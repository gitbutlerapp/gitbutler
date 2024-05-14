<script lang="ts">
	import Icon from './Icon.svelte';
	import SnapshotAttachment from './SnapshotAttachment.svelte';
	import Tag from './Tag.svelte';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { createEventDispatcher } from 'svelte';
	import type { Snapshot, SnapshotDetails } from './HistoryNew.svelte';

	export let entry: Snapshot;
	export let isCurrent: boolean = false;
	export let isFaded: boolean = false;

	function getShortSha(sha: string | undefined) {
		if (!sha) return '';

		return `#${sha.slice(0, 7)}`;
	}

	function createdOnTime(dateNumber: number) {
		const d = new Date(dateNumber);
		return d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
	}

	const dispatch = createEventDispatcher<{ restoreClick: void; diffClick: string }>();

	function mapOperationToText(snapshotDetails: SnapshotDetails | undefined) {
		if (!snapshotDetails) return '';

		switch (snapshotDetails.operation) {
			// Branch operations
			case 'DeleteBranch':
				return `Delete branch "${entry.details?.trailers.find((t) => t.key === 'name')?.value}"`;
			case 'ApplyBranch':
				return `Apply branch "${entry.details?.trailers.find((t) => t.key === 'name')?.value}"`;
			case 'UpdateBranchName':
				return `Branch "${snapshotDetails.trailers.find((t) => t.key === 'before')?.value}" renamed to "${snapshotDetails.trailers.find((t) => t.key === 'after')?.value}"`;
			case 'CreateBranch':
				return `Create branch "${snapshotDetails.trailers.find((t) => t.key == 'name')?.value}"`;
			case 'UnapplyBranch':
				return `Unapply branch "${snapshotDetails.trailers.find((t) => t.key === 'name')?.value}"`;
			case 'SetBaseBranch':
				return 'Set base branch';
			case 'ReorderBranches':
				return 'Reorder branches';
			case 'SelectDefaultVirtualBranch':
				return 'Select default virtual branch';
			case 'UpdateBranchRemoteName':
				return 'Update branch remote name';
			case 'GenericBranchUpdate':
				return 'Generic branch update';
			// Commit operations
			case 'CreateCommit':
				return 'Commit';
			case 'AmendCommit':
				return 'Amend commit';
			case 'UndoCommit':
				return 'Undo commit';
			case 'SquashCommit':
				return 'Squash commit';
			case 'UpdateCommitMessage':
				return 'Update commit message';
			case 'MoveCommit':
				return 'Move commit';
			case 'ReorderCommit':
				return 'Reorder commit';
			case 'InsertBlankCommit':
				return 'Insert blank commit';
			case 'MoveCommitFile':
				return 'Move commit file';
			// File operations
			case 'MoveHunk':
				return `Move hunk to "${entry.details?.trailers.find((t) => t.key === 'name')?.value}"`;
			case 'DiscardHunk':
				return 'Discard hunk';
			case 'DiscardFile':
				return 'Discard file';
			case 'FileChanges':
				return 'File changes';
			// Other operations
			case 'MergeUpstream':
				return 'Merge upstream';
			case 'UpdateWorkspaceBase':
				return 'Update workspace base';
			case 'RestoreFromSnapshot':
				return 'Restore from snapshot';
			default:
				return snapshotDetails.operation;
		}
	}

	const isRestoreSnapshot = entry.details?.operation === 'RestoreFromSnapshot';

	function isRestorable() {
		return !isCurrent && !isRestoreSnapshot;
	}

	function getPathOnly(path: string) {
		return path.split('/').slice(0, -1).join('/');
	}
</script>

<div
	class="snapshot-card"
	class:restore-btn_visible={isRestorable()}
	class:restored-snapshot={isRestoreSnapshot}
>
	<span class="snapshot-time text-base-12">
		{createdOnTime(entry.createdAt)}
	</span>

	<div class="snapshot-line">
		{#if isRestoreSnapshot}
			<img src="/images/history/restore-icon.svg" alt="" />
		{:else}
			<Icon name="commit" />
		{/if}
	</div>

	<div class="snapshot-content">
		{#if isFaded}
			<span>faded</span>
		{/if}

		<div class="snapshot-details">
			{#if isCurrent}
				<div class="current-tag">
					<Tag style="pop" kind="soft">Current</Tag>
				</div>
			{/if}

			<div class="snapshot-title-wrap">
				<h4 class="snapshot-title text-base-body-13 text-semibold">
					<span>{mapOperationToText(entry.details)}</span>
					<span class="snapshot-sha text-base-body-12"> • {getShortSha(entry.id)}</span>
				</h4>

				{#if isRestorable()}
					<div class="restore-btn">
						<Tag
							style="ghost"
							kind="solid"
							clickable
							on:click={() => {
								dispatch('restoreClick');
							}}>Restore</Tag
						>
					</div>
				{/if}
			</div>

			<!-- {#if isCardInFocus && !entry.isCurrent} -->
			<!-- <div class="restore-btn">
				<Tag
					style="ghost"
					kind="solid"
					icon="update-small"
					clickable
					on:click={() => {
						console.log('Restore');
					}}>Restore</Tag
				>
			</div> -->
			<!-- {/if} -->
		</div>

		{#if entry.filesChanged.length > 0}
			<SnapshotAttachment
				foldable={entry.filesChanged.length > 2}
				foldedAmount={entry.filesChanged.length - 2}
			>
				<div class="files-attacment">
					{#each entry.filesChanged as filePath}
						<button
							class="files-attacment__file"
							on:click={() => {
								dispatch('diffClick', filePath);
							}}
						>
							<img
								draggable="false"
								class="files-attacment__file-icon"
								src={getVSIFileIcon(filePath)}
								alt=""
							/>
							<div class="text-base-12 files-attacment__file-path-and-name">
								<span class="files-attacment__file-name">
									{filePath.split('/').pop()}
								</span>
								<span class="files-attacment__file-path">
									{getPathOnly(filePath)}
								</span>
							</div>
						</button>
					{/each}
				</div>
			</SnapshotAttachment>
		{/if}

		{#if isRestoreSnapshot}
			<SnapshotAttachment>
				<div class="restored-attacment">
					<Icon name="commit" />
					<div class="restored-attacment__content">
						<h4 class="text-base-13 text-semibold">Snapshot title</h4>
						<span class="restored-attacment__details text-base-12">
							{getShortSha(
								entry.details?.trailers.find((t) => t.key === 'restored_from')
									?.value
							)} • date
						</span>
					</div>
				</div>
			</SnapshotAttachment>
		{/if}
	</div>
</div>

<style lang="postcss">
	/* SNAPSHOT CARD */
	.snapshot-card {
		position: relative;
		display: flex;
		gap: var(--size-10);
		padding: var(--size-10) var(--size-14) var(--size-8) var(--size-14);
		overflow: hidden;
		background-color: var(--clr-bg-1);
		transition: padding 0.2s;
	}

	.restore-btn_visible {
		/* padding: var(--size-10) var(--size-14) var(--size-12) var(--size-14); */

		&:hover {
			& .restore-btn {
				display: flex;
			}

			background-color: var(--clr-bg-2);
		}
	}

	.restore-btn {
		display: none;
		/* padding: var(--size-8) var(--size-14); */
	}

	/* .restore-btn {
		height: 0;
		opacity: 0;
		margin-top: calc(var(--size-6) * -1);
		overflow: hidden;
		transition:
			height 0.2s,
			opacity 0.3s,
			margin-top 0.2s;
	} */

	.snapshot-time {
		color: var(--clr-text-2);
		/* background-color: #ffcf887d; */
		width: 2.15rem;

		text-align: right;
		line-height: 1.5;
		/* margin-top: var(--size-2); */
	}

	.snapshot-line {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;
		gap: var(--size-4);
		/* margin-top: var(--size-2); */
		/* background-color: rgba(0, 255, 255, 0.299); */

		&::after {
			position: absolute;
			top: var(--size-20);
			content: '';
			height: calc(100% - var(--size-12));
			min-height: var(--size-8);
			width: 1px;
			background-color: var(--clr-border-2);
		}
	}

	/* CARD CONTENT */

	.snapshot-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--size-6);
	}

	.snapshot-details {
		display: flex;
		width: 100%;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--size-6);
		min-height: var(--size-tag);
	}

	.current-tag {
		margin-top: -0.188rem;
	}

	.snapshot-title-wrap {
		display: flex;
		gap: var(--size-6);
		width: 100%;
	}

	.snapshot-title {
		flex: 1;
	}

	.snapshot-sha {
		white-space: nowrap;
		color: var(--clr-text-3);
	}

	/* ATTACHMENT FILES */

	.files-attacment {
		display: flex;
		flex-direction: column;
		gap: var(--size-2);
		padding: var(--size-4);
	}

	.files-attacment__file {
		display: flex;
		align-items: center;
		gap: var(--size-6);
		padding: var(--size-4);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.files-attacment__file-path-and-name {
		display: flex;
		gap: var(--size-6);
		overflow: hidden;
	}

	.files-attacment__file-path {
		color: var(--clr-text-3);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.files-attacment__file-name {
		color: var(--clr-text-2);
		white-space: nowrap;
	}

	.files-attacment__file-icon {
		width: var(--size-12);
	}

	/* ATTACHMENT RESTORE */

	.restored-attacment {
		display: flex;
		padding: var(--size-12);
		gap: var(--size-6);
	}

	.restored-attacment__content {
		display: flex;
		flex-direction: column;
		gap: var(--size-4);
	}

	.restored-attacment__details {
		color: var(--clr-text-2);
	}

	/* RESTORED  */
	.restored-snapshot {
		background-color: var(--clr-bg-2);
	}
</style>
