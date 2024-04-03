<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import { Project } from '$lib/backend/projects';
	import Tag from '$lib/components/Tag.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { draggable } from '$lib/dragging/draggable';
	import { DraggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { createCommitStore, getSelectedFileIds, getSelectedFiles } from '$lib/vbranches/contexts';
	import { listRemoteCommitFiles } from '$lib/vbranches/remoteCommits';
	import { RemoteCommit, Commit, RemoteFile, Branch, BaseBranch } from '$lib/vbranches/types';
	import { slide } from 'svelte/transition';

	export let branch: Branch | undefined = undefined;
	export let commit: Commit | RemoteCommit;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
	export let isUnapplied = false;

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const selectedFiles = getSelectedFiles();
	const fileSelection = getSelectedFileIds();
	const selectedFileIds = $fileSelection.fileIds;

	const commitStore = createCommitStore(commit);
	$: commitStore.set(commit);

	$: selectedFile =
		$selectedFileIds &&
		$fileSelection.length == 1 &&
		$fileSelection.toOnly().context == commit.id &&
		files.find((f) => f.id == $fileSelection.toOnly().fileId);
	$: if (selectedFile) selectedFiles.set([selectedFile]);

	const currentCommitMessage = persistedCommitMessage(project.id, branch?.id || '');

	let showFiles = false;

	let files: RemoteFile[] = [];

	async function loadFiles() {
		files = await listRemoteCommitFiles(project.id, commit.id);
	}

	function toggleFiles() {
		showFiles = !showFiles;
		if (showFiles) loadFiles();
	}

	function onKeyup(e: KeyboardEvent) {
		if (e.key == 'Enter' || e.key == ' ') {
			toggleFiles();
		}
	}

	function resetHeadCommit() {
		if (!branch || !$baseBranch) {
			console.error('Unable to reset head commit');
			return;
		}
		if (branch.commits.length > 1) {
			branchController.resetBranch(branch.id, branch.commits[1].id);
		} else if (branch.commits.length === 1 && $baseBranch) {
			branchController.resetBranch(branch.id, $baseBranch.baseSha);
		}
	}

	const isUndoable = isHeadCommit && !isUnapplied;
	const hasCommitUrl = !commit.isLocal && commitUrl;
</script>

<div
	use:draggable={commit instanceof Commit
		? {
				data: new DraggableCommit(commit.branchId, commit, isHeadCommit)
			}
		: nonDraggable()}
	class="commit"
	class:is-commit-open={showFiles}
>
	<div class="commit__header" on:click={toggleFiles} on:keyup={onKeyup} role="button" tabindex="0">
		<div class="commit__message">
			<div class="commit__row">
				<span class="commit__title text-semibold text-base-12" class:truncate={!showFiles}>
					{commit.descriptionTitle}
				</span>
				{#if isUndoable && !showFiles}
					<Tag
						style="ghost"
						kind="solid"
						icon="undo-small"
						clickable
						on:click={(e) => {
							currentCommitMessage.set(commit.description);
							e.stopPropagation();
							resetHeadCommit();
						}}>Undo</Tag
					>
				{/if}
			</div>
			{#if showFiles && commit.descriptionBody}
				<div class="commit__row" transition:slide={{ duration: 100 }}>
					<span class="commit__body text-base-body-12">
						{commit.descriptionBody}
					</span>
				</div>
			{/if}
		</div>
		<div class="commit__row">
			<div class="commit__author">
				<img
					class="commit__avatar"
					title="Gravatar for {commit.author.email}"
					alt="Gravatar for {commit.author.email}"
					srcset="{commit.author.gravatarUrl} 2x"
					width="100"
					height="100"
					on:error
				/>
				<span class="commit__author-name text-base-12 truncate">{commit.author.name}</span>
			</div>
			<span class="commit__time text-base-11">
				<TimeAgo date={commit.createdAt} />
			</span>
		</div>
	</div>

	{#if showFiles}
		<div class="files-container" transition:slide={{ duration: 100 }}>
			<BranchFilesList {files} {isUnapplied} readonly />
		</div>

		{#if hasCommitUrl || isUndoable}
			<div class="files__footer">
				{#if isUndoable}
					<Tag
						style="ghost"
						kind="solid"
						icon="undo-small"
						clickable
						on:click={(e) => {
							currentCommitMessage.set(commit.description);
							e.stopPropagation();
							resetHeadCommit();
						}}>Undo</Tag
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
						}}>Open commit</Tag
					>
				{/if}
			</div>
		{/if}
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
		flex-direction: column;

		border-radius: var(--size-6);
		background-color: var(--clr-container-light);
		border: 1px solid var(--clr-container-outline-light);
		overflow: hidden;
		transition: background-color var(--transition-fast);

		&:not(.is-commit-open):hover {
			border: 1px solid
				color-mix(in srgb, var(--clr-container-outline-light), var(--darken-tint-mid));
			background-color: color-mix(
				in srgb,
				var(--clr-container-light),
				var(--darken-tint-extralight)
			);
		}
	}

	.commit__header {
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: var(--size-10);
		padding: var(--size-14);
	}

	.is-commit-open {
		background-color: color-mix(in srgb, var(--clr-container-light), var(--darken-tint-extralight));

		& .commit__header {
			padding-bottom: var(--size-16);
			border-bottom: 1px solid var(--clr-container-outline-light);

			&:hover {
				background-color: color-mix(in srgb, var(--clr-container-light), var(--darken-tint-light));
			}
		}

		& .commit__message {
			margin-bottom: var(--size-4);
		}
	}

	.commit__message {
		display: flex;
		flex-direction: column;
		gap: var(--size-6);
	}

	.commit__title {
		flex: 1;
		display: block;
		color: var(--clr-scale-ntrl-0);
		width: 100%;
	}

	.commit__body {
		flex: 1;
		display: block;
		width: 100%;
		color: var(--clr-scale-ntrl-40);
		white-space: pre-line;
		word-wrap: anywhere;
	}

	.commit__row {
		display: flex;
		align-items: center;
		gap: var(--size-8);
	}

	.commit__author {
		display: block;
		flex: 1;
		display: flex;
		align-items: center;
		gap: var(--size-6);
	}

	.commit__avatar {
		width: var(--size-16);
		height: var(--size-16);
		border-radius: 100%;
	}

	.commit__author-name {
		max-width: calc(100% - var(--size-16));
	}

	.commit__time,
	.commit__author-name {
		color: var(--clr-scale-ntrl-50);
	}

	.files-container {
		background-color: var(--clr-container-light);
		padding: 0 var(--size-14) var(--size-14);
	}

	.files__footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--size-8);
		padding: var(--size-14);
		background-color: var(--clr-container-light);
		border-top: 1px solid var(--clr-container-outline-light);
	}
</style>
