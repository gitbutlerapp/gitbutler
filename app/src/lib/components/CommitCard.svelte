<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import Icon from './Icon.svelte';
	import { Project } from '$lib/backend/projects';
	import { clickOutside } from '$lib/clickOutside';
	import Button from '$lib/components/Button.svelte';
	import CommitMessageInput from '$lib/components/CommitMessageInput.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { featureAdvancedCommitOperations } from '$lib/config/uiFeatureFlags';
	import { draggable } from '$lib/dragging/draggable';
	import { DraggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { getTimeAgo } from '$lib/utils/timeAgo';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { createCommitStore, getSelectedFiles } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import {
		RemoteCommit,
		Commit,
		RemoteFile,
		Branch,
		BaseBranch,
		type CommitStatus
	} from '$lib/vbranches/types';
	import { slide } from 'svelte/transition';

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
	const selectedFiles = getSelectedFiles();
	const fileIdSelection = getContext(FileIdSelection);
	const advancedCommitOperations = featureAdvancedCommitOperations();

	const commitStore = createCommitStore(commit);
	$: commitStore.set(commit);

	const currentCommitMessage = persistedCommitMessage(project.id, branch?.id || '');

	let showDetails = false;
	let files: RemoteFile[] = [];

	$: selectedFile =
		$fileIdSelection.length == 1 &&
		fileIdSelection.only().commitId == commit.id &&
		files.find((f) => f.id == fileIdSelection.only().fileId);
	$: if (selectedFile) selectedFiles.set([selectedFile]);

	async function loadFiles() {
		files = await listRemoteCommitFiles(project.id, commit.id);
	}

	function toggleFiles() {
		showDetails = !showDetails;

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

	function insertBlankCommit(commit: Commit | RemoteCommit, offset: number) {
		if (!branch || !$baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		branchController.insertBlankCommit(branch.id, commit.id, offset);
	}

	function reorderCommit(commit: Commit | RemoteCommit, offset: number) {
		if (!branch || !$baseBranch) {
			console.error('Unable to move commit');
			return;
		}
		branchController.reorderCommit(branch.id, commit.id, offset);
	}

	let contextMenuVisible = false;
	let isUndoable = false;

	$: if ($advancedCommitOperations) {
		isUndoable = !!branch?.active && commit instanceof Commit;
	} else {
		isUndoable = isHeadCommit;
	}
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

	let meatballButton: HTMLButtonElement;

	// console log commit time
	// console.log(commit);
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
	use:draggable={commit instanceof Commit
		? {
				data: new DraggableCommit(commit.branchId, commit, isHeadCommit)
			}
		: nonDraggable()}
	class="commit"
	class:is-commit-open={showDetails}
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
	/>

	<div class="commit__content">
		<!-- GENERAL INFO -->
		<div class="commit__about" on:click={toggleFiles} on:keyup={onKeyup} role="button" tabindex="0">
			{#if first}
				<div class="commit__type text-semibold text-base-12">
					{#if type == 'remote'}
						Local and remote
					{:else if type == 'local'}
						Local <Icon name="local" />
					{:else if type == 'upstream'}
						Remote upstream <Icon name="remote" />
					{/if}
				</div>
			{/if}

			{#if isUndoable && !commit.descriptionTitle}
				<span class="commit__empty-title">empty commit message</span>
			{:else}
				<h5 class="commit__title text-base-body-13 text-semibold" class:truncate={!showDetails}>
					{commit.descriptionTitle}
				</h5>

				{#if $advancedCommitOperations}
					<div class="text-base-11 commit__subtitle">
						<span class="commit__id">
							{#if commit.changeId}
								{commit.changeId.split('-')[0]}
							{:else}
								{commit.id.substring(0, 6)}
							{/if}

							{#if commit.isSigned}
								<Icon name="locked-small" />
							{/if}
						</span>

						<span class="commit__subtitle-divider">•</span>

						<span
							>{getTimeAgo(commit.createdAt)}{type == 'remote'
								? ` by ${commit.author.name}`
								: ''}</span
						>
					</div>
				{/if}
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
							{#if $advancedCommitOperations}
								<Tag
									style="ghost"
									kind="solid"
									icon="edit-text"
									clickable
									on:click={openCommitMessageModal}>Edit message</Tag
								>
								<Tag
									style="ghost"
									kind="solid"
									clickable
									on:click={(e) => {
										e.stopPropagation();
										reorderCommit(commit, -1);
									}}>Move Up</Tag
								>
								<Tag
									style="ghost"
									kind="solid"
									clickable
									on:click={(e) => {
										e.stopPropagation();
										reorderCommit(commit, 1);
									}}>Move Down</Tag
								>
								<Tag
									style="ghost"
									kind="solid"
									clickable
									on:click={(e) => {
										e.stopPropagation();
										insertBlankCommit(commit, -1);
									}}>Add Before</Tag
								>
								<Tag
									style="ghost"
									kind="solid"
									clickable
									on:click={(e) => {
										e.stopPropagation();
										insertBlankCommit(commit, 1);
									}}>Add After</Tag
								>
							{/if}
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
		<div class="files-container" transition:slide={{ duration: 100 }}>
			<BranchFilesList {files} {isUnapplied} />
		</div>
	{/if}
</div>

<style lang="postcss">
	/* amend drop zone */
	:global(.amend-dz-active .amend-dz-marker) {
		display: flex;
	}
	:global(.amend-dz-hover .hover-text) {
		visibility: visible;
	}

	.commit {
		display: flex;
		position: relative;
		flex-direction: column;

		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-left: none;
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&.is-first {
			border-top-left-radius: var(--radius-m);
			border-top-right-radius: var(--radius-m);
		}
		&.is-last {
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
		/* transition: background-color var(--transition-fast); */

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.commit__title {
		flex: 1;
		display: block;
		color: var(--clr-scale-ntrl-0);
		width: 100%;
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

	.commit__id {
		display: flex;
		align-items: center;
		gap: var(--size-4);
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
		background-color: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
		padding: 0 var(--size-14) var(--size-14);
	}

	/* MODIFIERS */
	.is-commit-open {
		/* background-color: var(--clr-bg-2); */
		/* border-top: 1px solid var(--clr-border-2); */
		/* margin: var(--size-4) 0; */

		/* &:not(.is-last) {
			border-top: 1px solid var(--clr-border-2);
			border-top-left-radius: var(--radius-m);
			border-top-right-radius: var(--radius-m);
		} */

		& .commit__about {
			background-color: var(--clr-bg-1-muted);
		}
	}
</style>
