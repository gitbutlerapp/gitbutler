<script lang="ts">
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import { getColorFromCommitState } from '$components/lib';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { pushStatusToColor, pushStatusToIcon, type PushStatus } from '$lib/stacks/stack';
	import { Icon } from '@gitbutler/ui';
	import { getFileIcon } from '@gitbutler/ui/components/file/getFileIcon';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';

	type Props = {
		type: 'branch' | 'commit' | 'file' | 'folder' | 'hunk' | 'ai-session';
		label?: string;
		filePath?: string;
		commitType?: CommitStatusType;
		childrenAmount?: number;
		pushStatus?: PushStatus;
	};

	let { type, label, filePath, commitType, childrenAmount = 1, pushStatus }: Props = $props();

	const commitColor = $derived(
		type === 'commit' && commitType ? getColorFromCommitState(commitType, false) : undefined
	);
	const fileIcon = $derived(filePath ? getFileIcon(filePath) : undefined);
	// Debug log
</script>

{#if type === 'branch'}
	<div class="draggable-branch-card">
		{#if pushStatus}
			<BranchHeaderIcon
				iconName={pushStatusToIcon(pushStatus)}
				color={getColorFromBranchType(pushStatusToColor(pushStatus))}
				small
			/>
		{/if}
		<span class="truncate text-15 text-bold">
			{label}
		</span>
	</div>
{:else if type === 'commit'}
	<div
		class="draggable-commit-v3"
		class:draggable-commit-v3-local={commitType === 'LocalOnly' ||
			commitType === 'Integrated' ||
			commitType === 'Base'}
		class:draggable-commit-v3-remote={commitType !== 'LocalOnly' &&
			commitType !== 'Integrated' &&
			commitType !== 'Base'}
		style:--commit-color={commitColor}
	>
		<div class="draggable-commit-v3-indicator"></div>
		<div class="truncate text-13 text-semibold draggable-commit-v3-label">
			{label || 'Empty commit'}
		</div>
	</div>
{:else if type === 'ai-session'}
	<div class="dragchip-container">
		<div class="dragchip-ai-session-container">
			<Icon name="ai-small" />
			{#if label}
				<span class="text-12 text-semibold truncate dragchip-ai-session-label">{label}</span>
			{/if}
			<Icon name="draggable" />
		</div>
	</div>
{:else}
	<!-- File, Folder, or Hunk chips -->
	<div
		class="dragchip-container"
		class:dragchip-two={childrenAmount === 2}
		class:dragchip-multiple={childrenAmount > 2}
	>
		<div class="dragchip">
			{#if type === 'file'}
				<div class="dragchip-file-container">
					{#if fileIcon}
						<img src={fileIcon} alt="" class="dragchip-file-icon" />
					{/if}
					<span class="text-12 text-semibold truncate dragchip-file-name">
						{label || 'Empty file'}
					</span>
				</div>
			{:else if type === 'folder'}
				<div class="dragchip-file-container">
					<Icon name="folder" />
					<span class="text-12 text-semibold truncate dragchip-file-name">
						{label || 'Empty folder'}
					</span>
				</div>
			{:else if type === 'hunk'}
				<div class="dragchip-hunk-container">
					<div class="dragchip-hunk-decorator">〈/〉</div>
					<span class="dragchip-hunk-label">{label || 'Empty hunk'}</span>
				</div>
			{/if}

			{#if childrenAmount > 1}
				<div class="text-11 text-bold dragchip-amount">{childrenAmount}</div>
			{/if}
		</div>
	</div>
{/if}
