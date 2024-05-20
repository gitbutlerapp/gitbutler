<script lang="ts">
	import Icon from './Icon.svelte';
	import SnapshotAttachment from './SnapshotAttachment.svelte';
	import Tag from './Tag.svelte';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { createdOnDay } from '$lib/history/history';
	import { toHumanReadableTime } from '$lib/utils/time';
	import { createEventDispatcher } from 'svelte';
	import type { Snapshot, SnapshotDetails } from '$lib/history/types';
	import type iconsJson from '$lib/icons/icons.json';

	export let entry: Snapshot;
	export let isCurrent: boolean = false;
	export let selectedFile:
		| {
				entryId: string;
				path: string;
		  }
		| undefined = undefined;

	function getShortSha(sha: string | undefined) {
		if (!sha) return '';

		return `#${sha.slice(0, 7)}`;
	}

	function createdOnDayAndTime(epoch: number) {
		const date = new Date(epoch);
		return `${createdOnDay(date)}, ${toHumanReadableTime(date)}`;
	}

	const dispatch = createEventDispatcher<{ restoreClick: void; diffClick: string }>();

	function camelToTitleCase(str: string | undefined) {
		if (!str) return '';
		const lowerCaseStr = str.replace(/([a-z])([A-Z])/g, '$1 $2').toLowerCase();
		return lowerCaseStr.charAt(0).toUpperCase() + lowerCaseStr.slice(1);
	}

	function mapOperation(snapshotDetails: SnapshotDetails | undefined): {
		text: string;
		icon: keyof typeof iconsJson;
	} {
		if (!snapshotDetails) return { text: '', icon: 'commit' };

		switch (snapshotDetails.operation) {
			// Branch operations
			case 'DeleteBranch':
				return {
					text: `Delete branch "${entry.details?.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'delete-branch'
				};
			case 'ApplyBranch':
				return {
					text: `Apply branch "${entry.details?.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'apply-branch'
				};
			case 'UpdateBranchName':
				return {
					text: `Renamed branch "${snapshotDetails.trailers.find((t) => t.key == 'before')?.value}" to "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'update-branch-name'
				};
			case 'CreateBranch':
				return {
					text: `Create branch "${snapshotDetails.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'create-branch'
				};
			case 'UnapplyBranch':
				return {
					text: `Unapply branch "${snapshotDetails.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'unapply-branch'
				};
			case 'ReorderBranches':
				return {
					text: `Reorder branches "${snapshotDetails.trailers.find((t) => t.key == 'before')?.value}" and "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'reorder-branches'
				};
			case 'SelectDefaultVirtualBranch':
				return {
					text: `Select default virtual branch "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'set-default-branch'
				};
			case 'UpdateBranchRemoteName':
				return {
					text: `Update branch remote name "${snapshotDetails.trailers.find((t) => t.key == 'before')?.value}" to "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'update-branch-name'
				};
			case 'SetBaseBranch':
				return { text: 'Set base branch', icon: 'set-base-branch' };
			case 'GenericBranchUpdate':
				return { text: 'Generic branch update', icon: 'update-branch-name' };
			// Commit operations
			case 'CreateCommit':
				return {
					text: `Create commit ${getShortSha(entry.details?.trailers.find((t) => t.key == 'sha')?.value)}`,
					icon: 'new-commit'
				};
			case 'AmendCommit':
				return { text: 'Amend commit', icon: 'amend-commit' };
			case 'UndoCommit':
				return {
					text: `Undo commit ${getShortSha(entry.details?.trailers.find((t) => t.key == 'sha')?.value)}`,
					icon: 'undo-commit'
				};
			case 'SquashCommit':
				return { text: 'Squash commit', icon: 'squash-commit' };
			case 'UpdateCommitMessage':
				return { text: 'Update commit message', icon: 'edit-text' };
			case 'MoveCommit':
				return { text: 'Move commit', icon: 'move-commit' };
			case 'ReorderCommit':
				return { text: 'Reorder commit', icon: 'move-commit' };
			case 'InsertBlankCommit':
				return { text: 'Insert blank commit', icon: 'blank-commit' };
			case 'MoveCommitFile':
				return { text: 'Move commit file', icon: 'move-commit-file-small' };
			// File operations
			case 'MoveHunk':
				return {
					text: `Move hunk to "${entry.details?.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'move-hunk'
				};
			case 'DiscardHunk':
				return { text: 'Discard hunk', icon: 'discard-hunk' };
			case 'DiscardFile':
				return { text: 'Discard file', icon: 'discard-file-small' };
			case 'FileChanges':
				return { text: 'File changes', icon: 'file-changes-small' };
			// Other operations
			case 'MergeUpstream':
				return { text: 'Merge upstream', icon: 'merged-pr-small' };
			case 'UpdateWorkspaceBase':
				return { text: 'Update workspace base', icon: 'rebase-small' };
			case 'RestoreFromSnapshot':
				return { text: 'Restore from snapshot', icon: 'empty' };
			default:
				return { text: snapshotDetails.operation, icon: 'commit' };
		}
	}

	const isRestoreSnapshot = entry.details?.operation == 'RestoreFromSnapshot';

	const operation = mapOperation(entry.details);

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
		{toHumanReadableTime(entry.createdAt)}
	</span>

	<div class="snapshot-line">
		{#if isRestoreSnapshot}
			<img src="/images/history/restore-icon.svg" alt="" />
		{:else}
			<Icon name={operation.icon} />
		{/if}
	</div>

	<div class="snapshot-content">
		<div class="snapshot-details">
			{#if isCurrent}
				<Tag style="pop" kind="soft">Current</Tag>
			{/if}

			<div class="snapshot-title-wrap">
				<h4 class="snapshot-title text-base-body-13 text-semibold">
					<span>{operation.text}</span>
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
		</div>

		{#if entry.filesChanged.length > 0 && !isRestoreSnapshot}
			<SnapshotAttachment
				foldable={entry.filesChanged.length > 2}
				foldedAmount={entry.filesChanged.length - 2}
			>
				<div class="files-attacment">
					{#each entry.filesChanged as filePath}
						<button
							class="files-attacment__file"
							class:file-selected={selectedFile?.path == filePath &&
								selectedFile?.entryId == entry.id}
							on:click={() => {
								dispatch('diffClick', filePath);
								// console.log('diffClick', filePath);
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
						<h4 class="text-base-13 text-semibold">
							{camelToTitleCase(
								entry.details?.trailers.find((t) => t.key == 'restored_operation')?.value
							)}
						</h4>
						<span class="restored-attacment__details text-base-12">
							{getShortSha(entry.details?.trailers.find((t) => t.key == 'restored_from')?.value)} • {createdOnDayAndTime(
								parseInt(entry.details?.trailers.find((t) => t.key == 'restored_date')?.value || '')
							)}
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
		&:hover {
			& .restore-btn {
				display: flex;
			}

			background-color: var(--clr-bg-2);
		}
	}

	.restore-btn {
		display: none;
	}

	.snapshot-time {
		color: var(--clr-text-2);
		width: 3.7rem;

		text-align: right;
		line-height: 1.5;
	}

	.snapshot-line {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;
		gap: var(--size-4);

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
		overflow: hidden;
	}

	.snapshot-details {
		display: flex;
		width: 100%;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--size-6);
		min-height: var(--size-tag);
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

		&:not(.file-selected):hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.file-selected {
		background-color: var(--clr-scale-pop-80);

		& .files-attacment__file-name {
			opacity: 0.9;
		}
	}

	.files-attacment__file-path-and-name {
		display: flex;
		gap: var(--size-6);
		overflow: hidden;
	}

	.files-attacment__file-path {
		color: var(--clr-text-1);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		opacity: 0.2;
	}

	.files-attacment__file-name {
		color: var(--clr-text-1);
		opacity: 0.6;
		white-space: nowrap;
	}

	.files-attacment__file-icon {
		width: var(--size-12);
	}

	/* ATTACHMENT RESTORE */

	.restored-attacment {
		display: flex;
		padding: var(--size-12);
		gap: var(--size-8);
	}

	.restored-attacment__content {
		display: flex;
		flex-direction: column;
		gap: var(--size-6);
	}

	.restored-attacment__details {
		color: var(--clr-text-2);
	}

	/* RESTORED  */
	.restored-snapshot {
		background-color: var(--clr-bg-2);
	}
</style>
