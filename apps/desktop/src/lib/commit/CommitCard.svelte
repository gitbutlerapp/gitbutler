<script lang="ts">
	import CommitContextMenu from './CommitContextMenu.svelte';
	import CommitDragItem from './CommitDragItem.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import CommitMessageInput from '$lib/commit/CommitMessageInput.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { draggableCommit } from '$lib/dragging/draggable';
	import { DraggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import BranchFilesList from '$lib/file/BranchFilesList.svelte';
	import { ModeService } from '$lib/modes/service';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { createCommitStore } from '$lib/vbranches/contexts';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import {
		Commit,
		DetailedCommit,
		RemoteFile,
		VirtualBranch,
		type CommitStatus
	} from '$lib/vbranches/types';
	import { getContext, getContextStore, maybeGetContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { getTimeAndAuthor } from '@gitbutler/ui/utils/getTimeAndAuthor';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { type Snippet } from 'svelte';

	export let branch: VirtualBranch | undefined = undefined;
	export let commit: DetailedCommit | Commit;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
	export let isUnapplied = false;
	export let first = false;
	export let last = false;
	export let type: CommitStatus;
	export let lines: Snippet<[number]> | undefined = undefined;

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const modeService = maybeGetContext(ModeService);

	const commitStore = createCommitStore(commit);
	$: commitStore.set(commit);

	const currentCommitMessage = persistedCommitMessage(project.id, branch?.id || '');

	let draggableCommitElement: HTMLElement | null = null;
	let contextMenu: ReturnType<typeof CommitContextMenu> | undefined;
	let files: RemoteFile[] = [];
	let showDetails = false;

	$: conflicted = commit.conflicted;
	$: isAncestorMostConflicted = branch?.ancestorMostConflictedCommit?.id === commit.id;

	async function loadFiles() {
		files = await listRemoteCommitFiles(project.id, commit.id);
	}

	export let filesToggleable = true;

	function toggleFiles() {
		if (!filesToggleable) return;
		showDetails = !showDetails;

		if (showDetails) loadFiles();
	}

	function onKeyup(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ') {
			toggleFiles();
		}
	}

	function undoCommit(commit: DetailedCommit | Commit) {
		if (!branch || !$baseBranch) {
			console.error('Unable to undo commit');
			return;
		}
		branchController.undoCommit(branch.id, commit.id);
	}

	let isUndoable = commit instanceof DetailedCommit;

	let commitMessageModal: Modal;
	let commitMessageValid = false;
	let description = '';

	let conflictResolutionConfirmationModal: ReturnType<typeof Modal> | undefined;

	function openCommitMessageModal(e: Event) {
		e.stopPropagation();

		description = commit.description;

		commitMessageModal.show();
	}

	function submitCommitMessageModal() {
		commit.description = description;

		if (branch) {
			branchController.updateCommitMessage(branch.id, commit.id, description);
		}

		commitMessageModal.close();
	}

	const commitShortSha = commit.id.substring(0, 7);

	let topHeightPx = 24;

	$: {
		topHeightPx = 24;
		if (first) topHeightPx = 58;
		if (showDetails && !first) topHeightPx += 12;
	}

	let dragDirection: 'up' | 'down' | undefined;
	let isDragTargeted = false;

	function canEdit() {
		if (isUnapplied) return false;
		if (!modeService) return false;
		if (!branch) return false;

		return true;
	}

	async function editPatch() {
		if (!canEdit()) return;
		modeService!.enterEditMode(commit.id, branch!.refname);
	}

	async function handleEditPatch() {
		if (conflicted && !isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch();
	}
</script>

<Modal bind:this={commitMessageModal} width="small" onSubmit={submitCommitMessageModal}>
	{#snippet children(_, close)}
		<CommitMessageInput
			focusOnMount
			bind:commitMessage={description}
			bind:valid={commitMessageValid}
			isExpanded={true}
			cancel={close}
			commit={submitCommitMessageModal}
		/>
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline onclick={close}>Cancel</Button>
		<Button style="neutral" type="submit" kind="solid" grow disabled={!commitMessageValid}>
			Submit
		</Button>
	{/snippet}
</Modal>

<Modal bind:this={conflictResolutionConfirmationModal} width="small" onSubmit={editPatch}>
	{#snippet children()}
		<div>
			<p>It's generally better to start resolving conflicts from the bottom up.</p>
			<br />
			<p>Are you sure you want to resolve conflicts for this commit?</p>
		</div>
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" outline type="submit">Yes</Button>
	{/snippet}
</Modal>

<CommitContextMenu
	bind:this={contextMenu}
	targetElement={draggableCommitElement}
	{commit}
	{commitUrl}
/>

<div
	class="commit-row"
	class:is-commit-open={showDetails}
	class:is-first={first}
	class:is-last={last}
	class:has-lines={lines}
>
	{#if dragDirection && isDragTargeted}
		<div
			class="pseudo-reorder-zone"
			class:top={dragDirection === 'up'}
			class:bottom={dragDirection === 'down'}
			class:is-first={first}
			class:is-last={last}
		></div>
	{/if}

	{#if lines}
		<div>
			{@render lines(topHeightPx)}
		</div>
	{/if}

	<div class="commit-card" class:is-first={first} class:is-last={last}>
		<CommitDragItem {commit}>
			<!-- GENERAL INFO -->
			<div
				bind:this={draggableCommitElement}
				class="commit__header"
				on:click={toggleFiles}
				on:keyup={onKeyup}
				role="button"
				tabindex="0"
				on:contextmenu={(e) => {
					contextMenu?.open(e);
				}}
				on:dragenter={() => {
					isDragTargeted = true;
				}}
				on:dragleave={() => {
					isDragTargeted = false;
				}}
				on:drop={() => {
					isDragTargeted = false;
				}}
				on:drag={(e) => {
					const target = e.target as HTMLElement;
					const targetHeight = target.offsetHeight;
					const targetTop = target.getBoundingClientRect().top;
					const mouseY = e.clientY;

					const isTop = mouseY < targetTop + targetHeight / 2;

					dragDirection = isTop ? 'up' : 'down';
				}}
				use:draggableCommit={commit instanceof DetailedCommit &&
				!isUnapplied &&
				type !== 'integrated'
					? {
							label: commit.descriptionTitle,
							sha: commitShortSha,
							date: getTimeAgo(commit.createdAt),
							authorImgUrl: commit.author.gravatarUrl,
							commitType: type,
							data: new DraggableCommit(commit.branchId, commit, isHeadCommit),
							viewportId: 'board-viewport'
						}
					: nonDraggable()}
			>
				<div
					class="accent-border-line"
					class:is-first={first}
					class:is-last={last}
					class:local={type === 'local'}
					class:local-and-remote={type === 'localAndRemote'}
					class:upstream={type === 'remote'}
					class:integrated={type === 'integrated'}
				></div>

				{#if !isUnapplied}
					{#if type === 'local' || type === 'localAndRemote'}
						<div class="commit__drag-icon">
							<Icon name="draggable-narrow" />
						</div>
					{/if}
				{/if}

				{#if first}
					<div class="commit__type text-semibold text-12">
						{#if type === 'remote'}
							Remote <Icon name="remote" />
						{:else if type === 'local'}
							Local <Icon name="local" />
						{:else if type === 'localAndRemote'}
							Local and remote
						{:else if type === 'integrated'}
							Integrated
						{/if}
					</div>
				{/if}

				{#if isUndoable && !commit.descriptionTitle}
					<span class="text-13 text-body text-semibold commit__empty-title"
						>empty commit message</span
					>
				{:else}
					<h5 class="text-13 text-body text-semibold commit__title" class:truncate={!showDetails}>
						{commit.descriptionTitle}
					</h5>

					<div class="text-11 commit__subtitle">
						{#if commit.isSigned}
							<Tooltip text="Signed">
								<div class="commit__signed">
									<Icon name="success-outline-small" />
								</div>
							</Tooltip>

							<span class="commit__subtitle-divider">•</span>
						{/if}

						{#if conflicted}
							<Tooltip
								text={"Conflicted commits must be resolved before they can be ammended or squashed.\nPlease resolve conflicts using the 'Resolve conflicts' button"}
							>
								<div class="commit__conflicted">
									<Icon name="warning-small" />

									Conflicted
								</div>
							</Tooltip>

							<span class="commit__subtitle-divider">•</span>
						{/if}

						<button
							class="commit__subtitle-btn commit__subtitle-btn_dashed"
							on:click|stopPropagation={() => copyToClipboard(commit.id)}
						>
							<span>{commitShortSha}</span>

							<div class="commit__subtitle-btn__icon">
								<Icon name="copy-small" />
							</div>
						</button>

						{#if showDetails && commitUrl}
							<span class="commit__subtitle-divider">•</span>

							<button
								class="commit__subtitle-btn"
								on:click|stopPropagation={() => {
									if (commitUrl) openExternalUrl(commitUrl);
								}}
							>
								<span>Open</span>

								<div class="commit__subtitle-btn__icon">
									<Icon name="open-link" />
								</div>
							</button>
						{/if}
						<span class="commit__subtitle-divider">•</span>

						<span>{getTimeAndAuthor(commit.createdAt, commit.author.name)}</span>
					</div>
				{/if}
			</div>

			<!-- HIDDEN -->
			{#if showDetails}
				{#if commit.descriptionBody || isUndoable}
					<div class="commit__details">
						{#if commit.descriptionBody}
							<span class="commit__description text-12 text-body">
								{commit.descriptionBody}
							</span>
						{/if}

						{#if isUndoable}
							<div class="commit__actions hide-native-scrollbar">
								{#if isUndoable}
									{#if !conflicted}
										<Button
											size="tag"
											style="ghost"
											outline
											icon="undo-small"
											onclick={(e: MouseEvent) => {
												currentCommitMessage.set(commit.description);
												e.stopPropagation();
												undoCommit(commit);
											}}>Uncommit</Button
										>
									{/if}
									<Button
										size="tag"
										style="ghost"
										outline
										icon="edit-small"
										onclick={openCommitMessageModal}>Edit message</Button
									>
								{/if}
								{#if canEdit()}
									<Button size="tag" style="ghost" outline onclick={handleEditPatch}>
										{#if conflicted}
											Resolve conflicts
										{:else}
											Edit patch
										{/if}
									</Button>
								{/if}
							</div>
						{/if}
					</div>
				{/if}

				<div class="files-container">
					<BranchFilesList
						allowMultiple={!isUnapplied && type !== 'remote'}
						{files}
						{isUnapplied}
						readonly={type === 'remote' || isUnapplied}
						conflictedFiles={commit.conflictedFiles}
					/>
				</div>
			{/if}
		</CommitDragItem>
	</div>
</div>

<style lang="postcss">
	.commit-row {
		position: relative;
		display: flex;

		&.has-lines {
			padding-right: 14px;
		}

		&:not(.is-first) {
			border-top: 1px dotted var(--clr-border-2);
		}
	}

	.commit-card {
		display: flex;
		position: relative;
		flex-direction: column;
		flex: 1;

		background-color: var(--clr-bg-1);
		border-right: 1px solid var(--clr-border-2);
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&.is-first {
			margin-top: 12px;
			border-top: 1px solid var(--clr-border-2);
			border-top-left-radius: var(--radius-m);
			border-top-right-radius: var(--radius-m);
		}
		&.is-last {
			border-bottom: 1px solid var(--clr-border-2);
			border-bottom-left-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
		}
		&:not(.is-first) {
			border-top: none;
		}
	}

	.commit__conflicted {
		display: flex;
		align-items: center;
		gap: 4px;

		color: var(--clr-core-err-40);
	}

	.accent-border-line {
		position: absolute;
		top: 0;
		left: 0;
		width: 4px;
		height: 100%;
		z-index: var(--z-ground);

		&.local {
			background-color: var(--clr-commit-local);
		}
		&.local-and-remote {
			background-color: var(--clr-commit-remote);
		}
		&.upstream {
			background-color: var(--clr-commit-upstream);
		}
		&.integrated {
			background-color: var(--clr-commit-shadow);
		}
	}

	.commit__type {
		opacity: 0.4;
	}

	/* HEADER */
	.commit__header {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px;

		&:hover {
			background-color: var(--clr-bg-1-muted);

			& .commit__drag-icon {
				opacity: 1;
			}
		}
	}

	.commit__drag-icon {
		pointer-events: none;
		position: absolute;
		display: flex;
		top: 4px;
		right: 2px;
		color: var(--clr-text-3);

		opacity: 0;
		transition: opacity var(--transition-fast);
	}

	.commit__title {
		flex: 1;
		display: block;
		color: var(--clr-text-1);
		width: 100%;
	}

	.commit__description {
		color: var(--clr-text-2);
		white-space: pre-wrap;
		user-select: text;
		cursor: text;
	}

	.commit__empty-title {
		color: var(--clr-text-3);
	}

	.commit__subtitle {
		display: flex;
		align-items: center;
		flex-wrap: nowrap;
		gap: 4px;
		color: var(--clr-text-2);
		overflow: hidden;

		& > span {
			white-space: nowrap;
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	.commit__signed {
		display: flex;
	}

	/* SUBTITLE LINK BUTTON */
	.commit__subtitle-btn {
		flex-shrink: 0;
		display: flex;
		align-items: center;

		text-decoration: underline;
		text-underline-offset: 2px;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);

			& .commit__subtitle-btn__icon {
				width: var(--size-icon);
				opacity: 1;
				transform: scale(1);
				margin-left: 2px;
			}
		}
	}

	.commit__subtitle-btn_dashed {
		text-decoration-style: dashed;
	}

	.commit__subtitle-btn__icon {
		display: flex;
		margin-left: 0;
		width: 0;
		opacity: 0;
		transform: scale(0.6);
		transition:
			width var(--transition-medium),
			opacity var(--transition-fast),
			color var(--transition-fast),
			transform var(--transition-medium),
			margin-left var(--transition-fast);
	}

	/* DIVIDER - DOT SYMBOL */
	.commit__subtitle-divider {
		opacity: 0.4;
	}

	/* DETAILS */
	.commit__details {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 10px 14px;
		border-top: 1px solid var(--clr-border-2);
	}

	.commit__actions {
		overflow: visible;
		display: flex;
		gap: 4px;
		overflow-x: auto;
		margin: 0 -14px;
		padding: 4px 14px;
	}

	/* FILES */
	.files-container {
		border-top: 1px solid var(--clr-border-2);
	}

	/* MODIFIERS */
	.is-commit-open {
		& .commit-card {
			border-radius: var(--radius-m);

			&:not(.is-first) {
				margin-top: 12px;
				border-top: 1px solid var(--clr-border-2);
			}

			&:not(.is-last) {
				margin-bottom: 12px;
				border-bottom: 1px solid var(--clr-border-2);
			}
		}

		& .commit__subtitle-btn__icon {
			width: var(--size-icon);
			opacity: 1;
			transform: scale(1);
			margin-left: 2px;
		}
	}

	/* PSUEDO DROPZONE */
	.pseudo-reorder-zone {
		z-index: var(--z-lifted);
		position: absolute;
		height: 2px;
		width: 100%;
		background-color: var(--clr-theme-pop-element);
	}

	.pseudo-reorder-zone.top {
		top: -1px;
	}

	.pseudo-reorder-zone.bottom {
		bottom: -1px;
	}

	.pseudo-reorder-zone.top.is-first {
		top: 6px;
	}

	.pseudo-reorder-zone.bottom.is-last {
		bottom: -6px;
	}
</style>
