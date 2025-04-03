<script lang="ts">
	import CommitContextMenu from '$components/v3/CommitContextMenu.svelte';
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import { isLocalAndRemoteCommit, isUpstreamCommit } from '$components/v3/lib';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { isCommit, type Commit, type UpstreamCommit } from '$lib/branches/v3';
	import { CommitDropData } from '$lib/commits/dropHandler';
	import { draggableCommit } from '$lib/dragging/draggable';
	import { NON_DRAGGABLE } from '$lib/dragging/draggables';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	type Props = {
		projectId: string;
		branchName: string;
		stackId: string;
		commit: Commit | UpstreamCommit;
		first?: boolean;
		lastCommit?: boolean;
		lastBranch?: boolean;
		selected?: boolean;
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
		opacity,
		borderTop,
		draggable,
		disableCommitActions = false,
		onclick
	}: Props = $props();

	const baseBranch = getContext(BaseBranch);
	const stackService = getContext(StackService);
	const modeService = maybeGetContext(ModeService);

	const [uncommit] = stackService.uncommit();

	const commitUrl = undefined;
	const conflicted = $derived(isCommit(commit) ? commit.hasConflicts : false);
	const isAncestorMostConflicted = false; // TODO
	const isUnapplied = false; // TODO
	const branchRefName = undefined;

	let commitRowElement = $state<HTMLDivElement>();
	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let conflictResolutionConfirmationModal = $state<ReturnType<typeof Modal>>();

	let isOpenedByKebabButton = $state(false);
	let isOpenedByMouse = $state(false);

	async function handleUncommit() {
		if (!baseBranch) {
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
	role="button"
	tabindex="0"
	aria-label="Commit row"
	class="commit-row__main"
	class:menu-shown={isOpenedByKebabButton || isOpenedByMouse}
	class:first
	class:selected
	style:opacity
	class:border-top={borderTop || first}
	bind:this={commitRowElement}
	class:last={lastCommit}
	onclick={(e) => {
		e.preventDefault();
		e.stopPropagation();
		if (disableCommitActions) return;
		onclick?.();
	}}
	onkeydown={(e) => {
		if (disableCommitActions) return;
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onclick?.();
		}
	}}
	oncontextmenu={(e) => {
		if (disableCommitActions) return;
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
	<CommitLine {commit} {lastCommit} {lastBranch} />

	<div class="commit-content">
		<!-- <button type="button" {onclick} tabindex="0"> -->
		<div class="commit-name truncate">
			<CommitHeader {commit} row class="text-13 text-semibold" />
		</div>

		{#if conflicted}
			<div class="commit-conflict-indicator">
				<Icon name="warning" size={12} />
			</div>
		{/if}

		<button
			type="button"
			bind:this={kebabMenuTrigger}
			class="commit-menu-btn"
			class:activated={isOpenedByKebabButton}
			onmousedown={(e) => {
				e.preventDefault();
				e.stopPropagation();
				isOpenedByKebabButton = true;
				contextMenu?.toggle();
			}}
			onclick={(e) => {
				e.preventDefault();
				e.stopPropagation();
			}}
		>
			<Icon name="kebab" /></button
		>
	</div>
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
	bind:menu={contextMenu}
	{projectId}
	leftClickTrigger={kebabMenuTrigger}
	rightClickTrigger={commitRowElement}
	{baseBranch}
	{stackId}
	{commit}
	{commitUrl}
	onUncommitClick={handleUncommit}
	onEditMessageClick={openCommitMessageModal}
	onPatchEditClick={handleEditPatch}
	onToggle={(isOpen, isLeftClick) => {
		if (isLeftClick) {
			isOpenedByKebabButton = isOpen;
		} else {
			isOpenedByMouse = isOpen;
		}
	}}
/>

<style lang="postcss">
	.commit-row__main {
		position: relative;
		display: flex;
		width: 100%;
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&::before {
			content: '';
			position: absolute;
			top: 50%;
			left: 0;
			width: 4px;
			height: 45%;
			transform: translateX(-100%) translateY(-50%);
			border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
			background-color: var(--clr-selected-in-focus-element);
			transition: transform var(--transition-fast);
		}

		&:hover,
		&.menu-shown {
			background-color: var(--clr-bg-1-muted);

			& .commit-menu-btn {
				display: flex;
			}
		}

		&:not(.last) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.last {
			border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		}

		&:focus-within,
		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);

			& .commit-menu-btn {
				display: flex;
			}

			&:before {
				transform: translateX(0%) translateY(-50%);
			}
		}

		&:focus-within.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}
	}

	.commit-content {
		display: flex;
		align-items: center;
		position: relative;
		gap: 4px;
		width: 100%;
		overflow: hidden;
		padding-right: 10px;
	}

	.commit-name {
		flex: 1;
		padding: 14px 0 14px 4px;
		display: flex;
	}

	.commit-menu-btn {
		display: none;
		padding: 3px;
		color: var(--clr-text-1);
		opacity: 0.5;
		transition: opacity var(--transition-fast);

		&:hover,
		&.activated {
			opacity: 1;
		}
	}

	.commit-conflict-indicator {
		position: absolute;
		/* Account for the kebab menu that appears on hover */
		right: 42px;
		color: var(--clr-theme-err-element);
	}
</style>
