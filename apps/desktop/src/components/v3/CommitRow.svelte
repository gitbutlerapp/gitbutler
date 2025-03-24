<script lang="ts">
	import CommitContextMenu from '$components/v3/CommitContextMenu.svelte';
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import { isLocalAndRemoteCommit, isUpstreamCommit } from '$components/v3/lib';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { CommitDropData } from '$lib/commits/dropHandler';
	import { draggableCommit } from '$lib/dragging/draggable';
	import { NON_DRAGGABLE } from '$lib/dragging/draggables';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext, getContextStore, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	type Props = {
		projectId: string;
		branchName: string;
		stackId: string;
		commit: Commit | UpstreamCommit;
		first?: boolean;
		lastCommit?: boolean;
		lastBranch?: boolean;
		selected?: boolean;
		lineColor?: string;
		opacity?: number;
		borderTop?: boolean;
		draggable?: boolean;
		disableCommitActions?: boolean;
		onclick?: () => void;
	};

	const {
		projectId,
		branchName,
		stackId,
		commit,
		first,
		lastCommit,
		lastBranch,
		selected,
		lineColor,
		opacity,
		borderTop,
		draggable,
		disableCommitActions = false,
		onclick
	}: Props = $props();

	const baseBranch = getContextStore(BaseBranch);
	const stackService = getContext(StackService);
	const modeService = maybeGetContext(ModeService);

	const [uncommit] = stackService.uncommit();

	const commitUrl = undefined;
	const conflicted = false; // TODO
	const isAncestorMostConflicted = false; // TODO
	const isUnapplied = false; // TODO
	const branchRefName = undefined;

	let commitRowElement = $state<HTMLDivElement>();
	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let conflictResolutionConfirmationModal = $state<ReturnType<typeof Modal>>();

	let isOpenedByKebabButton = $state(false);

	async function handleUncommit() {
		if (!$baseBranch) {
			console.error('Unable to undo commit');
			return;
		}
		await uncommit({ projectId, stackId, commitId: commit.id });
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
		modeService!.enterEditMode(commit.id, stackId);
	}

	async function handleEditPatch() {
		if (conflicted && !isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch();
	}

	const commitShortSha = commit.id.substring(0, 7);
</script>

<div
	bind:this={commitRowElement}
	role="listitem"
	class="commit"
	class:last={lastCommit}
	oncontextmenu={(e) => {
		e.preventDefault();
		isOpenedByKebabButton = false;
		contextMenu?.open(e);
	}}
	use:draggableCommit={draggable
		? {
				disabled: false,
				label: commit.message.split('\n')[0],
				sha: commitShortSha,
				date: getTimeAgo(commit.createdAt),
				authorImgUrl: undefined,
				commitType: 'LocalAndRemote',
				data: new CommitDropData(
					stackId,
					{
						id: commit.id,
						isRemote: isUpstreamCommit(commit),
						isConflicted: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
						isIntegrated: isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated'
					},
					false,
					branchName
				),
				viewportId: 'board-viewport'
			}
		: NON_DRAGGABLE}
>
	<div
		class="commit-row__main"
		class:first
		class:selected
		style:opacity
		class:border-top={borderTop || first}
	>
		<CommitLine {commit} {lastCommit} {lastBranch} {lineColor} />

		<div class="commit-content">
			<button type="button" {onclick} tabindex="0">
				<CommitHeader {commit} row />
			</button>
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
</div>

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
		overflow: hiddend;

		&:hover :global(.commit-row-actions-menu) {
			--show: true;
		}
		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}
	.commit-row__main {
		position: relative;
		display: flex;
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

		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);
		}

		/* &:focus.selected {
			background-color: var(--clr-selected-in-focus-bg);
		} */

		&:focus-within.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}

		&.selected::before {
			transform: none;
		}
	}

	.commit-content {
		display: flex;
		flex-direction: column;
		position: relative;
		gap: 6px;
		width: 100%;
		overflow: hidden;

		& button {
			padding: 14px 14px 14px 0;
			display: flex;
			justify-items: start;
		}
	}
</style>
