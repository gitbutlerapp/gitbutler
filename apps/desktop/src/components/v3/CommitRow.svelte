<script lang="ts">
	import CommitContextMenu from '$components/v3/CommitContextMenu.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { ModeService } from '$lib/mode/modeService';
	import { showError } from '$lib/notifications/toasts';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext, getContextStore, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';
	import type { CommitKey } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		branchName: string;
		stackId: string;
		commitKey: CommitKey;
		commit: Commit | UpstreamCommit;
		first?: boolean;
		lastCommit?: boolean;
		lastBranch?: boolean;
		selected?: boolean;
		lineColor?: string;
		opacity?: number;
		borderTop?: boolean;
		href?: string;
		disableCommitActions?: boolean;
	};

	const {
		projectId,
		stackId,
		commit,
		first,
		lastCommit,
		lastBranch,
		selected,
		lineColor,
		opacity,
		borderTop,
		href,
		disableCommitActions = false
	}: Props = $props();

	const baseBranch = getContextStore(BaseBranch);
	const stackService = getContext(StackService);
	const modeService = maybeGetContext(ModeService);

	const commitTitle = $derived(commit.message.split('\n')[0]);
	const commitUrl = undefined;
	const conflicted = false; // TODO
	const isAncestorMostConflicted = false; // TODO
	const isUnapplied = false; // TODO
	const branchRefName = undefined;

	let commitRowElement = $state<HTMLAnchorElement>();
	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let conflictResolutionConfirmationModal = $state<ReturnType<typeof Modal>>();

	let isOpenedByKebabButton = $state(false);

	async function handleUncommit() {
		if (!$baseBranch) {
			console.error('Unable to undo commit');
			return;
		}
		const { error } = await stackService.uncommit(projectId, stackId, commit.id);
		if (error) {
			showError('Failed to uncommit', error);
			console.error('Failed to uncommit', error);
		}
	}

	function openCommitMessageModal() {
		// TODO: Implement openCommitMessageModal
	}

	function canEdit() {
		if (isUnapplied) return false;
		if (!modeService) return false;

		return true;
	}

	async function editPatch() {
		if (!canEdit() || !branchRefName) return;
		modeService!.enterEditMode(commit.id, branchRefName);
	}

	async function handleEditPatch() {
		if (conflicted && !isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch();
	}
</script>

<a
	bind:this={commitRowElement}
	type="button"
	class="commit"
	{href}
	oncontextmenu={(e) => {
		e.preventDefault();
		isOpenedByKebabButton = false;
		contextMenu?.open(e);
	}}
>
	<div
		class="commit-row__main"
		class:first
		class:lastCommit
		class:selected
		style:opacity
		class:border-top={borderTop || first}
	>
		<CommitLine {commit} {lastCommit} {lastBranch} {lineColor} />

		<div class="commit-content">
			<span class="commit-title text-13 text-semibold">
				{commitTitle}
			</span>

			<div class="commit-arrow">
				<Icon name="chevron-right" />
			</div>
		</div>
	</div>

	{#if !disableCommitActions}
		<PopoverActionsContainer class="commit-row-actions-menu" thin stayOpen={isOpenedByKebabButton}>
			<PopoverActionsItem
				bind:el={kebabMenuTrigger}
				activated={isOpenedByKebabButton}
				icon="kebab"
				tooltip="More options"
				thin
				onclick={() => {
					contextMenu?.toggle();
				}}
			/>
		</PopoverActionsContainer>
	{/if}
</a>

<Modal bind:this={conflictResolutionConfirmationModal} width="small" onSubmit={editPatch}>
	{#snippet children()}
		<div>
			<p>It's generally better to start resolving conflicts from the bottom up.</p>
			<br />
			<p>Are you sure you want to resolve conflicts for this commit?</p>
		</div>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit">Yes</Button>
	{/snippet}
</Modal>

<CommitContextMenu
	{projectId}
	leftClickTrigger={kebabMenuTrigger}
	rightClickTrigger={commitRowElement}
	onToggle={(isOpen, isLeftClick) => {
		if (isLeftClick) {
			isOpenedByKebabButton = isOpen;
		}
	}}
	bind:menu={contextMenu}
	baseBranch={$baseBranch}
	branchId={stackId}
	{commit}
	{commitUrl}
	onUncommitClick={handleUncommit}
	onEditMessageClick={openCommitMessageModal}
	onPatchEditClick={handleEditPatch}
/>

<style lang="postcss">
	.commit {
		position: relative;
		display: flex;
		align-items: center;
		width: 100%;

		&:hover :global(.commit-row-actions-menu) {
			--show: true;
		}
	}
	.commit-row__main {
		position: relative;
		display: flex;
		align-items: center;
		text-align: left;
		width: 100%;
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&::before {
			content: '';
			position: absolute;
			left: 0;
			width: 3px;
			height: 100%;
			transform: translateX(-100%);
			background-color: var(--clr-theme-pop-element);
			transition: transform var(--transition-fast);
		}

		&:last-child {
			border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		}

		&:not(.lastCommit) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.border-top {
			border-top: 1px solid var(--clr-border-2);
		}

		&.selected {
			background-color: var(--clr-bg-1-muted);
		}

		&.selected::before {
			transform: none;
		}

		&.selected .commit-arrow {
			margin-right: -6px;
			opacity: 0.3;
			transition:
				margin-right var(--transition-medium),
				opacity var(--transition-medium) 0.05s;
		}
	}

	.commit-content {
		display: flex;
		gap: 4px;
		align-items: center;
		width: 100%;
		padding: 14px 14px 14px 0;
		overflow: hidden;
	}

	.commit-title {
		flex-grow: 1;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}

	.commit-arrow {
		display: flex;
		align-items: center;
		opacity: 0;
		margin-right: -20px;
		transition:
			margin-right var(--transition-medium) 0.05s,
			opacity var(--transition-medium);
	}
</style>
