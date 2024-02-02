<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import BranchFilesList from './BranchFilesList.svelte';
	import FileDiff from './FileDiff.svelte';
	import FileTree from './FileTree.svelte';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableCommit, nonDraggable } from '$lib/dragging/draggables';
	import { getVSIFileIcon } from '$lib/ext-icons';
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import { Ownership } from '$lib/vbranches/ownership';
	import { listRemoteCommitFiles, parseRemoteFiles } from '$lib/vbranches/remoteCommits';
	import { LocalFile, RemoteCommit, Commit, RemoteFile } from '$lib/vbranches/types';
	import { open } from '@tauri-apps/api/shell';
	import { writable, type Writable } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import type { ContentSection, HunkSection } from '$lib/utils/fileSections';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let commit: Commit | RemoteCommit;
	export let projectId: string;
	export let commitUrl: string | undefined = undefined;
	export let isHeadCommit: boolean = false;
	export let resetHeadCommit: () => void | undefined = () => undefined;
	export let isUnapplied = false;
	export let branchController: BranchController;
	export let projectPath: string;
	export let selectedFiles: Writable<(LocalFile | RemoteFile)[]>;

	const selectedOwnership = writable(Ownership.default());

	let previewCommitModal: Modal;
	let showFiles = false;
	let selectedListMode: string;

	let files: RemoteFile[] = [];
	let parsedFiles: [RemoteFile, (ContentSection | HunkSection)[]][];
	let isLoading = false;

	async function loadFiles() {
		isLoading = true;
		files = await listRemoteCommitFiles(projectId, commit.id);
		parsedFiles = parseRemoteFiles(files);
		isLoading = false;
	}

	function onClick() {
		showFiles = !showFiles;
		if (showFiles) loadFiles();
		// previewCommitModal.show();
	}
</script>

<div
	use:draggable={commit instanceof Commit
		? draggableCommit(commit.branchId, commit)
		: nonDraggable()}
	class="commit"
	class:is-head-commit={isHeadCommit}
>
	<div class="commit__header" on:click={onClick} on:keyup={onClick} role="button" tabindex="0">
		<div class="commit__row">
			<span class="commit__description text-base-12 truncate">
				{commit.description}
			</span>
			{#if isHeadCommit && !isUnapplied}
				<Tag
					color="ghost"
					icon="undo-small"
					border
					clickable
					on:click={(e) => {
						e.stopPropagation();
						resetHeadCommit();
					}}>Undo</Tag
				>
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
		<div transition:slide={{ duration: 100 }}>
			<div class="commit__files-header">
				<BranchFilesHeader
					{files}
					{selectedOwnership}
					showCheckboxes={false}
					bind:selectedListMode
				/>
			</div>
			<div class="commit__files">
				{#if selectedListMode == 'list'}
					<BranchFilesList
						branchId="blah"
						{files}
						{selectedOwnership}
						{selectedFiles}
						{isUnapplied}
						readonly={true}
					/>
				{:else}
					<FileTree
						node={filesToFileTree(files)}
						branchId="blah"
						isRoot={true}
						{selectedOwnership}
						{selectedFiles}
						{isUnapplied}
					/>
				{/if}
			</div>
		</div>
	{/if}
</div>

<Modal
	width="large"
	bind:this={previewCommitModal}
	icon="commit"
	title={commit.description}
	hoverText={commit.id}
>
	<svelte:fragment slot="header_controls">
		{#if !commit.isLocal && commitUrl}
			<Button
				color="neutral"
				kind="outlined"
				icon="open-link"
				on:click={() => {
					if (commitUrl) open(commitUrl);
				}}>Open commit</Button
			>
		{/if}
	</svelte:fragment>

	<div class="commit-modal__body">
		{#if isLoading}
			<div class="flex w-full justify-center">
				<div class="border-gray-900 h-8 w-8 animate-spin rounded-full border-b-2" />
			</div>
		{:else}
			{#each parsedFiles as [remoteFile, sections] (remoteFile.id)}
				<div class="commit-modal__file-section">
					<div
						class="text-color-3 flex flex-grow items-center overflow-hidden text-ellipsis whitespace-nowrap font-bold"
						title={remoteFile.path}
					>
						<img
							src={getVSIFileIcon(remoteFile.path)}
							alt="js"
							width="13"
							style="width: 0.8125rem"
							class="mr-1 inline"
						/>

						{remoteFile.path}
					</div>
					<div class="commit-modal__code-container custom-scrollbar">
						<div class="commit-modal__code-wrapper">
							<FileDiff
								readonly={true}
								filePath={remoteFile.path}
								isBinary={remoteFile.binary}
								isLarge={false}
								{projectPath}
								{isUnapplied}
								{branchController}
								selectable={false}
								branchId={commit instanceof Commit ? commit.branchId : undefined}
								{sections}
							/>
						</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>

	<svelte:fragment slot="controls">
		<Button color="primary" on:click={() => previewCommitModal.close()}>Close</Button>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	/* amend drop zone */
	:global(.amend-dz-active .amend-dz-marker) {
		@apply flex;
	}
	:global(.amend-dz-hover .hover-text) {
		@apply visible;
	}

	.commit {
		display: flex;
		flex-direction: column;
		cursor: default;
		gap: var(--space-10);
		border-radius: var(--space-6);
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		transition: background-color var(--transition-fast);

		&:hover {
			border: 1px solid var(--clr-theme-container-outline-pale);
		}
	}

	.commit__header {
		display: flex;
		flex-direction: column;
		gap: var(--space-10);
		padding: var(--space-12);
	}

	.commit__description {
		flex: 1;
		display: block;
		color: var(--clr-theme-scale-ntrl-0);
		line-height: 120%;
		width: 100%;
	}

	.commit__row {
		display: flex;
		align-items: center;
		gap: var(--space-8);
	}

	.commit__files {
		padding-top: 0;
		padding-left: var(--space-12);
		padding-right: var(--space-12);
		padding-bottom: var(--space-12);
	}

	.commit__files-header {
		border-top: 1px solid var(--clr-theme-container-outline-light);
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
		padding-left: var(--space-20);
		padding-right: var(--space-12);
	}

	.commit__author {
		display: block;
		flex: 1;
		display: flex;
		align-items: center;
		gap: var(--space-6);
	}

	.commit__avatar {
		width: var(--space-16);
		height: var(--space-16);
		border-radius: 100%;
	}

	.commit__author-name {
		max-width: calc(100% - var(--space-16));
	}

	.commit__time,
	.commit__author-name {
		color: var(--clr-theme-scale-ntrl-50);
	}

	.is-head-commit {
		gap: var(--space-6);
	}

	/* modal */
	.commit-modal__code-container {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-theme-container-outline-light);
		overflow-x: auto;
		overflow-y: hidden;
		user-select: text;
	}

	.commit-modal__code-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		min-width: max-content;
	}

	.commit-modal__file-section {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
	}

	.commit-modal__body {
		display: flex;
		flex-direction: column;
		gap: var(--space-20);
	}
</style>
