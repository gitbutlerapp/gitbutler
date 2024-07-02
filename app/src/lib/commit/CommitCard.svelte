<script lang="ts">
	import CommitDragItem from './CommitDragItem.svelte';
	import { Project } from '$lib/backend/projects';
	import CommitMessageInput from '$lib/commit/CommitMessageInput.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { draggable } from '$lib/dragging/draggable';
	import { DraggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import BranchFilesList from '$lib/file/BranchFilesList.svelte';
	import Button from '$lib/shared/Button.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import Modal from '$lib/shared/Modal.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { getTimeAgo } from '$lib/utils/timeAgo';
	import { tooltip } from '$lib/utils/tooltip';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { createCommitStore } from '$lib/vbranches/contexts';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import {
		RemoteCommit,
		Commit,
		RemoteFile,
		Branch,
		BaseBranch,
		type CommitStatus
	} from '$lib/vbranches/types';
	import { createEventDispatcher, type Snippet } from 'svelte';

	export let branch: Branch | undefined = undefined;
	export let commit: Commit | RemoteCommit;
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

	const commitStore = createCommitStore(commit);
	$: commitStore.set(commit);

	const currentCommitMessage = persistedCommitMessage(project.id, branch?.id || '');

	const dispatch = createEventDispatcher<{ toggle: void }>();

	let files: RemoteFile[] = [];
	let showDetails = false;

	async function loadFiles() {
		files = await listRemoteCommitFiles(project.id, commit.id);
	}

	function toggleFiles() {
		showDetails = !showDetails;
		dispatch('toggle');

		if (showDetails) loadFiles();
	}

	function onKeyup(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ') {
			toggleFiles();
		}
	}

	function undoCommit(commit: Commit | RemoteCommit) {
		if (!branch || !$baseBranch) {
			console.error('Unable to undo commit');
			return;
		}
		branchController.undoCommit(branch.id, commit.id);
	}

	let isUndoable = !!branch?.active && commit instanceof Commit;

	const hasCommitUrl = !commit.isLocal && commitUrl;

	let commitMessageModal: Modal;
	let commitMessageValid = false;
	let description = '';

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

	let topHeightPx = 24;

	$: {
		topHeightPx = 24;
		if (first) topHeightPx = 58;
		if (showDetails && !first) topHeightPx += 12;
	}
</script>

<Modal bind:this={commitMessageModal} width="small">
	<CommitMessageInput
		bind:commitMessage={description}
		bind:valid={commitMessageValid}
		isExpanded={true}
	/>
	{#snippet controls(close)}
		<Button style="ghost" outline on:click={close}>Cancel</Button>
		<Button
			style="neutral"
			kind="solid"
			grow
			disabled={!commitMessageValid}
			on:click={submitCommitMessageModal}
		>
			Submit
		</Button>
	{/snippet}
</Modal>

<div
	class="commit-row"
	class:is-commit-open={showDetails}
	class:is-first={first}
	class:is-last={last}
	class:has-lines={lines}
>
	{#if lines}
		<div>
			{@render lines(topHeightPx)}
		</div>
	{/if}
	<CommitDragItem {commit}>
		<div class="commit-card" class:is-first={first} class:is-last={last}>
			<div
				class="accent-border-line"
				class:is-first={first}
				class:is-last={last}
				class:local={type === 'local'}
				class:local-and-remote={type === 'localAndRemote'}
				class:upstream={type === 'remote'}
				class:integrated={type === 'integrated'}
			></div>

			<!-- GENERAL INFO -->
			<div
				class="commit__header"
				on:click={toggleFiles}
				on:keyup={onKeyup}
				role="button"
				tabindex="0"
				use:draggable={commit instanceof Commit
					? {
							data: new DraggableCommit(commit.branchId, commit, isHeadCommit),
							extendWithClass: 'commit_draggable'
						}
					: nonDraggable()}
			>
				<div class="commit__drag-icon">
					<Icon name="draggable-narrow" />
				</div>

				{#if first}
					<div class="commit__type text-semibold text-base-12">
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
					<span class="text-base-body-13 text-semibold commit__empty-title"
						>empty commit message</span
					>
				{:else}
					<h5 class="text-base-body-13 text-semibold commit__title" class:truncate={!showDetails}>
						{commit.descriptionTitle}
					</h5>

					<div class="text-base-11 commit__subtitle">
						{#if commit.isSigned}
							<div class="commit__signed" use:tooltip={{ text: 'Signed', delay: 500 }}>
								<Icon name="success-outline-small" />
							</div>

							<span class="commit__subtitle-divider">•</span>
						{/if}

						<button
							class="commit__subtitle-btn commit__subtitle-btn_dashed"
							on:click|stopPropagation={() => copyToClipboard(commit.id)}
						>
							<span>{commit.id.substring(0, 7)}</span>

							<div class="commit__subtitle-btn__icon">
								<Icon name="copy-small" />
							</div>
						</button>

						{#if showDetails && hasCommitUrl}
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

						<span
							>{getTimeAgo(commit.createdAt)}{type === 'localAndRemote' || type === 'remote'
								? ` by ${commit.author.name}`
								: ' by you'}</span
						>
					</div>
				{/if}
			</div>

			<!-- HIDDEN -->
			{#if showDetails}
				{#if commit.descriptionBody || isUndoable}
					<div class="commit__details">
						{#if commit.descriptionBody}
							<span class="commit__description text-base-body-12">
								{commit.descriptionBody}
							</span>
						{/if}

						{#if isUndoable}
							<div class="commit__actions hide-native-scrollbar">
								{#if isUndoable}
									<Button
										size="tag"
										style="ghost"
										outline
										icon="undo-small"
										on:click={(e) => {
											currentCommitMessage.set(commit.description);
											e.stopPropagation();
											undoCommit(commit);
										}}>Undo</Button
									>
									<Button
										size="tag"
										style="ghost"
										outline
										icon="edit-small"
										on:click={openCommitMessageModal}>Edit message</Button
									>
								{/if}
							</div>
						{/if}
					</div>
				{/if}

				<div class="files-container">
					<BranchFilesList {files} {isUnapplied} readonly={type === 'remote'} />
				</div>
			{/if}
		</div>
	</CommitDragItem>
</div>

<style lang="postcss">
	/* amend drop zone */
	:global(.amend-dz-active .amend-dz-marker) {
		display: flex;
	}
	:global(.amend-dz-hover .hover-text) {
		visibility: visible;
	}
	:global(.commit_draggable) {
		cursor: grab;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-m);
		border: none;
	}

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

	.accent-border-line {
		position: absolute;
		width: 4px;
		height: 100%;
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
</style>
