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
	export let selectedFile:
		| {
				entryId: string;
				path: string;
		  }
		| undefined = undefined;

	function getShortSha(sha: string | undefined) {
		if (!sha) return '';

		return `${sha.slice(0, 7)}`;
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
		commitMessage?: string;
	} {
		if (!snapshotDetails) return { text: '', icon: 'commit' };

		switch (snapshotDetails.operation) {
			// BRANCH OPERATIONS
			case 'DeleteBranch':
				return {
					text: `Delete branch "${entry.details?.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'item-cross'
				};
			case 'ApplyBranch':
				return {
					text: `Apply branch "${entry.details?.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'item-tick'
				};
			case 'UnapplyBranch':
				return {
					text: `Unapply branch "${snapshotDetails.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'item-dashed'
				};
			case 'UpdateBranchName':
				return {
					text: `Renamed branch "${snapshotDetails.trailers.find((t) => t.key == 'before')?.value}" to "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'item-slash'
				};
			case 'CreateBranch':
				return {
					text: `Create branch "${snapshotDetails.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'item-plus'
				};
			case 'ReorderBranches':
				return {
					text: `Reorder branches "${snapshotDetails.trailers.find((t) => t.key == 'before')?.value}" and "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'item-link'
				};
			case 'SelectDefaultVirtualBranch':
				return {
					text: `Select default virtual branch "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'item-dot'
				};
			case 'UpdateBranchRemoteName':
				return {
					text: `Update branch remote name "${snapshotDetails.trailers.find((t) => t.key == 'before')?.value}" to "${snapshotDetails.trailers.find((t) => t.key == 'after')?.value}"`,
					icon: 'item-slash'
				};
			case 'SetBaseBranch':
				return { text: 'Set base branch', icon: 'item-slash' };
			case 'GenericBranchUpdate':
				return { text: 'Generic branch update', icon: 'item-slash' };

			// COMMIT OPERATIONS
			case 'CreateCommit':
				return {
					text: `Create commit ${getShortSha(entry.details?.trailers.find((t) => t.key == 'sha')?.value)}`,
					icon: 'new-commit',
					commitMessage: entry.details?.trailers.find((t) => t.key == 'message')?.value
				};
			case 'UndoCommit':
				return {
					text: `Undo commit ${getShortSha(entry.details?.trailers.find((t) => t.key == 'sha')?.value)}`,
					icon: 'undo-commit',
					commitMessage: entry.details?.trailers.find((t) => t.key == 'message')?.value
				};
			case 'AmendCommit':
				return { text: 'Amend commit', icon: 'amend-commit' };
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

			// FILE OPERATIONS
			case 'MoveHunk':
				return {
					text: `Move hunk to "${entry.details?.trailers.find((t) => t.key == 'name')?.value}"`,
					icon: 'item-move'
				};
			case 'DiscardHunk':
				return { text: 'Discard hunk', icon: 'item-cross' };
			case 'DiscardFile':
				return { text: 'Discard file', icon: 'discard-file-small' };
			case 'FileChanges':
				return { text: 'File changes', icon: 'file-changes-small' };

			// OTHER OPERATIONS
			case 'MergeUpstream':
				return { text: 'Merge upstream', icon: 'merged-pr-small' };
			case 'UpdateWorkspaceBase':
				return { text: 'Update workspace base', icon: 'rebase-small' };
			case 'RestoreFromSnapshot':
				return { text: 'Revert snapshot', icon: 'empty' };
			default:
				return { text: snapshotDetails.operation, icon: 'commit' };
		}
	}

	const isRestoreSnapshot = entry.details?.operation == 'RestoreFromSnapshot';

	const operation = mapOperation(entry.details);

	function getPathOnly(path: string) {
		return path.split('/').slice(0, -1).join('/');
	}
</script>

<div class="snapshot-card show-restore-on-hover" class:restored-snapshot={isRestoreSnapshot}>
	<div class="snapshot-right-container">
		<div class="restore-btn">
			<Tag
				style="ghost"
				kind="solid"
				clickable
				on:click={() => {
					dispatch('restoreClick');
				}}>Revert</Tag
			>
		</div>
		<span class="snapshot-time text-base-11">
			{toHumanReadableTime(entry.createdAt)}
		</span>
	</div>

	<div class="snapshot-line">
		{#if isRestoreSnapshot}
			<img src="/images/history/restore-icon.svg" alt="" />
		{:else}
			<Icon name={operation.icon} />
		{/if}
	</div>

	<div class="snapshot-content">
		<div class="snapshot-details">
			<h4 class="snapshot-title text-base-body-13 text-semibold">
				<span>{operation.text}</span>
				<span class="snapshot-sha text-base-body-12"> • {getShortSha(entry.id)}</span>
			</h4>

			{#if operation.commitMessage}
				<p class="text-base-12 snapshot-commit-message">
					<span>Message:</span>
					{operation.commitMessage}
				</p>
			{/if}
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
		gap: var(--size-12);
		padding: var(--size-10) var(--size-14) var(--size-8) var(--size-14);
		overflow: hidden;
		background-color: var(--clr-bg-1);
		transition: padding 0.2s;
	}

	.show-restore-on-hover {
		&:hover {
			& .restore-btn {
				display: flex;
			}

			& .snapshot-time {
				display: none;
			}

			background-color: var(--clr-bg-2);
		}
	}

	.snapshot-right-container {
		display: flex;
		justify-content: flex-end;
		width: 3.7rem;
	}

	.restore-btn {
		display: none;
	}

	.snapshot-time {
		color: var(--clr-text-2);
		text-align: right;
		line-height: 1.8;
		margin-top: var(--size-2);
	}

	.snapshot-line {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;
		margin-top: 0.188rem;

		&::after {
			position: absolute;
			top: var(--size-24);
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
		min-height: var(--size-tag);
		overflow: hidden;
		/* padding-bottom: var(--size-4); */
	}

	.snapshot-details {
		display: flex;
		width: 100%;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--size-6);
		margin-top: var(--size-2);
		margin-bottom: var(--size-4);
	}

	.snapshot-title {
		flex: 1;
	}

	.snapshot-commit-message {
		color: var(--clr-text-2);
		margin-bottom: var(--size-2);

		& span {
			color: var(--clr-text-3);
		}
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
