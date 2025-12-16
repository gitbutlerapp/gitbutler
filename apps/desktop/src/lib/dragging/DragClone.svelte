<script lang="ts">
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import { getColorFromCommitState } from '$components/lib';
	import { type CommitStatusType } from '$lib/commits/commit';
	import { type PushStatus } from '$lib/stacks/stack';
	import { Icon } from '@gitbutler/ui';
	import { getFileIcon } from '@gitbutler/ui/components/file/getFileIcon';

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
			<BranchHeaderIcon iconName="branch-local" color="var(--clr-commit-local)" small />
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

<style>
	/* DRAG CHIPS.
	 * General styles
	 * Basic container for single and multiple items */
	.dragchip-container {
		display: flex;
		position: absolute;
		top: -1000px;
		left: -1000px;
		pointer-events: none;
	}

	.dragchip {
		display: flex;
		z-index: 3;
		position: relative;
		min-width: 50px;
		max-width: 240px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.dragchip-amount {
		display: flex;
		position: absolute;
		top: -6px;
		right: -8px;
		align-items: center;
		justify-content: center;
		min-width: 16px;
		margin-left: 5px;
		padding: 2px 4px;
		border-radius: 16px;
		background-color: var(--clr-theme-gray-element);
		color: var(--clr-theme-gray-on-element);
	}

	/* if dragging more then one item */
	.dragchip-two:after,
	.dragchip-multiple:before,
	.dragchip-multiple:after {
		position: absolute;
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		content: '';
	}

	.dragchip-two {
		&::after {
			z-index: 2;
			top: 6px;
			left: 6px;
		}
	}

	.dragchip-multiple {
		&::before {
			z-index: 2;
			top: 6px;
			left: 6px;
		}

		&::after {
			z-index: 1;
			top: 12px;
			left: 12px;
		}
	}

	/* FILE DRAG */
	.dragchip-file-container {
		display: flex;
		position: relative;
		align-items: center;
		padding: 8px;
		overflow: hidden;
		gap: 6px;
	}

	.dragchip-file-name {
		color: var(--clr-text-1);
	}

	.dragchip-file-icon {
		flex-shrink: 0;
		width: 16px;
		height: 16px;
		color: var(--clr-text-2);
	}

	/* HUNK DRAG */
	.dragchip-hunk-container {
		display: flex;
		font-size: 12px;
		font-family: var(--font-mono);
	}

	.dragchip-hunk-decorator {
		padding: 6px 5px;
		border-right: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m) 0 0 var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
		font-variant-ligatures: none;
		letter-spacing: -1px;
	}

	.dragchip-hunk-label {
		padding: 6px 7px;
	}

	/* AI SESSION DRAG */
	.dragchip-ai-session-container {
		display: flex;
		align-items: center;
		height: var(--size-tag);
		padding: 0 4px;
		gap: 4px;
		border: none;
		border-radius: var(--radius-m);
		background-position: 0% 50%;
		background-size: 200% 200%;
		background: var(--codegen-gradient);
		color: var(--codegen-color);
	}

	/* BRANCH DRAG CARD */

	.draggable-branch-card {
		display: flex;
		position: absolute;
		align-items: center;
		min-width: 50px;
		max-width: 220px;
		height: 36px;
		padding: 0 10px;
		overflow: hidden;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	/* COMMIT DRAG CARD V3 */
	.draggable-commit-v3 {
		display: flex;
		position: absolute;
		align-items: center;
		min-width: 50px;
		max-width: 240px;
		height: 36px;
		padding: 0 10px;
		overflow: hidden;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);

		&::before {
			z-index: 1;
			position: absolute;
			top: 0;
			left: 14px;
			width: 2px;
			height: 100%;
			background-color: var(--commit-color);
			content: '';
		}
	}

	.draggable-commit-v3-indicator {
		z-index: 2;
		flex-shrink: 0;
		width: 10px;
		height: 10px;
		outline: 3px solid var(--clr-bg-1);
		background-color: var(--commit-color);
	}

	.draggable-commit-v3-local {
		& .draggable-commit-v3-indicator {
			border-radius: 50%;
		}
	}

	.draggable-commit-v3-remote {
		& .draggable-commit-v3-indicator {
			transform: rotate(45deg) scale(0.9);
			border-radius: 2px;
		}
	}

	@keyframes dropzone-scale {
		from {
			transform: scale(0.96);
			opacity: 0;
		}
		to {
			transform: scale(1);
			opacity: 1;
		}
	}

	/* Dim the original element when it's being dragged */
	:global(.dragging) {
		opacity: 0.5;
		transition: opacity 0.2s ease;
	}
</style>
