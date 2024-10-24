<script lang="ts">
	import CommitContextMenu from './CommitContextMenu.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import CommitMessageInput from '$lib/commit/CommitMessageInput.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
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
	import PopoverActionsContainer from '@gitbutler/ui/popoverActions/PopoverActionsContainer.svelte';
	import PopoverActionsItem from '@gitbutler/ui/popoverActions/PopoverActionsItem.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { type Snippet } from 'svelte';

	interface Props {
		branch?: VirtualBranch | undefined;
		commit: DetailedCommit | Commit;
		commitUrl?: string | undefined;
		isHeadCommit?: boolean;
		isUnapplied?: boolean;
		last?: boolean;
		type: CommitStatus;
		lines?: Snippet | undefined;
		filesToggleable?: boolean;
		seriesName: string;
	}

	const {
		branch = undefined,
		commit,
		commitUrl = undefined,
		isHeadCommit = false,
		isUnapplied = false,
		last = false,
		type,
		lines = undefined,
		filesToggleable = true,
		seriesName
	}: Props = $props();

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const modeService = maybeGetContext(ModeService);

	const commitStore = createCommitStore(commit);
	$effect(() => {
		commitStore.set(commit);
	});

	const currentCommitMessage = persistedCommitMessage(project.id, branch?.id || '');

	let kebabMenuTrigger = $state<HTMLButtonElement>();
	let draggableCommitElement = $state<HTMLElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabContextMenu = $state<ReturnType<typeof ContextMenu>>();
	let isKebabContextMenuOpen = $state(false);
	let files = $state<RemoteFile[]>([]);
	let showDetails = $state(false);
	let conflictResolutionConfirmationModal = $state<ReturnType<typeof Modal>>();

	const conflicted = $derived(commit.conflicted);
	const isAncestorMostConflicted = $derived(branch?.ancestorMostConflictedCommit?.id === commit.id);
	async function loadFiles() {
		files = await listRemoteCommitFiles(project.id, commit.id);
	}

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

	let isUndoable = commit instanceof DetailedCommit && type !== 'remote';

	let commitMessageModal: ReturnType<typeof Modal> | undefined;
	let commitMessageValid = $state(false);
	let description = $state('');

	function openCommitMessageModal(e: MouseEvent) {
		e.stopPropagation();
		description = commit.description;
		commitMessageModal?.show();
	}

	function submitCommitMessageModal() {
		commit.description = description;

		if (branch) {
			branchController.updateCommitMessage(branch.id, commit.id, description);
		}

		commitMessageModal?.close();
	}

	const commitShortSha = commit.id.substring(0, 7);

	function handleUncommit(e: MouseEvent) {
		e.stopPropagation();
		currentCommitMessage.set(commit.description);
		undoCommit(commit);
	}

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

	const showOpenInBrowser = $derived(commitUrl && (type === 'remote' || type === 'localAndRemote'));
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

{#snippet commitContextMenuSnippet(parent: ReturnType<typeof ContextMenu>)}
	{#if contextMenu}
		<CommitContextMenu
			{parent}
			baseBranch={$baseBranch}
			{branch}
			{commit}
			isRemote={type === 'remote'}
			commitUrl={showOpenInBrowser ? commitUrl : undefined}
			onUncommitClick={handleUncommit}
			onEditMessageClick={openCommitMessageModal}
			onPatchEditClick={handleEditPatch}
		/>
	{/if}
{/snippet}

{#if draggableCommitElement}
	<ContextMenu bind:this={contextMenu} target={draggableCommitElement} openByMouse>
		{@render commitContextMenuSnippet(contextMenu)}
	</ContextMenu>
{/if}

{#if kebabMenuTrigger}
	<ContextMenu
		bind:this={kebabContextMenu}
		target={kebabMenuTrigger}
		onopen={() => (isKebabContextMenuOpen = true)}
		onclose={() => (isKebabContextMenuOpen = false)}
	>
		{@render commitContextMenuSnippet(kebabContextMenu)}
	</ContextMenu>
{/if}

<div
	class="commit-row"
	class:is-commit-open={showDetails}
	class:commit-card-activated={isKebabContextMenuOpen}
	class:is-last={last}
	onclick={(e) => {
		e.preventDefault();
		toggleFiles();
	}}
	oncontextmenu={(e) => {
		e.preventDefault();
		contextMenu?.open(e);
	}}
	onkeyup={onKeyup}
	role="button"
	tabindex="0"
	use:draggableCommit={commit instanceof DetailedCommit && !isUnapplied && type !== 'integrated'
		? {
				label: commit.descriptionTitle,
				sha: commitShortSha,
				date: getTimeAgo(commit.createdAt),
				authorImgUrl: commit.author.gravatarUrl,
				commitType: type,
				data: new DraggableCommit(commit.branchId, commit, isHeadCommit, seriesName),
				viewportId: 'board-viewport'
			}
		: nonDraggable()}
>
	{#if lines}
		<div>
			{@render lines()}
		</div>
	{/if}

	<PopoverActionsContainer class="commit-actions-menu" thin stayOpen={isKebabContextMenuOpen}>
		<PopoverActionsItem
			bind:el={kebabMenuTrigger}
			activated={isKebabContextMenuOpen}
			icon="kebab"
			tooltip="More options"
			thin
			onclick={(e) => {
				kebabContextMenu?.toggle(e);
			}}
		/>
	</PopoverActionsContainer>

	<div class="commit-card" class:is-last={last}>
		<!-- GENERAL INFO -->
		<div bind:this={draggableCommitElement} class="commit__header" role="button" tabindex="-1">
			<div class="commit__drag-icon">
				<Icon name="draggable" />
			</div>

			{#if isUndoable && !commit.descriptionTitle}
				<span class="text-13 text-body text-semibold commit__empty-title">empty commit message</span
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

					<Tooltip text={commit.author.name}>
						<img class="commit__subtitle-avatar" src={commit.author.gravatarUrl} alt="" />
					</Tooltip>

					<span class="commit__subtitle-divider">•</span>

					<button
						type="button"
						class="commit__subtitle-btn commit__subtitle-btn_dashed"
						onclick={(e) => {
							e.stopPropagation();
							copyToClipboard(commit.id);
						}}
					>
						{commitShortSha}

						<div class="commit__subtitle-btn__icon">
							<Icon name="copy-small" />
						</div>
					</button>

					{#if showDetails && showOpenInBrowser}
						<span class="commit__subtitle-divider">•</span>

						<button
							type="button"
							class="commit__subtitle-btn"
							onclick={(e) => {
								e.stopPropagation();
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
					<span>{getTimeAgo(commit.createdAt)}</span>
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
											handleUncommit(e);
										}}>Uncommit</Button
									>
								{/if}
								<Button
									size="tag"
									style="ghost"
									outline
									icon="edit-small"
									onclick={(e: MouseEvent) => {
										openCommitMessageModal(e);
									}}>Edit message</Button
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
					conflictedFiles={commit.conflictedFiles}
					readonly={type === 'remote' || isUnapplied}
				/>
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.commit-row {
		position: relative;
		display: flex;
		gap: 10px;
		width: 100%;
		background-color: var(--clr-bg-1);
		transition: background-color var(--transition-fast);

		&:focus {
			outline: none;
		}

		&:hover {
			& :global(.commit-actions-menu) {
				--show: true;
			}
		}

		&:not(.is-commit-open) {
			&:hover {
				background-color: var(--clr-bg-1-muted);

				& .commit__drag-icon {
					opacity: 1;
				}
			}
		}

		&:not(.is-last) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.is-last {
			border-radius: 0 0 var(--radius-m) var(--radius-m);
		}
	}

	.commit-card-activated {
		background-color: var(--clr-bg-1-muted);
	}

	.commit-card {
		display: flex;
		position: relative;
		flex-direction: column;
		flex: 1;
		overflow: hidden;
	}

	.commit__conflicted {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-core-err-40);
	}

	/* HEADER */
	.commit__header {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 14px 14px 0;
	}

	.commit__drag-icon {
		cursor: grab;
		position: absolute;
		display: flex;
		bottom: 10px;
		right: 6px;
		color: var(--clr-text-3);

		opacity: 0;
		transition: opacity var(--transition-fast);
	}

	.commit__title {
		flex: 1;
		text-align: left;
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

		text-decoration-line: underline;
		text-underline-offset: 2px;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);

			& .commit__subtitle-btn__icon {
				width: var(--size-icon);
				opacity: 1;
				margin-left: 2px;
				transform: scale3d(1, 1, 1);
			}
		}
	}

	.commit__subtitle-btn_dashed {
		text-decoration-style: dashed;
	}

	.commit__subtitle-btn__icon {
		display: flex;
		width: 0;
		opacity: 0;
		margin-left: 0;
		transform: scale3d(0.6, 0.6, 0.6); /* CSS glitch fix */
		transition:
			width var(--transition-medium),
			opacity var(--transition-fast),
			color var(--transition-fast),
			transform var(--transition-medium),
			margin var(--transition-fast);
	}

	.commit__subtitle-avatar {
		width: 12px;
		height: 12px;
		border-radius: 50%;
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
		padding-bottom: 12px;
	}

	.commit__actions {
		overflow: visible;
		display: flex;
		gap: 4px;
		overflow-x: auto;
	}

	/* FILES */
	.files-container {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		margin-right: 14px;
		margin-bottom: 14px;
		overflow: hidden;
	}

	/* MODIFIERS */
	.is-commit-open {
		& .commit__subtitle-btn__icon {
			width: var(--size-icon);
			opacity: 1;
			margin-left: 2px;
			transform: scale3d(1, 1, 1);
		}
	}
</style>
