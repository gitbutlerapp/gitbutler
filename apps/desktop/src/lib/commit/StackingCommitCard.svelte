<script lang="ts">
	import CommitContextMenu from './CommitContextMenu.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import CommitMessageInput from '$lib/commit/CommitMessageInput.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { draggableCommit } from '$lib/dragging/draggable';
	import { DraggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import BranchFilesList from '$lib/file/BranchFilesList.svelte';
	import { ModeService } from '$lib/modes/service';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { getContext, getContextStore, maybeGetContext } from '$lib/utils/context';
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
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
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
		filesToggleable = true
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

	let draggableCommitElement = $state<HTMLElement>();
	let contextMenu = $state<ReturnType<typeof CommitContextMenu>>();
	let files = $state<RemoteFile[]>([]);
	let showDetails = $state(false);

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

	let isUndoable = commit instanceof DetailedCommit;

	let commitMessageModal: Modal;
	let commitMessageValid = $state(false);
	let description = $state('');

	let createRefModal: Modal;
	let createRefName = $state($baseBranch.remoteName + '/');

	function openCreateRefModal(e: Event, commit: DetailedCommit | Commit) {
		e.stopPropagation();
		createRefModal.show(commit);
	}

	function pushCommitRef(commit: DetailedCommit) {
		if (branch && commit.remoteRef) {
			branchController.pushChangeReference(branch.id, commit.remoteRef);
		}
	}

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

	let dragDirection: 'up' | 'down' | undefined = $state();
	let isDragTargeted = $state(false);

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

	const conflicted = $derived(commit instanceof DetailedCommit && commit.conflicted);
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

<Modal bind:this={createRefModal} width="small">
	{#snippet children(commit)}
		<TextBox label="Remote branch name" id="newRemoteName" bind:value={createRefName} focus />
		<Button
			style="pop"
			kind="solid"
			onclick={() => {
				branchController.createChangeReference(
					branch?.id || '',
					'refs/remotes/' + createRefName,
					commit.changeId
				);
				createRefModal.close();
			}}
		>
			Ok
		</Button>
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
	{/snippet}
</Modal>

{#if draggableCommitElement}
	<CommitContextMenu
		bind:this={contextMenu}
		targetElement={draggableCommitElement}
		{commit}
		{commitUrl}
	/>
{/if}

<div
	class="commit-row"
	class:is-commit-open={showDetails}
	class:is-last={last}
	onclick={toggleFiles}
	onkeyup={onKeyup}
	role="button"
	tabindex="0"
	ondragenter={() => {
		isDragTargeted = true;
	}}
	ondragleave={() => {
		isDragTargeted = false;
	}}
	ondrop={() => {
		isDragTargeted = false;
	}}
	ondrag={(e) => {
		const target = e.target as HTMLElement;
		const targetHeight = target.offsetHeight;
		const targetTop = target.getBoundingClientRect().top;
		const mouseY = e.clientY;

		const isTop = mouseY < targetTop + targetHeight / 2;

		dragDirection = isTop ? 'up' : 'down';
	}}
	use:draggableCommit={commit instanceof DetailedCommit && !isUnapplied && type !== 'integrated'
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
	{#if dragDirection && isDragTargeted}
		<div
			class="pseudo-reorder-zone"
			class:top={dragDirection === 'up'}
			class:bottom={dragDirection === 'down'}
			class:is-last={last}
		></div>
	{/if}

	{#if lines}
		<div>
			{@render lines()}
		</div>
	{/if}

	<div class="commit-card" class:is-last={last}>
		<!-- GENERAL INFO -->
		<div
			bind:this={draggableCommitElement}
			class="commit__header"
			role="button"
			tabindex="-1"
			oncontextmenu={(e) => {
				contextMenu?.open(e);
			}}
		>
			{#if !isUnapplied}
				{#if type === 'local' || type === 'localAndRemote'}
					<div class="commit__drag-icon">
						<Icon name="draggable-narrow" />
					</div>
				{/if}
			{/if}

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

					{#if showDetails && commitUrl}
						<span class="commit__subtitle-divider">•</span>

						<button
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
											currentCommitMessage.set(commit.description);
											e.stopPropagation();
											undoCommit(commit);
										}}>Undo</Button
									>
								{/if}
								<Button
									size="tag"
									style="ghost"
									outline
									icon="edit-small"
									onclick={openCommitMessageModal}>Edit message</Button
								>
								{#if commit instanceof DetailedCommit && !commit.remoteRef}
									<Button
										size="tag"
										style="ghost"
										outline
										icon="virtual-branch-small"
										onclick={(e: Event) => {
											openCreateRefModal(e, commit);
										}}>Create ref</Button
									>
								{/if}
								{#if commit instanceof DetailedCommit && commit.remoteRef}
									<Button
										size="tag"
										style="ghost"
										outline
										icon="remote"
										onclick={() => {
											pushCommitRef(commit);
										}}>Push ref</Button
									>
								{/if}
							{/if}
							{#if canEdit() && project.succeedingRebases}
								<Button size="tag" style="ghost" outline onclick={editPatch}>
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

		&:not(.is-commit-open) {
			&:hover {
				background-color: var(--clr-bg-1-muted);
			}
		}

		&:not(.is-last) {
			border-bottom: 1px solid var(--clr-border-2);
		}
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
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 14px 14px 0;

		&:hover {
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

	.pseudo-reorder-zone.bottom.is-last {
		bottom: -6px;
	}
</style>
