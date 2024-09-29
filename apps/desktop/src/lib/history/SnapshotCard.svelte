<script lang="ts">
	import SnapshotAttachment from './SnapshotAttachment.svelte';
	import { createdOnDay } from '$lib/history/history';
	import { ModeService } from '$lib/modes/service';
	import { getContext } from '$lib/utils/context';
	import { splitFilePath } from '$lib/utils/filePath';
	import { toHumanReadableTime } from '$lib/utils/time';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';
	import { createEventDispatcher } from 'svelte';
	import type { Snapshot, SnapshotDetails } from '$lib/history/types';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		entry: Snapshot;
		isWithinRestore?: boolean;
		selectedFile?:
			| {
					entryId: string;
					path: string;
			  }
			| undefined;
	}

	let { entry, isWithinRestore = true, selectedFile = undefined }: Props = $props();

	function getShortSha(sha: string | undefined) {
		if (!sha) return '';

		return `${sha.slice(0, 7)}`;
	}

	function createdOnDayAndTime(epoch: number) {
		const date = new Date(epoch);
		return `${createdOnDay(date)}, ${toHumanReadableTime(date)}`;
	}

	const dispatch = createEventDispatcher<{
		restoreClick: void;
		diffClick: string;
		visible: void;
	}>();

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
					text: `Delete branch "${entry.details?.trailers.find((t) => t.key === 'name')?.value}"`,
					icon: 'item-cross'
				};
			case 'ApplyBranch':
				return {
					text: `Apply branch "${entry.details?.trailers.find((t) => t.key === 'name')?.value}"`,
					icon: 'item-tick'
				};
			case 'UnapplyBranch':
				return {
					text: `Unapply branch "${snapshotDetails.trailers.find((t) => t.key === 'name')?.value}"`,
					icon: 'item-dashed'
				};
			case 'UpdateBranchName':
				return {
					text: `Renamed branch "${snapshotDetails.trailers.find((t) => t.key === 'before')?.value}" to "${snapshotDetails.trailers.find((t) => t.key === 'after')?.value}"`,
					icon: 'item-slash'
				};
			case 'CreateBranch':
				return {
					text: `Create branch "${snapshotDetails.trailers.find((t) => t.key === 'name')?.value}"`,
					icon: 'item-plus'
				};
			case 'ReorderBranches':
				return {
					text: `Reorder branches "${snapshotDetails.trailers.find((t) => t.key === 'before')?.value}" and "${snapshotDetails.trailers.find((t) => t.key === 'after')?.value}"`,
					icon: 'item-link'
				};
			case 'SelectDefaultVirtualBranch':
				return {
					text: `Select default virtual branch "${snapshotDetails.trailers.find((t) => t.key === 'after')?.value}"`,
					icon: 'item-dot'
				};
			case 'UpdateBranchRemoteName':
				return {
					text: `Update branch remote name "${snapshotDetails.trailers.find((t) => t.key === 'before')?.value}" to "${snapshotDetails.trailers.find((t) => t.key === 'after')?.value}"`,
					icon: 'item-slash'
				};
			case 'SetBaseBranch':
				return { text: 'Set base branch', icon: 'item-slash' };
			case 'GenericBranchUpdate':
				return { text: 'Generic branch update', icon: 'item-slash' };

			// COMMIT OPERATIONS
			case 'CreateCommit':
				return {
					text: `Create commit ${getShortSha(entry.details?.trailers.find((t) => t.key === 'sha')?.value)}`,
					icon: 'new-commit',
					commitMessage: entry.details?.trailers.find((t) => t.key === 'message')?.value
				};
			case 'UndoCommit':
				return {
					text: `Undo commit ${getShortSha(entry.details?.trailers.find((t) => t.key === 'sha')?.value)}`,
					icon: 'undo-commit',
					commitMessage: entry.details?.trailers.find((t) => t.key === 'message')?.value
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
					text: `Move hunk to "${entry.details?.trailers.find((t) => t.key === 'name')?.value}"`,
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
				return { text: 'Update workspace base', icon: 'rebase' };
			case 'RestoreFromSnapshot':
				return { text: 'Revert snapshot', icon: 'empty' };
			case 'EnterEditMode':
				return { text: 'Enter Edit Mode', icon: 'edit-text' };
			default:
				return { text: snapshotDetails.operation, icon: 'commit' };
		}
	}

	const isRestoreSnapshot = entry.details?.operation === 'RestoreFromSnapshot';
	const error = entry.details?.trailers.find((t) => t.key === 'error')?.value;

	const operation = mapOperation(entry.details);

	const modeService = getContext(ModeService);
	const mode = modeService.mode;
</script>

<div
	class="snapshot-card show-restore-on-hover"
	class:restored-snapshot={isRestoreSnapshot || isWithinRestore}
>
	<div class="snapshot-right-container">
		<div class="restore-btn">
			<Button
				size="tag"
				style="ghost"
				outline
				tooltip="Restores GitButler and your files to the state before this operation. Revert actions can also be undone."
				onclick={() => {
					dispatch('restoreClick');
				}}
				disabled={$mode?.type !== 'OpenWorkspace'}>Revert</Button
			>
		</div>
		<span class="snapshot-time text-11">
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
			<h4 class="snapshot-title text-13 text-body text-semibold">
				<span>{operation.text}</span>
				<span class="snapshot-sha text-12 text-body"> • {getShortSha(entry.id)}</span>
			</h4>

			{#if operation.commitMessage}
				<p class="text-12 snapshot-commit-message">
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
							class:file-selected={selectedFile?.path === filePath &&
								selectedFile?.entryId === entry.id}
							onclick={() => {
								dispatch('diffClick', filePath);
							}}
						>
							<FileIcon fileName={filePath} size={14} />
							<div class="text-12 files-attacment__file-path-and-name">
								<span class="files-attacment__file-name">
									{splitFilePath(filePath).filename}
								</span>
								<span class="files-attacment__file-path">
									{splitFilePath(filePath).path}
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
						<h4 class="text-13 text-semibold">
							{camelToTitleCase(
								entry.details?.trailers.find((t) => t.key === 'restored_operation')?.value
							)}
						</h4>
						<span class="restored-attacment__details text-12">
							{getShortSha(entry.details?.trailers.find((t) => t.key === 'restored_from')?.value)} •
							{createdOnDayAndTime(
								parseInt(
									entry.details?.trailers.find((t) => t.key === 'restored_date')?.value || ''
								)
							)}
						</span>
					</div>
				</div>
			</SnapshotAttachment>
		{/if}
		{#if error}
			<div class="error-text text-12 text-body">
				{error}
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	/* SNAPSHOT CARD */
	.snapshot-card {
		position: relative;
		display: flex;
		gap: 12px;
		padding: 10px 14px 8px 14px;
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
		width: 60px;
	}

	.restore-btn {
		display: none;
	}

	.snapshot-time {
		color: var(--clr-text-2);
		text-align: right;
		line-height: 1.8;
		margin-top: 2px;
	}

	.snapshot-line {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;
		margin-top: 3px;

		&::after {
			position: absolute;
			top: 24px;
			content: '';
			height: calc(100% - 14px);
			min-height: 8px;
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
		gap: 6px;
		min-height: var(--size-tag);
		overflow: hidden;
		/* padding-bottom: 4px; */
	}

	.snapshot-details {
		display: flex;
		width: 100%;
		flex-direction: column;
		align-items: flex-start;
		gap: 6px;
		margin-top: 2px;
		margin-bottom: 4px;
	}

	.snapshot-title {
		flex: 1;
	}

	.snapshot-commit-message {
		color: var(--clr-text-2);
		margin-bottom: 2px;

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
	}

	.files-attacment__file {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px;
		border-bottom: 1px solid var(--clr-border-3);

		&:not(.file-selected):hover {
			background-color: var(--clr-bg-1-muted);
		}

		&:last-child {
			border-bottom: none;
		}
	}

	.file-selected {
		background-color: var(--clr-theme-pop-bg);

		& .files-attacment__file-name {
			opacity: 0.9;
		}
	}

	.files-attacment__file-path-and-name {
		display: flex;
		gap: 6px;
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

	/* ATTACHMENT RESTORE */

	.restored-attacment {
		display: flex;
		padding: 12px;
		gap: 8px;
	}

	.restored-attacment__content {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.restored-attacment__details {
		color: var(--clr-text-2);
	}

	/* RESTORED  */
	.restored-snapshot {
		background-color: var(--clr-bg-2);
	}

	/* --- */
	.error-text {
		display: flex;
		padding: 6px 10px;
		background-color: var(--clr-theme-err-bg);
		border-radius: var(--radius-m);
		width: 100%;
		color: var(--clr-scale-err-40);
	}
</style>
