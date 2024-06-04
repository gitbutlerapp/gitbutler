<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import CommitDragItem from './CommitDragItem.svelte';
	import Icon from './Icon.svelte';
	import { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import CommitMessageInput from '$lib/components/CommitMessageInput.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { draggable } from '$lib/dragging/draggable';
	import { DraggableCommit, nonDraggable } from '$lib/dragging/draggables';
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
	import { createEventDispatcher } from 'svelte';
	// import { slide } from 'svelte/transition';

	export let branch: Branch | undefined = undefined;
	export let commit: Commit | RemoteCommit;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
	export let isUnapplied = false;
	export let first = false;
	export let last = false;
	export let type: CommitStatus;

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
		if (e.key == 'Enter' || e.key == ' ') {
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
</script>

<Modal bind:this={commitMessageModal}>
	<CommitMessageInput bind:commitMessage={description} bind:valid={commitMessageValid} />
	<svelte:fragment slot="controls">
		<Button style="ghost" kind="solid" on:click={() => commitMessageModal.close()}>Cancel</Button>
		<Button
			style="pop"
			kind="solid"
			grow
			disabled={!commitMessageValid}
			on:click={submitCommitMessageModal}>Submit</Button
		>
	</svelte:fragment>
</Modal>

<div
	class="commit-row"
	class:is-commit-open={showDetails}
	class:is-first={first}
	class:is-last={last}
	class:has-lines={$$slots.lines}
>
	<slot name="lines" />
	<CommitDragItem {commit}>
		<div
			use:draggable={commit instanceof Commit
				? {
						data: new DraggableCommit(commit.branchId, commit, isHeadCommit)
					}
				: nonDraggable()}
			class="commit-card"
			class:is-first={first}
			class:is-last={last}
		>
			<div
				class="accent-border-line"
				class:is-first={first}
				class:is-last={last}
				class:local={type == 'local'}
				class:remote={type == 'remote'}
				class:upstream={type == 'upstream'}
				class:integrated={type == 'integrated'}
			/>

			<div class="commit__content">
				<!-- GENERAL INFO -->
				<div
					class="commit__about"
					on:click={toggleFiles}
					on:keyup={onKeyup}
					role="button"
					tabindex="0"
				>
					{#if first}
						<div class="commit__type text-semibold text-base-12">
							{#if type == 'upstream'}
								Remote <Icon name="remote" />
							{:else if type == 'local'}
								Local <Icon name="local" />
							{:else if type == 'remote'}
								Local and remote
							{:else if type == 'integrated'}
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
							{/if}
							<button
								class="commit__id"
								on:click|stopPropagation={() => copyToClipboard(commit.id)}
							>
								{commit.id.substring(0, 7)}
							</button>

							<span class="commit__subtitle-divider">•</span>

							<span
								>{getTimeAgo(commit.createdAt)}{type == 'remote' || type == 'upstream'
									? ` by ${commit.author.name}`
									: ''}</span
							>
						</div>
					{/if}
				</div>

				<!-- HIDDEN -->
				{#if showDetails}
					<div class="commit__details">
						{#if hasCommitUrl || isUndoable}
							<div class="commit__actions hide-native-scrollbar">
								{#if isUndoable}
									<Tag
										style="ghost"
										kind="solid"
										icon="undo-small"
										clickable
										on:click={(e) => {
											currentCommitMessage.set(commit.description);
											e.stopPropagation();
											undoCommit(commit);
										}}>Undo</Tag
									>
									<Tag
										style="ghost"
										kind="solid"
										icon="edit-text"
										clickable
										on:click={openCommitMessageModal}>Edit message</Tag
									>
								{/if}
								{#if hasCommitUrl}
									<Tag
										style="ghost"
										kind="solid"
										icon="open-link"
										clickable
										on:click={() => {
											if (commitUrl) openExternalUrl(commitUrl);
										}}>Open</Tag
									>
								{/if}
							</div>
						{/if}

						{#if commit.descriptionBody}
							<span class="commit__description text-base-body-12">
								{commit.descriptionBody}
							</span>
						{/if}
					</div>
				{/if}
			</div>

			{#if showDetails}
				<div class="files-container">
					<BranchFilesList title="Files" {files} {isUnapplied} readonly={type == 'upstream'} />
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

	.commit-row {
		position: relative;
		display: flex;
		gap: var(--size-8);
		&.has-lines {
			padding-right: var(--size-14);
		}

		/* border-top: 1px solid var(--clr-border-2); */
		/* padding-left: var(--size-8); */

		&:not(.is-first) {
			border-top: 1px solid var(--clr-border-3);
		}
	}

	.commit-card {
		display: flex;
		position: relative;
		flex-direction: column;

		background-color: var(--clr-bg-1);
		/* border: 1px solid var(--clr-border-2);
		border-top: none;
		border-bottom: none;
		border-left: none; */
		border-right: 1px solid var(--clr-border-2);
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&.is-first {
			margin-top: var(--size-12);
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
		width: var(--size-4);
		height: 100%;
		&.local {
			background-color: var(--clr-commit-local);
		}
		&.remote {
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
	.commit__content {
		display: flex;
		flex-direction: column;
	}

	.commit__about {
		display: flex;
		flex-direction: column;
		gap: var(--size-6);
		padding: var(--size-14);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
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
		gap: var(--size-4);
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

	.commit__id {
		display: flex;
		align-items: center;
		gap: var(--size-4);

		&:hover {
			color: var(--clr-text-1);
		}
	}

	.commit__subtitle-divider {
		opacity: 0.4;
	}

	/* DETAILS */

	.commit__details {
		display: flex;
		flex-direction: column;
		gap: var(--size-12);
		padding: var(--size-14);
		border-top: 1px solid var(--clr-border-2);
	}

	.commit__actions {
		display: flex;
		gap: var(--size-4);
		overflow-x: auto;
		margin: 0 calc(var(--size-14) * -1);
		padding: 0 var(--size-14);
	}

	/* FILES */
	.files-container {
		border-top: 1px solid var(--clr-border-2);
	}

	/* MODIFIERS */
	.is-commit-open {
		& .commit__about {
			background-color: var(--clr-bg-1-muted);
		}

		& .commit-card {
			&:not(.is-first) {
				margin-top: var(--size-8);
				border-top: 1px solid var(--clr-border-2);
			}

			&:not(.is-last) {
				margin-bottom: var(--size-8);
				border-bottom: 1px solid var(--clr-border-2);
			}
		}
	}
</style>
